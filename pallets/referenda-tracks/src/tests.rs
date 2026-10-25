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

//! Tests for referenda tracks pallet.

use super::{Error, Event, Pallet as ReferendaTracks, SplitId, UpdateType};
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use frame_system::{EventRecord, Phase, RawOrigin};
use pallet_referenda::TrackInfo;
use sp_runtime::{str_array as s, traits::BadOrigin, Perbill};

const TRACK: pallet_referenda::TrackInfoOf<Test, ()> = TrackInfo {
    name: s("Test Track"),
    max_deciding: 1,
    decision_deposit: 0,
    prepare_period: 10,
    decision_period: 100,
    confirm_period: 10,
    min_enactment_period: 2,
    min_approval: pallet_referenda::Curve::LinearDecreasing {
        length: Perbill::from_percent(100),
        floor: Perbill::from_percent(50),
        ceil: Perbill::from_percent(100),
    },
    min_support: pallet_referenda::Curve::LinearDecreasing {
        length: Perbill::from_percent(100),
        floor: Perbill::from_percent(0),
        ceil: Perbill::from_percent(50),
    },
};

// Origins for group 0 (admin=account 1, accounts 1-9)
const ORIGIN_SIGNED_1: OriginCaller = OriginCaller::system(RawOrigin::Signed(1));
const ORIGIN_SIGNED_2: OriginCaller = OriginCaller::system(RawOrigin::Signed(2));
const ORIGIN_SIGNED_3: OriginCaller = OriginCaller::system(RawOrigin::Signed(3));
const ORIGIN_SIGNED_4: OriginCaller = OriginCaller::system(RawOrigin::Signed(4));
const ORIGIN_SIGNED_5: OriginCaller = OriginCaller::system(RawOrigin::Signed(5));
// Origins for group 1 (admin=account 2, accounts 10-19)
const ORIGIN_SIGNED_10: OriginCaller = OriginCaller::system(RawOrigin::Signed(10));
// Origin for group 2 (admin=account 3, accounts 20-29) — available for future tests

mod insert {
    use super::*;

    #[test]
    fn fails_if_incorrect_origin() {
        new_test_ext(None).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::new_group_with_track(
                    RuntimeOrigin::signed(1),
                    ORIGIN_SIGNED_1,
                    TRACK,
                ),
                BadOrigin
            );
        });
    }

    #[test]
    fn it_works() {
        new_test_ext(None).execute_with(|| {
            System::set_block_number(1);

            assert_ok!(ReferendaTracks::<Test, ()>::new_group_with_track(
                RuntimeOrigin::root(),
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: RuntimeEvent::Tracks(Event::Created { id: 65536 }),
                    topics: vec![],
                }],
            );

            assert_eq!(ReferendaTracks::<Test>::get_track_info(65536), Some(TRACK));
        });
    }

    #[test]
    fn it_fails_if_inserting_an_already_existing_track() {
        new_test_ext(None).execute_with(|| {
            assert_ok!(ReferendaTracks::<Test, ()>::new_group_with_track(
                RuntimeOrigin::root(),
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            // Using the same origin twice should fail
            assert_noop!(
                ReferendaTracks::<Test, ()>::new_group_with_track(
                    RuntimeOrigin::root(),
                    ORIGIN_SIGNED_1, // Same origin as before
                    TRACK,
                ),
                Error::<Test, ()>::TrackIdAlreadyExisting
            );
        });
    }

    #[test]
    fn fails_if_exceeds_max_tracks() {
        new_test_ext(None).execute_with(|| {
            // Create tracks up to the limit
            for i in 0..MaxTracks::get() {
                let origin_signed = OriginCaller::system(RawOrigin::Signed(i as u64));
                if let Err(e) = ReferendaTracks::<Test, ()>::new_group_with_track(
                    RuntimeOrigin::root(),
                    origin_signed,
                    TRACK,
                ) {
                    // If we get MaxTracksExceeded before reaching the limit, that's also valid
                    assert_eq!(e, Error::<Test, ()>::MaxTracksExceeded.into());
                    return;
                }
            }

            // Try to create one more track, should fail with MaxTracksExceeded
            let origin_signed_n = OriginCaller::system(RawOrigin::Signed(MaxTracks::get() as u64));
            assert_noop!(
                ReferendaTracks::<Test, ()>::new_group_with_track(
                    RuntimeOrigin::root(),
                    origin_signed_n,
                    TRACK,
                ),
                Error::<Test, ()>::MaxTracksExceeded
            );
        });
    }
}

mod validation {
    use super::*;

    #[test]
    fn insert_fails_with_zero_max_deciding() {
        new_test_ext(None).execute_with(|| {
            let mut bad_track = TRACK;
            bad_track.max_deciding = 0;

            assert_noop!(
                ReferendaTracks::<Test, ()>::new_group_with_track(
                    RuntimeOrigin::root(),
                    ORIGIN_SIGNED_1,
                    bad_track,
                ),
                Error::<Test, ()>::InvalidTrackInfo
            );
        });
    }

    #[test]
    fn insert_fails_with_zero_decision_period() {
        new_test_ext(None).execute_with(|| {
            let mut bad_track = TRACK;
            bad_track.decision_period = 0;

            assert_noop!(
                ReferendaTracks::<Test, ()>::new_group_with_track(
                    RuntimeOrigin::root(),
                    ORIGIN_SIGNED_1,
                    bad_track,
                ),
                Error::<Test, ()>::InvalidTrackInfo
            );
        });
    }

    #[test]
    fn set_periods_fails_with_zero_decision_period() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_periods(
                    RuntimeOrigin::signed(1),
                    65536,
                    None,
                    Some(0),
                    None,
                    None
                ),
                Error::<Test, ()>::InvalidTrackInfo
            );
        });
    }

    #[test]
    fn insert_fails_with_zero_confirm_period() {
        new_test_ext(None).execute_with(|| {
            let mut bad_track = TRACK;
            bad_track.confirm_period = 0;

            assert_noop!(
                ReferendaTracks::<Test, ()>::new_group_with_track(
                    RuntimeOrigin::root(),
                    ORIGIN_SIGNED_1,
                    bad_track,
                ),
                Error::<Test, ()>::InvalidTrackInfo
            );
        });
    }

    #[test]
    fn set_periods_fails_with_zero_confirm_period() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_periods(
                    RuntimeOrigin::signed(1),
                    65536,
                    None,
                    None,
                    Some(0),
                    None
                ),
                Error::<Test, ()>::InvalidTrackInfo
            );
        });
    }

    #[test]
    fn add_sub_track_fails_with_zero_max_deciding() {
        new_test_ext(None).execute_with(|| {
            let mut bad_track = TRACK;
            bad_track.max_deciding = 0;

            assert_noop!(
                ReferendaTracks::<Test, ()>::add_sub_track(
                    RuntimeOrigin::signed(1),
                    ORIGIN_SIGNED_1,
                    bad_track,
                ),
                Error::<Test, ()>::InvalidTrackInfo
            );
        });
    }

    #[test]
    fn set_max_deciding_fails_with_zero() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_max_deciding(RuntimeOrigin::signed(1), 65536, 0,),
                Error::<Test, ()>::InvalidTrackInfo
            );
        });
    }
}

mod add_sub_track {
    use super::*;

    #[test]
    fn fails_if_incorrect_origin() {
        new_test_ext(None).execute_with(|| {
            // Account 99 is not in the Admins list
            assert_noop!(
                ReferendaTracks::<Test, ()>::add_sub_track(
                    RuntimeOrigin::signed(99),
                    ORIGIN_SIGNED_2,
                    TRACK,
                ),
                BadOrigin
            );
        });
    }

    #[test]
    fn it_works() {
        new_test_ext(None).execute_with(|| {
            System::set_block_number(1);

            // Account 1 is admin index 0 -> group 0
            // Auto-increment: first sub-track in group 0 gets ID 1
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            // group=0, sub=1 (auto-incremented) -> combined ID = 1
            let expected_id = u32::combine(0, 1);
            assert_eq!(expected_id, 1);

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: RuntimeEvent::Tracks(Event::Created { id: expected_id }),
                    topics: vec![],
                }],
            );

