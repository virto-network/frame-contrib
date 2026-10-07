use super::*;
use crate::{
    types::CommunityState::{self, Active, Blocked},
    DecisionMethod, Event,
};
use frame_contrib_traits::memberships::{
    GenericRank, Inspect, InspectEnumerable, Issue, Manager, Rank, RankOnTransfer, Receivers,
    Transfer, TransferPolicy,
};
use frame_support::assert_noop;
use frame_system::RawOrigin::Root;
use sp_runtime::{traits::BadOrigin, DispatchError};

const COMMUNITY_NON_MEMBER: AccountId = AccountId::new([0; 32]);
const COMMUNITY_MEMBER_1: AccountId = AccountId::new([1; 32]);
const COMMUNITY_MEMBER_2: AccountId = AccountId::new([2; 32]);
const MEMBERSHIP_1: MembershipId = 1;
const MEMBERSHIP_2: MembershipId = 2;
const MEMBERSHIP_3: MembershipId = 3;

const OTHER_COMMUNITY: CommunityId = 2;
const OTHER_MEMBERSHIP: MembershipId = 100;

fn stock(community_id: CommunityId) -> Vec<MembershipId> {
    let mut stock =
        MembershipsManager::group_available_memberships(&community_id).collect::<Vec<_>>();
    stock.sort();
    stock
}

fn set_policy(receivers: Receivers, rank: RankOnTransfer) {
    assert_ok!(Communities::set_transfer_policy(
        COMMUNITY_ORIGIN.into(),
        TransferPolicy { receivers, rank }
    ));
}

mod add_member {
    use super::*;

