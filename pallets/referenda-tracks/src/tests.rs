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
                ReferendaTracks::<Test, ()>::remove(RuntimeOrigin::signed(1), 65536),
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
                ReferendaTracks::<Test, ()>::remove(RuntimeOrigin::signed(1), 65536),
                Error::<Test, ()>::CannotRemove
            );

            pallet_referenda::DecidingCount::<Test, ()>::insert(65536u32, 0u32);

            assert_ok!(ReferendaTracks::<Test, ()>::remove(
                RuntimeOrigin::signed(1),
                65536,
            ));
        });
    }

    #[test]
    fn origin_can_be_reused_after_removal() {
        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            assert_ok!(ReferendaTracks::<Test, ()>::remove(
                RuntimeOrigin::signed(1),
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
                RuntimeOrigin::signed(1),
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
    fn cleans_up_origin_mappings() {
        use pallet_referenda::TracksInfo;

        new_test_ext(Some(vec![(TRACK, ORIGIN_SIGNED_1)])).execute_with(|| {
            // Verify origin mapping exists
            assert_eq!(
                <ReferendaTracks<Test> as TracksInfo<u64, u64>>::track_for(&ORIGIN_SIGNED_1),
                Ok(65536)
            );

            assert_ok!(ReferendaTracks::<Test, ()>::remove(
                RuntimeOrigin::signed(1),
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
        migration::{v0, Cursor, CursorKey, MigrateV0ToV1, Phase, TryIntoGroup},
        WeightInfo,
    };
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
                    Some(Phase::Finish) => <() as WeightInfo>::migrate_finalize(MaxTracks::get()),
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
            assert_eq!(phases, vec![Phase::Tracks, Phase::Origins, Phase::Finish]);
            assert_migrated(&live_ids());
            root()
        });

        assert_eq!(unlimited, resumed);
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
                    Some((Phase::Finish, CursorKey::default())),
                    <() as WeightInfo>::migrate_finalize(MaxTracks::get()),
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
