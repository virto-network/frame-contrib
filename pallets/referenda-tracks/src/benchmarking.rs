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

/// Fill storage with tracks up to (MaxTracks - 1), inserted directly.
/// If `full`, also insert the last track using `do_insert`.
/// Returns the ID of the last track inserted (only when `full`).
fn prepare_tracks<T: Config<I>, I: 'static>(full: bool) -> Option<TrackIdOf<T, I>> {
    let track = track_info_of::<T, I>();

    for i in 0..max_tracks::<T, I>() - 1 {
        let id = T::BenchmarkHelper::track_id(i);
        let (group, sub) = id.split();
        let origin: PalletsOriginOf<T> =
            RawOrigin::Signed(frame_benchmarking::account("origin", i, 0)).into();

        TracksIds::<T, I>::mutate(|ids| {
            ids.try_insert(id).expect("within MaxTracks");
        });
        Tracks::<T, I>::insert(group, sub, track.clone());
        OriginToTrackId::<T, I>::insert(origin.clone(), id);
        TrackIdToOrigin::<T, I>::insert(id, origin);
    }

    if full {
        let id = T::BenchmarkHelper::track_id(max_tracks::<T, I>() - 1);
        let caller_origin: PalletsOriginOf<T> = RawOrigin::Signed(whitelisted_caller()).into();
        ReferendaTracks::<T, I>::do_insert(id, track, caller_origin).expect("inserts last track");
        Some(id)
    } else {
        None
    }
}

/// Occupies groups with one track each, to build the worst case of the group allocator:
/// the extrinsic has to skip every one of them.
///
/// The groups are taken from the allocator itself, so the number of them is bounded by what the
/// types allow, not by `MaxTracks` alone: the group half of a `TrackId` may hold fewer than
/// `MaxTracks` groups (a `u32` id has `u16` groups, and the allocator never hands out the group
/// `0`). The allocator fills until it reaches `MaxTracks - 1` groups (the last track is for the
/// extrinsic) or until just one group is left, whichever comes first.
///
/// Afterwards `NextGroupId` is back where it was, as when the runtime chose those groups itself
/// (see [`Pallet::do_insert`]): the allocator starts over and skips all of them.
///
/// Returns the number of groups occupied, which is also the number of tracks. It assumes that
/// there are no tracks yet.
fn prefill_groups<T: Config<I>, I: 'static>() -> u32 {
    let track = track_info_of::<T, I>();
    let first = NextGroupId::<T, I>::get();
    let mut ids = alloc::collections::BTreeSet::new();

    while (ids.len() as u32) < max_tracks::<T, I>() - 1 {
        // Peek without consuming: two groups after the last one handed out means that, once this
        // one is taken, there is still one left.
        if NextGroupId::<T, I>::get()
            .increment()
            .and_then(|group| group.increment())
            .is_none()
        {
            break;
        }

        let id = ReferendaTracks::<T, I>::next_group_track_id().expect("peeked: a group is left");
        let (group, sub) = id.split();
        let origin: PalletsOriginOf<T> =
            RawOrigin::Signed(frame_benchmarking::account("origin", ids.len() as u32, 0)).into();
        Tracks::<T, I>::insert(group, sub, track.clone());
        OriginToTrackId::<T, I>::insert(origin.clone(), id);
        TrackIdToOrigin::<T, I>::insert(id, origin);
        ids.insert(id);
    }

    let prefilled = ids.len() as u32;
    // Written once: inserting id by id would decode and encode the whole set each time.
    TracksIds::<T, I>::put(
        frame_support::BoundedBTreeSet::try_from(ids).expect("within MaxTracks"),
    );
    NextGroupId::<T, I>::put(first);
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

        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, sub_origin, track);

        Ok(())
    }

    #[benchmark]
    pub fn remove() -> Result<(), BenchmarkError> {
        let id = prepare_tracks::<T, I>(true).expect("full prepare returns id");
        let origin =
            T::RemoveOrigin::try_successful_origin(&id).map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, id);

        assert_last_event::<T, I>(Event::Removed { id }.into());
        Ok(())
    }

    #[benchmark]
    pub fn set_decision_deposit() -> Result<(), BenchmarkError> {
        let id = prepare_tracks::<T, I>(true).expect("full prepare returns id");
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
        let id = prepare_tracks::<T, I>(true).expect("full prepare returns id");
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
        let id = prepare_tracks::<T, I>(true).expect("full prepare returns id");
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
    #[benchmark]
    pub fn remove_group(n: Linear<1, { max_tracks::<T, I>() }>) -> Result<(), BenchmarkError> {
        let track = track_info_of::<T, I>();
        let origin: PalletsOriginOf<T> = RawOrigin::Signed(whitelisted_caller()).into();
        // Set-up only: insert through the pallet's internals, not through an origin.
        let first = ReferendaTracks::<T, I>::next_group_track_id().expect("a group is available");
        ReferendaTracks::<T, I>::do_insert(first, track.clone(), origin).expect("inserts track");

        let id = TracksIds::<T, I>::get()
            .into_iter()
            .next()
            .expect("at least one track");
        let (group, _) = id.split();

        // The sub-tracks of a group can be fewer than `MaxTracks` (a `u32` id has `u16` of them,
        // and the first one is the track of the group), so `n` is clamped to what the type
        // allows: the group is filled until the allocator has no sub-track left.
        let mut count = 1;
        while count < n {
            let Some(sub_id) = ReferendaTracks::<T, I>::next_sub_track_id(group.clone()) else {
                break;
            };
            let sub_origin: PalletsOriginOf<T> =
                RawOrigin::Signed(account("sub_track", count, 0)).into();
            ReferendaTracks::<T, I>::do_insert(sub_id, track.clone(), sub_origin)
                .expect("inserts sub-track");
            count += 1;
        }
        let n = count;
        assert_eq!(Tracks::<T, I>::iter_key_prefix(&group).count() as u32, n);

        let caller = T::RemoveGroupOrigin::try_successful_origin(&group)
            .map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, group.clone(), n);

        assert_eq!(Tracks::<T, I>::iter_key_prefix(&group).count(), 0);
        Ok(())
    }

    #[benchmark]
    pub fn set_max_deciding() -> Result<(), BenchmarkError> {
        let id = prepare_tracks::<T, I>(true).expect("full prepare returns id");
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
