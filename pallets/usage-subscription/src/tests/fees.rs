//! `PayingGroupCommunity`, the `fc-pallet-fees` adapter (`REQ-PC-2`, `REQ-PC-3`).

use super::*;
use fc_pallet_fees::AccountCommunity;

type Detector = PayingGroupCommunity<Test>;

// REQ-PC-2 (named group)
#[test]
fn named_group_is_the_community() {
    new_test_ext().execute_with(|| {
        add_member(GROUP_A, 1, &ALICE);
        add_member(GROUP_B, 2, &ALICE);
        assert_ok!(UsageSubscription::set_paying_group(
            RuntimeOrigin::signed(ALICE),
            Some(GROUP_B)
        ));
        assert_eq!(Detector::community_of(&ALICE), Some(GROUP_B));
    });
}

// REQ-PC-2 (the only group)
#[test]
fn single_group_is_the_community() {
    new_test_ext().execute_with(|| {
        add_member(GROUP_A, 1, &ALICE);
        assert_eq!(Detector::community_of(&ALICE), Some(GROUP_A));
    });
}

// REQ-PC-2 (no group), AC-C3.1
#[test]
fn no_resolved_group_means_no_community() {
    new_test_ext().execute_with(|| {
        // No membership.
        assert_eq!(Detector::community_of(&BOB), None);
        // Several groups, none named.
        add_member(GROUP_A, 1, &ALICE);
        add_member(GROUP_B, 2, &ALICE);
        assert_eq!(Detector::community_of(&ALICE), None);
    });
}
