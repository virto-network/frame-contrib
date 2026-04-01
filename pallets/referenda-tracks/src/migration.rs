//! Storage migration from v0 (flat track IDs) to v1 (split group/sub-track IDs).
//!
//! In v0, each track had a flat `TrackId` used directly as storage key.
//! In v1, each `TrackId` is split into `(GroupId, SubTrackId)` via `SplitId`.
//!
//! Old track IDs are treated as group IDs: old track `n` becomes `combine(n, 0)`.

use super::*;
use alloc::vec::Vec;
use frame_support::{
    pallet_prelude::*,
    storage_alias,
    traits::{Incrementable, UncheckedOnRuntimeUpgrade},
    weights::Weight,
};

const LOG_TARGET: &str = "runtime::referenda-tracks::migration";

/// V0 storage types (before migration).
pub mod v0 {
    use super::*;

    #[storage_alias]
    pub type TracksIds<T: Config<I>, I: 'static> = StorageValue<
        Pallet<T, I>,
        BoundedVec<TrackIdOf<T, I>, <T as Config<I>>::MaxTracks>,
        ValueQuery,
    >;

    #[storage_alias]
    pub type Tracks<T: Config<I>, I: 'static> =
        StorageMap<Pallet<T, I>, Blake2_128Concat, TrackIdOf<T, I>, TrackInfoOf<T, I>>;
}

/// Migrate from v0 to v1.
///
/// - Old `TrackId` values are reinterpreted as group IDs: `old_id` → `combine(old_id, 0)`.
/// - `Tracks` changes from `StorageMap<TrackId, Info>` to `StorageDoubleMap<GroupId, SubTrackId, Info>`.
/// - `TracksIds` changes from `BoundedVec` to `BoundedBTreeSet` with new combined IDs.
/// - `OriginToTrackId` values are updated to the new combined IDs.
/// - `TrackIdToOrigin` is populated (reverse mapping).
/// - `NextGroupId` is initialized to `max(old_ids) + 1`.
pub struct MigrateV0ToV1<T, I = ()>(core::marker::PhantomData<(T, I)>);

