//! Storage migrations.
//!
//! # `MigrateV0ToV1`
//!
//! Storage version 0 had no per-device call filters in effect: a device was unrestricted, and
//! `SessionKeys` held `(account, until)`. Version 1 makes the filter explicit:
//!
//! - every device has an entry in [`DeviceFilters`]. The extension denies a device without one,
//!   so, without the migration, every device created before the upgrade would be locked out;
//! - every `SessionKeys` value is `(account, until, filter)`.
//!
//! The migration gives `Admin` to both, which is what they were before. It is a
//! [`SteppedMigration`] to be used with `pallet_migrations`, in two phases (devices first, then
//! session keys). Before every unit of work (one device or one session key) it checks the
//! weight meter: when the unit does not fit, it stops and returns the cursor, so the next call
//! resumes where it was. It returns [`SteppedMigrationError::InsufficientWeight`] only when not
//! even one unit fits a meter that is empty, which no amount of retrying fixes. An entry that
//! cannot be read is skipped and logged, never an error.
//!
//! Until it completes, devices that it has not reached yet cannot sign. Runtimes should run it
//! while ordinary transactions are paused, as `pallet_migrations` does.

use crate::{
    types::BlockNumberFor, Config, DeviceFilter, DeviceFilterOf, DeviceFilters, DeviceOf, Devices,
    Pallet, WeightInfo,
};
use alloc::vec::Vec;
use codec::{Decode, DecodeAll, Encode, MaxEncodedLen};
use core::marker::PhantomData;
use frame_support::{
    migrations::{MigrationId, SteppedMigration, SteppedMigrationError},
    pallet_prelude::{BoundedVec, ConstU32, StorageVersion},
    storage::StoragePrefixedMap,
    traits::PalletInfoAccess,
    weights::WeightMeter,
};

#[cfg(feature = "try-runtime")]
use {frame_support::traits::GetStorageVersion, sp_runtime::TryRuntimeError};

/// The most bytes of a raw storage key that a [`Cursor`] can hold.
pub const MAX_KEY_LEN: u32 = 256;
/// What a raw key holds besides the encoded `AccountId`: the two storage prefixes (32 bytes), and
/// the hasher of the first key (16 bytes), plus, in `Devices`, the second key (16 + 32 bytes).
pub const MAX_KEY_OVERHEAD: usize = 32 + 16 + 16 + 32;

/// A raw storage key, bounded.
pub type CursorKey = BoundedVec<u8, ConstU32<MAX_KEY_LEN>>;

/// The two phases of [`MigrateV0ToV1`], in order.
#[derive(Encode, Decode, MaxEncodedLen, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Backfills `DeviceFilters`.
    Devices,
    /// Converts the values of `SessionKeys`.
    SessionKeys,
}

/// The progress of [`MigrateV0ToV1`]: the phase, and the raw key of the last entry that was
/// processed in it (empty when the phase has just begun).
pub type Cursor = (Phase, CursorKey);

mod v0 {
    use super::*;
    use frame_support::{storage_alias, Blake2_128Concat};

    /// `SessionKeys` as it was at storage version 0.
    #[storage_alias]
    pub type SessionKeys<T: Config<I>, I: 'static> = StorageMap<
        Pallet<T, I>,
        Blake2_128Concat,
        <T as frame_system::Config>::AccountId,
        (<T as frame_system::Config>::AccountId, BlockNumberFor<T, I>),
    >;
}

type SessionKeyV1<T, I> = (
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T, I>,
    DeviceFilterOf<T, I>,
);

/// Migrates the pallet from storage version 0 to 1. See the [module docs][self].
pub struct MigrateV0ToV1<T, I = ()>(PhantomData<(T, I)>);

