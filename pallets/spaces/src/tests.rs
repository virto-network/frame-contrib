//! Tests for the Spaces pallet, with the mock verifier.

use crate::{
    mock::*, AnchorNumber, AnchorRecord, Anchors, EpochCause, Epochs, Error, Event, Head, Heads,
    MockVerifier, Pallet as SpacesPallet, ProgramId, Root, SpaceBinds, Spaces as SpacesStorage,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};

const OWNER: AccountId = 1;
const RELAYER: AccountId = 2;
const SPACE: u32 = 10;
const GENESIS: Root = [0u8; 32];

fn root(n: u8) -> Root {
    [n; 32]
}

fn register() {
    assert_ok!(Spaces::register(
        RuntimeOrigin::signed(OWNER),
        SPACE,
        PROGRAM,
        GENESIS
    ));
}

fn head() -> Head<u64> {
    Heads::<Test>::get(SPACE).expect("registered")
}

/// A proof for anchoring `to` on the Space's current head, made by `program`.
fn prove_with(program: &ProgramId, to: Root, public: &[u8]) -> Vec<u8> {
    let statement = SpacesPallet::<Test>::anchor_statement(SPACE, &head(), to);
    MockVerifier::prove(program, public, &statement.encode())
}

fn prove(to: Root, public: &[u8]) -> Vec<u8> {
    prove_with(&PROGRAM, to, public)
}

fn anchor(
    number: AnchorNumber,
    to: Root,
    proof: Vec<u8>,
    public: &[u8],
) -> frame_support::dispatch::DispatchResultWithPostInfo {
    Spaces::anchor(
        RuntimeOrigin::signed(RELAYER),
        SPACE,
        number,
        to,
        BoundedVec::truncate_from(proof),
        BoundedVec::truncate_from(public.to_vec()),
    )
}

/// Anchor the next number to `to` with a valid proof.
fn anchor_next(to: Root) {
    let number = head().number + 1;
    assert_ok!(anchor(number, to, prove(to, b"input"), b"input"));
}

mod register {
    use super::*;

    #[test]
    fn opens_epoch_zero_at_genesis() {
        new_test_ext().execute_with(|| {
            register();
            let info = SpacesStorage::<Test>::get(SPACE).unwrap();
            assert_eq!((info.owner, info.program, info.binds), (OWNER, PROGRAM, 0));
            assert_eq!(
                head(),
                Head {
                    epoch: 0,
                    number: 0,
                    root: GENESIS,
                    at: 1
                }
            );
            let epoch = Epochs::<Test>::get(SPACE, 0).unwrap();
            assert_eq!(
                (epoch.root, epoch.base, epoch.cause),
                (GENESIS, 0, EpochCause::Registered)
            );
            System::assert_last_event(
                Event::<Test>::Registered {
                    space: SPACE,
                    owner: OWNER,
                    program: PROGRAM,
                    genesis: GENESIS,
                }
                .into(),
            );
        });
    }

    #[test]
    fn refuses_an_existing_space() {
        new_test_ext().execute_with(|| {
            register();
            assert_noop!(
                Spaces::register(RuntimeOrigin::signed(3), SPACE, OTHER_PROGRAM, root(9)),
                Error::<Test>::SpaceExists
            );
        });
    }

    #[test]
    fn needs_the_register_origin() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Spaces::register(RuntimeOrigin::none(), SPACE, PROGRAM, GENESIS),
                sp_runtime::DispatchError::BadOrigin
            );
        });
    }
}

mod anchor {
    use super::*;

    #[test]
    fn a_valid_proof_moves_the_head() {
        new_test_ext().execute_with(|| {
            register();
            anchor_next(root(1));
            assert_eq!(
                head(),
                Head {
                    epoch: 0,
                    number: 1,
                    root: root(1),
                    at: 1
                }
            );
            assert_eq!(
                Anchors::<Test>::get(SPACE, 1),
                Some(AnchorRecord {
                    root: root(1),
                    epoch: 0,
                    at: 1
                })
            );
            System::assert_last_event(
                Event::<Test>::Anchored {
                    space: SPACE,
                    epoch: 0,
                    number: 1,
                    root: root(1),
                    by: RELAYER,
                }
                .into(),
            );
            anchor_next(root(2));
            assert_eq!(head().number, 2);
            assert_eq!(head().root, root(2));
        });
    }