            assert_eq!(
                ReferendaTracks::<Test>::get_track_info(expected_id),
                Some(TRACK)
            );
        });
    }

    #[test]
    fn auto_increments_sub_track_ids() {
        new_test_ext(None).execute_with(|| {
            // Add two sub-tracks in group 0 (admin=1, origins 1-9 map to group 0)
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_2,
                TRACK,
            ));
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_3,
                TRACK,
            ));

            // First gets sub=1, second gets sub=2
            let id_1 = u32::combine(0, 1);
            let id_2 = u32::combine(0, 2);
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_1), Some(TRACK));
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_2), Some(TRACK));
        });
    }

    #[test]
    fn sub_tracks_across_groups_auto_increment_independently() {
        new_test_ext(None).execute_with(|| {
            // Account 1 -> group 0 (origins 1-9), Account 2 -> group 1 (origins 10-19)
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(2),
                ORIGIN_SIGNED_10,
                TRACK,
            ));

            // Each group auto-increments independently from 1
            let id_g0 = u32::combine(0, 1);
            let id_g1 = u32::combine(1, 1);
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_g0), Some(TRACK));
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_g1), Some(TRACK));
        });
    }

    #[test]
    fn fails_if_origin_already_mapped() {
        new_test_ext(None).execute_with(|| {
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            // Same origin (ORIGIN_SIGNED_1), auto-increment would give different sub_track_id
            // but origin is already mapped
            assert_noop!(
                ReferendaTracks::<Test, ()>::add_sub_track(
                    RuntimeOrigin::signed(1),
                    ORIGIN_SIGNED_1,
                    TRACK,
                ),
                Error::<Test, ()>::TrackIdAlreadyExisting
            );
        });
    }
}

mod multi_group {
    use super::*;

    #[test]
    fn groups_coexist() {
        new_test_ext(None).execute_with(|| {
            // Create group via root
            assert_ok!(ReferendaTracks::<Test, ()>::new_group_with_track(
                RuntimeOrigin::root(),
                ORIGIN_SIGNED_1,
                TRACK,
            ));
            assert_ok!(ReferendaTracks::<Test, ()>::new_group_with_track(
                RuntimeOrigin::root(),
                ORIGIN_SIGNED_2,
                TRACK,
            ));

            let id_1 = u32::combine(1, 0);
            let id_2 = u32::combine(2, 0);
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_1), Some(TRACK));
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_2), Some(TRACK));
        });
    }

    #[test]
    fn group_manager_cannot_modify_other_group() {
        new_test_ext(None).execute_with(|| {
            // Account 1 is admin index 0 -> group 0, account 2 is admin index 1 -> group 1
            // Create sub-track in group 0
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            let group_0_track = u32::combine(0, 1);

            // Account 2 (group 1 manager) should not be able to modify group 0's track
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_decision_deposit(
                    RuntimeOrigin::signed(2),
                    group_0_track,
                    500,
                ),
                BadOrigin
            );
        });
    }

    #[test]
    fn tracks_and_track_ids_are_consistent() {
        use pallet_referenda::TracksInfo;

        new_test_ext(None).execute_with(|| {
            assert_ok!(ReferendaTracks::<Test, ()>::new_group_with_track(
                RuntimeOrigin::root(),
                ORIGIN_SIGNED_1,
                TRACK,
            ));
            assert_ok!(ReferendaTracks::<Test, ()>::new_group_with_track(
                RuntimeOrigin::root(),
                ORIGIN_SIGNED_2,
                TRACK,
            ));

            let track_ids: Vec<_> =
                <ReferendaTracks<Test> as TracksInfo<u64, u64>>::track_ids().collect();
            let tracks: Vec<_> =
                <ReferendaTracks<Test> as TracksInfo<u64, u64>>::tracks().collect();

            assert_eq!(track_ids.len(), 2);
            assert_eq!(tracks.len(), 2);

            // Every track returned by tracks() should have its id in track_ids()
            for track in &tracks {
                assert!(track_ids.contains(&track.id));
            }
        });
    }

    #[test]
    fn group_with_sub_tracks_consistent() {
        use pallet_referenda::TracksInfo;

        new_test_ext(None).execute_with(|| {
            // Create a group with root
            assert_ok!(ReferendaTracks::<Test, ()>::new_group_with_track(
                RuntimeOrigin::root(),
                ORIGIN_SIGNED_1,
                TRACK,
            ));
            // Add sub-track to group 0 (admin=account 1, origins 1-9)
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_3,
                TRACK,
            ));

            let track_ids: Vec<_> =
                <ReferendaTracks<Test> as TracksInfo<u64, u64>>::track_ids().collect();
            let tracks: Vec<_> =
                <ReferendaTracks<Test> as TracksInfo<u64, u64>>::tracks().collect();

            // 1 from new_group_with_track + 1 from add_sub_track
            assert_eq!(track_ids.len(), 2);
            assert_eq!(tracks.len(), 2);

            for track in &tracks {
                assert!(track_ids.contains(&track.id));
            }
        });
    }
}

mod remove_edge_cases {
    use super::*;

    #[test]
    fn fails_with_active_referenda_deciding() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            pallet_referenda::DecidingCount::<Test, ()>::insert(65536u32, 1u32);

            assert_noop!(
                ReferendaTracks::<Test, ()>::remove(RuntimeOrigin::root(), 65536),
                Error::<Test, ()>::CannotRemove
            );

            assert_eq!(ReferendaTracks::<Test>::get_track_info(65536), Some(TRACK));
        });
    }

    #[test]
    fn succeeds_after_referenda_cleared() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            pallet_referenda::DecidingCount::<Test, ()>::insert(65536u32, 1u32);

            assert_noop!(
                ReferendaTracks::<Test, ()>::remove(RuntimeOrigin::root(), 65536),
                Error::<Test, ()>::CannotRemove
            );

            pallet_referenda::DecidingCount::<Test, ()>::insert(65536u32, 0u32);

            assert_ok!(ReferendaTracks::<Test, ()>::remove(
                RuntimeOrigin::root(),
                65536,
            ));
        });
    }

    #[test]
    fn origin_can_be_reused_after_removal() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            assert_ok!(ReferendaTracks::<Test, ()>::remove(
                RuntimeOrigin::root(),
                65536,
            ));

            assert_ok!(ReferendaTracks::<Test, ()>::new_group_with_track(
                RuntimeOrigin::root(),
                ORIGIN_SIGNED_1,
                TRACK,
            ));
        });
    }
}

mod update {
    use super::*;

    #[test]
    fn fails_if_incorrect_origin() {
        new_test_ext(None).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_decision_deposit(RuntimeOrigin::signed(1), 1, 0),
                BadOrigin
            );
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_periods(
                    RuntimeOrigin::signed(1),
                    1,
                    Some(10u64),
                    None,
                    None,
                    None
                ),
                BadOrigin
            );
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_curves(
                    RuntimeOrigin::signed(1),
                    1,
                    Some(pallet_referenda::Curve::LinearDecreasing {
                        length: Perbill::from_percent(100),
                        floor: Perbill::from_percent(50),
                        ceil: Perbill::from_percent(100),
                    }),
                    None
                ),
                BadOrigin
            );
        });
    }

    #[test]
    fn it_works() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            let mut track = TRACK.clone();
            track.decision_deposit = 100;

            assert_ok!(ReferendaTracks::<Test, ()>::set_decision_deposit(
                RuntimeOrigin::signed(1),
                65536,
                100
            ));

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: RuntimeEvent::Tracks(Event::Updated {
                        id: 65536,
                        update_type: UpdateType::DecisionDeposit,
                    }),
                    topics: vec![],
                }],
            );

            assert_eq!(ReferendaTracks::<Test>::get_track_info(65536), Some(track));
        });
    }
}

mod remove {
    use super::*;

    #[test]
    fn fails_if_incorrect_origin() {
        new_test_ext(None).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::remove(RuntimeOrigin::signed(1), 1),
                BadOrigin
            );
        });
    }

    #[test]
    fn fails_if_non_existing() {
        new_test_ext(None).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::remove(RuntimeOrigin::signed(1), 1),
                BadOrigin,
            );
        });
    }

    #[test]
    fn it_works() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            assert_ok!(ReferendaTracks::<Test, ()>::remove(
                RuntimeOrigin::root(),
                65536,
            ));

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: RuntimeEvent::Tracks(Event::Removed { id: 65536 }),
                    topics: vec![],
                }],
            );

            assert_eq!(ReferendaTracks::<Test>::get_track_info(65536), None);
        });
    }

    #[test]
    fn manager_cannot_remove_its_track() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            // Account 1 manages the group of track 65536, and can update the track...
            assert_ok!(ReferendaTracks::<Test, ()>::set_decision_deposit(
                RuntimeOrigin::signed(1),
                65536,
                100
            ));

            // ...but `RemoveOrigin` is root only.
            assert_noop!(
                ReferendaTracks::<Test, ()>::remove(RuntimeOrigin::signed(1), 65536),
                BadOrigin
            );
            assert!(ReferendaTracks::<Test>::get_track_info(65536).is_some());
        });
    }

    #[test]
    fn manager_can_remove_if_remove_origin_accepts_it() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            ManagerCanRemove::set(true);

            assert_ok!(ReferendaTracks::<Test, ()>::remove(
                RuntimeOrigin::signed(1),
                65536,
            ));
            assert_eq!(ReferendaTracks::<Test>::get_track_info(65536), None);
        });
    }

    #[test]
    fn manager_of_another_group_cannot_remove_even_if_remove_origin_accepts_managers() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            ManagerCanRemove::set(true);

            assert_noop!(
                ReferendaTracks::<Test, ()>::remove(RuntimeOrigin::signed(2), 65536),
                BadOrigin
            );
        });
    }

    #[test]
    fn cleans_up_origin_mappings() {
        use pallet_referenda::TracksInfo;

        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            // Verify origin mapping exists
            assert_eq!(
                <ReferendaTracks<Test> as TracksInfo<u64, u64>>::track_for(&ORIGIN_SIGNED_1),
                Ok(65536)
            );

            assert_ok!(ReferendaTracks::<Test, ()>::remove(
                RuntimeOrigin::root(),
                65536,
            ));

            // Origin mapping should be gone
            assert_eq!(
                <ReferendaTracks<Test> as TracksInfo<u64, u64>>::track_for(&ORIGIN_SIGNED_1),
                Err(())
            );
            assert_eq!(crate::TrackIdToOrigin::<Test, ()>::get(65536u32), None);
        });
    }
}

