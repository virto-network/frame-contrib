// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Benchmarks for referenda-tracks pallet

use super::*;
use crate::{
    Event, OriginToTrackId, Pallet as ReferendaTracks, TrackIdToOrigin, Tracks, TracksIds,
    UpdateType,
};
use codec::Encode;
use frame_benchmarking::v2::*;
use frame_support::traits::{EnsureOriginWithArg, Incrementable};
use frame_system::RawOrigin;
use pallet_referenda::{Curve, TrackInfo, TrackInfoOf};
use sp_core::Get;
use sp_runtime::{str_array as s, traits::AtLeast32Bit, Perbill};

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type BalanceOf<T, I> =
    <<T as pallet_referenda::Config<I>>::Currency as frame_support::traits::Currency<
        AccountIdOf<T>,
    >>::Balance;

fn assert_last_event<T: Config<I>, I: 'static>(
    generic_event: <T as frame_system::Config>::RuntimeEvent,
) {
    frame_system::Pallet::<T>::assert_last_event(generic_event);
}

fn track_info_of<T, I: 'static>() -> TrackInfoOf<T, I>
where
    T: pallet_referenda::Config<I>,
    BalanceOf<T, I>: AtLeast32Bit,
{
    TrackInfo {
        name: s("Test Track"),
        max_deciding: 1,
        decision_deposit: 0u32.into(),
        prepare_period: 10u32.into(),
        decision_period: 100u32.into(),
        confirm_period: 10u32.into(),
        min_enactment_period: 2u32.into(),
        min_approval: Curve::LinearDecreasing {
            length: Perbill::from_percent(100),
            floor: Perbill::from_percent(50),
            ceil: Perbill::from_percent(100),
        },
        min_support: Curve::LinearDecreasing {
            length: Perbill::from_percent(100),
            floor: Perbill::from_percent(0),
            ceil: Perbill::from_percent(50),
        },
    }
}

fn max_tracks<T: Config<I>, I: 'static>() -> u32 {
    T::MaxTracks::get()
}

/// The most that the sub-tracks of `remove_group` take in the benchmark.
///
/// It is a constant and not `MaxTracks`: Kreivo has `MaxTracks = 65_536`, and a group that grew
/// up to it would be built on every one of the runs of the benchmark (50 steps x 20 repeats),
/// and removed. The cost of the call is linear in the sub-tracks of the group, so the weight that
/// is generated from `1..=MAX_COMPONENT` extrapolates to any value above it with the same slope.
/// What does not scale with the group but with `MaxTracks` (the one rewrite of the `TracksIds`
/// value, which holds up to `MaxTracks` ids) is part of the intercept, so the set-up fills that
/// value to `MaxTracks` with [`put_ids`], which writes it only once.
const MAX_COMPONENT: u32 = 1_024;

/// The upper bound of the sub-tracks of `remove_group`: `MAX_COMPONENT`, or `MaxTracks` if it
/// is lower.
fn max_component<T: Config<I>, I: 'static>() -> u32 {
    max_tracks::<T, I>().min(MAX_COMPONENT)
}

/// Writes `TracksIds` in one go. Inserting id by id (`TracksIds::mutate` in a loop) decodes and
/// encodes the whole value each time, which is quadratic in the number of ids.
fn put_ids<T: Config<I>, I: 'static>(ids: alloc::collections::BTreeSet<TrackIdOf<T, I>>) {
    TracksIds::<T, I>::put(
        frame_support::BoundedBTreeSet::try_from(ids).expect("within MaxTracks"),
    );
}

/// Adds ids from the benchmark helper to `ids` until it holds `fill` of them, leaving out the
/// ones that `skip` selects. The tracks they stand for are not stored: of the `MaxTracks` ids in
/// the `TracksIds` value, only its size counts to the cost of the calls that read and write it,
/// and the lookups in the other maps do not depend on how many entries those hold.
fn filler_ids<T: Config<I>, I: 'static>(
    mut ids: alloc::collections::BTreeSet<TrackIdOf<T, I>>,
    fill: u32,
    skip: impl Fn(&TrackIdOf<T, I>) -> bool,
) -> alloc::collections::BTreeSet<TrackIdOf<T, I>> {
    for i in 0..max_tracks::<T, I>() {
        if ids.len() as u32 >= fill {
            break;
        }
        let id = T::BenchmarkHelper::track_id(i);
        if !skip(&id) {
            ids.insert(id);
        }
    }
    ids
}