    #[test]
    fn an_unknown_space_is_refused() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                anchor(1, root(1), vec![0; 96], b""),
                Error::<Test>::UnknownSpace
            );
        });
    }

    #[test]
    fn a_tampered_proof_is_invalid() {
        new_test_ext().execute_with(|| {
            register();
            let mut proof = prove(root(1), b"input");
            proof[70] ^= 1;
            assert_noop!(
                anchor(1, root(1), proof, b"input"),
                Error::<Test>::InvalidProof
            );
        });
    }

    #[test]
    fn undecodable_bytes_are_malformed() {
        new_test_ext().execute_with(|| {
            register();
            assert_noop!(
                anchor(1, root(1), vec![1, 2, 3], b"input"),
                Error::<Test>::MalformedProof
            );
        });
    }

    #[test]
    fn a_proof_of_another_program_is_refused() {
        new_test_ext().execute_with(|| {
            register();
            let proof = prove_with(&OTHER_PROGRAM, root(1), b"input");
            assert_noop!(
                anchor(1, root(1), proof, b"input"),
                Error::<Test>::WrongProgram
            );
        });
    }

    #[test]
    fn a_proof_must_bind_the_new_root() {
        new_test_ext().execute_with(|| {
            register();
            // Proved for root 1, submitted for root 2.
            let proof = prove(root(1), b"input");
            assert_noop!(anchor(1, root(2), proof, b"input"), Error::<Test>::NotBound);
        });
    }

    #[test]
    fn a_proof_must_bind_the_public_input() {
        new_test_ext().execute_with(|| {
            register();
            let proof = prove(root(1), b"input");
            assert_noop!(
                anchor(1, root(1), proof, b"other input"),
                Error::<Test>::NotBound
            );
        });
    }

    #[test]
    fn a_proof_must_start_from_the_head() {
        new_test_ext().execute_with(|| {
            register();
            // A transition proved from another root than the head's.
            let statement = SpacesPallet::<Test>::anchor_statement(
                SPACE,
                &Head {
                    root: root(9),
                    ..head()
                },
                root(1),
            );
            let proof = MockVerifier::prove(&PROGRAM, b"input", &statement.encode());
            assert_noop!(anchor(1, root(1), proof, b"input"), Error::<Test>::NotBound);
        });
    }

    #[test]
    fn a_proof_for_another_space_is_refused() {
        new_test_ext().execute_with(|| {
            register();
            assert_ok!(Spaces::register(
                RuntimeOrigin::signed(OWNER),
                SPACE + 1,
                PROGRAM,
                GENESIS
            ));
            // Same program, same genesis, same number: only the Space differs.
            let statement = SpacesPallet::<Test>::anchor_statement(SPACE + 1, &head(), root(1));
            let proof = MockVerifier::prove(&PROGRAM, b"input", &statement.encode());
            assert_noop!(anchor(1, root(1), proof, b"input"), Error::<Test>::NotBound);
        });
    }

    #[test]
    fn numbers_must_follow_the_head() {
        new_test_ext().execute_with(|| {
            register();
            let proof = prove(root(2), b"input");
            assert_noop!(
                anchor(2, root(2), proof, b"input"),
                Error::<Test>::AnchorOutOfOrder
            );
            assert_noop!(
                anchor(0, root(2), prove(root(2), b"input"), b"input"),
                Error::<Test>::AnchorOutOfOrder
            );
        });
    }

    #[test]
    fn resubmitting_a_stored_anchor_is_a_no_op() {
        new_test_ext().execute_with(|| {
            register();
            let proof = prove(root(1), b"input");
            assert_ok!(anchor(1, root(1), proof.clone(), b"input"));
            let events = System::events().len();
            let before = head();
            let post = anchor(1, root(1), proof, b"input").unwrap();
            assert_eq!(
                post.actual_weight,
                Some(<() as crate::WeightInfo>::anchor_replay())
            );
            assert_eq!(head(), before);
            assert_eq!(System::events().len(), events);
        });
    }

    #[test]
    fn a_stored_number_with_another_root_conflicts() {
        new_test_ext().execute_with(|| {
            register();
            anchor_next(root(1));
            assert_noop!(
                anchor(1, root(2), vec![], b""),
                Error::<Test>::AnchorConflict
            );
        });
    }
}

