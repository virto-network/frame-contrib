//! Storage migration from v0 (flat track ids) to v1 (group and sub-track).
//!
//! # `MigrateV0ToV1`
//!
//! At storage version 0 a track was identified by a flat id: `Tracks` was a `StorageMap<Id, Info>`,
//! `TracksIds` a `BoundedVec<Id>` and `OriginToTrackId` held the same flat id. At version 1 an id is
//! split in two halves (see [`SplitId`]): `Tracks` is a `StorageDoubleMap<Group, SubTrack, Info>`,
//! `TracksIds` a `BoundedBTreeSet`, and `TrackIdToOrigin` and `NextGroupId` are new.
//!
//! Old track `n` becomes `combine(group(n), 0)`: **the old id is the group**, and the track that
//! it was becomes the first one of the group (sub-track `0`). That keeps the group-then-track
//! association: a runtime that keys groups by something it owns (Kreivo: the community id) keeps
//! doing so, and adds sub-tracks to the group afterwards.
//!
//! ## What the runtime supplies
//!
//! The pre-upgrade id type (`OldId`, e.g. `u16`) can differ from [`Config::TrackId`] (e.g. `u32`),
//! and its keys are hashed over its own encoding, so it has to be named. A
//! [`Convert<OldId, Option<GroupIdOf<T, I>>>`][sp_runtime::traits::Convert] gives the group that
//! an old id becomes. [`TryIntoGroup`] does it with `TryInto`. An id that does not fit the group
//! half is **skipped and logged**, never an error: its entries stay as they were.
//!
//! ## How to register it
//!
//! It is a [`SteppedMigration`]: register it in `pallet_migrations::Config::Migrations`, **in the same
//! runtime upgrade** that changes `Config::TrackId` (and the pallet's code to version 1):
//!
//! ```ignore
//! type Migrations = (
//!     fc_pallet_referenda_tracks::migration::MigrateV0ToV1<
//!         Runtime,
//!         CommunityTracksInstance,
//!         u16, // the pre-upgrade `TrackId`
//!         fc_pallet_referenda_tracks::migration::TryIntoGroup,
//!     >,
//! );
//! ```
//!
//! Do not add a separate "bump the storage version" migration: this one bumps it, in its last
//! unit, and not before. It is not a single-block migration: do not put it in `Executive`'s tuple.
//! Call [`MigrateV0ToV1::integrity_test`] from the runtime's integrity test.
//!
//! Chains that never had version 0 (version 1 at genesis) need nothing: the migration returns at
//! once when the on-chain version is already 1.
//!
//! ## The contract
//!
//! It runs in four phases, each unit being atomic:
//!
//! 1. [`Phase::Tracks`]: one `v0` `Tracks` entry per unit moves to the double map.
//! 2. [`Phase::Origins`]: one `OriginToTrackId` entry per unit gets its value rewritten, and its
//!    `TrackIdToOrigin` entry written (v0 had no reverse map, so this is the first place where the
//!    origin of a track is known).
//! 3. [`Phase::TracksIds`]: one track of the double map per unit is added to `TracksIds` (which
//!    the previous phase emptied, as it held the old `Vec`), and `NextGroupId` moves past its
//!    group if it has to (it never goes down).
//! 4. [`Phase::Finish`]: a last unit bumps the storage version to 1.
//!
//! No unit costs more with the number of tracks: the only one that grows with a bound is the
//! third, which reads and writes the `TracksIds` value at its largest, `MaxTracks` ids. A runtime
//! has to make sure that it fits one block (see `WeightInfo::migrate_ids`).
//!
//! Before every unit it calls `meter.try_consume(unit)`. When the unit does not fit, it stops and
//! returns `Ok(Some(cursor))`; it returns [`SteppedMigrationError::InsufficientWeight`] only when
//! not even one unit fits an *empty* meter. Anything odd in the data (an id that does not
//! convert, a value that does not decode, a conflicting entry) is skipped and logged. The cursor
//! is a phase and a bounded raw storage key.
//!
//! v0 and v1 `Tracks` entries **share one storage prefix**, so the first phase walks raw keys and
//! tells the shapes apart by the exact length of the key (and by its hash): a typed iteration
//! would read a migrated entry as a v0 one. For the same reason it is idempotent: entries that
//! were already migrated are recognised and left alone, and running it again changes nothing.
//!
//! Not covered: `pallet_referenda` keeps its own storage keyed by the track id (`DecidingCount`,
//! `TrackQueue`, and `ReferendumInfoFor` for ongoing referenda). Widening `TrackId` orphans those
//! keys, so a runtime with referenda in flight must re-key them itself.