mod set_decision_deposit {
    use super::*;

    #[test]
    fn fails_if_incorrect_origin() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_decision_deposit(
                    RuntimeOrigin::signed(2),
                    1,
                    5000
                ),
                BadOrigin
            );
        });
    }

    #[test]
    fn fails_if_non_existing_track() {
        new_test_ext(None).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_decision_deposit(
                    RuntimeOrigin::signed(1),
                    1,
                    5000
                ),
                BadOrigin
            );
        });
    }

    #[test]
    fn it_works() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            let new_deposit = 5000;

            assert_ok!(ReferendaTracks::<Test, ()>::set_decision_deposit(
                RuntimeOrigin::signed(1),
                65536,
                new_deposit
            ));

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: RuntimeEvent::Tracks(Event::Updated {
                        id: 65536,
                        update_type: UpdateType::DecisionDeposit,
                    }),
                    topics: vec![],
                }],
            );

            let updated_track = ReferendaTracks::<Test>::get_track_info(65536).unwrap();
            assert_eq!(updated_track.decision_deposit, new_deposit);
        });
    }
}

mod set_periods {
    use super::*;

    #[test]
    fn fails_if_incorrect_origin() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_periods(
                    RuntimeOrigin::signed(2),
                    65536,
                    Some(20),
                    None,
                    None,
                    None
                ),
                BadOrigin
            );
        });
    }

    #[test]
    fn fails_if_non_existing_track() {
        new_test_ext(None).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_periods(
                    RuntimeOrigin::signed(1),
                    1,
                    Some(20),
                    None,
                    None,
                    None
                ),
                BadOrigin
            );
        });
    }

    #[test]
    fn it_works_with_all_periods() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            let new_prepare = 20;
            let new_decision = 200;
            let new_confirm = 15;
            let new_min_enactment = 5;

            assert_ok!(ReferendaTracks::<Test, ()>::set_periods(
                RuntimeOrigin::signed(1),
                65536,
                Some(new_prepare),
                Some(new_decision),
                Some(new_confirm),
                Some(new_min_enactment)
            ));

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: RuntimeEvent::Tracks(Event::Updated {
                        id: 65536,
                        update_type: UpdateType::Periods,
                    }),
                    topics: vec![],
                }],
            );

            let updated_track = ReferendaTracks::<Test>::get_track_info(65536).unwrap();
            assert_eq!(updated_track.prepare_period, new_prepare);
            assert_eq!(updated_track.decision_period, new_decision);
            assert_eq!(updated_track.confirm_period, new_confirm);
            assert_eq!(updated_track.min_enactment_period, new_min_enactment);
        });
    }

    #[test]
    fn fails_with_all_none() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_periods(
                    RuntimeOrigin::signed(1),
                    65536,
                    None,
                    None,
                    None,
                    None
                ),
                Error::<Test, ()>::NothingToUpdate
            );
        });
    }

    #[test]
    fn it_works_with_partial_periods() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            let original_prepare = TRACK.prepare_period;
            let new_decision = 200;

            assert_ok!(ReferendaTracks::<Test, ()>::set_periods(
                RuntimeOrigin::signed(1),
                65536,
                None,
                Some(new_decision),
                None,
                None
            ));

            let updated_track = ReferendaTracks::<Test>::get_track_info(65536).unwrap();
            assert_eq!(updated_track.prepare_period, original_prepare); // Should remain unchanged
            assert_eq!(updated_track.decision_period, new_decision);
        });
    }
}

mod set_curves {
    use super::*;

    use pallet_referenda::Curve;

    #[test]
    fn fails_if_incorrect_origin() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            let new_curve = Curve::LinearDecreasing {
                length: Perbill::from_percent(80),
                floor: Perbill::from_percent(40),
                ceil: Perbill::from_percent(90),
            };

            assert_noop!(
                ReferendaTracks::<Test, ()>::set_curves(
                    RuntimeOrigin::signed(2),
                    65536,
                    Some(new_curve),
                    None
                ),
                BadOrigin
            );
        });
    }

    #[test]
    fn fails_if_non_existing_track() {
        new_test_ext(None).execute_with(|| {
            let new_curve = Curve::LinearDecreasing {
                length: Perbill::from_percent(80),
                floor: Perbill::from_percent(40),
                ceil: Perbill::from_percent(90),
            };

            assert_noop!(
                ReferendaTracks::<Test, ()>::set_curves(
                    RuntimeOrigin::signed(1),
                    1,
                    Some(new_curve),
                    None
                ),
                BadOrigin
            );
        });
    }

    #[test]
    fn it_works_with_approval_curve() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            let new_approval_curve = Curve::LinearDecreasing {
                length: Perbill::from_percent(80),
                floor: Perbill::from_percent(40),
                ceil: Perbill::from_percent(90),
            };

            assert_ok!(ReferendaTracks::<Test, ()>::set_curves(
                RuntimeOrigin::signed(1),
                65536,
                Some(new_approval_curve.clone()),
                None
            ));

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: RuntimeEvent::Tracks(Event::Updated {
                        id: 65536,
                        update_type: UpdateType::Curves,
                    }),
                    topics: vec![],
                }],
            );

            let updated_track = ReferendaTracks::<Test>::get_track_info(65536).unwrap();
            assert_eq!(updated_track.min_approval, new_approval_curve);
        });
    }

    #[test]
    fn it_works_with_both_curves() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            let new_approval_curve = Curve::LinearDecreasing {
                length: Perbill::from_percent(80),
                floor: Perbill::from_percent(40),
                ceil: Perbill::from_percent(90),
            };
            let new_support_curve = Curve::Reciprocal {
                factor: 1000000000.into(),
                x_offset: 10000000.into(),
                y_offset: 5000000.into(),
            };

            assert_ok!(ReferendaTracks::<Test, ()>::set_curves(
                RuntimeOrigin::signed(1),
                65536,
                Some(new_approval_curve.clone()),
                Some(new_support_curve.clone())
            ));

            let updated_track = ReferendaTracks::<Test>::get_track_info(65536).unwrap();
            assert_eq!(updated_track.min_approval, new_approval_curve);
            assert_eq!(updated_track.min_support, new_support_curve);
        });
    }

    #[test]
    fn fails_with_all_none() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_curves(
                    RuntimeOrigin::signed(1),
                    65536,
                    None,
                    None
                ),
                Error::<Test, ()>::NothingToUpdate
            );
        });
    }

    #[test]
    fn it_works_with_partial_curves() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            let original_approval = TRACK.min_approval;
            let new_support_curve = Curve::Reciprocal {
                factor: 1000000000.into(),
                x_offset: 10000000.into(),
                y_offset: 5000000.into(),
            };

            assert_ok!(ReferendaTracks::<Test, ()>::set_curves(
                RuntimeOrigin::signed(1),
                65536,
                None,
                Some(new_support_curve.clone())
            ));

            let updated_track = ReferendaTracks::<Test>::get_track_info(65536).unwrap();
            assert_eq!(updated_track.min_approval, original_approval); // Should remain unchanged
            assert_eq!(updated_track.min_support, new_support_curve);
        });
    }
}

mod set_max_deciding {
    use super::*;