mod refound {
    use super::*;

    #[test]
    fn continues_the_sequence_from_a_new_genesis() {
        new_test_ext().execute_with(|| {
            register();
            anchor_next(root(1));
            anchor_next(root(2));

            assert_ok!(Spaces::refound(
                RuntimeOrigin::signed(OWNER),
                SPACE,
                root(100)
            ));
            System::assert_last_event(
                Event::<Test>::Refounded {
                    space: SPACE,
                    epoch: 1,
                    base: 2,
                    genesis: root(100),
                }
                .into(),
            );
            assert_eq!(
                head(),
                Head {
                    epoch: 1,
                    number: 2,
                    root: root(100),
                    at: 1
                }
            );
            let epoch = Epochs::<Test>::get(SPACE, 1).unwrap();
            assert_eq!(
                (epoch.root, epoch.base, epoch.cause),
                (root(100), 2, EpochCause::Refounded)
            );

            // The new run's first anchor is number 3, proved from the new genesis.
            anchor_next(root(101));
            assert_eq!(head().number, 3);
            assert_eq!(Anchors::<Test>::get(SPACE, 3).unwrap().epoch, 1);
            // The earlier run's anchors stay as they were.
            assert_eq!(
                Anchors::<Test>::get(SPACE, 2),
                Some(AnchorRecord {
                    root: root(2),
                    epoch: 0,
                    at: 1
                })
            );
        });
    }

    #[test]
    fn only_by_the_owner() {
        new_test_ext().execute_with(|| {
            register();
            assert_noop!(
                Spaces::refound(RuntimeOrigin::signed(RELAYER), SPACE, root(100)),
                Error::<Test>::NotOwner
            );
            assert_noop!(
                Spaces::refound(RuntimeOrigin::signed(OWNER), SPACE + 1, root(100)),
                Error::<Test>::UnknownSpace
            );
        });
    }

    #[test]
    fn not_while_anything_is_bound() {
        new_test_ext().execute_with(|| {
            register();
            assert_ok!(<SpacesPallet<Test> as SpaceBinds<_, _>>::bind(&SPACE, &5));
            assert_noop!(
                Spaces::refound(RuntimeOrigin::signed(OWNER), SPACE, root(100)),
                Error::<Test>::HasBinds
            );
            assert_ok!(<SpacesPallet<Test> as SpaceBinds<_, _>>::unbind(&SPACE, &5));
            assert_ok!(Spaces::refound(
                RuntimeOrigin::signed(OWNER),
                SPACE,
                root(100)
            ));
        });
    }

    #[test]
    fn invalidates_proofs_made_against_the_old_head() {
        new_test_ext().execute_with(|| {
            register();
            let stale = prove(root(1), b"input");
            assert_ok!(Spaces::refound(
                RuntimeOrigin::signed(OWNER),
                SPACE,
                root(100)
            ));
            assert_noop!(anchor(1, root(1), stale, b"input"), Error::<Test>::NotBound);
        });
    }
}

mod set_current_head {
    use super::*;

    #[test]
    fn sets_the_head_and_keeps_the_anchors() {
        new_test_ext().execute_with(|| {
            register();
            anchor_next(root(1));
            assert_ok!(Spaces::set_current_head(
                RuntimeOrigin::root(),
                SPACE,
                root(50)
            ));
            System::assert_last_event(
                Event::<Test>::HeadSet {
                    space: SPACE,
                    epoch: 1,
                    base: 1,
                    previous: root(1),
                    root: root(50),
                    binds: 0,
                }
                .into(),
            );
            assert_eq!(
                head(),
                Head {
                    epoch: 1,
                    number: 1,
                    root: root(50),
                    at: 1
                }
            );
            assert_eq!(
                Epochs::<Test>::get(SPACE, 1).unwrap().cause,
                EpochCause::Reset
            );
            assert_eq!(Anchors::<Test>::get(SPACE, 1).unwrap().root, root(1));
            anchor_next(root(51));
            assert_eq!(head().number, 2);
        });
    }

