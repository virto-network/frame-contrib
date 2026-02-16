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

//! Benchmarks for remarks pallet

use std::collections::BTreeSet;

use super::*;
use crate::{Event, OriginToTrackId, Pallet as ReferendaTracks, Tracks, TracksIds, UpdateType};
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use pallet_referenda::{Curve, TrackInfo, TrackInfoOf};
use sp_core::Get;
use sp_runtime::{str_array as s, traits::AtLeast32Bit, BoundedBTreeSet, Perbill};

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

fn max_track_id<T: Config<I>, I: 'static>() -> TrackIdOf<T, I> {
    T::BenchmarkHelper::track_id(max_tracks::<T, I>())
}

fn prepare_tracks<T: Config<I>, I: 'static>(full: bool) {
    let ids =
        BTreeSet::from_iter((0..max_tracks::<T, I>() - 1).map(|x| T::BenchmarkHelper::track_id(x)));
    let track = track_info_of::<T, I>();
    let origin: PalletsOriginOf<T> = RawOrigin::Signed(whitelisted_caller()).into();

    TracksIds::<T, I>::mutate(|tracks_ids| {
        *tracks_ids = BoundedBTreeSet::try_from(ids.clone()).unwrap();
    });
    ids.iter().for_each(|id| {
        let (group, sub) = id.split();
        Tracks::<T, I>::insert(group, sub, track.clone());
        OriginToTrackId::<T, I>::insert(origin.clone(), id);
    });

    if full {
        ReferendaTracks::<T, I>::new_group_with_track(RawOrigin::Root.into(), origin, track)
            .expect("inserts last track");
    }
}

#[instance_benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    pub fn new_group_with_track() {
        // Setup code
        prepare_tracks::<T, I>(false);

        let id = max_track_id::<T, I>();
        let track = track_info_of::<T, I>();
        let origin: PalletsOriginOf<T> = RawOrigin::Signed(whitelisted_caller()).into();

        #[extrinsic_call]
        _(RawOrigin::Root, origin, track);

        // Verification code
        assert_last_event::<T, I>(Event::Created { id }.into());
    }

    #[benchmark]
    pub fn remove() {
        // Setup code
        prepare_tracks::<T, I>(true);

        let id = max_track_id::<T, I>();
        let origin = RawOrigin::Signed(whitelisted_caller()).into();

        #[extrinsic_call]
        _(RawOrigin::Root, id, origin);

        // Verification code
        assert_last_event::<T, I>(Event::Removed { id }.into());
    }

    #[benchmark]
    pub fn set_decision_deposit() {
        // Setup code
        prepare_tracks::<T, I>(true);

        let id = max_track_id::<T, I>();
        let caller = whitelisted_caller();
        let deposit: BalanceOf<T, I> = 1000u32.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), id, deposit);

        // Verification code
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
        // Setup code
        prepare_tracks::<T, I>(true);

        let id = max_track_id::<T, I>();
        let caller = whitelisted_caller();
        let prepare_period: BlockNumberFor<T, I> = 20u32.into();
        let decision_period: BlockNumberFor<T, I> = 200u32.into();

        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller),
            id,
            Some(prepare_period),
            Some(decision_period),
            None,
            None,
        );

        // Verification code
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
        // Setup code
        prepare_tracks::<T, I>(true);

        let id = max_track_id::<T, I>();
        let caller = whitelisted_caller();
        let curve = Curve::LinearDecreasing {
            length: Perbill::from_percent(80),
            floor: Perbill::from_percent(40),
            ceil: Perbill::from_percent(90),
        };

        #[extrinsic_call]
        _(RawOrigin::Signed(caller), id, Some(curve), None);

        // Verification code
        assert_last_event::<T, I>(
            Event::Updated {
                id,
                update_type: UpdateType::Curves,
            }
            .into(),
        );
    }

    impl_benchmark_test_suite!(
        ReferendaTracks,
        crate::mock::new_test_ext(None),
        crate::mock::Test
    );
}
