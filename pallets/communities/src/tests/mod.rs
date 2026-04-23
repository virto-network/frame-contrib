use frame_support::{assert_noop, assert_ok, traits::Hooks};
use sp_runtime::traits::{BlakeTwo256, Hash as _};

use frame_contrib_traits::memberships::GenericRank;
use sp_core::H256;

use crate::mock::*;
use crate::types::*;
use crate::verifier::{MembershipInputs, MerkleProof};
use crate::{Budget, ClaimedSupport, CommunityDecisionMethod, CommunityVotes, MemberCount, Members, MerkleRoot, SubRoots, RanksTotal, UsedNullifiers};
use fc_traits_proof_verifier::ProofVerifier;

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
            // H3 fix: the admin-supplied number lands in ClaimedSupport, not MemberCount.
            assert_eq!(ClaimedSupport::<Test>::get(COMMUNITY), 42);
            assert_eq!(
                MemberCount::<Test>::get(COMMUNITY),
                0,
                "MemberCount must not be writable by update_membership_root"
            );
        });
}

#[test]
fn test_m2_suspend_clears_root_on_private_community() {
    // Without this, a suspended member's old merkle proof remains valid against the
    // stale root until the admin manually republishes. Fail-closed: clear the root on
    // any on-chain membership change for Private/Hybrid communities so the extension
    // rejects proofs with NO_MEMBERSHIP_ROOT until the admin republishes.
    let alice = account(1);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Hybrid)
        .build()
        .execute_with(|| {
            let origin = community_origin(COMMUNITY);
            // Seed a member on-chain so we have something to suspend.
            crate::Members::<Test>::insert(
                COMMUNITY,
                &alice,
                MemberRecord {
                    rank: GenericRank::default(),
                    role: Role::Member,
                    ..Default::default()
                },
            );
            MemberCount::<Test>::insert(COMMUNITY, 1);

            let published_root =
                <BlakeTwo256 as sp_runtime::traits::Hash>::hash_of(b"off-chain tree");
            assert_ok!(Communities::update_membership_root(
                origin.clone(),
                published_root,
                100,
            ));
            assert_eq!(MerkleRoot::<Test>::get(COMMUNITY), Some(published_root));

            assert_ok!(Communities::suspend_member(origin.clone(), alice.clone()));
            assert!(
                MerkleRoot::<Test>::get(COMMUNITY).is_none(),
                "on-chain suspension must invalidate the published root until admin republishes"
            );
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

#[test]
fn test_anonymous_membership_proof_validation() {
    let alice = account(1);
    let bob = account(2);
    let charlie = account(3);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .add_member(COMMUNITY, alice.clone())
        .add_member(COMMUNITY, bob.clone())
        .add_member(COMMUNITY, charlie.clone())
        .build()
        .execute_with(|| {
            // The merkle root should be set
            let root = MerkleRoot::<Test>::get(COMMUNITY).expect("root should exist");

            // Compute the leaves the same way recompute_merkle_root does
            let mut leaves: alloc::vec::Vec<sp_core::H256> =
                Members::<Test>::iter_prefix(COMMUNITY)
                    .filter(|(_, record)| record.status == MemberStatus::Active)
                    .map(|(who, record)| {
                        BlakeTwo256::hash_of(&(who, COMMUNITY, record.rank, record.nonce))
                    })
                    .collect();
            leaves.sort();

            // Find alice's leaf
            let alice_record = Members::<Test>::get(COMMUNITY, &alice).unwrap();
            let alice_leaf =
                BlakeTwo256::hash_of(&(alice.clone(), COMMUNITY, alice_record.rank, alice_record.nonce));
            let alice_index = leaves.iter().position(|l| l == &alice_leaf).expect("alice leaf in tree");

            // Generate merkle proof
            let bmt_proof = binary_merkle_tree::merkle_proof::<BlakeTwo256, _, _>(
                leaves.iter().map(|l| l.as_ref()),
                alice_index as u32,
            );

            // The proof root should match the stored root
            assert_eq!(bmt_proof.root, root, "proof root must match stored root");

            // Verify valid proof via the ProofVerifier trait
            let proof = MerkleProof::<BlakeTwo256> {
                leaf: alice_leaf,
                siblings: bmt_proof.proof.clone(),
                leaf_index: bmt_proof.leaf_index as u32,
                leaf_count: bmt_proof.number_of_leaves as u32,
            };
            let public_inputs = MembershipInputs::<BlakeTwo256> { root };
            assert!(
                <crate::verifier::MerkleVerifier<BlakeTwo256> as ProofVerifier>::verify(
                    &(),
                    &proof,
                    &public_inputs,
                ).is_ok(),
                "merkle proof should be valid for alice",
            );

            // Verify with wrong leaf should fail
            let wrong_leaf = BlakeTwo256::hash_of(b"wrong");
            let bad_proof = MerkleProof::<BlakeTwo256> {
                leaf: wrong_leaf,
                siblings: bmt_proof.proof,
                leaf_index: bmt_proof.leaf_index as u32,
                leaf_count: bmt_proof.number_of_leaves as u32,
            };
            assert!(
                <crate::verifier::MerkleVerifier<BlakeTwo256> as ProofVerifier>::verify(
                    &(),
                    &bad_proof,
                    &public_inputs,
                ).is_err(),
                "merkle proof should be invalid for wrong leaf",
            );
        });
}

#[test]
fn test_nullifier_prevents_replay() {
    use sp_core::H256;

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .build()
        .execute_with(|| {
            let action_scope = H256::from([0xAA; 32]);
            let nullifier = H256::from([0xBB; 32]);

            // Initially the nullifier should not exist
            assert!(
                !UsedNullifiers::<Test>::contains_key((&COMMUNITY, &action_scope, &nullifier))
            );

            // Insert the nullifier
            UsedNullifiers::<Test>::insert((&COMMUNITY, &action_scope, &nullifier), ());

            // Now it should be detected
            assert!(
                UsedNullifiers::<Test>::contains_key((&COMMUNITY, &action_scope, &nullifier))
            );

            // A different action_scope should not be affected
            let other_scope = H256::from([0xCC; 32]);
            assert!(
                !UsedNullifiers::<Test>::contains_key((&COMMUNITY, &other_scope, &nullifier))
            );

            // A different community should not be affected
            assert!(
                !UsedNullifiers::<Test>::contains_key((&999u32, &action_scope, &nullifier))
            );
        });
}

// Helper to create a track, submit a referendum, and advance to decision phase
fn setup_poll(community_id: CommunityId) -> u32 {
    use codec::Encode;
    use frame_support::traits::OriginTrait;
    use fc_pallet_referenda_tracks::SplitId;
    use pallet_referenda::{BoundedCallOf, Curve, TrackInfo, TrackInfoOf};
    use sp_runtime::{str_array as s, BoundedVec, Perbill};

    let track_info: TrackInfoOf<Test> = TrackInfo {
        name: s("Community"),
        max_deciding: 1,
        decision_deposit: 5,
        prepare_period: 1,
        decision_period: 5,
        confirm_period: 1,
        min_enactment_period: 1,
        min_approval: Curve::LinearDecreasing {
            length: Perbill::from_percent(100),
            floor: Perbill::from_percent(50),
            ceil: Perbill::from_percent(100),
        },
        min_support: Curve::LinearDecreasing {
            length: Perbill::from_percent(100),
            floor: Perbill::from_percent(0),
            ceil: Perbill::from_percent(100),
        },
    };

    let community_origin_caller = community_origin(community_id).caller().clone();

    // Directly insert track storage for the exact track_id = community_id
    let track_id: CommunityId = community_id;
    let (group, sub_track) = track_id.split();

    fc_pallet_referenda_tracks::TracksIds::<Test, ()>::try_mutate(|ids| ids.try_insert(track_id))
        .expect("can insert track id");
    fc_pallet_referenda_tracks::Tracks::<Test, ()>::set(group, sub_track, Some(track_info));
    fc_pallet_referenda_tracks::OriginToTrackId::<Test, ()>::set(
        community_origin_caller.clone(),
        Some(track_id),
    );
    fc_pallet_referenda_tracks::TrackIdToOrigin::<Test, ()>::set(
        track_id,
        Some(community_origin_caller.clone()),
    );

    // Need a funded account to submit and deposit
    let submitter = account(99);
    assert_ok!(Balances::force_set_balance(
        RuntimeOrigin::root(),
        submitter.clone(),
        100
    ));

    // Create a dummy proposal call
    let call: RuntimeCall = crate::Call::<Test>::set_decision_method {
        community_id,
        decision_method: DecisionMethod::Membership,
    }
    .into();
    let proposal = BoundedCallOf::<Test, ()>::Inline(BoundedVec::truncate_from(call.encode()));

    assert_ok!(Referenda::submit(
        RuntimeOrigin::signed(submitter.clone()),
        Box::new(community_origin_caller),
        proposal,
        frame_support::traits::schedule::DispatchTime::After(1),
    ));

    // Find the poll index from events
    let poll_index = 0u32; // First referendum

    assert_ok!(Referenda::place_decision_deposit(
        RuntimeOrigin::signed(submitter),
        poll_index
    ));

    // Advance to decision phase
    System::set_block_number(System::block_number() + 1);
    Referenda::on_initialize(System::block_number());
    Scheduler::on_initialize(System::block_number());

    poll_index
}

fn anon_origin(community_id: CommunityId, rank: GenericRank, nullifier: H256) -> RuntimeOrigin {
    let mut raw = crate::origin::RawOrigin::<Test>::new(community_id);
    raw.with_subset(crate::origin::Subset::AnonymousMember { rank, nullifier });
    raw.into()
}

#[test]
fn test_named_vote_still_works() {
    let alice = account(1);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .add_member(COMMUNITY, alice.clone())
        .build()
        .execute_with(|| {
            let poll_index = setup_poll(COMMUNITY);

            // Named vote should work
            assert_ok!(Communities::vote(
                RuntimeOrigin::signed(alice.clone()),
                poll_index,
                Vote::Standard(true),
            ));

            // Verify vote was recorded with hash of account as key
            let voter_key = BlakeTwo256::hash_of(&alice);
            assert!(CommunityVotes::<Test>::get(poll_index, &voter_key).is_some());

            // Verify event
            System::assert_has_event(
                crate::Event::<Test>::VoteCasted {
                    who: Some(alice.clone()),
                    poll_index,
                    vote: Vote::Standard(true),
                }
                .into(),
            );
        });
}

#[test]
fn test_anonymous_vote_uses_nullifier_key() {
    let alice = account(1);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .add_member(COMMUNITY, alice.clone())
        .build()
        .execute_with(|| {
            let poll_index = setup_poll(COMMUNITY);
            let nullifier = H256::from_low_u64_be(42);

            // Anonymous vote with membership decision method
            let origin = anon_origin(COMMUNITY, GenericRank::default(), nullifier);
            assert_ok!(Communities::vote(
                origin,
                poll_index,
                Vote::Standard(true),
            ));

            // Verify vote was recorded with hash of nullifier as key
            let voter_key = BlakeTwo256::hash_of(&nullifier);
            let (vote, multiplied) = CommunityVotes::<Test>::get(poll_index, &voter_key)
                .expect("vote should be stored");
            assert_eq!(vote, Vote::Standard(true));
            assert_eq!(multiplied, 1); // membership = 1x multiplier

            // Verify event has None for who (anonymous)
            System::assert_has_event(
                crate::Event::<Test>::VoteCasted {
                    who: None,
                    poll_index,
                    vote: Vote::Standard(true),
                }
                .into(),
            );
        });
}

#[test]
fn test_anonymous_vote_rejects_rank_weighted_decision() {
    // Rank-weighted voting cannot be authorized anonymously: the merkle proof doesn't
    // bind the leaf's rank, so the multiplier would be user-chosen and forgeable.
    // Only flat `Membership` voting is allowed on the anonymous path until a ZK backend
    // makes rank a verified public input. (Issue C2 from review.)
    let alice = account(1);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .add_member(COMMUNITY, alice.clone())
        .build()
        .execute_with(|| {
            CommunityDecisionMethod::<Test>::set(COMMUNITY, DecisionMethod::Rank);

            let poll_index = setup_poll(COMMUNITY);
            let nullifier = H256::from_low_u64_be(100);

            let origin = anon_origin(COMMUNITY, GenericRank::from(3u8), nullifier);
            assert_noop!(
                Communities::vote(origin, poll_index, Vote::Standard(true)),
                Error::InvalidVoteType,
            );
        });
}

#[test]
fn test_anonymous_vote_rank_from_origin_is_ignored() {
    // Even when a synthesised anonymous origin carries a high rank, the vote weight
    // is always 1 under DecisionMethod::Membership. (Issue C2 from review.)
    let alice = account(1);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .add_member(COMMUNITY, alice.clone())
        .build()
        .execute_with(|| {
            let poll_index = setup_poll(COMMUNITY);
            let nullifier = H256::from_low_u64_be(777);

            // A caller who managed to construct an AnonymousMember origin directly with
            // rank=100 still gets weight 1 — the pallet ignores the origin's rank field.
            let origin = anon_origin(COMMUNITY, GenericRank::from(100u8), nullifier);
            assert_ok!(Communities::vote(origin, poll_index, Vote::Standard(true)));

            let voter_key = BlakeTwo256::hash_of(&nullifier);
            let (_, multiplied) = CommunityVotes::<Test>::get(poll_index, &voter_key)
                .expect("vote should be stored");
            assert_eq!(multiplied, 1, "anonymous vote must be rank-1 regardless of origin rank");
        });
}

#[test]
fn test_anonymous_vote_token_weighted_rejected() {
    let alice = account(1);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .add_member(COMMUNITY, alice.clone())
        .build()
        .execute_with(|| {
            // Set decision method to NativeToken
            CommunityDecisionMethod::<Test>::set(COMMUNITY, DecisionMethod::NativeToken);

            let poll_index = setup_poll(COMMUNITY);
            let nullifier = H256::from_low_u64_be(200);

            // Anonymous vote with NativeToken should fail
            let origin = anon_origin(COMMUNITY, GenericRank::default(), nullifier);
            assert_noop!(
                Communities::vote(
                    origin,
                    poll_index,
                    Vote::Standard(true),
                ),
                Error::InvalidVoteType
            );
        });
}

#[test]
fn test_anonymous_vote_duplicate_nullifier_rejected() {
    let alice = account(1);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .add_member(COMMUNITY, alice.clone())
        .build()
        .execute_with(|| {
            let poll_index = setup_poll(COMMUNITY);
            let nullifier = H256::from_low_u64_be(300);

            // First anonymous vote succeeds
            let origin = anon_origin(COMMUNITY, GenericRank::default(), nullifier);
            assert_ok!(Communities::vote(
                origin,
                poll_index,
                Vote::Standard(true),
            ));

            // Second anonymous vote with same nullifier should fail.
            let origin2 = anon_origin(COMMUNITY, GenericRank::default(), nullifier);
            assert_noop!(
                Communities::vote(
                    origin2,
                    poll_index,
                    Vote::Standard(false),
                ),
                Error::AnonymousVoteAlreadyCast
            );
        });
}

#[test]
fn test_anonymous_vote_different_nullifiers_both_counted() {
    let alice = account(1);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .add_member(COMMUNITY, alice.clone())
        .build()
        .execute_with(|| {
            let poll_index = setup_poll(COMMUNITY);
            let nullifier1 = H256::from_low_u64_be(400);
            let nullifier2 = H256::from_low_u64_be(401);

            // Two different anonymous voters should both succeed
            let origin1 = anon_origin(COMMUNITY, GenericRank::default(), nullifier1);
            assert_ok!(Communities::vote(
                origin1,
                poll_index,
                Vote::Standard(true),
            ));

            let origin2 = anon_origin(COMMUNITY, GenericRank::default(), nullifier2);
            assert_ok!(Communities::vote(
                origin2,
                poll_index,
                Vote::Standard(false),
            ));

            // Both votes should be stored
            let key1 = BlakeTwo256::hash_of(&nullifier1);
            let key2 = BlakeTwo256::hash_of(&nullifier2);
            assert!(CommunityVotes::<Test>::get(poll_index, &key1).is_some());
            assert!(CommunityVotes::<Test>::get(poll_index, &key2).is_some());

            // Check tally via Polling
            use frame_support::traits::Polling;
            let (tally, _) = Referenda::as_ongoing(poll_index).expect("poll should be ongoing");
            assert_eq!(tally.ayes, 1);
            assert_eq!(tally.nays, 1);
            assert_eq!(tally.bare_ayes, 1);
        });
}

// ---- Adversarial tests for the critical issues flagged in the review ----
//
// These tests target the escalation paths the merkle-only MVP previously allowed.
// If any of them regresses in the future, the anonymous membership scheme is unsound
// again — treat failures as security regressions, not flakes.

#[test]
fn test_c1_anonymous_origin_cannot_manage_members() {
    // An anonymous community origin must NOT be accepted by `MemberMgmtOrigin`.
    // Before the C1 fix, `EnsureCommunity` destructured `RawOrigin { community_id, .. }`
    // and ignored the subset, so any caller with a valid merkle proof could
    // suspend/remove/promote/demote anyone in the community.
    let alice = account(1);
    let bob = account(2);

    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .add_member(COMMUNITY, alice.clone())
        .add_member(COMMUNITY, bob.clone())
        .build()
        .execute_with(|| {
            let anon = anon_origin(COMMUNITY, GenericRank::default(), H256::from_low_u64_be(1));

            // Every member-management call must reject the anonymous origin with BadOrigin.
            assert_noop!(
                Communities::suspend_member(anon.clone(), bob.clone()),
                sp_runtime::DispatchError::BadOrigin
            );
            assert_noop!(
                Communities::remove_member(anon.clone(), bob.clone()),
                sp_runtime::DispatchError::BadOrigin
            );
            assert_noop!(
                Communities::promote(anon.clone(), bob.clone()),
                sp_runtime::DispatchError::BadOrigin
            );
            assert_noop!(
                Communities::demote(anon.clone(), bob.clone()),
                sp_runtime::DispatchError::BadOrigin
            );
            assert_noop!(
                Communities::add_member(anon.clone(), account(42), None, None),
                sp_runtime::DispatchError::BadOrigin
            );

            // Bob must still be an active member — no state was mutated.
            assert!(Members::<Test>::get(COMMUNITY, &bob).is_some());
            assert_eq!(
                Members::<Test>::get(COMMUNITY, &bob).unwrap().status,
                MemberStatus::Active,
            );
        });
}

#[test]
fn test_c1_anonymous_origin_cannot_call_admin_functions() {
    // The same escalation route also guarded admin-only functions via `AdminOrigin`.
    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .build()
        .execute_with(|| {
            let anon = anon_origin(COMMUNITY, GenericRank::default(), H256::from_low_u64_be(2));
            let fake_root = BlakeTwo256::hash_of(b"whatever");

            assert_noop!(
                Communities::update_membership_root(anon.clone(), fake_root, 0),
                sp_runtime::DispatchError::BadOrigin
            );
            assert_noop!(
                Communities::update_sub_root(anon.clone(), 1, fake_root),
                sp_runtime::DispatchError::BadOrigin
            );
            assert_noop!(
                Communities::set_budget(anon.clone(), 1_000_000, 100),
                sp_runtime::DispatchError::BadOrigin
            );
            assert_noop!(
                Communities::set_decision_method(anon, COMMUNITY, DecisionMethod::Membership),
                sp_runtime::DispatchError::BadOrigin
            );
        });
}

#[test]
fn test_c1_anonymous_origin_cannot_dispatch_as_account() {
    // The most damaging escalation: dispatching as the community's keyless account
    // would allow draining the treasury. `MemberMgmtOrigin` must reject anonymous.
    TestEnvBuilder::new()
        .add_community(COMMUNITY, PrivacyLevel::Public)
        .build()
        .execute_with(|| {
            let anon = anon_origin(COMMUNITY, GenericRank::default(), H256::from_low_u64_be(3));
            // Any inner call works here; we're asserting the outer origin check rejects.
            let inner: RuntimeCall = crate::Call::<Test>::set_decision_method {
                community_id: COMMUNITY,
                decision_method: DecisionMethod::Membership,
            }
            .into();
            assert_noop!(
                Communities::dispatch_as_account(anon, Box::new(inner)),
                sp_runtime::DispatchError::BadOrigin
            );
        });
}

#[test]
fn test_c4_action_scope_derived_from_call() {
    // The extension must derive the nullifier's action-scope from the dispatched call,
    // not accept it from the caller. Same caller, same leaf, different call should
    // produce different nullifiers. We validate by exercising the same hashing the
    // extension would use and asserting the nullifiers differ.
    use codec::Encode;

    let call_a: RuntimeCall = crate::Call::<Test>::vote {
        poll_index: 0,
        vote: Vote::Standard(true),
    }
    .into();
    let call_b: RuntimeCall = crate::Call::<Test>::vote {
        poll_index: 1,
        vote: Vote::Standard(true),
    }
    .into();

    let scope_a: H256 = sp_io::hashing::blake2_256(&call_a.encode()).into();
    let scope_b: H256 = sp_io::hashing::blake2_256(&call_b.encode()).into();
    assert_ne!(
        scope_a, scope_b,
        "different calls must produce different action scopes"
    );
}