    #[test]
    fn fails_when_community_is_not_active() {
        new_test_ext(&[], &[MEMBERSHIP_1]).execute_with(|| {
            Communities::force_state(&COMMUNITY, Blocked);
            assert_noop!(
                Communities::add_member(COMMUNITY_ORIGIN.into(), COMMUNITY_MEMBER_1),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn fails_when_caller_not_a_valid_origin() {
        new_test_ext(&[], &[MEMBERSHIP_1]).execute_with(|| {
            assert_noop!(
                Communities::add_member(RuntimeOrigin::none(), COMMUNITY_MEMBER_1),
                DispatchError::BadOrigin
            );
            assert_noop!(
                Communities::add_member(Root.into(), COMMUNITY_MEMBER_1),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn adds_members() {
        new_test_ext(&[], &[MEMBERSHIP_1, MEMBERSHIP_2]).execute_with(|| {
            // Successfully adds members
            assert_ok!(Communities::add_member(
                COMMUNITY_ORIGIN.into(),
                COMMUNITY_MEMBER_1
            ));
            assert_ok!(Communities::add_member(
                COMMUNITY_ORIGIN.into(),
                COMMUNITY_MEMBER_2
            ));

            assert!(Communities::is_member(&COMMUNITY, &COMMUNITY_MEMBER_1));
            assert!(Communities::is_member(&COMMUNITY, &COMMUNITY_MEMBER_2));
        });
    }

    #[test]
    fn takes_the_membership_from_the_stock() {
        new_test_ext(&[], &[MEMBERSHIP_1, MEMBERSHIP_2]).execute_with(|| {
            assert_eq!(stock(COMMUNITY), vec![MEMBERSHIP_1, MEMBERSHIP_2]);

            assert_ok!(Communities::add_member(
                COMMUNITY_ORIGIN.into(),
                COMMUNITY_MEMBER_1
            ));

            let held = Communities::get_memberships(COMMUNITY, &COMMUNITY_MEMBER_1);
            assert_eq!(held.len(), 1);
            assert!(!stock(COMMUNITY).contains(&held[0]));
            assert_eq!(stock(COMMUNITY).len(), 1);
            assert_eq!(MembershipsManager::members_total(&COMMUNITY), 1);
            System::assert_last_event(
                Event::MemberAdded {
                    who: COMMUNITY_MEMBER_1,
                    membership_id: held[0],
                }
                .into(),
            );
        });
    }

    #[test]
    fn fails_when_the_stock_is_empty() {
        new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1]).execute_with(|| {
            assert_noop!(
                Communities::add_member(COMMUNITY_ORIGIN.into(), COMMUNITY_MEMBER_2),
                Error::CommunityAtCapacity
            );
        });
    }

    #[test]
    fn ignores_items_the_community_account_holds_in_other_collections() {
        // NOTES C.16: the community account's first item in any collection is not its stock.
        TestEnvBuilder::new()
            .add_community(COMMUNITY, DecisionMethod::Membership, &[], &[], None)
            .add_community(
                OTHER_COMMUNITY,
                DecisionMethod::Membership,
                &[],
                &[OTHER_MEMBERSHIP],
                None,
            )
            .build()
            .execute_with(|| {
                let account = Communities::community_account(&COMMUNITY);
                assert_ok!(MembershipsManager::assign(
                    &OTHER_COMMUNITY,
                    &OTHER_MEMBERSHIP,
                    &account
                ));

                assert_noop!(
                    Communities::add_member(COMMUNITY_ORIGIN.into(), COMMUNITY_MEMBER_1),
                    Error::CommunityAtCapacity
                );
            });
    }

    // REQ-MI-13
    #[test]
    fn the_community_account_cannot_be_added_as_a_member() {
        new_test_ext(&[], &[MEMBERSHIP_1]).execute_with(|| {
            let account = Communities::community_account(&COMMUNITY);
            assert!(!Communities::is_member(&COMMUNITY, &account));
            assert_noop!(
                Communities::add_member(COMMUNITY_ORIGIN.into(), account),
                Error::NotSameGroup
            );
        });
    }

    #[test]
    fn can_add_member_twice() {
        // As memberships could be transferred there is no use in restricting adding the same member
        // twice.
        new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1, MEMBERSHIP_2]).execute_with(|| {
            // Fails to add a member twice
            assert_ok!(Communities::add_member(
                COMMUNITY_ORIGIN.into(),
                COMMUNITY_MEMBER_1
            ));
            assert_eq!(
                Communities::get_memberships(COMMUNITY, &COMMUNITY_MEMBER_1),
                vec![MEMBERSHIP_1, MEMBERSHIP_2]
            );
        });
    }
}

mod remove_member {
    use super::*;

    #[test]
    fn fails_when_community_is_not_active() {
        new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1]).execute_with(|| {
            Communities::force_state(&COMMUNITY, Blocked);
            assert_noop!(
                Communities::remove_member(
                    COMMUNITY_ORIGIN.into(),
                    COMMUNITY_MEMBER_1,
                    MEMBERSHIP_1
                ),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn fails_when_caller_not_a_privileged_origin() {
        new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1]).execute_with(|| {
            assert_noop!(
                Communities::remove_member(RuntimeOrigin::none(), COMMUNITY_MEMBER_1, MEMBERSHIP_1),
                DispatchError::BadOrigin
            );
            assert_noop!(
                Communities::remove_member(Root.into(), COMMUNITY_MEMBER_1, MEMBERSHIP_1),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn fails_when_not_a_community_member() {
        new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1]).execute_with(|| {
            assert_noop!(
                Communities::remove_member(
                    COMMUNITY_ORIGIN.into(),
                    COMMUNITY_NON_MEMBER,
                    MEMBERSHIP_1
                ),
                Error::NotAMember
            );
        });
    }

    #[test]
    fn fails_when_the_member_does_not_hold_the_membership() {
        // NOTES C.15: removing a member checks who holds the membership.
        new_test_ext(
            &[COMMUNITY_MEMBER_1, COMMUNITY_MEMBER_2],
            &[MEMBERSHIP_1, MEMBERSHIP_2],
        )
        .execute_with(|| {
            assert_noop!(
                Communities::remove_member(
                    COMMUNITY_ORIGIN.into(),
                    COMMUNITY_MEMBER_1,
                    MEMBERSHIP_2
                ),
                Error::NotAMember
            );
        });
    }

    #[test]
    fn fails_for_a_membership_in_the_stock() {
        new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1, MEMBERSHIP_2]).execute_with(|| {
            let account = Communities::community_account(&COMMUNITY);
            assert_noop!(
                Communities::remove_member(COMMUNITY_ORIGIN.into(), account, MEMBERSHIP_2),
                Error::NotAMember
            );
        });
    }

    #[test]
    fn it_works() {
        new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1]).execute_with(|| {
            assert_ok!(Communities::remove_member(
                COMMUNITY_ORIGIN.into(),
                COMMUNITY_MEMBER_1,
                MEMBERSHIP_1
            ));
        });
    }

    // REQ-MI-D2
    #[test]
    fn a_retiring_membership_cannot_be_put_back_in_the_stock() {
        new_test_ext(&[], &[MEMBERSHIP_1]).execute_with(|| {
            assert_ok!(MembershipsManager::retire(
                &COMMUNITY,
                &MEMBERSHIP_1,
                &RETIREMENT_HOLDER
            ));

            assert_noop!(
                Communities::remove_member(
                    COMMUNITY_ORIGIN.into(),
                    RETIREMENT_HOLDER,
                    MEMBERSHIP_1
                ),
                Error::NotAMember
            );
            assert!(!Communities::is_member(&COMMUNITY, &RETIREMENT_HOLDER));
            assert_eq!(stock(COMMUNITY), Vec::<MembershipId>::new());
            assert_noop!(
                Communities::add_member(COMMUNITY_ORIGIN.into(), COMMUNITY_MEMBER_1),
                Error::CommunityAtCapacity
            );
        });
    }

    // AC-F3.1
    #[test]
    fn returns_the_membership_to_the_stock_with_its_rank_reset() {
        new_test_ext(
            &[COMMUNITY_MEMBER_1, COMMUNITY_MEMBER_2],
            &[MEMBERSHIP_1, MEMBERSHIP_2],
        )
        .execute_with(|| {
            assert_ok!(Communities::promote(COMMUNITY_ORIGIN.into(), MEMBERSHIP_1));
            assert_ok!(Communities::promote(COMMUNITY_ORIGIN.into(), MEMBERSHIP_2));
            assert_eq!(MembershipsManager::ranks_total(&COMMUNITY), 2);

            assert_ok!(Communities::remove_member(
                COMMUNITY_ORIGIN.into(),
                COMMUNITY_MEMBER_1,
                MEMBERSHIP_1
            ));

            assert_eq!(stock(COMMUNITY), vec![MEMBERSHIP_1]);
            assert!(!Communities::is_member(&COMMUNITY, &COMMUNITY_MEMBER_1));
            assert_eq!(
                Communities::member_rank(&COMMUNITY, &MEMBERSHIP_1),
                0.into()
            );
            assert_eq!(MembershipsManager::ranks_total(&COMMUNITY), 1);
            assert_eq!(MembershipsManager::members_total(&COMMUNITY), 1);
            System::assert_last_event(
                Event::MemberRemoved {
                    who: COMMUNITY_MEMBER_1,
                    membership_id: MEMBERSHIP_1,
                }
                .into(),
            );
        });
    }
}

mod transfer_membership {
    use super::*;

    /// MEMBER_1 holds MEMBERSHIP_1, MEMBER_2 holds MEMBERSHIP_2, MEMBERSHIP_3 is in the stock.
    fn new_test_ext() -> sp_io::TestExternalities {
        super::new_test_ext(
            &[COMMUNITY_MEMBER_1, COMMUNITY_MEMBER_2],
            &[MEMBERSHIP_1, MEMBERSHIP_2, MEMBERSHIP_3],
        )
    }

    #[test]
    fn fails_when_origin_is_not_a_member_origin() {
        new_test_ext().execute_with(|| {
            set_policy(Receivers::ToAnyAccount, RankOnTransfer::Keep);
            assert_noop!(
                Communities::transfer_membership(
                    RuntimeOrigin::none(),
                    MEMBERSHIP_1,
                    COMMUNITY_NON_MEMBER
                ),
                BadOrigin
            );
            assert_noop!(
                Communities::transfer_membership(
                    COMMUNITY_ORIGIN.into(),
                    MEMBERSHIP_1,
                    COMMUNITY_NON_MEMBER
                ),
                BadOrigin
            );
        });
    }

    // REQ-MI-14, REQ-MI-9, CTR-CALL-2
    #[test]
    fn memberships_pushed_by_another_community_never_hide_one() {
        let pushed = (OTHER_MEMBERSHIP..OTHER_MEMBERSHIP + 65).collect::<Vec<_>>();
        TestEnvBuilder::new()
            .add_community(
                COMMUNITY,
                DecisionMethod::Membership,
                &[COMMUNITY_MEMBER_1, COMMUNITY_MEMBER_2],
                &[MEMBERSHIP_1, MEMBERSHIP_2],
                None,
            )
            .add_community(
                OTHER_COMMUNITY,
                DecisionMethod::Membership,
                &[],
                &pushed,
                None,
            )
            .build()
            .execute_with(|| {
                // The other community's admin gives the member every one of its memberships.
                let other = TestEnvBuilder::create_community_origin(&OTHER_COMMUNITY);
                for _ in &pushed {
                    assert_ok!(Communities::add_member(other.clone(), COMMUNITY_MEMBER_1));
                }
                set_policy(Receivers::ToExistingMembers, RankOnTransfer::Keep);

                assert_ok!(Communities::transfer_membership(
                    RuntimeOrigin::signed(COMMUNITY_MEMBER_1),
                    MEMBERSHIP_1,
                    COMMUNITY_MEMBER_2
                ));
                assert!(MembershipsManager::holds(
                    &COMMUNITY,
                    &COMMUNITY_MEMBER_2,
                    &MEMBERSHIP_1
                ));
            });
    }

    #[test]
    fn fails_when_the_caller_does_not_hold_the_membership() {
        new_test_ext().execute_with(|| {
            set_policy(Receivers::ToAnyAccount, RankOnTransfer::Keep);
            assert_noop!(
                Communities::transfer_membership(
                    RuntimeOrigin::signed(COMMUNITY_MEMBER_2),
                    MEMBERSHIP_1,
                    COMMUNITY_NON_MEMBER
                ),
                Error::NotAMember
            );
            // The stock is not the community account's to transfer.
            assert_noop!(
                Communities::transfer_membership(
                    RuntimeOrigin::signed(Communities::community_account(&COMMUNITY)),
                    MEMBERSHIP_3,
                    COMMUNITY_NON_MEMBER
                ),
                Error::NotAMember
            );
        });
    }

    // AC-F4.4
    #[test]
    fn the_default_policy_and_disabled_refuse_a_transfer() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Communities::transfer_membership(
                    RuntimeOrigin::signed(COMMUNITY_MEMBER_1),
                    MEMBERSHIP_1,
                    COMMUNITY_MEMBER_2
                ),
                Error::TransferDisabled
            );

            set_policy(Receivers::Disabled, RankOnTransfer::Keep);
            assert_noop!(
                Communities::transfer_membership(
                    RuntimeOrigin::signed(COMMUNITY_MEMBER_1),
                    MEMBERSHIP_1,
                    COMMUNITY_MEMBER_2
                ),
                Error::TransferDisabled
            );
        });
    }

    // AC-F4.1
    #[test]
    fn transfer_to_an_existing_member_resets_the_rank() {
        new_test_ext().execute_with(|| {
            assert_ok!(Communities::promote(COMMUNITY_ORIGIN.into(), MEMBERSHIP_1));
            set_policy(Receivers::ToExistingMembers, RankOnTransfer::Reset);

            assert_ok!(Communities::transfer_membership(
                RuntimeOrigin::signed(COMMUNITY_MEMBER_1),
                MEMBERSHIP_1,
                COMMUNITY_MEMBER_2
            ));

            assert!(MembershipsManager::holds(
                &COMMUNITY,
                &COMMUNITY_MEMBER_2,
                &MEMBERSHIP_1
            ));
            assert_eq!(
                Communities::member_rank(&COMMUNITY, &MEMBERSHIP_1),
                0.into()
            );
            assert_eq!(MembershipsManager::members_total(&COMMUNITY), 2);
            assert_eq!(stock(COMMUNITY), vec![MEMBERSHIP_3]);
            System::assert_last_event(
                Event::MembershipTransferred {
                    id: COMMUNITY,
                    membership_id: MEMBERSHIP_1,
                    from: COMMUNITY_MEMBER_1,
                    to: COMMUNITY_MEMBER_2,
                }
                .into(),
            );
        });
    }

    // AC-F4.2
    #[test]
    fn transfer_to_existing_members_refuses_a_non_member_and_the_community_account() {
        new_test_ext().execute_with(|| {
            set_policy(Receivers::ToExistingMembers, RankOnTransfer::Reset);
            assert_noop!(
                Communities::transfer_membership(
                    RuntimeOrigin::signed(COMMUNITY_MEMBER_1),
                    MEMBERSHIP_1,
                    COMMUNITY_NON_MEMBER
                ),
                Error::NotSameGroup
            );
            assert_noop!(
                Communities::transfer_membership(
                    RuntimeOrigin::signed(COMMUNITY_MEMBER_1),
                    MEMBERSHIP_1,
                    Communities::community_account(&COMMUNITY)
                ),
                Error::NotSameGroup
            );
        });
    }

    // AC-F4.5
    #[test]
    fn transfer_to_any_account_makes_the_recipient_a_member() {
        new_test_ext().execute_with(|| {
            set_policy(Receivers::ToAnyAccount, RankOnTransfer::Reset);
            assert!(!Communities::is_member(&COMMUNITY, &COMMUNITY_NON_MEMBER));

            assert_ok!(Communities::transfer_membership(
                RuntimeOrigin::signed(COMMUNITY_MEMBER_1),
                MEMBERSHIP_1,
                COMMUNITY_NON_MEMBER
            ));

            assert!(Communities::is_member(&COMMUNITY, &COMMUNITY_NON_MEMBER));
            assert!(!Communities::is_member(&COMMUNITY, &COMMUNITY_MEMBER_1));
            assert_eq!(
                Communities::get_memberships(COMMUNITY, &COMMUNITY_NON_MEMBER),
                vec![MEMBERSHIP_1]
            );
            assert_noop!(
                Communities::transfer_membership(
                    RuntimeOrigin::signed(COMMUNITY_NON_MEMBER),
                    MEMBERSHIP_1,
                    Communities::community_account(&COMMUNITY)
                ),
                Error::NotSameGroup
            );
        });
    }

    // AC-F4.6
    #[test]
    fn a_kept_rank_travels_with_the_membership() {
        new_test_ext().execute_with(|| {
            assert_ok!(Communities::promote(COMMUNITY_ORIGIN.into(), MEMBERSHIP_1));
            assert_ok!(Communities::promote(COMMUNITY_ORIGIN.into(), MEMBERSHIP_1));
            set_policy(Receivers::ToAnyAccount, RankOnTransfer::Keep);

            assert_ok!(Communities::transfer_membership(
                RuntimeOrigin::signed(COMMUNITY_MEMBER_1),
                MEMBERSHIP_1,
                COMMUNITY_NON_MEMBER
            ));

            assert_eq!(
                Communities::member_rank(&COMMUNITY, &MEMBERSHIP_1),
                GenericRank::from(2)
            );
            assert_eq!(MembershipsManager::ranks_total(&COMMUNITY), 2);
        });
    }
}