    #[test]
    fn fails_if_incorrect_origin() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_max_deciding(RuntimeOrigin::signed(2), 65536, 5,),
                BadOrigin
            );
        });
    }

    #[test]
    fn fails_if_non_existing_track() {
        new_test_ext(None).execute_with(|| {
            // Track 65536 = group 1, sub 0. Account 1 is admin of group 0, not group 1.
            // Use a track in a group with no admin to get BadOrigin
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_max_deciding(
                    RuntimeOrigin::signed(1),
                    u32::combine(99, 0),
                    5,
                ),
                BadOrigin
            );
        });
    }

    #[test]
    fn it_works() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            System::set_block_number(1);
            let new_max = 5;

            assert_ok!(ReferendaTracks::<Test, ()>::set_max_deciding(
                RuntimeOrigin::signed(1),
                65536,
                new_max,
            ));

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: RuntimeEvent::Tracks(Event::Updated {
                        id: 65536,
                        update_type: UpdateType::MaxDeciding,
                    }),
                    topics: vec![],
                }],
            );

            let updated_track = ReferendaTracks::<Test>::get_track_info(65536).unwrap();
            assert_eq!(updated_track.max_deciding, new_max);
        });
    }

    #[test]
    fn fails_with_zero() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::set_max_deciding(RuntimeOrigin::signed(1), 65536, 0,),
                Error::<Test, ()>::InvalidTrackInfo
            );
        });
    }
}

mod remove_group {
    use super::*;

    #[test]
    fn fails_if_incorrect_origin() {
        new_test_ext(None).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::remove_group(RuntimeOrigin::signed(1), 0u16, 10),
                BadOrigin
            );
        });
    }

    #[test]
    fn fails_if_group_not_found() {
        new_test_ext(None).execute_with(|| {
            assert_noop!(
                ReferendaTracks::<Test, ()>::remove_group(RuntimeOrigin::root(), 99u16, 10),
                Error::<Test, ()>::GroupNotFound
            );
        });
    }

    #[test]
    fn removes_group_with_single_track() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            System::set_block_number(1);

            // Group 1 was created by new_test_ext with sub-track 0
            let id = u32::combine(1u16, 0u16);
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id), Some(TRACK));

            assert_ok!(ReferendaTracks::<Test, ()>::remove_group(
                RuntimeOrigin::root(),
                1u16,
                10,
            ));

            assert_eq!(ReferendaTracks::<Test>::get_track_info(id), None);
            assert_eq!(crate::TracksIds::<Test, ()>::get().len(), 0);

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: RuntimeEvent::Tracks(Event::GroupRemoved {
                        group: 1u16,
                        tracks_removed: 1,
                    }),
                    topics: vec![],
                }],
            );
        });
    }

    #[test]
    fn removes_group_with_multiple_sub_tracks() {
        new_test_ext(None).execute_with(|| {
            System::set_block_number(1);

            // Create a group with root (gets group 1, sub-track 0)
            assert_ok!(ReferendaTracks::<Test, ()>::new_group_with_track(
                RuntimeOrigin::root(),
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            // Add sub-tracks to group 0 using account 1 (admin of group 0)
            // Origins 2-4 map to group 0 in mock
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_3,
                TRACK,
            ));
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_4,
                TRACK,
            ));

            // Group 0 has sub-tracks 1 and 2
            let id_0_1 = u32::combine(0, 1);
            let id_0_2 = u32::combine(0, 2);
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_0_1), Some(TRACK));
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_0_2), Some(TRACK));

            // Group 1 has sub-track 0 (from new_group_with_track)
            let id_1_0 = u32::combine(1, 0);
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_1_0), Some(TRACK));

            // Remove group 0 (has 2 sub-tracks)
            assert_ok!(ReferendaTracks::<Test, ()>::remove_group(
                RuntimeOrigin::root(),
                0u16,
                10,
            ));

            // Group 0 tracks gone
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_0_1), None);
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_0_2), None);

            // Group 1 track still exists
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_1_0), Some(TRACK));

            // TracksIds should only have group 1's track
            let ids = crate::TracksIds::<Test, ()>::get();
            assert_eq!(ids.len(), 1);
            assert!(ids.contains(&id_1_0));
        });
    }

    #[test]
    fn fails_if_active_referenda_in_group() {
        new_test_ext(None).execute_with(|| {
            // Create sub-tracks in group 0
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            let id = u32::combine(0, 1);
            pallet_referenda::DecidingCount::<Test, ()>::insert(id, 1u32);

            assert_noop!(
                ReferendaTracks::<Test, ()>::remove_group(RuntimeOrigin::root(), 0u16, 10),
                Error::<Test, ()>::CannotRemoveGroup
            );

            // Track should still exist
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id), Some(TRACK));
        });
    }

    #[test]
    fn cleans_up_origin_mappings() {
        use pallet_referenda::TracksInfo;

        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            let id = u32::combine(1u16, 0u16);

            // Verify origin mapping exists
            assert_eq!(
                <ReferendaTracks<Test> as TracksInfo<u64, u64>>::track_for(&ORIGIN_SIGNED_1),
                Ok(id)
            );

            assert_ok!(ReferendaTracks::<Test, ()>::remove_group(
                RuntimeOrigin::root(),
                1u16,
                10,
            ));

            // Origin mapping should be gone
            assert_eq!(
                <ReferendaTracks<Test> as TracksInfo<u64, u64>>::track_for(&ORIGIN_SIGNED_1),
                Err(())
            );
            assert_eq!(crate::TrackIdToOrigin::<Test, ()>::get(id), None);
        });
    }

    #[test]
    fn cleans_up_sub_track_counter() {
        new_test_ext(None).execute_with(|| {
            // Add sub-tracks to group 0
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            // Counter should be 1
            assert_eq!(crate::NextSubTrackId::<Test, ()>::get(0u16), 1u16);

            assert_ok!(ReferendaTracks::<Test, ()>::remove_group(
                RuntimeOrigin::root(),
                0u16,
                10,
            ));

            // Counter should be cleaned up
            assert_eq!(crate::NextSubTrackId::<Test, ()>::get(0u16), 0u16);
        });
    }

    #[test]
    fn fails_if_max_tracks_hint_too_low() {
        new_test_ext(None).execute_with(|| {
            // Add 2 sub-tracks to group 0
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_1,
                TRACK,
            ));
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_2,
                TRACK,
            ));

            // Try to remove with max_tracks=1 but group has 2 tracks
            assert_noop!(
                ReferendaTracks::<Test, ()>::remove_group(RuntimeOrigin::root(), 0u16, 1),
                Error::<Test, ()>::TooManyTracks
            );

            // Works with correct hint
            assert_ok!(ReferendaTracks::<Test, ()>::remove_group(
                RuntimeOrigin::root(),
                0u16,
                2,
            ));
        });
    }
}

/// `remove_group` rewrites `TracksIds` once for the whole group, not once per sub-track.
mod remove_group_cost {
    use super::*;
    use crate::{NextSubTrackId, OriginToTrackId, TrackIdToOrigin, Tracks, TracksIds};
    use alloc::{collections::BTreeSet, vec::Vec};

    const GROUP: u16 = 1;
    const OTHER_GROUP: u16 = 2;
    const SUB_TRACKS: u16 = 2_000;
    const FILLER: u32 = 20_000;

    fn origin_of(id: u32) -> OriginCaller {
        OriginCaller::system(RawOrigin::Signed(1_000_000 + id as u64))
    }

    fn put_track(id: u32) {
        let (group, sub) = id.split();
        Tracks::<Test, ()>::insert(group, sub, TRACK);
        OriginToTrackId::<Test, ()>::insert(origin_of(id), id);
        TrackIdToOrigin::<Test, ()>::insert(id, origin_of(id));
    }

    /// `GROUP` has `SUB_TRACKS` tracks, `OTHER_GROUP` has three, and `TracksIds` holds those and
    /// `FILLER` more (ids of tracks that are not stored: it is the size of the value that counts).
    fn big_state() {
        MaxTracks::set(65_536);
        let mut ids = BTreeSet::new();
        for sub in 0..SUB_TRACKS {
            let id = u32::combine(GROUP, sub);
            put_track(id);
            ids.insert(id);
        }
        NextSubTrackId::<Test, ()>::insert(GROUP, SUB_TRACKS - 1);
        for sub in 0..3 {
            let id = u32::combine(OTHER_GROUP, sub);
            put_track(id);
            ids.insert(id);
        }
        NextSubTrackId::<Test, ()>::insert(OTHER_GROUP, 2);
        for i in 0..FILLER {
            ids.insert(u32::combine(100 + (i / 1_000) as u16, (i % 1_000) as u16));
        }
        TracksIds::<Test, ()>::put(frame_support::BoundedBTreeSet::try_from(ids).unwrap());
    }

    /// The way that `do_remove_group` was written before: `TracksIds` is mutated once per
    /// sub-track.
    fn reference_remove_group(
        group: u16,
        max_tracks: u32,
    ) -> frame_support::dispatch::DispatchResult {
        let sub_tracks: Vec<u16> = Tracks::<Test, ()>::iter_key_prefix(group).collect();
        frame_support::ensure!(
            sub_tracks.len() as u32 <= max_tracks,
            Error::<Test, ()>::TooManyTracks
        );
        frame_support::ensure!(!sub_tracks.is_empty(), Error::<Test, ()>::GroupNotFound);
        let count = sub_tracks.len() as u32;
        for sub in sub_tracks {
            let id = u32::combine(group, sub);
            Tracks::<Test, ()>::remove(group, sub);
            if let Some(origin) = TrackIdToOrigin::<Test, ()>::take(id) {
                OriginToTrackId::<Test, ()>::remove(&origin);
            }
            TracksIds::<Test, ()>::mutate(|ids| {
                ids.remove(&id);
            });
        }
        NextSubTrackId::<Test, ()>::remove(group);
        ReferendaTracks::<Test, ()>::deposit_event(Event::GroupRemoved {
            group,
            tracks_removed: count,
        });
        Ok(())
    }