impl<T: Config<I>, I: 'static> UncheckedOnRuntimeUpgrade for MigrateV0ToV1<T, I> {
    fn on_runtime_upgrade() -> Weight {
        let mut reads: u64 = 1; // TracksIds read
        let mut writes: u64 = 0;

        // 1. Read all old tracks
        let old_ids = v0::TracksIds::<T, I>::get();
        let mut tracks_data: Vec<(TrackIdOf<T, I>, TrackInfoOf<T, I>)> = Vec::new();

        for old_id in old_ids.iter() {
            reads += 1;
            if let Some(info) = v0::Tracks::<T, I>::get(old_id) {
                tracks_data.push((*old_id, info));
            } else {
                log::warn!(
                    target: LOG_TARGET,
                    "Track {:?} found in TracksIds but not in Tracks storage, skipping",
                    old_id
                );
            }
        }

        // 2. Clear old Tracks storage
        let removed = v0::Tracks::<T, I>::clear(old_ids.len() as u32, None);
        reads += removed.loops as u64;
        writes += removed.unique as u64;

        // 3. Write new storage
        let mut new_ids = BoundedBTreeSet::<TrackIdOf<T, I>, T::MaxTracks>::new();
        let mut max_group = GroupIdOf::<T, I>::default();

        for (old_id, info) in &tracks_data {
            // Old track IDs should be small values that fit in the Half type.
            // split() puts upper bits in first, lower bits in second.
            // For small IDs (< Half::MAX), upper bits are 0 and lower bits = the ID.
            let (upper, old_as_half) = old_id.split();

            // Defensive: verify upper half is zero (old ID fits in Half range).
            // If not, the ID would lose bits during migration.
            if upper != GroupIdOf::<T, I>::default() {
                log::error!(
                    target: LOG_TARGET,
                    "Track {:?} has non-zero upper half {:?}, cannot safely migrate — skipping",
                    old_id, upper
                );
                continue;
            }

            let group = old_as_half;
            let sub = SubTrackIdOf::<T, I>::default();
            let new_id = T::TrackId::combine(group.clone(), sub.clone());

            // Write to new Tracks DoubleMap
            super::Tracks::<T, I>::insert(&group, &sub, info);
            writes += 1;

            // Track the new combined ID
            let _ = new_ids.try_insert(new_id);

            // Track max group for NextGroupId calculation
            if group > max_group {
                max_group = group.clone();
            }

            log::info!(
                target: LOG_TARGET,
                "Migrated track {:?} -> new id {:?}",
                old_id, new_id
            );
        }

        // 4. Write new TracksIds
        super::TracksIds::<T, I>::put(new_ids);
        writes += 1;

        // 5. Collect OriginToTrackId entries first, then mutate
        // (avoids iterating storage while mutating it)
        let origin_entries: Vec<_> = OriginToTrackId::<T, I>::iter().collect();
        reads += origin_entries.len() as u64;

        for (key, old_id) in origin_entries {
            let (upper, old_as_half) = old_id.split();

            // Skip entries with IDs that don't fit in Half (same defensive check)
            if upper != GroupIdOf::<T, I>::default() {
                log::error!(
                    target: LOG_TARGET,
                    "OriginToTrackId entry {:?} has non-zero upper half, skipping",
                    old_id
                );
                continue;
            }

            let new_id = T::TrackId::combine(old_as_half, SubTrackIdOf::<T, I>::default());

            if new_id != old_id {
                OriginToTrackId::<T, I>::insert(&key, new_id);
                writes += 1;
            }

            TrackIdToOrigin::<T, I>::insert(new_id, &key);
            writes += 1;
        }

        // 6. Set NextGroupId (max_group tracks the highest group = highest old ID as half)
        let next_group = max_group
            .increment()
            .expect("NextGroupId overflow: too many groups to migrate");
        NextGroupId::<T, I>::put(next_group.clone());
        writes += 1;

        log::info!(
            target: LOG_TARGET,
            "Migration complete: {} tracks migrated, NextGroupId set to {:?}",
            tracks_data.len(),
            next_group
        );

        T::DbWeight::get().reads_writes(reads, writes)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
        let old_ids = v0::TracksIds::<T, I>::get();
        let count = old_ids.len() as u32;

        // Verify all old IDs fit in the Half range
        for old_id in old_ids.iter() {
            let (upper, _) = old_id.split();
            frame_support::ensure!(
                upper == GroupIdOf::<T, I>::default(),
                sp_runtime::TryRuntimeError::Other(
                    "Old track ID exceeds Half range, migration would lose data"
                )
            );
        }

        log::info!(
            target: LOG_TARGET,
            "Pre-upgrade: {} tracks to migrate",
            count
        );
        Ok(count.encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
        let expected_count = u32::decode(&mut &state[..]).map_err(|_| {
            sp_runtime::TryRuntimeError::Other("Failed to decode pre-upgrade state")
        })?;

        let new_ids = super::TracksIds::<T, I>::get();
        let actual_count = new_ids.len() as u32;

        frame_support::ensure!(
            expected_count == actual_count,
            sp_runtime::TryRuntimeError::Other("Track count mismatch after migration")
        );

        // Verify all tracks are accessible via new DoubleMap
        for id in new_ids.iter() {
            let (group, sub) = id.split();
            frame_support::ensure!(
                super::Tracks::<T, I>::contains_key(&group, &sub),
                sp_runtime::TryRuntimeError::Other("Track not found in new storage")
            );
        }

        // Verify reverse mapping is populated
        for (_, id) in OriginToTrackId::<T, I>::iter() {
            frame_support::ensure!(
                TrackIdToOrigin::<T, I>::contains_key(id),
                sp_runtime::TryRuntimeError::Other("TrackIdToOrigin missing entry")
            );
        }

        // Verify NextGroupId is set correctly
        let next_group = NextGroupId::<T, I>::get();
        frame_support::ensure!(
            next_group != GroupIdOf::<T, I>::default() || actual_count == 0,
            sp_runtime::TryRuntimeError::Other("NextGroupId not initialized after migration")
        );

        log::info!(
            target: LOG_TARGET,
            "Post-upgrade: {} tracks verified, NextGroupId = {:?}",
            actual_count,
            next_group
        );
        Ok(())
    }
}

/// Versioned migration wrapper. Only runs if on-chain version is 0 and sets it to 1.
pub type MigrateToV1<T, I = ()> = frame_support::migrations::VersionedMigration<
    0,
    1,
    MigrateV0ToV1<T, I>,
    Pallet<T, I>,
    <T as frame_system::Config>::DbWeight,
>;
