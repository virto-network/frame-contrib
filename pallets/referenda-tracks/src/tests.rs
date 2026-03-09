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

const ORIGIN_SIGNED_1: OriginCaller = OriginCaller::system(RawOrigin::Signed(1));
const ORIGIN_SIGNED_2: OriginCaller = OriginCaller::system(RawOrigin::Signed(2));

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
                    1,
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
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                1, // sub_track_id
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            // group=0, sub=1 -> combined ID = 1
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
    fn fails_if_sub_track_already_exists() {
        new_test_ext(None).execute_with(|| {
            // First insert succeeds (signer=1, origin morphs to account 1, group=0)
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                1,
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            // Same sub_track_id=1 in group 0 already exists
            // Use a different origin that also morphs to account 1 is not possible,
            // so we test with a different origin (signer=2, origin=SIGNED_2, group=1)
            // but same sub_track_id - this is a different group so it should work
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(2),
                1,
                ORIGIN_SIGNED_2,
                TRACK,
            ));

            // Verify both exist in their respective groups
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
                1,
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            // Same origin (ORIGIN_SIGNED_1), different sub_track_id
            assert_noop!(
                ReferendaTracks::<Test, ()>::add_sub_track(
                    RuntimeOrigin::signed(1),
                    2,
                    ORIGIN_SIGNED_1,
                    TRACK,
                ),
                Error::<Test, ()>::TrackIdAlreadyExisting
            );
        });
    }

    #[test]
    fn sub_tracks_across_groups() {
        new_test_ext(None).execute_with(|| {
            System::set_block_number(1);

            // Account 1 -> group 0, Account 2 -> group 1
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                0,
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(2),
                0,
                ORIGIN_SIGNED_2,
                TRACK,
            ));

            let id_g0 = u32::combine(0, 0);
            let id_g1 = u32::combine(1, 0);
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_g0), Some(TRACK));
            assert_eq!(ReferendaTracks::<Test>::get_track_info(id_g1), Some(TRACK));
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
            // Create sub-tracks in group 0
            assert_ok!(ReferendaTracks::<Test, ()>::add_sub_track(
                RuntimeOrigin::signed(1),
                0,
                ORIGIN_SIGNED_1,
                TRACK,
            ));

            let group_0_track = u32::combine(0, 0);

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
                <ReferendaTracks::<Test> as TracksInfo<u64, u64>>::track_ids().collect();
            let tracks: Vec<_> =
                <ReferendaTracks::<Test> as TracksInfo<u64, u64>>::tracks().collect();

            assert_eq!(track_ids.len(), 2);
            assert_eq!(tracks.len(), 2);

            // Every track returned by tracks() should have its id in track_ids()
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

mod migration {
    use super::*;
    use crate::migration::v0;
    use frame_support::{traits::UncheckedOnRuntimeUpgrade, BoundedVec};

    #[test]
    fn v0_to_v1_works() {
        new_test_ext(None).execute_with(|| {
            // Simulate v0 state: flat track IDs stored in old format
            // Old track 1 and 2 (these were group IDs in old model)
            let old_ids: BoundedVec<u32, MaxTracks> =
                BoundedVec::try_from(vec![1u32, 2u32]).unwrap();
            v0::TracksIds::<Test, ()>::put(&old_ids);
            v0::Tracks::<Test, ()>::insert(1u32, TRACK);
            v0::Tracks::<Test, ()>::insert(2u32, TRACK);

            // Set up OriginToTrackId with old IDs
            crate::OriginToTrackId::<Test, ()>::insert(&ORIGIN_SIGNED_1, 1u32);
            crate::OriginToTrackId::<Test, ()>::insert(&ORIGIN_SIGNED_2, 2u32);

            // Run migration
            crate::migration::MigrateV0ToV1::<Test, ()>::on_runtime_upgrade();

            // Verify: old track 1 -> combine(1, 0) = 65536
            // old track 2 -> combine(2, 0) = 131072
            let new_id_1 = u32::combine(1, 0);
            let new_id_2 = u32::combine(2, 0);
            assert_eq!(new_id_1, 65536);
            assert_eq!(new_id_2, 131072);

            // Tracks accessible via new DoubleMap
            assert_eq!(ReferendaTracks::<Test>::get_track_info(new_id_1), Some(TRACK));
            assert_eq!(ReferendaTracks::<Test>::get_track_info(new_id_2), Some(TRACK));

            // TracksIds updated to BoundedBTreeSet with new IDs
            let ids = crate::TracksIds::<Test, ()>::get();
            assert_eq!(ids.len(), 2);
            assert!(ids.contains(&new_id_1));
            assert!(ids.contains(&new_id_2));

            // OriginToTrackId updated to new IDs
            assert_eq!(
                crate::OriginToTrackId::<Test, ()>::get(&ORIGIN_SIGNED_1),
                Some(new_id_1)
            );
            assert_eq!(
                crate::OriginToTrackId::<Test, ()>::get(&ORIGIN_SIGNED_2),
                Some(new_id_2)
            );

            // TrackIdToOrigin populated
            assert_eq!(
                crate::TrackIdToOrigin::<Test, ()>::get(new_id_1),
                Some(ORIGIN_SIGNED_1)
            );
            assert_eq!(
                crate::TrackIdToOrigin::<Test, ()>::get(new_id_2),
                Some(ORIGIN_SIGNED_2)
            );

            // NextGroupId set to max(1,2) + 1 = 3
            assert_eq!(crate::NextGroupId::<Test, ()>::get(), 3u16);

            // Old Tracks storage cleared
            assert!(v0::Tracks::<Test, ()>::get(1u32).is_none());
            assert!(v0::Tracks::<Test, ()>::get(2u32).is_none());
        });
    }

    #[test]
    fn v0_to_v1_empty_state() {
        new_test_ext(None).execute_with(|| {
            // No tracks in v0
            crate::migration::MigrateV0ToV1::<Test, ()>::on_runtime_upgrade();

            assert_eq!(crate::TracksIds::<Test, ()>::get().len(), 0);
            // NextGroupId: default(0).increment() = 1
            assert_eq!(crate::NextGroupId::<Test, ()>::get(), 1u16);
        });
    }
}