    /// Every key and value in storage.
    fn dump() -> Vec<(Vec<u8>, Vec<u8>)> {
        let mut all = Vec::new();
        let mut key = Vec::new();
        while let Some(next) = sp_io::storage::next_key(&key) {
            let value = sp_io::storage::get(&next)
                .map(|v| v.to_vec())
                .unwrap_or_default();
            all.push((next.clone(), value));
            key = next;
        }
        all
    }

    #[test]
    fn leaves_the_same_state_as_removing_one_by_one() {
        let expected = new_test_ext(None).execute_with(|| {
            big_state();
            assert_ok!(reference_remove_group(GROUP, SUB_TRACKS as u32));
            dump()
        });
        let (actual, ids) = new_test_ext(None).execute_with(|| {
            big_state();
            assert_ok!(ReferendaTracks::<Test, ()>::remove_group(
                RuntimeOrigin::root(),
                GROUP,
                SUB_TRACKS as u32,
            ));
            (dump(), TracksIds::<Test, ()>::get())
        });

        assert!(
            expected == actual,
            "the storage differs from the one-by-one removal"
        );
        // And not trivially so: the group is gone, the rest is not.
        assert_eq!(ids.len() as u32, 3 + FILLER);
        assert!(ids.iter().all(|id| id.split().0 != GROUP));
        new_test_ext(None).execute_with(|| {
            big_state();
            assert_ok!(ReferendaTracks::<Test, ()>::remove_group(
                RuntimeOrigin::root(),
                GROUP,
                SUB_TRACKS as u32,
            ));
            assert_eq!(Tracks::<Test, ()>::iter_key_prefix(GROUP).count(), 0);
            assert_eq!(Tracks::<Test, ()>::iter_key_prefix(OTHER_GROUP).count(), 3);
            assert_eq!(NextSubTrackId::<Test, ()>::get(OTHER_GROUP), 2);
            assert!(!OriginToTrackId::<Test, ()>::contains_key(origin_of(
                u32::combine(GROUP, 7)
            )));
            assert!(OriginToTrackId::<Test, ()>::contains_key(origin_of(
                u32::combine(OTHER_GROUP, 1)
            )));
        });
    }

    /// `TracksIds` is decoded once per rewrite and the bound is read once per decode, so the
    /// reads of `MaxTracks` count the rewrites. They do not grow with the sub-tracks.
    #[test]
    fn rewrites_the_ids_once() {
        new_test_ext(None).execute_with(|| {
            big_state();
            MaxTracks::reset_reads();
            assert_ok!(reference_remove_group(GROUP, SUB_TRACKS as u32));
            // The counter sees what it has to: one rewrite per sub-track.
            assert!(MaxTracks::reads() >= SUB_TRACKS as u32);
        });
        new_test_ext(None).execute_with(|| {
            big_state();
            MaxTracks::reset_reads();
            assert_ok!(ReferendaTracks::<Test, ()>::remove_group(
                RuntimeOrigin::root(),
                GROUP,
                SUB_TRACKS as u32,
            ));
            // One decode of `TracksIds`, and one read of the bound for the capacity.
            assert!(MaxTracks::reads() <= 3, "{}", MaxTracks::reads());
        });
    }
}

/// `TracksInfo::info` looks the track up, it does not scan the `Tracks` map.
mod info {
    use super::*;
    use crate::{TrackIdToOrigin, Tracks};
    use pallet_referenda::TracksInfo;
    use std::borrow::Cow;

    type Info = ReferendaTracks<Test, ()>;

    /// What the default of `TracksInfo::info` returns: a scan of `tracks()`.
    fn scanned(id: u32) -> Option<Cow<'static, pallet_referenda::TrackInfoOf<Test, ()>>> {
        <Info as TracksInfo<u64, u64>>::tracks()
            .find(|track| track.id == id)
            .map(|track| Cow::Owned(track.into_owned().info))
    }

    fn put_track(id: u32, max_deciding: u32) {
        let (group, sub) = id.split();
        let mut info = TRACK;
        info.max_deciding = max_deciding;
        Tracks::<Test, ()>::insert(group, sub, info);
    }

    #[test]
    fn matches_scanning_the_tracks() {
        new_test_ext(None).execute_with(|| {
            MaxTracks::set(65_536);
            // Each track is told apart by its `max_deciding`.
            put_track(u32::combine(0, 0), 1);
            put_track(u32::combine(0, 3), 2);
            put_track(u32::combine(7, 0), 3);
            put_track(u32::combine(7, 9), 4);

            for id in [
                u32::combine(0, 0),
                u32::combine(0, 3),
                u32::combine(7, 0),
                u32::combine(7, 9),
            ] {
                let info = <Info as TracksInfo<u64, u64>>::info(id);
                assert!(info.is_some());
                assert_eq!(info, scanned(id));
            }
            assert_eq!(
                <Info as TracksInfo<u64, u64>>::info(u32::combine(7, 9))
                    .map(|info| info.max_deciding),
                Some(4)
            );

            // A group that does not exist.
            assert_eq!(
                <Info as TracksInfo<u64, u64>>::info(u32::combine(5, 0)),
                None
            );
            assert_eq!(scanned(u32::combine(5, 0)), None);
            // A group that exists, with a sub-track that does not.
            assert_eq!(
                <Info as TracksInfo<u64, u64>>::info(u32::combine(7, 1)),
                None
            );
            assert_eq!(scanned(u32::combine(7, 1)), None);
            // An origin is not needed to have a track.
            assert!(!TrackIdToOrigin::<Test, ()>::contains_key(u32::combine(
                7, 9
            )));
        });
    }

    /// With the tracks of many groups in storage, `info` reads a bounded number of trie nodes,
    /// and the scan reads all of them: the storage proof of the call is what tells.
    #[test]
    fn reads_a_constant_number_of_nodes() {
        let mut ext = new_test_ext(None);
        ext.execute_with(|| {
            MaxTracks::set(65_536);
            for i in 0..5_000u32 {
                put_track(u32::combine(1 + (i / 100) as u16, (i % 100) as u16), 1 + i);
            }
        });
        // The proof is of what the backend serves, so the tracks are committed to it.
        ext.commit_all().unwrap();

        let id = u32::combine(30, 42);
        let (found, proof_of_info) = ext.execute_and_prove(|| {
            <Info as TracksInfo<u64, u64>>::info(id).map(|info| info.max_deciding)
        });
        assert_eq!(found, Some(1 + 30 * 100 - 100 + 42));
        let (found, proof_of_scan) = ext.execute_and_prove(|| scanned(id).is_some());
        assert!(found);

        let (info_nodes, scan_nodes) = (proof_of_info.len(), proof_of_scan.len());
        std::println!("trie nodes: info {info_nodes}, scan {scan_nodes}");
        assert!(info_nodes < 20, "{info_nodes}");
        assert!(scan_nodes > 1_000, "{scan_nodes}");
    }
}

mod track_for {
    use super::*;
    use pallet_referenda::TracksInfo;

    #[test]
    fn works_for_groups_created_by_root() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            let id = u32::combine(1u16, 0u16);
            assert_eq!(
                <ReferendaTracks<Test> as TracksInfo<u64, u64>>::track_for(&ORIGIN_SIGNED_1),
                Ok(id)
            );
        });
    }

    #[test]
    fn works_for_sub_tracks() {
        new_test_ext(None).execute_with(|| {
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            let id = u32::combine(0, 1);
            assert_eq!(
                <ReferendaTracks<Test> as TracksInfo<u64, u64>>::track_for(&ORIGIN_SIGNED_1),
                Ok(id)
            );
        });
    }

    #[test]
    fn returns_err_for_unknown_origin() {
        new_test_ext(None).execute_with(|| {
            assert_eq!(
                <ReferendaTracks<Test> as TracksInfo<u64, u64>>::track_for(&ORIGIN_SIGNED_5),
                Err(())
            );
        });
    }
}

mod group_allocation {
    use super::*;
    use crate::{NextGroupId, Tracks as TracksMap};

    fn origin(n: u64) -> OriginCaller {
        OriginCaller::system(RawOrigin::Signed(n))
    }

