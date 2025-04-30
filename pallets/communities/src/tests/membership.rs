use super::*;
use crate::types::CommunityState::Blocked;
use frame_support::assert_noop;
use frame_system::RawOrigin::Root;
use sp_runtime::{traits::BadOrigin, DispatchError};

const COMMUNITY_NON_MEMBER: AccountId = AccountId::new([0; 32]);
const COMMUNITY_MEMBER_1: AccountId = AccountId::new([1; 32]);
const COMMUNITY_MEMBER_2: AccountId = AccountId::new([2; 32]);
const MEMBERSHIP_1: MembershipId = 1;
const MEMBERSHIP_2: MembershipId = 2;

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
    fn it_works() {
        new_test_ext(&[COMMUNITY_MEMBER_1], &[MEMBERSHIP_1]).execute_with(|| {
            assert_ok!(Communities::remove_member(
                COMMUNITY_ORIGIN.into(),
                COMMUNITY_MEMBER_1,
                MEMBERSHIP_1
            ));
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
