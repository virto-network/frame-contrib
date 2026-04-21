use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::BlakeTwo256;

use frame_contrib_traits::memberships::GenericRank;

use crate::mock::*;
use crate::types::*;
use crate::{Budget, MemberCount, Members, MerkleRoot, SubRoots, RanksTotal};

type Error = crate::Error<Test>;

#[test]
fn test_add_remove_member() {
    let alice = account(1);
    let bob = account(2);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);

            // Add alice
            assert_ok!(Communities::add_member(origin.clone(), alice.clone(), None, None));
            assert!(Members::<Test>::get(COMMUNITY, &alice).is_some());
            assert_eq!(MemberCount::<Test>::get(COMMUNITY), 1);

            // Add bob
            assert_ok!(Communities::add_member(origin.clone(), bob.clone(), None, None));
            assert_eq!(MemberCount::<Test>::get(COMMUNITY), 2);

            // Cannot add alice again
            assert_noop!(
                Communities::add_member(origin.clone(), alice.clone(), None, None),
                Error::AlreadyMember
            );

            // Remove alice
            assert_ok!(Communities::remove_member(origin.clone(), alice.clone()));
            assert!(Members::<Test>::get(COMMUNITY, &alice).is_none());
            assert_eq!(MemberCount::<Test>::get(COMMUNITY), 1);

            // Cannot remove alice again
            assert_noop!(
                Communities::remove_member(origin.clone(), alice.clone()),
                Error::NotAMember
            );
        });
}

#[test]
fn test_add_member_with_rank_and_role() {
    let alice = account(1);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);

            assert_ok!(Communities::add_member(
                origin.clone(),
                alice.clone(),
                Some(GenericRank::from(3u8)),
                Some(Role::Admin),
            ));

            let record = Members::<Test>::get(COMMUNITY, &alice).unwrap();
            assert_eq!(record.role, Role::Admin);
            let rank_val: u32 = record.rank.into();
            assert_eq!(rank_val, 3);
            assert_eq!(RanksTotal::<Test>::get(COMMUNITY), 3);
        });
}

#[test]
fn test_suspend_member() {
    let alice = account(1);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .add_member(COMMUNITY, alice.clone())
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);
            assert_eq!(MemberCount::<Test>::get(COMMUNITY), 1);

            // Suspend alice
            assert_ok!(Communities::suspend_member(origin.clone(), alice.clone()));

            let record = Members::<Test>::get(COMMUNITY, &alice).unwrap();
            assert_eq!(record.status, MemberStatus::Suspended);
            assert_eq!(record.nonce, 1);
            // Suspended members don't count
            assert_eq!(MemberCount::<Test>::get(COMMUNITY), 0);

            // Cannot suspend again
            assert_noop!(
                Communities::suspend_member(origin.clone(), alice.clone()),
                Error::MemberIsSuspended
            );
        });
}

#[test]
fn test_merkle_root_updates() {
    let alice = account(1);
    let bob = account(2);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);

            // No root initially
            assert!(MerkleRoot::<Test>::get(COMMUNITY).is_none());

            // Add alice, root should be set
            assert_ok!(Communities::add_member(origin.clone(), alice.clone(), None, None));
            let root1 = MerkleRoot::<Test>::get(COMMUNITY);
            assert!(root1.is_some());

            // Add bob, root should change
            assert_ok!(Communities::add_member(origin.clone(), bob.clone(), None, None));
            let root2 = MerkleRoot::<Test>::get(COMMUNITY);
            assert!(root2.is_some());
            assert_ne!(root1, root2);

            // Remove alice, root should change
            assert_ok!(Communities::remove_member(origin.clone(), alice.clone()));
            let root3 = MerkleRoot::<Test>::get(COMMUNITY);
            assert!(root3.is_some());
            assert_ne!(root2, root3);

            // Remove bob, root should be removed (no active members)
            assert_ok!(Communities::remove_member(origin.clone(), bob.clone()));
            assert!(MerkleRoot::<Test>::get(COMMUNITY).is_none());
        });
}

#[test]
fn test_private_community_root_update() {
    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Private)
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);

            let fake_root = <BlakeTwo256 as sp_runtime::traits::Hash>::hash_of(b"test_root");

            assert_ok!(Communities::update_membership_root(
                origin.clone(),
                fake_root,
                42,
            ));

            assert_eq!(MerkleRoot::<Test>::get(COMMUNITY), Some(fake_root));
            assert_eq!(MemberCount::<Test>::get(COMMUNITY), 42);
        });
}