use super::*;
use alloc::vec::Vec;
use codec::{DecodeAll, Encode, FullCodec, MaxEncodedLen};
use core::{fmt::Debug, marker::PhantomData};
use frame_support::{
    migrations::{MigrationId, SteppedMigration, SteppedMigrationError},
    pallet_prelude::{BoundedVec, ConstU32, StorageVersion},
    storage::unhashed,
    traits::{Get, GetStorageVersion, Incrementable, PalletInfoAccess},
    weights::WeightMeter,
};
use sp_runtime::traits::Convert;

#[cfg(feature = "try-runtime")]
use {alloc::collections::BTreeMap, sp_runtime::TryRuntimeError};

const LOG_TARGET: &str = "runtime::referenda-tracks::migration";

/// The size of a storage prefix (pallet and storage name hashes).
const PREFIX_LEN: usize = 32;
/// The size of the `Blake2_128` part of a `Blake2_128Concat` key.
const HASH_LEN: usize = 16;

/// The most bytes of a raw storage key that a [`Cursor`] can hold.
pub const MAX_KEY_LEN: u32 = 256;

/// A raw storage key, bounded.
pub type CursorKey = BoundedVec<u8, ConstU32<MAX_KEY_LEN>>;

/// The phases of [`MigrateV0ToV1`], in order.
#[derive(Encode, codec::Decode, MaxEncodedLen, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Moves the `Tracks` entries to the double map.
    Tracks,
    /// Rewrites `OriginToTrackId` and fills `TrackIdToOrigin`.
    Origins,
    /// Adds each track to `TracksIds` and moves `NextGroupId` past its group.
    TracksIds,
    /// Bumps the storage version.
    Finish,
}

/// The progress of [`MigrateV0ToV1`]: the phase, and the raw key of the last entry that was
/// processed in it (empty when the phase has just begun).
pub type Cursor = (Phase, CursorKey);

/// The prefix of the storage item named `storage` of this pallet.
pub(crate) fn storage_prefix_of<T: Config<I>, I: 'static>(storage: &[u8]) -> [u8; PREFIX_LEN] {
    frame_support::storage::storage_prefix(Pallet::<T, I>::name().as_bytes(), storage)
}

/// Converts an old id to a group with `TryInto`: ids that do not fit are skipped.
pub struct TryIntoGroup;
impl<Old: TryInto<Group>, Group> Convert<Old, Option<Group>> for TryIntoGroup {
    fn convert(old: Old) -> Option<Group> {
        old.try_into().ok()
    }
}

/// V0 storage types (before migration).
pub mod v0 {
    use super::*;
    use frame_support::{storage_alias, Blake2_128Concat};

    /// `TracksIds` as it was at storage version 0.
    #[storage_alias]
    pub type TracksIds<T: Config<I>, I: 'static, OldId: FullCodec + MaxEncodedLen> = StorageValue<
        Pallet<T, I>,
        BoundedVec<OldId, <T as Config<I>>::MaxTracks>,
        frame_support::pallet_prelude::ValueQuery,
    >;

    /// `Tracks` as it was at storage version 0. It lives under the same prefix as the double map
    /// of version 1.
    #[storage_alias]
    pub type Tracks<T: Config<I>, I: 'static, OldId: FullCodec + MaxEncodedLen> =
        StorageMap<Pallet<T, I>, Blake2_128Concat, OldId, TrackInfoOf<T, I>>;
}

/// What an `OriginToTrackId` value is.
enum OriginValue<Id, OldId> {
    /// Already in the shape of version 1.
    Migrated(Id),
    /// In the shape of version 0.
    V0(OldId),
    /// In neither.
    Unreadable,
}

/// Migrates the pallet from storage version 0 to 1. See the [module docs][self].
pub struct MigrateV0ToV1<T, I, OldId, C>(PhantomData<(T, I, OldId, C)>);