impl<T: Config<I>, I: 'static> MigrateV0ToV1<T, I> {
    /// The weight of one unit of work in the given phase.
    fn unit(phase: Phase) -> frame_support::weights::Weight {
        match phase {
            Phase::Devices => T::WeightInfo::migrate_device_filter(),
            Phase::SessionKeys => T::WeightInfo::migrate_session_key(),
        }
    }

    /// The prefix of all the keys of a storage map.
    fn prefix_of<V: codec::FullCodec, M: StoragePrefixedMap<V>>() -> Vec<u8> {
        M::final_prefix().to_vec()
    }

    /// The raw key where the next unit of a phase starts after: the last processed key or, if
    /// none was, the prefix of the storage being iterated.
    fn start_after(last: &CursorKey, prefix: Vec<u8>) -> Vec<u8> {
        if last.is_empty() {
            prefix
        } else {
            last.to_vec()
        }
    }

    fn to_cursor_key(raw: &[u8]) -> Result<CursorKey, SteppedMigrationError> {
        CursorKey::try_from(raw.to_vec()).map_err(|_| {
            // `integrity_test` makes this unreachable.
            log::error!(target: "pallet_pass", "migration: storage key does not fit the cursor");
            SteppedMigrationError::Failed
        })
    }

    /// Gives `Admin` to the device, unless it already has a filter.
    fn backfill_device_filter(address: &T::AccountId, device_id: &fc_traits_authn::DeviceId) {
        if !DeviceFilters::<T, I>::contains_key(address, device_id) {
            DeviceFilters::<T, I>::insert(address, device_id, DeviceFilter::Admin);
        }
    }

    /// Rewrites the value of a session key with `Admin` as its filter, unless it has already
    /// been converted. Values it cannot read are left as they are.
    fn convert_session_key(raw_key: &[u8]) {
        let Some(raw_value) = frame_support::storage::unhashed::get_raw(raw_key) else {
            return;
        };
        // `decode_all` matters: a value in the new shape starts with the old one, so a plain
        // `decode` would read it as old, and replace its filter.
        match <(T::AccountId, BlockNumberFor<T, I>)>::decode_all(&mut &raw_value[..]) {
            Ok((account, until)) => {
                let value: SessionKeyV1<T, I> = (account, until, DeviceFilter::Admin);
                frame_support::storage::unhashed::put(raw_key, &value);
            }
            Err(_) if SessionKeyV1::<T, I>::decode_all(&mut &raw_value[..]).is_ok() => {
                // Already converted.
            }
            Err(e) => {
                log::warn!(
                    target: "pallet_pass",
                    "migration: skipping a session key that cannot be decoded: {:?}", e
                );
            }
        }
    }
}

impl<T: Config<I>, I: 'static> SteppedMigration for MigrateV0ToV1<T, I> {
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
            Self::prefix_of::<DeviceFilterOf<T, I>, DeviceFilters<T, I>>(),
            Self::prefix_of::<(T::AccountId, BlockNumberFor<T, I>), v0::SessionKeys<T, I>>(),
        ])
    }

    fn step(
        cursor: Option<Self::Cursor>,
        meter: &mut WeightMeter,
    ) -> Result<Option<Self::Cursor>, SteppedMigrationError> {
        let (mut phase, mut last) = cursor.unwrap_or((Phase::Devices, CursorKey::default()));
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
                Phase::Devices => {
                    let mut devices = Devices::<T, I>::iter_keys_from(Self::start_after(
                        &last,
                        Self::prefix_of::<DeviceOf<T, I>, Devices<T, I>>(),
                    ));
                    match devices.next() {
                        Some((address, device_id)) => {
                            Self::backfill_device_filter(&address, &device_id);
                            last = Self::to_cursor_key(devices.last_raw_key())?;
                        }
                        None => (phase, last) = (Phase::SessionKeys, CursorKey::default()),
                    }
                }
                Phase::SessionKeys => {
                    let mut keys = v0::SessionKeys::<T, I>::iter_keys_from(Self::start_after(
                        &last,
                        Self::prefix_of::<
                            (T::AccountId, BlockNumberFor<T, I>),
                            v0::SessionKeys<T, I>,
                        >(),
                    ));
                    match keys.next() {
                        Some(_) => {
                            let raw_key = keys.last_raw_key().to_vec();
                            Self::convert_session_key(&raw_key);
                            last = Self::to_cursor_key(&raw_key)?;
                        }
                        None => {
                            StorageVersion::new(1).put::<Pallet<T, I>>();
                            return Ok(None);
                        }
                    }
                }
            }
        }
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        Ok(Snapshot::take::<T, I>().encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
        let before = Snapshot::decode(&mut &state[..])
            .map_err(|_| TryRuntimeError::Other("cannot decode the pre-upgrade state"))?;
        let after = Snapshot::take::<T, I>();

        frame_support::ensure!(
            Pallet::<T, I>::on_chain_storage_version() >= 1,
            "the storage version was not bumped"
        );
        frame_support::ensure!(
            after.devices_without_filter == 0,
            "a device still has no filter"
        );
        frame_support::ensure!(
            after.devices == before.devices && after.devices_digest == before.devices_digest,
            "devices changed"
        );
        frame_support::ensure!(
            after.filters == before.filters + before.devices_without_filter,
            "more or fewer filters than devices that needed one"
        );
        frame_support::ensure!(
            after.restricted_filters_digest == before.restricted_filters_digest,
            "a filter that was not `Admin` changed"
        );
        frame_support::ensure!(
            after.session_keys == before.session_keys
                && after.session_keys_counter == before.session_keys_counter,
            "session keys were added or removed, or their counter went out of sync"
        );
        frame_support::ensure!(
            after.session_keys_old_shape == 0,
            "a session key is still in the old shape"
        );
        frame_support::ensure!(
            after.session_keys_unreadable == before.session_keys_unreadable,
            "a session key that could be read cannot be read anymore"
        );
        Ok(())
    }
}