#[test]
fn test_cannot_update_root_on_public_community() {
    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);
            let fake_root = <BlakeTwo256 as sp_runtime::traits::Hash>::hash_of(b"test_root");

            assert_noop!(
                Communities::update_membership_root(origin.clone(), fake_root, 10),
                Error::CommunityIsPublic
            );
        });
}

#[test]
fn test_cannot_add_member_to_private_community() {
    let alice = account(1);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Private)
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);

            assert_noop!(
                Communities::add_member(origin.clone(), alice.clone(), None, None),
                Error::CommunityIsPrivate
            );
        });
}

#[test]
fn test_sub_root_update() {
    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Private)
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);
            let fake_root = <BlakeTwo256 as sp_runtime::traits::Hash>::hash_of(b"sub_root");

            assert_ok!(Communities::update_sub_root(origin.clone(), 42, fake_root));
            assert_eq!(SubRoots::<Test>::get(COMMUNITY, 42), Some(fake_root));
        });
}

#[test]
fn test_role_default_is_member() {
    let alice = account(1);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .add_member(COMMUNITY, alice.clone())
        .build()
        .execute_with(|| {
            let record = Members::<Test>::get(COMMUNITY, &alice).unwrap();
            assert_eq!(record.role, Role::Member);
        });
}

#[test]
fn test_promote_demote_updates_merkle_root() {
    let alice = account(1);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .add_member(COMMUNITY, alice.clone())
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);
            let root_before = MerkleRoot::<Test>::get(COMMUNITY);

            assert_ok!(Communities::promote(origin.clone(), alice.clone()));
            let root_after_promote = MerkleRoot::<Test>::get(COMMUNITY);
            assert_ne!(root_before, root_after_promote);

            assert_ok!(Communities::demote(origin.clone(), alice.clone()));
            let root_after_demote = MerkleRoot::<Test>::get(COMMUNITY);
            assert_ne!(root_after_promote, root_after_demote);
            // After demote back to 0, should match original root
            assert_eq!(root_before, root_after_demote);
        });
}

#[test]
fn test_set_budget() {
    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);

            assert_ok!(Communities::set_budget(origin.clone(), 1000, 100));

            let budget = Budget::<Test>::get(COMMUNITY).expect("budget should exist");
            assert_eq!(budget.capacity, 1000);
            assert_eq!(budget.used, 0);
            assert_eq!(budget.session_length, 100);
            assert_eq!(budget.session_start, 1); // block 1 from test setup
        });
}

#[test]
fn test_budget_check_and_burn() {
    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);

            assert_ok!(Communities::set_budget(origin.clone(), 1000, 100));

            // Check budget available
            let remaining = Communities::check_budget(&COMMUNITY, 200).expect("should have budget");
            assert_eq!(remaining, 800);

            // Burn some budget
            Communities::burn_budget(&COMMUNITY, 300);
            let budget = Budget::<Test>::get(COMMUNITY).unwrap();
            assert_eq!(budget.used, 300);

            // Check again with reduced budget
            let remaining = Communities::check_budget(&COMMUNITY, 200).expect("should have budget");
            assert_eq!(remaining, 500);

            // Refund some
            Communities::refund_budget(&COMMUNITY, 100);
            let budget = Budget::<Test>::get(COMMUNITY).unwrap();
            assert_eq!(budget.used, 200);
        });
}

#[test]
fn test_budget_session_reset() {
    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);

            // Set budget at block 1 with session length 10
            assert_ok!(Communities::set_budget(origin.clone(), 1000, 10));

            // Burn some budget
            Communities::burn_budget(&COMMUNITY, 500);
            assert_eq!(Budget::<Test>::get(COMMUNITY).unwrap().used, 500);

            // Advance past session end (block 1 + 10 = 11)
            frame_system::Pallet::<Test>::set_block_number(11);

            // check_budget should reset the session
            let remaining = Communities::check_budget(&COMMUNITY, 100).expect("should have budget");
            assert_eq!(remaining, 900); // full capacity minus cost

            // burn_budget should also reset the session
            frame_system::Pallet::<Test>::set_block_number(22);
            Communities::burn_budget(&COMMUNITY, 200);
            let budget = Budget::<Test>::get(COMMUNITY).unwrap();
            assert_eq!(budget.used, 200);
            assert_eq!(budget.session_start, 22);
        });
}

#[test]
fn test_budget_exhaustion() {
    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);

            assert_ok!(Communities::set_budget(origin.clone(), 100, 50));

            // Burn all budget
            Communities::burn_budget(&COMMUNITY, 100);

            // Check should fail
            assert!(Communities::check_budget(&COMMUNITY, 1).is_err());

            // Check on community without budget should also fail
            assert!(Communities::check_budget(&999, 1).is_err());
        });
}