impl<T, I, OldId, C> MigrateV0ToV1<T, I, OldId, C>
where
    T: Config<I>,
    I: 'static,
    OldId: FullCodec + MaxEncodedLen + Copy + Debug + 'static,
    C: Convert<OldId, Option<GroupIdOf<T, I>>>,
{
    /// The weight of one unit of work in the given phase.
    fn unit(phase: Phase) -> frame_support::weights::Weight {
        match phase {
            Phase::Tracks => <T as Config<I>>::WeightInfo::migrate_track(),
            Phase::Origins => <T as Config<I>>::WeightInfo::migrate_origin(),
            Phase::TracksIds => <T as Config<I>>::WeightInfo::migrate_ids(T::MaxTracks::get()),
            Phase::Finish => <T as Config<I>>::WeightInfo::migrate_finalize(),
        }
    }

    /// Checks that every raw key that the migration stores in its cursor fits it. Call it from
    /// the runtime's `integrity_test`.
    pub fn integrity_test() {
        assert!(
            PREFIX_LEN + HASH_LEN + OldId::max_encoded_len() <= MAX_KEY_LEN as usize,
            "an old track id does not fit the cursor of the migration"
        );
        assert!(
            PREFIX_LEN + HASH_LEN + PalletsOriginOf::<T>::max_encoded_len() <= MAX_KEY_LEN as usize,
            "an origin does not fit the cursor of the migration"
        );
    }

    /// The size of the raw key of a `v0::Tracks` entry.
    fn v0_track_key_len() -> usize {
        PREFIX_LEN + HASH_LEN + OldId::max_encoded_len()
    }

    /// The size of the raw key of an entry of the double map.
    fn v1_track_key_len() -> usize {
        PREFIX_LEN
            + HASH_LEN
            + GroupIdOf::<T, I>::max_encoded_len()
            + HASH_LEN
            + SubTrackIdOf::<T, I>::max_encoded_len()
    }

    fn tracks_prefix() -> Vec<u8> {
        storage_prefix_of::<T, I>(b"Tracks").to_vec()
    }

    fn origins_prefix() -> Vec<u8> {
        storage_prefix_of::<T, I>(b"OriginToTrackId").to_vec()
    }

    /// The raw key that follows `last` (or the first one of `prefix`), if it is under `prefix`.
    fn next_key(prefix: &[u8], last: &CursorKey) -> Option<Vec<u8>> {
        let from = if last.is_empty() {
            prefix.to_vec()
        } else {
            last.to_vec()
        };
        sp_io::storage::next_key(&from).filter(|key| key.starts_with(prefix))
    }

    fn to_cursor_key(raw: &[u8]) -> Result<CursorKey, SteppedMigrationError> {
        CursorKey::try_from(raw.to_vec()).map_err(|_| {
            // `integrity_test` makes this unreachable.
            log::error!(target: LOG_TARGET, "storage key does not fit the cursor");
            SteppedMigrationError::Failed
        })
    }

    /// Decodes the part of a `Blake2_128Concat` key that follows its prefix, if the hash matches.
    fn unhash<K: codec::Decode>(raw_key: &[u8]) -> Option<K> {
        let material = raw_key.get(PREFIX_LEN..)?;
        let (hash, encoded) = (material.get(..HASH_LEN)?, material.get(HASH_LEN..)?);
        if sp_io::hashing::blake2_128(encoded) != hash {
            return None;
        }
        K::decode_all(&mut &encoded[..]).ok()
    }

    /// The raw keys under `prefix`, in storage order.
    fn raw_keys(prefix: Vec<u8>) -> impl Iterator<Item = Vec<u8>> {
        let mut last = prefix.clone();
        core::iter::from_fn(move || {
            let key = sp_io::storage::next_key(&last).filter(|key| key.starts_with(&prefix))?;
            last = key.clone();
            Some(key)
        })
    }

    /// The group and sub-track of a raw key of the double map, if it is one (and not a key of
    /// version 0).
    fn v1_track_of(raw_key: &[u8]) -> Option<(GroupIdOf<T, I>, SubTrackIdOf<T, I>)> {
        if raw_key.len() != Self::v1_track_key_len() {
            return None;
        }
        let (hash_group, rest) = raw_key.get(PREFIX_LEN..)?.split_at(HASH_LEN);
        let (group, rest) = rest.split_at(GroupIdOf::<T, I>::max_encoded_len());
        let (hash_sub, sub) = rest.split_at(HASH_LEN);
        if sp_io::hashing::blake2_128(group) != hash_group
            || sp_io::hashing::blake2_128(sub) != hash_sub
        {
            return None;
        }
        Some((
            GroupIdOf::<T, I>::decode_all(&mut &group[..]).ok()?,
            SubTrackIdOf::<T, I>::decode_all(&mut &sub[..]).ok()?,
        ))
    }

    /// The old id and the new one of a `v0::Tracks` raw key, or `None` if the key is not one of
    /// version 0 or if the id does not convert (the latter is logged).
    fn v0_track_of(raw_key: &[u8]) -> Option<(OldId, GroupIdOf<T, I>)> {
        if raw_key.len() != Self::v0_track_key_len() {
            return None;
        }
        let old = Self::unhash::<OldId>(raw_key)?;
        match C::convert(old) {
            Some(group) => Some((old, group)),
            None => {
                log::warn!(
                    target: LOG_TARGET,
                    "track {:?} does not fit a group: skipping it", old
                );
                None
            }
        }
    }

    /// Moves one `v0::Tracks` entry to the double map. Anything unexpected is left as it was.
    fn migrate_track(raw_key: &[u8]) {
        let Some((old, group)) = Self::v0_track_of(raw_key) else {
            return;
        };
        let sub = SubTrackIdOf::<T, I>::default();
        let Some(info) = unhashed::get_raw(raw_key)
            .and_then(|raw| TrackInfoOf::<T, I>::decode_all(&mut &raw[..]).ok())
        else {
            log::warn!(
                target: LOG_TARGET,
                "track {:?} cannot be read: skipping it", old
            );
            return;
        };
        if Tracks::<T, I>::contains_key(&group, &sub) {
            log::warn!(
                target: LOG_TARGET,
                "track {:?} would replace the track of group {:?}: skipping it", old, group
            );
            return;
        }
        Tracks::<T, I>::insert(&group, &sub, info);
        unhashed::kill(raw_key);
    }

    fn origin_value(
        origin: &PalletsOriginOf<T>,
        raw_value: &[u8],
    ) -> OriginValue<TrackIdOf<T, I>, OldId> {
        if let Ok(id) = TrackIdOf::<T, I>::decode_all(&mut &raw_value[..]) {
            // With ids of the same size, the shapes are told apart by the reverse map.
            if TrackIdToOrigin::<T, I>::get(id).as_ref() == Some(origin) {
                return OriginValue::Migrated(id);
            }
        }
        match OldId::decode_all(&mut &raw_value[..]) {
            Ok(old) => OriginValue::V0(old),
            Err(_) => match TrackIdOf::<T, I>::decode_all(&mut &raw_value[..]) {
                Ok(id) => OriginValue::Migrated(id),
                Err(_) => OriginValue::Unreadable,
            },
        }
    }

    /// The id that a track of version 0 has now, if the track was migrated.
    fn new_id_of(old: OldId) -> Option<TrackIdOf<T, I>> {
        let group = C::convert(old)?;
        Tracks::<T, I>::contains_key(&group, SubTrackIdOf::<T, I>::default())
            .then(|| T::TrackId::combine(group, SubTrackIdOf::<T, I>::default()))
    }

    /// Rewrites the value of one `OriginToTrackId` entry, and fills its reverse. Anything
    /// unexpected is left as it was.
    fn migrate_origin(raw_key: &[u8]) {
        let Some(origin) = Self::unhash::<PalletsOriginOf<T>>(raw_key) else {
            log::warn!(target: LOG_TARGET, "an origin cannot be read: skipping it");
            return;
        };
        let Some(raw_value) = unhashed::get_raw(raw_key) else {
            return;
        };
        match Self::origin_value(&origin, &raw_value) {
            OriginValue::Migrated(_) => {}
            OriginValue::V0(old) => {
                let Some(new_id) = Self::new_id_of(old) else {
                    log::warn!(
                        target: LOG_TARGET,
                        "the origin of track {:?} points to a track that was not migrated: skipping it", old
                    );
                    return;
                };
                if TrackIdToOrigin::<T, I>::get(new_id).is_some_and(|other| other != origin) {
                    log::warn!(
                        target: LOG_TARGET,
                        "track {:?} already has another origin: skipping it", new_id
                    );
                    return;
                }
                TrackIdToOrigin::<T, I>::insert(new_id, &origin);
                OriginToTrackId::<T, I>::insert(&origin, new_id);
            }
            OriginValue::Unreadable => {
                log::warn!(
                    target: LOG_TARGET,
                    "the track of an origin cannot be read: skipping it"
                );
            }
        }
    }

    /// Adds one track of the double map to `TracksIds`, and moves `NextGroupId` past its group.
    /// Keys that are not of the double map are skipped.
    fn add_to_ids(raw_key: &[u8]) {
        let Some((group, sub)) = Self::v1_track_of(raw_key) else {
            return;
        };
        let id = T::TrackId::combine(group.clone(), sub);
        let inserted = TracksIds::<T, I>::try_mutate(|ids| ids.try_insert(id));
        if inserted.is_err() {
            log::error!(
                target: LOG_TARGET,
                "more tracks than `MaxTracks`: leaving {:?} out", id
            );
        }
        let next = group.clone().increment().unwrap_or(group);
        if next > NextGroupId::<T, I>::get() {
            NextGroupId::<T, I>::put(next);
        }
    }
}