    #[test]
    fn works_with_binds_and_keeps_them() {
        new_test_ext().execute_with(|| {
            register();
            anchor_next(root(1));
            assert_eq!(
                <SpacesPallet<Test> as SpaceBinds<_, _>>::bind(&SPACE, &5),
                Ok(1)
            );
            assert_ok!(Spaces::set_current_head(
                RuntimeOrigin::root(),
                SPACE,
                root(50)
            ));
            System::assert_last_event(
                Event::<Test>::HeadSet {
                    space: SPACE,
                    epoch: 1,
                    base: 1,
                    previous: root(1),
                    root: root(50),
                    binds: 1,
                }
                .into(),
            );
            assert!(<SpacesPallet<Test> as SpaceBinds<_, _>>::is_bound(
                &SPACE, &5
            ));
            assert_eq!(SpacesStorage::<Test>::get(SPACE).unwrap().binds, 1);
        });
    }

    #[test]
    fn invalidates_proofs_made_against_the_old_head() {
        new_test_ext().execute_with(|| {
            register();
            let stale = prove(root(1), b"input");
            // Even a reset to the same root opens a new epoch, which the statement names.
            assert_ok!(Spaces::set_current_head(
                RuntimeOrigin::root(),
                SPACE,
                GENESIS
            ));
            assert_noop!(anchor(1, root(1), stale, b"input"), Error::<Test>::NotBound);
        });
    }

    #[test]
    fn needs_the_reset_origin() {
        new_test_ext().execute_with(|| {
            register();
            assert_noop!(
                Spaces::set_current_head(RuntimeOrigin::signed(OWNER), SPACE, root(50)),
                sp_runtime::DispatchError::BadOrigin
            );
            assert_noop!(
                Spaces::set_current_head(RuntimeOrigin::root(), SPACE + 1, root(50)),
                Error::<Test>::UnknownSpace
            );
        });
    }
}

mod binds {
    use super::*;

    type Binds = SpacesPallet<Test>;

    #[test]
    fn bind_and_unbind() {
        new_test_ext().execute_with(|| {
            register();
            anchor_next(root(1));
            assert_eq!(<Binds as SpaceBinds<_, _>>::bind(&SPACE, &5), Ok(1));
            System::assert_last_event(
                Event::<Test>::Bound {
                    space: SPACE,
                    key: 5,
                    epoch: 0,
                    after_anchor: 1,
                }
                .into(),
            );
            assert_noop!(
                <Binds as SpaceBinds<_, _>>::bind(&SPACE, &5),
                Error::<Test>::AlreadyBound
            );
            assert_ok!(<Binds as SpaceBinds<_, _>>::bind(&SPACE, &6));
            assert_eq!(SpacesStorage::<Test>::get(SPACE).unwrap().binds, 2);
            assert_ok!(<Binds as SpaceBinds<_, _>>::unbind(&SPACE, &5));
            assert!(!<Binds as SpaceBinds<_, _>>::is_bound(&SPACE, &5));
            assert_eq!(SpacesStorage::<Test>::get(SPACE).unwrap().binds, 1);
            assert_noop!(
                <Binds as SpaceBinds<_, _>>::unbind(&SPACE, &5),
                Error::<Test>::UnknownBind
            );
            assert_noop!(
                <Binds as SpaceBinds<_, _>>::bind(&(SPACE + 1), &5),
                Error::<Test>::UnknownSpace
            );
        });
    }
}

mod statement {
    use super::*;

    #[test]
    fn names_the_chain_space_epoch_number_and_roots() {
        new_test_ext().execute_with(|| {
            register();
            anchor_next(root(1));
            let s = SpacesPallet::<Test>::anchor_statement(SPACE, &head(), root(2));
            assert_eq!(s.domain, *b"fc-spaces/anchor");
            assert_eq!(s.version, 1);
            assert_eq!(s.chain, System::block_hash(0).0);
            assert_eq!((s.space, s.epoch, s.number), (SPACE, 0, 2));
            assert_eq!((s.prev_root, s.root), (root(1), root(2)));
        });
    }
}