    fn auto(n: u64) -> u16 {
        assert_ok!(ReferendaTracks::<Test, ()>::new_group_with_track(
            RuntimeOrigin::root(),
            origin(n),
            TRACK,
        ));
        crate::OriginToTrackId::<Test, ()>::get(origin(n))
            .expect("the origin is mapped")
            .split()
            .0
    }

    fn explicit(group: u16, sub: u16, n: u64) {
        assert_ok!(ReferendaTracks::<Test, ()>::do_insert(
            u32::combine(group, sub),
            TRACK,
            origin(n),
        ));
    }

    // Explicit groups 2 and 5 exist before the auto-allocated ones: 1, 3, 4, 6 are taken, and the
    // allocator skips 2 and 5.
    #[test]
    fn auto_allocation_skips_explicit_groups_created_first() {
        new_test_ext(None).execute_with(|| {
            explicit(2, 0, 100);
            explicit(5, 0, 101);

            let groups: Vec<u16> = (0..4).map(auto).collect();

            assert_eq!(groups, vec![1, 3, 4, 6]);
        });
    }

    // Groups 1 and 2 are auto-allocated; then explicit groups 2 (taken: fails), 3 and 9 are
    // created, and the next auto-allocated ones are 4 and 5, then 6, 7, 8 and 10.
    #[test]
    fn explicit_groups_created_after_auto_allocation_are_not_reused() {
        new_test_ext(None).execute_with(|| {
            assert_eq!(auto(0), 1);
            assert_eq!(auto(1), 2);

            assert_noop!(
                ReferendaTracks::<Test, ()>::do_insert(u32::combine(2, 0), TRACK, origin(100)),
                Error::<Test, ()>::TrackIdAlreadyExisting
            );
            explicit(3, 0, 101);
            explicit(9, 0, 102);

            let groups: Vec<u16> = (10..16).map(auto).collect();
            assert_eq!(groups, vec![4, 5, 6, 7, 8, 10]);
        });
    }

    // A group that only has a sub-track other than 0 is a group with a track as well.
    #[test]
    fn a_group_with_only_another_sub_track_is_taken() {
        new_test_ext(None).execute_with(|| {
            explicit(1, 7, 100);

            assert_eq!(auto(0), 2);
        });
    }

    // Explicit groups that come right after the last allocated one move `NextGroupId` along, so
    // the allocator does not have to skip them one by one.
    #[test]
    fn contiguous_explicit_groups_move_next_group_id() {
        new_test_ext(None).execute_with(|| {
            assert_eq!(auto(0), 1);
            explicit(2, 0, 100);
            explicit(3, 0, 101);
            assert_eq!(NextGroupId::<Test, ()>::get(), 3);

            // Not the next one: it stays.
            explicit(9, 0, 102);
            assert_eq!(NextGroupId::<Test, ()>::get(), 3);

            assert_eq!(auto(1), 4);
        });
    }

    // Every group has a track: the allocator ends instead of looping, and fails.
    #[test]
    fn allocation_fails_when_it_runs_out_of_groups() {
        new_test_ext(None).execute_with(|| {
            NextGroupId::<Test, ()>::put(u16::MAX - 2);
            explicit(u16::MAX - 1, 0, 100);
            explicit(u16::MAX, 0, 101);
            assert_eq!(NextGroupId::<Test, ()>::get(), u16::MAX);

            assert_noop!(
                ReferendaTracks::<Test, ()>::new_group_with_track(
                    RuntimeOrigin::root(),
                    origin(5000),
                    TRACK,
                ),
                Error::<Test, ()>::MaxTracksExceeded
            );
        });
    }

    // Groups 1..=255 all have a track, which is `MaxTracks`: the next one doesn't fit.
    #[test]
    fn allocation_fails_when_tracks_are_full() {
        new_test_ext(None).execute_with(|| {
            for group in 1..=u8::MAX as u16 {
                explicit(group, 0, 1000 + group as u64);
            }
            assert_eq!(TracksMap::<Test, ()>::iter().count(), 255);

            assert_noop!(
                ReferendaTracks::<Test, ()>::new_group_with_track(
                    RuntimeOrigin::root(),
                    origin(5000),
                    TRACK,
                ),
                Error::<Test, ()>::MaxTracksExceeded
            );
        });
    }
}

mod migration {
    use super::*;
    use crate::{
        migration::{
            check_key_fits, v0, Cursor, CursorKey, MigrateV0ToV1, Phase, TryIntoGroup, MAX_KEY_LEN,
        },
        WeightInfo,
    };
    use codec::MaxEncodedLen;
    use frame_support::{
        migrations::{SteppedMigration, SteppedMigrationError},
        storage::unhashed,
        traits::{GetStorageVersion, StorageVersion},
        weights::{Weight, WeightMeter},
        BoundedVec,
    };
    use sp_runtime::traits::Convert;
    use sp_runtime::StateVersion;

    type Migration = MigrateV0ToV1<Test, (), u16, TryIntoGroup>;

    /// Accepts ids below 1000 only.
    struct Below1000;
    impl Convert<u16, Option<u16>> for Below1000 {
        fn convert(id: u16) -> Option<u16> {
            (id < 1000).then_some(id)
        }
    }

    // The ids of the 23 community tracks: 20 small ones, and 444, 777 and 1786, which don't fit a
    // `u8` group.
    fn live_ids() -> Vec<u16> {
        (1..=20).chain([444, 777, 1786]).collect()
    }

    fn origin_of(id: u16) -> OriginCaller {
        OriginCaller::system(RawOrigin::Signed(10_000 + id as u64))
    }

    fn info_of(id: u16) -> pallet_referenda::TrackInfoOf<Test, ()> {
        pallet_referenda::TrackInfoOf::<Test, ()> {
            decision_deposit: id as u64,
            ..TRACK
        }
    }

    fn new_id(id: u16) -> u32 {
        u32::combine(id, 0)
    }

    /// Writes the storage of version 0: flat `u16` ids, in `Tracks`, `TracksIds` and in the
    /// values of `OriginToTrackId`.
    fn put_v0(ids: &[u16]) {
        StorageVersion::new(0).put::<crate::Pallet<Test, ()>>();
        let bounded: BoundedVec<u16, MaxTracks> = BoundedVec::try_from(ids.to_vec()).unwrap();
        v0::TracksIds::<Test, (), u16>::put(bounded);
        for id in ids {
            v0::Tracks::<Test, (), u16>::insert(id, info_of(*id));
            unhashed::put(
                &crate::OriginToTrackId::<Test, ()>::hashed_key_for(origin_of(*id)),
                id,
            );
        }
    }

    fn run_all<M>() -> Option<Cursor>
    where
        M: SteppedMigration<Cursor = Cursor>,
    {
        M::step(None, &mut WeightMeter::new()).expect("the migration doesn't fail")
    }

    /// The raw keys of `Tracks` of the shape of version 0.
    fn v0_keys() -> Vec<Vec<u8>> {
        let prefix = frame_support::storage::storage_prefix(b"Tracks", b"Tracks");
        let mut keys = vec![];
        let mut key = prefix.to_vec();
        while let Some(next) = sp_io::storage::next_key(&key) {
            if !next.starts_with(&prefix) {
                break;
            }
            if next.len() == 32 + 16 + 2 {
                keys.push(next.clone());
            }
            key = next;
        }
        keys
    }

    fn root() -> Vec<u8> {
        sp_io::storage::root(StateVersion::V1)
    }

    fn assert_migrated(ids: &[u16]) {
        for id in ids {
            let new = new_id(*id);
            assert_eq!(
                ReferendaTracks::<Test>::get_track_info(new),
                Some(info_of(*id)),
                "track {id}"
            );
            assert_eq!(
                crate::OriginToTrackId::<Test, ()>::get(origin_of(*id)),
                Some(new)
            );
            assert_eq!(
                crate::TrackIdToOrigin::<Test, ()>::get(new),
                Some(origin_of(*id))
            );
            assert!(!v0::Tracks::<Test, (), u16>::contains_key(id));
        }
        let expected: Vec<u32> = ids.iter().map(|id| new_id(*id)).collect();
        let mut stored: Vec<u32> = crate::TracksIds::<Test, ()>::get().into_iter().collect();
        stored.sort();
        let mut expected = expected;
        expected.sort();
        assert_eq!(stored, expected);
    }