mod set_transfer_policy {
    use super::*;

    const ANY_KEEP: TransferPolicy = TransferPolicy {
        receivers: Receivers::ToAnyAccount,
        rank: RankOnTransfer::Keep,
    };

    // AC-F6.1
    #[test]
    fn a_community_that_never_set_a_policy_reads_disabled_and_reset() {
        new_test_ext(&[], &[]).execute_with(|| {
            assert_eq!(
                MembershipsManager::transfer_policy(&COMMUNITY),
                TransferPolicy {
                    receivers: Receivers::Disabled,
                    rank: RankOnTransfer::Reset,
                }
            );
        });
    }

    // AC-F6.2
    #[test]
    fn the_admin_sets_the_policy_and_the_next_transfer_follows_it() {
        new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1]).execute_with(|| {
            assert_ok!(Communities::set_transfer_policy(
                COMMUNITY_ORIGIN.into(),
                ANY_KEEP
            ));

            assert_eq!(MembershipsManager::transfer_policy(&COMMUNITY), ANY_KEEP);
            System::assert_last_event(
                Event::TransferPolicySet {
                    id: COMMUNITY,
                    policy: ANY_KEEP,
                }
                .into(),
            );
            assert_ok!(Communities::transfer_membership(
                RuntimeOrigin::signed(COMMUNITY_MEMBER_1),
                MEMBERSHIP_1,
                COMMUNITY_NON_MEMBER
            ));
        });
    }

    // AC-F6.3
    #[test]
    fn any_other_origin_is_refused() {
        TestEnvBuilder::new()
            .add_community(
                COMMUNITY,
                DecisionMethod::Membership,
                &[COMMUNITY_MEMBER_1],
                &[MEMBERSHIP_1],
                None,
            )
            .add_community(OTHER_COMMUNITY, DecisionMethod::Membership, &[], &[], None)
            .build()
            .execute_with(|| {
                // A member's signed origin, Root, no origin, and the community account signing.
                for origin in [
                    RuntimeOrigin::signed(COMMUNITY_MEMBER_1),
                    Root.into(),
                    RuntimeOrigin::none(),
                    RuntimeOrigin::signed(Communities::community_account(&COMMUNITY)),
                ] {
                    assert_noop!(
                        Communities::set_transfer_policy(origin, ANY_KEEP),
                        BadOrigin
                    );
                }
                // Another community's admin only ever sets its own community's policy.
                assert_ok!(Communities::set_transfer_policy(
                    TestEnvBuilder::create_community_origin(&OTHER_COMMUNITY),
                    ANY_KEEP
                ));
                assert_eq!(
                    MembershipsManager::transfer_policy(&COMMUNITY),
                    TransferPolicy::default()
                );
                assert_eq!(
                    MembershipsManager::transfer_policy(&OTHER_COMMUNITY),
                    ANY_KEEP
                );
            });
    }

    #[test]
    fn fails_when_community_is_not_active() {
        new_test_ext(&[], &[]).execute_with(|| {
            Communities::force_state(&COMMUNITY, Blocked);
            assert_noop!(
                Communities::set_transfer_policy(COMMUNITY_ORIGIN.into(), ANY_KEEP),
                BadOrigin
            );
        });
    }
}