/// What the `try-runtime` checks compare before and after the migration.
#[cfg(feature = "try-runtime")]
#[derive(Encode, Decode)]
struct Snapshot {
    devices: u32,
    devices_without_filter: u32,
    /// Hash of every `Devices` entry.
    devices_digest: [u8; 32],
    filters: u32,
    /// Hash of every `DeviceFilters` entry that is not `Admin`.
    restricted_filters_digest: [u8; 32],
    session_keys: u32,
    session_keys_counter: u32,
    /// Session keys in the shape of storage version 0.
    session_keys_old_shape: u32,
    /// Session keys in neither shape. The migration skips them.
    session_keys_unreadable: u32,
}

#[cfg(feature = "try-runtime")]
impl Snapshot {
    fn take<T: Config<I>, I: 'static>() -> Self {
        let digest = |entries: Vec<Vec<u8>>| sp_io::hashing::blake2_256(&entries.encode());

        let devices_without_filter = Devices::<T, I>::iter_keys()
            .filter(|(address, device_id)| !DeviceFilters::<T, I>::contains_key(address, device_id))
            .count() as u32;
        let (mut session_keys_old_shape, mut session_keys_unreadable) = (0, 0);
        for key in crate::SessionKeys::<T, I>::iter_keys() {
            let raw_key = crate::SessionKeys::<T, I>::hashed_key_for(&key);
            let raw = frame_support::storage::unhashed::get_raw(&raw_key).unwrap_or_default();
            if SessionKeyV1::<T, I>::decode_all(&mut &raw[..]).is_ok() {
                continue;
            }
            if <(T::AccountId, BlockNumberFor<T, I>)>::decode_all(&mut &raw[..]).is_ok() {
                session_keys_old_shape += 1;
            } else {
                session_keys_unreadable += 1;
            }
        }

        Self {
            devices: Devices::<T, I>::iter_keys().count() as u32,
            devices_without_filter,
            devices_digest: digest(
                Devices::<T, I>::iter()
                    .map(|(address, device_id, device)| (address, device_id, device).encode())
                    .collect(),
            ),
            filters: DeviceFilters::<T, I>::iter_keys().count() as u32,
            restricted_filters_digest: digest(
                DeviceFilters::<T, I>::iter()
                    .filter(|(_, _, filter)| !matches!(filter, DeviceFilter::Admin))
                    .map(|(address, device_id, filter)| (address, device_id, filter).encode())
                    .collect(),
            ),
            session_keys: crate::SessionKeys::<T, I>::iter_keys().count() as u32,
            session_keys_counter: crate::SessionKeys::<T, I>::count(),
            session_keys_old_shape,
            session_keys_unreadable,
        }
    }
}