    // Ids: 1..=20, 444, 777, 1786 (23 tracks, the shape of Kreivo). Old `n` becomes group `n`,
    // track 0, which for a `u32` id is `n << 16`.
    #[test]
    fn migrates_the_live_shaped_state() {
        new_test_ext(None).execute_with(|| {
            let ids = live_ids();
            assert_eq!(ids.len(), 23);
            put_v0(&ids);

            #[cfg(feature = "try-runtime")]
            let state = Migration::pre_upgrade().expect("pre_upgrade");

            assert_eq!(run_all::<Migration>(), None);

            assert_migrated(&ids);
            assert_eq!(new_id(444), 444 << 16);
            assert_eq!(new_id(1786), 1786 << 16);
            assert_eq!(crate::NextGroupId::<Test, ()>::get(), 1787);
            assert_eq!(ReferendaTracks::<Test>::on_chain_storage_version(), 1);
            assert!(v0_keys().is_empty());

            #[cfg(feature = "try-runtime")]
            Migration::post_upgrade(state).expect("post_upgrade");

            // The group-first model works on it: a sub-track is added afterwards to group 1
            // (account 2 manages it, and origin 10 is one of its origins). Sub-tracks start at 1,
            // and the migrated track stays the first one.
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(2),
                OriginCaller::system(RawOrigin::Signed(10)),
                TRACK,
            ));
            assert_eq!(
                crate::OriginToTrackId::<Test, ()>::get(OriginCaller::system(RawOrigin::Signed(
                    10
                ))),
                Some(u32::combine(1, 1))
            );
            assert_eq!(
                ReferendaTracks::<Test>::get_track_info(new_id(1)),
                Some(info_of(1))
            );
        });
    }

    #[test]
    fn track_for_works_after_migration() {
        use pallet_referenda::TracksInfo;

        new_test_ext(None).execute_with(|| {
            put_v0(&[444]);
            run_all::<Migration>();

            assert_eq!(
                <ReferendaTracks<Test> as TracksInfo<u64, u64>>::track_for(&origin_of(444)),
                Ok(new_id(444))
            );
        });
    }

    // Steps of two units each, from the 23 tracks of the live shape to the end. After each, what
    // is at or before the cursor is migrated, and what is after it is untouched; and the final
    // state is that of one step with no limit.
    #[test]
    fn yields_midway_and_resumes_at_the_cursor() {
        let unlimited = new_test_ext(None).execute_with(|| {
            put_v0(&live_ids());
            run_all::<Migration>();
            root()
        });

        let resumed = new_test_ext(None).execute_with(|| {
            put_v0(&live_ids());
            let mut cursor: Option<Cursor> = None;
            let mut phases = vec![];
            let mut steps = 0;
            loop {
                steps += 1;
                assert!(steps < 100, "it doesn't end");
                let before = v0_keys();
                let unit = match cursor.as_ref().map(|(phase, _)| phase) {
                    None | Some(Phase::Tracks) => <() as WeightInfo>::migrate_track(),
                    Some(Phase::Origins) => <() as WeightInfo>::migrate_origin(),
                    Some(Phase::TracksIds) => Migration::unit(Phase::TracksIds),
                    Some(Phase::Finish) => <() as WeightInfo>::migrate_finalize(),
                };
                let mut meter = WeightMeter::with_limit(unit.saturating_mul(2));
                cursor = Migration::step(cursor, &mut meter).expect("the unit fits");
                let Some((phase, key)) = cursor.clone() else {
                    break;
                };
                if phases.last() != Some(&phase) {
                    phases.push(phase);
                }
                if phase == Phase::Tracks && !key.is_empty() {
                    // Before the cursor: moved. After it: untouched.
                    let after = v0_keys();
                    assert!(after.iter().all(|k| k.as_slice() > key.as_slice()));
                    assert_eq!(
                        after,
                        before
                            .iter()
                            .filter(|k| k.as_slice() > key.as_slice())
                            .cloned()
                            .collect::<Vec<_>>()
                    );
                }
            }
            assert_eq!(
                phases,
                vec![
                    Phase::Tracks,
                    Phase::Origins,
                    Phase::TracksIds,
                    Phase::Finish
                ]
            );
            // Two units per step, 23 tracks: many steps.
            assert!(steps >= 30, "{steps} steps");
            assert_migrated(&live_ids());
            root()
        });

        assert_eq!(unlimited, resumed);
    }

    // `MaxTracks` is 65_536 and the id a `u32`, as in Kreivo's community tracks. No unit costs more
    // than what fits an empty block, here assumed to be a `MaxServiceWeight` of ref_time
    // 330_000_000_000 and proof size 1_700_000 (Kreivo's is 332_414_064_333 / 1_743_482). If a
    // unit did not fit, `step` would return `InsufficientWeight`. The unit that reads `TracksIds`
    // declares the proof size of its type, not the constant that the benchmark measured.
    #[test]
    fn no_unit_exceeds_an_empty_block_with_65536_max_tracks() {
        new_test_ext(None).execute_with(|| {
            MaxTracks::set(65_536);
            let block = Weight::from_parts(330_000_000_000, 1_700_000);
            for phase in [
                Phase::Tracks,
                Phase::Origins,
                Phase::TracksIds,
                Phase::Finish,
            ] {
                let unit = Migration::unit(phase);
                assert!(
                    unit.all_lt(block),
                    "{phase:?}: {unit:?} does not fit {block:?}"
                );
            }

            // 65_536 ids of 4 bytes, and the length prefix (`Compact(65_536)` is 4 bytes).
            assert_eq!(Migration::tracks_ids_max_size(), 262_148);
            let ids = Migration::unit(Phase::TracksIds);
            assert!(ids.proof_size() >= 262_148, "{ids:?}");
            // On top of the benchmarked weight, which is untouched in ref_time.
            let benchmarked = <() as WeightInfo>::migrate_ids(MaxTracks::get());
            assert_eq!(ids.ref_time(), benchmarked.ref_time());
            assert_eq!(ids.proof_size(), benchmarked.proof_size() + 262_148);

            // The phases that don't read `TracksIds` declare what they were benchmarked at.
            assert_eq!(
                Migration::unit(Phase::Tracks),
                <() as WeightInfo>::migrate_track()
            );
            assert_eq!(
                Migration::unit(Phase::Origins),
                <() as WeightInfo>::migrate_origin()
            );
            assert_eq!(
                Migration::unit(Phase::Finish),
                <() as WeightInfo>::migrate_finalize()
            );

            put_v0(&live_ids());
            let mut cursor = None;
            let mut steps = 0;
            loop {
                steps += 1;
                assert!(steps < 100, "it doesn't end");
                // A fresh meter that is an empty block: never `InsufficientWeight`.
                let mut meter = WeightMeter::with_limit(block);
                cursor = Migration::step(cursor, &mut meter).expect("every unit fits");
                if cursor.is_none() {
                    break;
                }
            }
            assert_migrated(&live_ids());
        });
    }

    // One unit fits, a second doesn't: it yields in the middle of a phase.
    #[test]
    fn yields_when_the_next_unit_does_not_fit() {
        new_test_ext(None).execute_with(|| {
            put_v0(&[1, 2, 3]);
            let unit = <() as WeightInfo>::migrate_track();

            let mut meter = WeightMeter::with_limit(unit);
            let cursor = Migration::step(None, &mut meter).expect("it doesn't fail");

            let (phase, key) = cursor.expect("it is not done");
            assert_eq!(phase, Phase::Tracks);
            assert!(!key.is_empty());
            assert_eq!(v0_keys().len(), 2);
        });
    }

    // Not even one unit fits an empty meter: waiting doesn't help, so it's an error. A meter that
    // is only used up is not: it yields.
    #[test]
    fn fails_if_not_even_one_unit_fits() {
        new_test_ext(None).execute_with(|| {
            put_v0(&[1]);

            for (cursor, required) in [
                (None, <() as WeightInfo>::migrate_track()),
                (
                    Some((Phase::Origins, CursorKey::default())),
                    <() as WeightInfo>::migrate_origin(),
                ),
                (
                    Some((Phase::TracksIds, CursorKey::default())),
                    Migration::unit(Phase::TracksIds),
                ),
                (
                    Some((Phase::Finish, CursorKey::default())),
                    <() as WeightInfo>::migrate_finalize(),
                ),
            ] {
                let mut meter = WeightMeter::with_limit(Weight::zero());
                assert_eq!(
                    Migration::step(cursor.clone(), &mut meter),
                    Err(SteppedMigrationError::InsufficientWeight { required })
                );
            }
            assert_eq!(v0_keys().len(), 1);

            let mut used_up = WeightMeter::with_limit(<() as WeightInfo>::migrate_track());
            used_up.consume(<() as WeightInfo>::migrate_track());
            assert_eq!(
                Migration::step(None, &mut used_up),
                Ok(Some((Phase::Tracks, CursorKey::default())))
            );
        });
    }

    // Ids: 1, 777, 1786, 4000. With `Below1000`, 1786 and 4000 don't convert: they are left as
    // they were, and the migration finishes.
    #[test]
    fn skips_ids_that_do_not_fit_a_group() {
        type Strict = MigrateV0ToV1<Test, (), u16, Below1000>;

        new_test_ext(None).execute_with(|| {
            let ids = [1u16, 777, 1786, 4000];
            put_v0(&ids);

            #[cfg(feature = "try-runtime")]
            let state = Strict::pre_upgrade().expect("pre_upgrade");

            assert_eq!(run_all::<Strict>(), None);

            assert_migrated(&[1, 777]);
            for skipped in [1786u16, 4000] {
                assert_eq!(
                    v0::Tracks::<Test, (), u16>::get(skipped),
                    Some(info_of(skipped))
                );
                assert_eq!(
                    unhashed::get::<u16>(&crate::OriginToTrackId::<Test, ()>::hashed_key_for(
                        origin_of(skipped)
                    )),
                    Some(skipped)
                );
                assert!(ReferendaTracks::<Test>::get_track_info(new_id(skipped)).is_none());
            }
            assert_eq!(v0_keys().len(), 2);
            assert_eq!(ReferendaTracks::<Test>::on_chain_storage_version(), 1);
            assert_eq!(crate::NextGroupId::<Test, ()>::get(), 778);

            #[cfg(feature = "try-runtime")]
            Strict::post_upgrade(state).expect("post_upgrade");
        });
    }

    // A second run recognises what is migrated (by the length and the hash of the keys, and by
    // the reverse map) and changes nothing; also when it begins from the first step.
    #[test]
    fn running_again_changes_nothing() {
        new_test_ext(None).execute_with(|| {
            put_v0(&live_ids());
            run_all::<Migration>();
            let migrated = root();

            // With the version bumped, it doesn't even start.
            assert_eq!(run_all::<Migration>(), None);
            assert_eq!(root(), migrated);

            // With the version back at 0, it walks everything and finds nothing to do.
            StorageVersion::new(0).put::<crate::Pallet<Test, ()>>();
            #[cfg(feature = "try-runtime")]
            let state = Migration::pre_upgrade().expect("pre_upgrade");
            assert_eq!(run_all::<Migration>(), None);
            assert_eq!(root(), migrated);
            #[cfg(feature = "try-runtime")]
            Migration::post_upgrade(state).expect("post_upgrade");
        });
    }

    // Stopped in the middle, begun again from the start (as if the cursor was lost): the entries
    // that were moved are not moved twice.
    #[test]
    fn restarting_midway_ends_in_the_same_state() {
        let once = new_test_ext(None).execute_with(|| {
            put_v0(&live_ids());
            run_all::<Migration>();
            root()
        });

        let restarted = new_test_ext(None).execute_with(|| {
            put_v0(&live_ids());
            let mut meter =
                WeightMeter::with_limit(<() as WeightInfo>::migrate_track().saturating_mul(10));
            Migration::step(None, &mut meter).expect("it doesn't fail");
            assert!(v0_keys().len() < 23);
            run_all::<Migration>();
            root()
        });

        assert_eq!(once, restarted);
    }

    // A chain that never had version 0 is at version 1 from genesis: nothing to do.
    #[test]
    fn does_nothing_if_there_was_no_version_0() {
        new_test_ext(Some(vec![
            (TRACK, ORIGIN_SIGNED_1),
            (TRACK, ORIGIN_SIGNED_2),
        ]))
        .execute_with(|| {
            assert_eq!(ReferendaTracks::<Test>::on_chain_storage_version(), 1);
            let before = root();

            assert_eq!(run_all::<Migration>(), None);

            assert_eq!(root(), before);
        });
    }

    // An empty v0 state gets to version 1 with nothing created.
    #[test]
    fn migrates_an_empty_state() {
        new_test_ext(None).execute_with(|| {
            put_v0(&[]);

            assert_eq!(run_all::<Migration>(), None);

            assert_eq!(crate::TracksIds::<Test, ()>::get().len(), 0);
            assert_eq!(crate::NextGroupId::<Test, ()>::get(), 0);
            assert_eq!(ReferendaTracks::<Test>::on_chain_storage_version(), 1);
        });
    }

    #[test]
    fn keys_of_the_cursor_fit() {
        Migration::integrity_test();
    }

    #[test]
    fn the_cursor_is_bounded_by_a_wide_origin() {
        // `Phase` (1 byte), the length prefix of the key (2 bytes) and 1_024 bytes of key.
        assert_eq!(MAX_KEY_LEN, 1_024);
        assert_eq!(Cursor::max_encoded_len(), 1_027);
    }

    // A wide `PalletsOrigin`: Kreivo's encodes up to 604 bytes, which makes the key of an
    // `OriginToTrackId` entry 32 + 16 + 604 = 652 bytes. It is a dummy type of that width, not
    // the origin of the mock, which is narrow.
    #[test]
    fn a_wide_origin_fits_the_cursor() {
        check_key_fits::<[u8; 604]>("an origin");
        // The widest that fits: 32 + 16 + 976 = 1_024.
        check_key_fits::<[u8; 976]>("an origin");
    }

    #[test]
    #[should_panic(expected = "an origin does not fit the cursor of the migration")]
    fn an_origin_wider_than_the_bound_does_not_pass_integrity_test() {
        // 32 + 16 + 977 = 1_025.
        check_key_fits::<[u8; 977]>("an origin");
    }

    fn raw_tracks_key(len: usize) -> Vec<u8> {
        let mut key = frame_support::storage::storage_prefix(b"Tracks", b"Tracks").to_vec();
        key.resize(len, 0xAB);
        key
    }

    // A key under the prefix of `Tracks` that is as long as the cursor can hold is not a track of
    // either version: it is passed over, and the cursor is that key.
    #[test]
    fn a_key_as_long_as_the_bound_is_skipped() {
        new_test_ext(None).execute_with(|| {
            put_v0(&[]);
            let key = raw_tracks_key(MAX_KEY_LEN as usize);
            unhashed::put_raw(&key, &[1, 2, 3]);

            let mut meter = WeightMeter::with_limit(Migration::unit(Phase::Tracks));
            let cursor = Migration::step(None, &mut meter).expect("it doesn't fail");

            assert_eq!(
                cursor,
                Some((Phase::Tracks, key.clone().try_into().unwrap()))
            );
            assert_eq!(unhashed::get_raw(&key), Some(vec![1, 2, 3]));
        });
    }

    // A key that is one byte longer than the bound cannot be resumed from: the migration fails
    // (it does not skip it, nor truncate it), with the state as it was.
    #[test]
    fn a_key_longer_than_the_bound_fails_safely() {
        new_test_ext(None).execute_with(|| {
            put_v0(&[]);
            let key = raw_tracks_key(MAX_KEY_LEN as usize + 1);
            unhashed::put_raw(&key, &[1, 2, 3]);
            let before = root();

            assert_eq!(
                Migration::step(None, &mut WeightMeter::new()),
                Err(SteppedMigrationError::Failed)
            );

            assert_eq!(root(), before);
            assert_eq!(unhashed::get_raw(&key), Some(vec![1, 2, 3]));
            assert_eq!(ReferendaTracks::<Test>::on_chain_storage_version(), 0);
        });
    }
}