mod community_state {
    use super::*;

    // CTR-MEM-3
    #[test]
    fn reads_the_state_of_a_community() {
        new_test_ext(&[], &[]).execute_with(|| {
            assert_eq!(Communities::community_state(&COMMUNITY), Some(Active));
            Communities::force_state(&COMMUNITY, Blocked);
            assert_eq!(
                Communities::community_state(&COMMUNITY),
                Some(CommunityState::Blocked)
            );
            assert_eq!(Communities::community_state(&OTHER_COMMUNITY), None);
        });
    }
}

mod member_rank {
    use super::*;

    mod promote_member {
        use super::*;

        #[test]
        fn fails_when_caller_not_admin_origin() {
            new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1]).execute_with(|| {
                assert_noop!(Communities::promote(Root.into(), MEMBERSHIP_1), BadOrigin);
            });
        }

        #[test]
        fn fails_when_not_a_community_member() {
            new_test_ext(&[], &[MEMBERSHIP_1]).execute_with(|| {
                assert_noop!(
                    Communities::promote(COMMUNITY_ORIGIN.into(), MEMBERSHIP_1),
                    Error::NotAMember,
                );
            });
        }

        #[test]
        fn it_works() {
            new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1]).execute_with(|| {
                assert_ok!(Communities::promote(COMMUNITY_ORIGIN.into(), MEMBERSHIP_1));
                assert_eq!(
                    Communities::member_rank(&COMMUNITY, &MEMBERSHIP_1),
                    1.into()
                );
            });
        }
    }

    mod demote_member {
        use super::*;

        #[test]
        fn fails_when_caller_not_a_privleged_origin() {
            new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1]).execute_with(|| {
                assert_noop!(Communities::demote(Root.into(), MEMBERSHIP_1), BadOrigin);
            });
        }

        #[test]
        fn fails_when_not_a_community_member() {
            new_test_ext(&[], &[]).execute_with(|| {
                assert_noop!(
                    Communities::demote(COMMUNITY_ORIGIN.into(), MEMBERSHIP_1),
                    Error::NotAMember,
                );
            });
        }

        #[test]
        fn it_works() {
            new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1]).execute_with(|| {
                Communities::promote(COMMUNITY_ORIGIN.into(), MEMBERSHIP_1).expect("can promote");
                Communities::promote(COMMUNITY_ORIGIN.into(), MEMBERSHIP_1).expect("can promote");
                assert_ok!(Communities::demote(COMMUNITY_ORIGIN.into(), MEMBERSHIP_1));
                assert_eq!(
                    Communities::member_rank(&COMMUNITY, &MEMBERSHIP_1),
                    1.into()
                );
            });
        }

        #[test]
        fn should_remain_at_min_rank() {
            new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1]).execute_with(|| {
                assert_eq!(
                    Communities::member_rank(&COMMUNITY, &MEMBERSHIP_1),
                    0.into()
                );
                assert_ok!(Communities::demote(COMMUNITY_ORIGIN.into(), MEMBERSHIP_1,));
                assert_eq!(
                    Communities::member_rank(&COMMUNITY, &MEMBERSHIP_1),
                    0.into()
                );
            });
        }
    }
}