/// A single track, for the calls that update it. They read and write that `Tracks` entry and
/// nothing else, so the cost does not depend on how many tracks there are. It is the last id
/// that the benchmark helper can give, so that the origin that is asked for (see
/// `try_successful_origin`) is the one for a runtime's highest track.
fn prepare_track<T: Config<I>, I: 'static>() -> TrackIdOf<T, I> {
    let id = T::BenchmarkHelper::track_id(max_tracks::<T, I>() - 1);
    let caller_origin: PalletsOriginOf<T> = RawOrigin::Signed(whitelisted_caller()).into();
    ReferendaTracks::<T, I>::do_insert(id, track_info_of::<T, I>(), caller_origin)
        .expect("inserts the track");
    id
}

/// Fills `TracksIds` with `MaxTracks - 1` ids, which leaves room for the one that the call
/// adds. Returns the number of ids.
fn prepare_full_ids<T: Config<I>, I: 'static>() -> u32 {
    let ids = filler_ids::<T, I>(Default::default(), max_tracks::<T, I>() - 1, |_| false);
    let len = ids.len() as u32;
    put_ids::<T, I>(ids);
    len
}

/// The worst case of `remove`: `TracksIds` is full (`MaxTracks` ids), and the last of them is the
/// track to remove, inserted as the pallet does. Returns its id.
fn prepare_removable_track<T: Config<I>, I: 'static>() -> TrackIdOf<T, I> {
    prepare_full_ids::<T, I>();
    let id = T::BenchmarkHelper::track_id(max_tracks::<T, I>() - 1);
    let caller_origin: PalletsOriginOf<T> = RawOrigin::Signed(whitelisted_caller()).into();
    ReferendaTracks::<T, I>::do_insert(id, track_info_of::<T, I>(), caller_origin)
        .expect("inserts last track");
    id
}

/// Occupies groups with one track each, to build the worst case of the group allocator:
/// the extrinsic has to skip every one of them.
///
/// The groups are taken the way the allocator does, one after the other from `NextGroupId`, so
/// the number of them is bounded by what the types allow, not by `MaxTracks` alone: the group
/// half of a `TrackId` may hold fewer than `MaxTracks` groups (a `u32` id has `u16` groups, and
/// the allocator never hands out the group `0`). It fills until it reaches `MaxTracks - 1` groups
/// (the last track is for the extrinsic) or until just one group is left, whichever comes first.
/// It does not call the allocator, whose check for a group that is taken is a storage scan: there
/// are no tracks yet, and it is the extrinsic that has to run it over all of them.
///
/// `NextGroupId` is not moved, as when the runtime chose those groups itself (see
/// [`Pallet::do_insert`]): the allocator starts over and skips all of them.
///
/// Returns the number of groups occupied, which is also the number of tracks. It assumes that
/// there are no tracks yet.
fn prefill_groups<T: Config<I>, I: 'static>() -> u32 {
    let track = track_info_of::<T, I>();
    let mut group = NextGroupId::<T, I>::get();
    let mut ids = alloc::collections::BTreeSet::new();

    while (ids.len() as u32) < max_tracks::<T, I>() - 1 {
        // Two groups after the last one taken means that, once this one is taken, there is still
        // one left.
        let Some(next) = group.increment() else {
            break;
        };
        if next.increment().is_none() {
            break;
        }
        group = next;

        let id = T::TrackId::combine(group.clone(), SubTrackIdOf::<T, I>::default());
        let origin: PalletsOriginOf<T> =
            RawOrigin::Signed(frame_benchmarking::account("origin", ids.len() as u32, 0)).into();
        Tracks::<T, I>::insert(
            group.clone(),
            SubTrackIdOf::<T, I>::default(),
            track.clone(),
        );
        OriginToTrackId::<T, I>::insert(origin.clone(), id);
        TrackIdToOrigin::<T, I>::insert(id, origin);
        ids.insert(id);
    }

    let prefilled = ids.len() as u32;
    put_ids::<T, I>(ids);
    prefilled
}