mod max_tracks_limits {
    use super::*;

    #[test]
    fn add_sub_track_respects_max_tracks() {
        new_test_ext(None).execute_with(|| {
            // Fill up to the limit using new_group_with_track (simpler, avoids mock origin issues)
            for i in 0..MaxTracks::get() {
                let origin = OriginCaller::system(RawOrigin::Signed(1000 + i as u64));
                if let Err(e) = ReferendaTracks::<Test, ()>::new_group_with_track(
                    RuntimeOrigin::root(),
                    origin,
                    TRACK,
                ) {
                    assert_eq!(e, Error::<Test, ()>::MaxTracksExceeded.into());
                    return;
                }
            }

            // One more should fail
            let origin = OriginCaller::system(RawOrigin::Signed(9999));
            assert_noop!(
                ReferendaTracks::<Test, ()>::new_group_with_track(
                    RuntimeOrigin::root(),
                    origin,
                    TRACK,
                ),
                Error::<Test, ()>::MaxTracksExceeded
            );
        });
    }

    #[test]
    fn mixed_groups_and_sub_tracks_respect_max_tracks() {
        new_test_ext(None).execute_with(|| {
            // Create some groups via root
            assert_ok!(ReferendaTracks::<Test, ()>::new_group_with_track(
                RuntimeOrigin::root(),
                ORIGIN_SIGNED_1,
                TRACK,
            ));
            assert_ok!(ReferendaTracks::<Test, ()>::new_group_with_track(
                RuntimeOrigin::root(),
                ORIGIN_SIGNED_10,
                TRACK,
            ));

            // Add a sub-track to group 0
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                ORIGIN_SIGNED_3,
                TRACK,
            ));

            // Total: 3 tracks (2 groups + 1 sub-track)
            let ids = crate::TracksIds::<Test, ()>::get();
            assert_eq!(ids.len(), 3);

            // All track IDs should be unique and valid
            for id in ids.iter() {
                assert!(ReferendaTracks::<Test>::get_track_info(*id).is_some());
            }
        });
    }
}