impl<T, I, OldId, C> SteppedMigration for MigrateV0ToV1<T, I, OldId, C>
where
    T: Config<I>,
    I: 'static,
    OldId: FullCodec + MaxEncodedLen + Copy + Debug + 'static,
    C: Convert<OldId, Option<GroupIdOf<T, I>>>,
{
    type Cursor = Cursor;
    type Identifier = MigrationId<16>;

    fn id() -> Self::Identifier {
        MigrationId {
            pallet_id: Pallet::<T, I>::name_hash(),
            version_from: 0,
            version_to: 1,
        }
    }

    fn migrating_prefixes() -> Option<impl IntoIterator<Item = Vec<u8>>> {
        Some([
            Self::tracks_prefix(),
            Self::origins_prefix(),
            storage_prefix_of::<T, I>(b"TrackIdToOrigin").to_vec(),
            TracksIds::<T, I>::hashed_key().to_vec(),
            NextGroupId::<T, I>::hashed_key().to_vec(),
        ])
    }

    fn step(
        cursor: Option<Self::Cursor>,
        meter: &mut WeightMeter,
    ) -> Result<Option<Self::Cursor>, SteppedMigrationError> {
        if cursor.is_none() && Pallet::<T, I>::on_chain_storage_version() >= 1 {
            // Nothing to migrate: a chain that never had version 0, or one that is done.
            return Ok(None);
        }
        let (mut phase, mut last) = cursor.unwrap_or((Phase::Tracks, CursorKey::default()));
        let mut progressed = false;

        loop {
            let required = Self::unit(phase);
            if meter.try_consume(required).is_err() {
                // Not even one unit would fit an empty meter: waiting does not help.
                if !progressed && required.any_gt(meter.limit()) {
                    return Err(SteppedMigrationError::InsufficientWeight { required });
                }
                return Ok(Some((phase, last)));
            }
            progressed = true;

            match phase {
                Phase::Tracks => match Self::next_key(&Self::tracks_prefix(), &last) {
                    Some(raw_key) => {
                        Self::migrate_track(&raw_key);
                        last = Self::to_cursor_key(&raw_key)?;
                    }
                    None => (phase, last) = (Phase::Origins, CursorKey::default()),
                },
                Phase::Origins => match Self::next_key(&Self::origins_prefix(), &last) {
                    Some(raw_key) => {
                        Self::migrate_origin(&raw_key);
                        last = Self::to_cursor_key(&raw_key)?;
                    }
                    None => {
                        // It holds the `Vec` of version 0: the next phase fills it again.
                        TracksIds::<T, I>::kill();
                        (phase, last) = (Phase::TracksIds, CursorKey::default())
                    }
                },
                Phase::TracksIds => match Self::next_key(&Self::tracks_prefix(), &last) {
                    Some(raw_key) => {
                        Self::add_to_ids(&raw_key);
                        last = Self::to_cursor_key(&raw_key)?;
                    }
                    None => (phase, last) = (Phase::Finish, CursorKey::default()),
                },
                Phase::Finish => {
                    StorageVersion::new(1).put::<Pallet<T, I>>();
                    return Ok(None);
                }
            }
        }
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        Ok(Self::expected().encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
        let (expected_tracks, expected_left_in_v0, expected_origins) =
            Expected::<T, I>::decode_all(&mut &state[..])
                .map_err(|_| TryRuntimeError::Other("cannot decode the pre-upgrade state"))?;

        frame_support::ensure!(
            Pallet::<T, I>::on_chain_storage_version() >= 1,
            "the storage version was not bumped"
        );

        // Same tracks, the ones that were migrated under their new ids.
        let mut tracks: Vec<_> = Self::raw_keys(Self::tracks_prefix())
            .filter_map(|key| Self::v1_track_of(&key))
            .filter_map(|(group, sub)| {
                let info = Tracks::<T, I>::get(&group, &sub)?;
                Some((T::TrackId::combine(group, sub), info))
            })
            .collect();
        tracks.sort_by_key(|(id, _)| *id);
        frame_support::ensure!(tracks == expected_tracks, "the tracks changed");

        let left_in_v0 = Self::raw_keys(Self::tracks_prefix())
            .filter(|key| Self::is_v0_track_key(key))
            .count() as u32;
        frame_support::ensure!(
            left_in_v0 == expected_left_in_v0,
            "tracks that were skipped changed, or tracks that were not skipped are still there"
        );

        let ids: Vec<_> = TracksIds::<T, I>::get().into_iter().collect();
        frame_support::ensure!(
            ids == expected_tracks
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            "TracksIds is not the set of tracks"
        );

        let origins = Self::origin_entries();
        frame_support::ensure!(origins == expected_origins, "origin mappings changed");
        for (origin, raw) in origins {
            if let Ok(id) = TrackIdOf::<T, I>::decode_all(&mut &raw[..]) {
                if expected_tracks.iter().any(|(track, _)| *track == id) {
                    frame_support::ensure!(
                        TrackIdToOrigin::<T, I>::get(id) == Some(origin),
                        "TrackIdToOrigin is missing an entry"
                    );
                }
            }
        }

        if let Some((id, _)) = expected_tracks.last() {
            frame_support::ensure!(
                NextGroupId::<T, I>::get() >= id.split().0,
                "NextGroupId is lower than the highest group"
            );
        }
        Ok(())
    }
}

/// What the state must be after the migration: computed before it, from the same rules.
///
/// - every track, sorted by id: the ones in the double map, and the migrated ones;
/// - the number of tracks of version 0 that are skipped, and stay;
/// - every origin with the raw value that it must have.
#[cfg(feature = "try-runtime")]
type Expected<T, I> = (
    Vec<(TrackIdOf<T, I>, TrackInfoOf<T, I>)>,
    u32,
    Vec<(PalletsOriginOf<T>, Vec<u8>)>,
);

#[cfg(feature = "try-runtime")]
impl<T, I, OldId, C> MigrateV0ToV1<T, I, OldId, C>
where
    T: Config<I>,
    I: 'static,
    OldId: FullCodec + MaxEncodedLen + Copy + Debug + 'static,
    C: Convert<OldId, Option<GroupIdOf<T, I>>>,
{
    fn is_v0_track_key(raw_key: &[u8]) -> bool {
        raw_key.len() == Self::v0_track_key_len() && Self::unhash::<OldId>(raw_key).is_some()
    }

    fn origin_entries() -> Vec<(PalletsOriginOf<T>, Vec<u8>)> {
        let mut entries = Vec::new();
        let mut keys = OriginToTrackId::<T, I>::iter_keys();
        while let Some(origin) = keys.next() {
            let raw = unhashed::get_raw(keys.last_raw_key()).unwrap_or_default();
            entries.push((origin, raw));
        }
        entries
    }

    fn expected() -> Expected<T, I> {
        let mut tracks: BTreeMap<TrackIdOf<T, I>, TrackInfoOf<T, I>> = BTreeMap::new();
        for (group, sub) in
            Self::raw_keys(Self::tracks_prefix()).filter_map(|key| Self::v1_track_of(&key))
        {
            if let Some(info) = Tracks::<T, I>::get(&group, &sub) {
                tracks.insert(T::TrackId::combine(group, sub), info);
            }
        }
        let sub = SubTrackIdOf::<T, I>::default();
        let mut left_in_v0 = 0;
        for key in Self::raw_keys(Self::tracks_prefix()).filter(|key| Self::is_v0_track_key(key)) {
            let info = unhashed::get_raw(&key)
                .and_then(|raw| TrackInfoOf::<T, I>::decode_all(&mut &raw[..]).ok());
            let new_id = Self::unhash::<OldId>(&key)
                .and_then(C::convert)
                .map(|group| T::TrackId::combine(group, sub.clone()));
            match (new_id, info) {
                (Some(id), Some(info)) if !tracks.contains_key(&id) => {
                    tracks.insert(id, info);
                }
                _ => left_in_v0 += 1,
            }
        }

        let origins = Self::origin_entries()
            .into_iter()
            .map(|(origin, raw)| {
                let value = match Self::origin_value(&origin, &raw) {
                    OriginValue::V0(old) => C::convert(old)
                        .map(|group| T::TrackId::combine(group, sub.clone()))
                        .filter(|id| tracks.contains_key(id))
                        .map_or(raw, |id| id.encode()),
                    _ => raw,
                };
                (origin, value)
            })
            .collect();

        (tracks.into_iter().collect(), left_in_v0, origins)
    }
}