/// Converts every old id to the first group. The benchmarks measure storage accesses, not values.
struct AnyGroup<T, I>(core::marker::PhantomData<(T, I)>);
impl<T: Config<I>, I: 'static> sp_runtime::traits::Convert<u16, Option<GroupIdOf<T, I>>>
    for AnyGroup<T, I>
{
    fn convert(_: u16) -> Option<GroupIdOf<T, I>> {
        Some(Default::default())
    }
}

/// The raw key of a `Tracks` entry in the shape of storage version 0.
fn v0_track_key<T: Config<I>, I: 'static>(id: u16) -> alloc::vec::Vec<u8> {
    let encoded = id.encode();
    let mut key = crate::migration::storage_prefix_of::<T, I>(b"Tracks").to_vec();
    key.extend_from_slice(&sp_io::hashing::blake2_128(&encoded));
    key.extend_from_slice(&encoded);
    key
}

#[instance_benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    pub fn new_group_with_track() -> Result<(), BenchmarkError> {
        // The origin to associate with the new group does not depend on the origin that dispatches.
        let origin: PalletsOriginOf<T> = RawOrigin::Signed(whitelisted_caller()).into();
        let caller = T::CreateOrigin::try_successful_origin(&origin)
            .map_err(|_| BenchmarkError::Weightless)?;

        // The worst case of the allocator: it skips as many groups with a track as it can,
        // which is `MaxTracks - 1` if the groups are that many, or all but one if they are not.
        let prefilled = prefill_groups::<T, I>();
        let track = track_info_of::<T, I>();

        // Not `Weightless`: the origin is available, so the call has to be dispatched.
        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, origin, track);

        assert_eq!(
            TracksIds::<T, I>::get().len() as u32,
            prefilled + 1,
            "the extrinsic creates exactly one track"
        );
        Ok(())
    }

    #[benchmark]
    pub fn add_sub_track() -> Result<(), BenchmarkError> {
        let track = track_info_of::<T, I>();
        let sub_origin: PalletsOriginOf<T> = RawOrigin::Signed(whitelisted_caller()).into();
        let caller = T::GroupManagerCreateOrigin::try_successful_origin(&sub_origin)
            .map_err(|_| BenchmarkError::Weightless)?;
        // Adding a track reads and writes `TracksIds`, which is at its largest: all but one of
        // its `MaxTracks` ids are in. The one that is not is the id that the call creates (the
        // benchmark helper may give ids of that group), so that it adds a new id and not one
        // that was in already.
        let group = T::GroupManagerCreateOrigin::ensure_origin(caller.clone(), &sub_origin)
            .map_err(|_| BenchmarkError::Weightless)?;
        let created = T::TrackId::combine(
            group.clone(),
            NextSubTrackId::<T, I>::get(&group)
                .increment()
                .expect("a sub-track is left"),
        );
        let ids = filler_ids::<T, I>(Default::default(), max_tracks::<T, I>() - 1, |id| {
            *id == created
        });
        let prefilled = ids.len() as u32;
        assert_eq!(
            prefilled,
            max_tracks::<T, I>() - 1,
            "the value is full but for one"
        );
        put_ids::<T, I>(ids);

        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, sub_origin, track);

        assert_eq!(
            TracksIds::<T, I>::get().len() as u32,
            prefilled + 1,
            "the extrinsic creates exactly one track"
        );
        Ok(())
    }

    #[benchmark]
    pub fn remove() -> Result<(), BenchmarkError> {
        let id = prepare_removable_track::<T, I>();
        let origin =
            T::RemoveOrigin::try_successful_origin(&id).map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, id);

        assert_last_event::<T, I>(Event::Removed { id }.into());
        Ok(())
    }

    #[benchmark]
    pub fn set_decision_deposit() -> Result<(), BenchmarkError> {
        let id = prepare_track::<T, I>();
        let caller = T::GroupManagerOrigin::try_successful_origin(&id)
            .map_err(|_| BenchmarkError::Weightless)?;
        let deposit: BalanceOf<T, I> = 1000u32.into();

        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, id, deposit);

        assert_last_event::<T, I>(
            Event::Updated {
                id,
                update_type: UpdateType::DecisionDeposit,
            }
            .into(),
        );
        Ok(())
    }

    #[benchmark]
    pub fn set_periods() -> Result<(), BenchmarkError> {
        let id = prepare_track::<T, I>();
        let caller = T::GroupManagerOrigin::try_successful_origin(&id)
            .map_err(|_| BenchmarkError::Weightless)?;
        let prepare_period: BlockNumberFor<T, I> = 20u32.into();
        let decision_period: BlockNumberFor<T, I> = 200u32.into();

        #[extrinsic_call]
        _(
            caller as T::RuntimeOrigin,
            id,
            Some(prepare_period),
            Some(decision_period),
            None,
            None,
        );

        assert_last_event::<T, I>(
            Event::Updated {
                id,
                update_type: UpdateType::Periods,
            }
            .into(),
        );
        Ok(())
    }

    #[benchmark]
    pub fn set_curves() -> Result<(), BenchmarkError> {
        let id = prepare_track::<T, I>();
        let caller = T::GroupManagerOrigin::try_successful_origin(&id)
            .map_err(|_| BenchmarkError::Weightless)?;
        let curve = Curve::LinearDecreasing {
            length: Perbill::from_percent(80),
            floor: Perbill::from_percent(40),
            ceil: Perbill::from_percent(90),
        };

        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, id, Some(curve), None);

        assert_last_event::<T, I>(
            Event::Updated {
                id,
                update_type: UpdateType::Curves,
            }
            .into(),
        );
        Ok(())
    }

    /// `n` is the number of tracks in the group: its first track plus `n - 1` sub-tracks.
    ///
    /// The cost of the call is linear in `n` (a read of the group's keys, then a few reads and
    /// writes per track), plus one rewrite of `TracksIds`, whose size is that of `MaxTracks`
    /// ids. `n` goes up to `MAX_COMPONENT` and not to `MaxTracks`, and the set-up fills
    /// `TracksIds` to `MaxTracks` so that the intercept has it: the weight function extrapolates
    /// the slope to a group of any size.
    #[benchmark]
    pub fn remove_group(n: Linear<1, { max_component::<T, I>() }>) -> Result<(), BenchmarkError> {
        let track = track_info_of::<T, I>();
        // Set-up only: the entries are written directly, as the pallet would write them, but
        // `TracksIds` is written once, at the end.
        let first = ReferendaTracks::<T, I>::next_group_track_id().expect("a group is available");
        let (group, _) = first.split();
        let mut group_ids = alloc::collections::BTreeSet::new();

        // The sub-tracks of a group can be fewer than `MaxTracks` (a `u32` id has `u16` of them,
        // and the first one is the track of the group), so `n` is clamped to what the type
        // allows: the group is filled until the allocator has no sub-track left.
        let mut id = first;
        let mut count = 0;
        loop {
            let (_, sub) = id.split();
            let origin: PalletsOriginOf<T> =
                RawOrigin::Signed(account("sub_track", count, 0)).into();
            Tracks::<T, I>::insert(group.clone(), sub, track.clone());
            OriginToTrackId::<T, I>::insert(origin.clone(), id);
            TrackIdToOrigin::<T, I>::insert(id, origin);
            group_ids.insert(id);
            count += 1;
            if count >= n {
                break;
            }
            let Some(next_id) = ReferendaTracks::<T, I>::next_sub_track_id(group.clone()) else {
                break;
            };
            id = next_id;
        }
        let n = count;
        let in_group = group.clone();
        put_ids::<T, I>(filler_ids::<T, I>(
            group_ids,
            max_tracks::<T, I>(),
            move |id| id.split().0 == in_group,
        ));
        assert_eq!(Tracks::<T, I>::iter_key_prefix(&group).count() as u32, n);

        let caller = T::RemoveGroupOrigin::try_successful_origin(&group)
            .map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, group.clone(), n);

        assert_eq!(Tracks::<T, I>::iter_key_prefix(&group).count(), 0);
        assert!(TracksIds::<T, I>::get()
            .iter()
            .all(|id| id.split().0 != group));
        Ok(())
    }

    #[benchmark]
    pub fn set_max_deciding() -> Result<(), BenchmarkError> {
        let id = prepare_track::<T, I>();
        let caller = T::GroupManagerOrigin::try_successful_origin(&id)
            .map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, id, 5u32);

        assert_last_event::<T, I>(
            Event::Updated {
                id,
                update_type: UpdateType::MaxDeciding,
            }
            .into(),
        );
        Ok(())
    }

    /// One unit of the first phase of `MigrateV0ToV1`.
    #[benchmark]
    pub fn migrate_track() {
        use crate::migration::{Cursor, MigrateV0ToV1, Phase};
        use frame_support::{migrations::SteppedMigration, weights::WeightMeter};

        // A track in the shape of storage version 0 (a `u16` id).
        let old_id = 1u16;
        let raw_key = v0_track_key::<T, I>(old_id);
        frame_support::storage::unhashed::put_raw(&raw_key, &track_info_of::<T, I>().encode());
        // The meter fits one unit, and no more.
        let mut meter = WeightMeter::with_limit(<T as Config<I>>::WeightInfo::migrate_track());
        let cursor: Option<Cursor> = Some((Phase::Tracks, Default::default()));

        #[block]
        {
            assert!(matches!(
                MigrateV0ToV1::<T, I, u16, AnyGroup<T, I>>::step(cursor, &mut meter),
                Ok(Some(_))
            ));
        }

        let (group, sub) = (
            GroupIdOf::<T, I>::default(),
            SubTrackIdOf::<T, I>::default(),
        );
        assert!(Tracks::<T, I>::contains_key(group, sub));
        assert!(!frame_support::storage::unhashed::exists(&raw_key));
    }

    /// One unit of the second phase of `MigrateV0ToV1`.
    #[benchmark]
    pub fn migrate_origin() {
        use crate::migration::{Cursor, MigrateV0ToV1, Phase};
        use frame_support::{migrations::SteppedMigration, weights::WeightMeter};

        // The track was already migrated, and its origin still has the old value.
        let (group, sub) = (
            GroupIdOf::<T, I>::default(),
            SubTrackIdOf::<T, I>::default(),
        );
        Tracks::<T, I>::insert(group.clone(), sub.clone(), track_info_of::<T, I>());
        let origin: PalletsOriginOf<T> = RawOrigin::Signed(whitelisted_caller()).into();
        frame_support::storage::unhashed::put(
            &OriginToTrackId::<T, I>::hashed_key_for(&origin),
            &1u16,
        );
        let mut meter = WeightMeter::with_limit(<T as Config<I>>::WeightInfo::migrate_origin());
        let cursor: Option<Cursor> = Some((Phase::Origins, Default::default()));

        #[block]
        {
            assert!(matches!(
                MigrateV0ToV1::<T, I, u16, AnyGroup<T, I>>::step(cursor, &mut meter),
                Ok(Some(_))
            ));
        }

        let id = T::TrackId::combine(group, sub);
        assert_eq!(OriginToTrackId::<T, I>::get(&origin), Some(id));
        assert_eq!(TrackIdToOrigin::<T, I>::get(id), Some(origin));
    }

    /// One unit of the third phase of `MigrateV0ToV1`, with `TracksIds` holding `n` ids.
    ///
    /// `n` goes up to `MaxTracks`, not to `MAX_COMPONENT`: the unit is declared at `MaxTracks`
    /// ids (see `MigrateV0ToV1::unit`) and has to fit one block, so it is measured there, not
    /// extrapolated. The set-up writes the ids in one go, and a run takes milliseconds.
    #[benchmark]
    pub fn migrate_ids(n: Linear<1, { max_tracks::<T, I>() }>) {
        use crate::migration::{Cursor, MigrateV0ToV1, Phase};
        use frame_support::{migrations::SteppedMigration, weights::WeightMeter};

        // The track to add is `track_id(0)`; the other `n - 1` are in already.
        let (group, sub) = T::BenchmarkHelper::track_id(0).split();
        Tracks::<T, I>::insert(group.clone(), sub, track_info_of::<T, I>());
        TracksIds::<T, I>::mutate(|ids| {
            for i in 1..n {
                ids.try_insert(T::BenchmarkHelper::track_id(i))
                    .expect("within MaxTracks");
            }
        });
        // The unit is declared at `MaxTracks` (and the proof size of `TracksIds` at its largest);
        // the meter fits that, and no more.
        let mut meter = WeightMeter::with_limit(MigrateV0ToV1::<T, I, u16, AnyGroup<T, I>>::unit(
            Phase::TracksIds,
        ));
        let cursor: Option<Cursor> = Some((Phase::TracksIds, Default::default()));

        #[block]
        {
            assert!(matches!(
                MigrateV0ToV1::<T, I, u16, AnyGroup<T, I>>::step(cursor, &mut meter),
                Ok(Some(_))
            ));
        }

        assert_eq!(TracksIds::<T, I>::get().len() as u32, n);
    }

    /// The last unit of `MigrateV0ToV1`.
    #[benchmark]
    pub fn migrate_finalize() {
        use crate::migration::{Cursor, MigrateV0ToV1, Phase};
        use frame_support::{
            migrations::SteppedMigration,
            traits::{GetStorageVersion, StorageVersion},
            weights::WeightMeter,
        };

        StorageVersion::new(0).put::<ReferendaTracks<T, I>>();
        let mut meter = WeightMeter::new();
        let cursor: Option<Cursor> = Some((Phase::Finish, Default::default()));

        #[block]
        {
            assert!(matches!(
                MigrateV0ToV1::<T, I, u16, AnyGroup<T, I>>::step(cursor, &mut meter),
                Ok(None)
            ));
        }

        assert_eq!(ReferendaTracks::<T, I>::on_chain_storage_version(), 1);
    }

    /// Tests for the origins the benchmarks dispatch with.
    #[cfg(test)]
    mod origin_tests {
        use super::*;
        use crate::mock::{new_test_ext, CreateIsNever, Test};

        /// A runtime that binds `CreateOrigin` to `EnsureNever` has no origin to dispatch
        /// `new_group_with_track` with: the benchmark is `Weightless`, not a failure.
        #[test]
        fn unreachable_call_is_weightless() {
            new_test_ext(None).execute_with(|| {
                CreateIsNever::set(true);
                assert!(matches!(
                    Pallet::<Test>::test_bench_by_name(b"new_group_with_track"),
                    Err(BenchmarkError::Weightless)
                ));
            });
        }

        #[test]
        fn reachable_call_is_benchmarked() {
            new_test_ext(None).execute_with(|| {
                CreateIsNever::set(false);
                assert!(Pallet::<Test>::test_bench_by_name(b"new_group_with_track").is_ok());
            });
        }
    }

    /// Tests for runtimes whose group space is smaller than `MaxTracks`.
    #[cfg(test)]
    mod group_space_tests {
        use super::*;
        use crate::{
            mock::{self, new_test_ext},
            mock_small,
        };

        const NAMES: [&[u8]; 12] = [
            b"new_group_with_track",
            b"add_sub_track",
            b"remove",
            b"set_decision_deposit",
            b"set_periods",
            b"set_curves",
            b"remove_group",
            b"set_max_deciding",
            b"migrate_track",
            b"migrate_origin",
            b"migrate_ids",
            b"migrate_finalize",
        ];

        /// `MaxTracks` fits in the groups: all but the last track are prefilled.
        #[test]
        fn prefill_fills_max_tracks_minus_one() {
            new_test_ext(None).execute_with(|| {
                assert_eq!(prefill_groups::<mock::Test, ()>(), 254);
                assert_eq!(TracksIds::<mock::Test>::get().len(), 254);
                assert_eq!(NextGroupId::<mock::Test>::get(), 0);
            });
        }

        /// The Kreivo case: `MaxTracks` is `65_536` and the groups are `u16`, `65_535` of them
        /// allocatable. One is left for the extrinsic.
        #[test]
        fn u16_groups_with_65_536_max_tracks() {
            new_test_ext(None).execute_with(|| {
                mock::MaxTracks::set(65_536);
                assert_eq!(prefill_groups::<mock::Test, ()>(), 65_534);
            });
            new_test_ext(None).execute_with(|| {
                mock::MaxTracks::set(65_536);
                assert!(Pallet::<mock::Test>::test_bench_by_name(b"new_group_with_track").is_ok());
            });
        }

        /// `MaxTracks` is `300` and the groups are `u8`, `255` of them allocatable.
        #[test]
        fn u8_groups_with_more_max_tracks() {
            mock_small::new_test_ext().execute_with(|| {
                assert_eq!(prefill_groups::<mock_small::Test, ()>(), 254);
                assert_eq!(NextGroupId::<mock_small::Test>::get(), 0);
                // The one that is left is for the extrinsic.
                let id = ReferendaTracks::<mock_small::Test>::next_group_track_id();
                assert_eq!(id.map(|id| id.split().0), Some(255));
                assert!(ReferendaTracks::<mock_small::Test>::next_group_track_id().is_none());
            });
        }

        /// Wall-clock of each benchmark at Kreivo's configuration (`MaxTracks = 65_536`, `u32`
        /// ids), at the lowest and the highest value of its components. Run it with
        /// `cargo test -p fc-pallet-referenda-tracks --all-features kreivo_scale_timing -- --ignored --nocapture`.
        #[test]
        #[ignore = "timing harness, prints per-benchmark wall-clock"]
        fn kreivo_scale_timing() {
            let only = std::env::var("ONLY").ok();
            for name in NAMES {
                let name_str = core::str::from_utf8(name).unwrap();
                if only.as_deref().is_some_and(|o| o != name_str) {
                    continue;
                }
                new_test_ext(None).execute_with(|| {
                    mock::MaxTracks::set(65_536);
                    let start = std::time::Instant::now();
                    let result = Pallet::<mock::Test>::test_bench_by_name(name);
                    std::println!("{name_str}: {:?} ({:?})", start.elapsed(), result.is_ok());
                });
            }
        }

        /// Kreivo's configuration (`MaxTracks = 65_536`, `u32` ids): every benchmark runs, with
        /// the set-ups and the components that are bounded to make a run cheap. The test is
        /// not about the time (see `kreivo_scale_timing`), but it would take hours, not
        /// seconds, if a set-up grew with `MaxTracks` again.
        #[test]
        fn every_benchmark_runs_at_kreivo_scale() {
            for name in NAMES {
                new_test_ext(None).execute_with(|| {
                    mock::MaxTracks::set(65_536);
                    assert!(
                        Pallet::<mock::Test>::test_bench_by_name(name).is_ok(),
                        "{}",
                        core::str::from_utf8(name).unwrap()
                    );
                });
            }
        }

        #[test]
        fn every_benchmark_runs_with_u8_groups() {
            for name in NAMES {
                mock_small::new_test_ext().execute_with(|| {
                    assert!(
                        Pallet::<mock_small::Test>::test_bench_by_name(name).is_ok(),
                        "{}",
                        core::str::from_utf8(name).unwrap()
                    );
                });
            }
        }
    }

    impl_benchmark_test_suite!(
        ReferendaTracks,
        crate::mock::new_test_ext(None),
        crate::mock::Test
    );
}
