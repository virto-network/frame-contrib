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
use frame_support::traits::EnsureOriginWithArg;
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
    pub fn new_group_with_track() {
        // The worst case of the allocator: every group from the first one up to the last-but-one
        // that fits has a track, so it skips as many as `MaxTracks` allows.
        let track = track_info_of::<T, I>();
        let mut group = GroupIdOf::<T, I>::default();
        for i in 0..max_tracks::<T, I>() - 1 {
            group = frame_support::traits::Incrementable::increment(&group).expect("fits a group");
            let id = T::TrackId::combine(group.clone(), SubTrackIdOf::<T, I>::default());
            let origin: PalletsOriginOf<T> =
                RawOrigin::Signed(frame_benchmarking::account("origin", i, 0)).into();
            TracksIds::<T, I>::mutate(|ids| {
                ids.try_insert(id).expect("within MaxTracks");
            });
            Tracks::<T, I>::insert(
                group.clone(),
                SubTrackIdOf::<T, I>::default(),
                track.clone(),
            );
            OriginToTrackId::<T, I>::insert(origin.clone(), id);
            TrackIdToOrigin::<T, I>::insert(id, origin);
        }

        let origin: PalletsOriginOf<T> = RawOrigin::Signed(whitelisted_caller()).into();

        #[extrinsic_call]
        _(RawOrigin::Root, origin, track);

        assert_eq!(TracksIds::<T, I>::get().len() as u32, max_tracks::<T, I>());
    }

    #[benchmark]
    pub fn add_sub_track() {
        let track = track_info_of::<T, I>();
        let sub_origin: PalletsOriginOf<T> = RawOrigin::Signed(whitelisted_caller()).into();

        #[extrinsic_call]
        _(RawOrigin::Root, sub_origin, track);
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
    pub fn set_decision_deposit() {
        let id = prepare_tracks::<T, I>(true).expect("full prepare returns id");
        let deposit: BalanceOf<T, I> = 1000u32.into();

        #[extrinsic_call]
        _(RawOrigin::Root, id, deposit);

        assert_last_event::<T, I>(
            Event::Updated {
                id,
                update_type: UpdateType::DecisionDeposit,
            }
            .into(),
        );
    }

    #[benchmark]
    pub fn set_periods() {
        let id = prepare_tracks::<T, I>(true).expect("full prepare returns id");
        let prepare_period: BlockNumberFor<T, I> = 20u32.into();
        let decision_period: BlockNumberFor<T, I> = 200u32.into();

        #[extrinsic_call]
        _(
            RawOrigin::Root,
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
    }

    #[benchmark]
    pub fn set_curves() {
        let id = prepare_tracks::<T, I>(true).expect("full prepare returns id");
        let curve = Curve::LinearDecreasing {
            length: Perbill::from_percent(80),
            floor: Perbill::from_percent(40),
            ceil: Perbill::from_percent(90),
        };

        #[extrinsic_call]
        _(RawOrigin::Root, id, Some(curve), None);

        assert_last_event::<T, I>(
            Event::Updated {
                id,
                update_type: UpdateType::Curves,
            }
            .into(),
        );
    }

    /// `n` is the number of tracks in the group: its first track plus `n - 1` sub-tracks.
    #[benchmark]
    pub fn remove_group(n: Linear<1, { max_tracks::<T, I>() }>) {
        let track = track_info_of::<T, I>();
        let origin: PalletsOriginOf<T> = RawOrigin::Signed(whitelisted_caller()).into();
        ReferendaTracks::<T, I>::new_group_with_track(
            RawOrigin::Root.into(),
            origin,
            track.clone(),
        )
        .expect("inserts track");

        let id = TracksIds::<T, I>::get()
            .into_iter()
            .next()
            .expect("at least one track");
        let (group, _) = id.split();

        for i in 1..n {
            let sub_id = ReferendaTracks::<T, I>::next_sub_track_id(group.clone())
                .expect("sub-track id available");
            let sub_origin: PalletsOriginOf<T> =
                RawOrigin::Signed(account("sub_track", i, 0)).into();
            ReferendaTracks::<T, I>::do_insert(sub_id, track.clone(), sub_origin)
                .expect("inserts sub-track");
        }
        assert_eq!(Tracks::<T, I>::iter_key_prefix(&group).count() as u32, n);

        #[extrinsic_call]
        _(RawOrigin::Root, group.clone(), n);

        assert_eq!(Tracks::<T, I>::iter_key_prefix(&group).count(), 0);
    }

    #[benchmark]
    pub fn set_max_deciding() {
        let id = prepare_tracks::<T, I>(true).expect("full prepare returns id");

        #[extrinsic_call]
        _(RawOrigin::Root, id, 5u32);

        assert_last_event::<T, I>(
            Event::Updated {
                id,
                update_type: UpdateType::MaxDeciding,
            }
            .into(),
        );
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

    impl_benchmark_test_suite!(
        ReferendaTracks,
        crate::mock::new_test_ext(None),
        crate::mock::Test
    );
}
