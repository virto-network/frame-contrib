use crate::{
    mock::*, ByCell, CellRef, Disclosure, DisclosureGrants, Error, Event, Joined, Pitches,
    SettlementPolicy,
};
use frame::deps::frame_support::{assert_noop, assert_ok};
use frame::deps::sp_core::H256;
use sp_runtime::{BoundedVec, Permill};

fn claim_public() {
    assert_ok!(Pitch::claim(
        RuntimeOrigin::signed(HOLDER),
        CellRef::Raw(CELL),
        Some(7),
        1,
        10,
        Disclosure::Public,
        Permill::from_percent(2),
        None,
        SettlementPolicy::Holder,
        None,
        None,
    ));
}

#[test]
fn public_pitch_is_claimed_and_indexed_by_cell() {
    new_test_ext().execute_with(|| {
        claim_public();

        let pitch = Pitches::<Test>::get(0).unwrap();
        assert_eq!(pitch.holder, HOLDER);
        assert_eq!(pitch.cell, CellRef::Raw(CELL));
        assert_eq!(ByCell::<Test>::get(CELL).as_slice(), &[0]);

        System::assert_last_event(
            Event::Claimed {
                pitch: 0,
                disclosure: Disclosure::Public,
                holder: HOLDER,
            }
            .into(),
        );
    });
}

#[test]
fn committed_pitch_is_not_indexed_by_raw_cell() {
    new_test_ext().execute_with(|| {
        assert_ok!(Pitch::claim(
            RuntimeOrigin::signed(HOLDER),
            CellRef::Commitment(H256::repeat_byte(7)),
            None,
            1,
            10,
            Disclosure::Sealed,
            Permill::zero(),
            None,
            SettlementPolicy::Holder,
            None,
            Some(BoundedVec::try_from(b"sealed-residue".to_vec()).unwrap()),
        ));

        assert_eq!(ByCell::<Test>::get(CELL).len(), 0);
        assert_eq!(
            Pitches::<Test>::get(0).unwrap().cell,
            CellRef::Commitment(H256::repeat_byte(7))
        );
    });
}

#[test]
fn validates_disclosure_and_resolution() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Pitch::claim(
                RuntimeOrigin::signed(HOLDER),
                CellRef::Commitment(H256::repeat_byte(1)),
                None,
                1,
                10,
                Disclosure::Public,
                Permill::zero(),
                None,
                SettlementPolicy::Holder,
                None,
                None,
            ),
            Error::<Test>::BadCellRef,
        );

        assert_noop!(
            Pitch::claim(
                RuntimeOrigin::signed(HOLDER),
                CellRef::Raw(CELL),
                Some(16),
                1,
                10,
                Disclosure::Public,
                Permill::zero(),
                None,
                SettlementPolicy::Holder,
                None,
                None,
            ),
            Error::<Test>::BadResolution,
        );
    });
}

#[test]
fn gated_pitch_requires_valid_membership_proof() {
    new_test_ext().execute_with(|| {
        assert_ok!(Pitch::claim(
            RuntimeOrigin::signed(HOLDER),
            CellRef::Raw(CELL),
            Some(7),
            1,
            10,
            Disclosure::Gated,
            Permill::zero(),
            Some(ROOT),
            SettlementPolicy::Holder,
            None,
            None,
        ));

        assert_noop!(
            Pitch::join(
                RuntimeOrigin::signed(OUTSIDER),
                0,
                BoundedVec::try_from(b"member".to_vec()).unwrap(),
            ),
            Error::<Test>::ProofInvalid,
        );

        assert_ok!(Pitch::join(
            RuntimeOrigin::signed(MEMBER),
            0,
            BoundedVec::try_from(b"member".to_vec()).unwrap(),
        ));
        assert!(Joined::<Test>::contains_key(0, MEMBER));
    });
}

#[test]
fn holder_can_amend_and_grant_disclosure() {
    new_test_ext().execute_with(|| {
        claim_public();

        assert_ok!(Pitch::amend(
            RuntimeOrigin::signed(HOLDER),
            0,
            Some(Permill::from_percent(5)),
            Some(Some(ROOT)),
            Some(12),
            None,
        ));
        assert_eq!(
            Pitches::<Test>::get(0).unwrap().local_tax,
            Permill::from_percent(5)
        );
        assert_eq!(Pitches::<Test>::get(0).unwrap().end, 12);

        assert_ok!(Pitch::grant_disclosure(
            RuntimeOrigin::signed(HOLDER),
            0,
            MEMBER,
            crate::DisclosureScope::Location,
        ));
        assert_eq!(
            DisclosureGrants::<Test>::get(0, MEMBER),
            Some(crate::DisclosureScope::Location)
        );
    });
}

#[test]
fn dissolve_removes_pitch_and_public_index() {
    new_test_ext().execute_with(|| {
        claim_public();
        assert_ok!(Pitch::dissolve(RuntimeOrigin::signed(HOLDER), 0));

        assert!(Pitches::<Test>::get(0).is_none());
        assert_eq!(ByCell::<Test>::get(CELL).len(), 0);
    });
}

#[test]
fn reap_only_after_grace_period() {
    new_test_ext().execute_with(|| {
        claim_public();

        System::set_block_number(11);
        assert_noop!(
            Pitch::reap(RuntimeOrigin::signed(OUTSIDER), 0),
            Error::<Test>::NotLive,
        );

        System::set_block_number(12);
        assert_ok!(Pitch::reap(RuntimeOrigin::signed(OUTSIDER), 0));
        assert!(Pitches::<Test>::get(0).is_none());
    });
}

#[test]
fn derives_stable_pitch_account() {
    new_test_ext().execute_with(|| {
        assert_eq!(Pitch::pitch_account(0), Pitch::pitch_account(0));
    });
}
