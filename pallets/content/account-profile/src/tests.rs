//! Tests for the account-profile pallet.

use super::{Error, Event};
use crate::mock::*;
use fc_pallet_content::{
    pallet::{IpfsHash, ItemId},
    Nonce, Pallet as Content, RETRACTABLE, REVISIONABLE,
};
use frame_support::{assert_noop, assert_ok};

fn publish_item(account: u64, flags: u8) -> ItemId {
    let item_id = Content::<Test>::get_item_id(account, Nonce::default());
    Content::<Test>::publish_item(
        RuntimeOrigin::signed(account),
        Nonce::default(),
        vec![],
        flags,
        vec![],
        IpfsHash::default(),
    )
    .expect("publish_item must succeed");
    item_id
}

// ── set_profile ──────────────────────────────────────────────────────────────

#[test]
fn set_profile_works() {
    new_test_ext().execute_with(|| {
        let item_id = publish_item(1, REVISIONABLE | RETRACTABLE);

        assert_ok!(AccountProfile::set_profile(
            RuntimeOrigin::signed(1),
            item_id.clone(),
        ));

        System::assert_has_event(
            Event::<Test>::ProfileSet {
                account: 1,
                item_id: item_id.clone(),
            }
            .into(),
        );

        assert_eq!(AccountProfile::profile_of(1), Some(item_id));
    });
}

#[test]
fn set_profile_updates_existing_profile() {
    new_test_ext().execute_with(|| {
        let item_id = publish_item(1, REVISIONABLE | RETRACTABLE);
        assert_ok!(AccountProfile::set_profile(
            RuntimeOrigin::signed(1),
            item_id.clone(),
        ));

        // Publish a second item with a different nonce for the same account.
        let nonce2 = Nonce([1u8; 32]);
        let item_id2 = Content::<Test>::get_item_id(1, nonce2.clone());
        Content::<Test>::publish_item(
            RuntimeOrigin::signed(1),
            nonce2,
            vec![],
            REVISIONABLE | RETRACTABLE,
            vec![],
            IpfsHash::default(),
        )
        .unwrap();

        assert_ok!(AccountProfile::set_profile(
            RuntimeOrigin::signed(1),
            item_id2.clone(),
        ));

        assert_eq!(AccountProfile::profile_of(1), Some(item_id2));
    });
}

#[test]
fn set_profile_fails_item_not_found() {
    new_test_ext().execute_with(|| {
        // Use an item_id that was never published.
        let item_id = Content::<Test>::get_item_id(1, Nonce::default());

        assert_noop!(
            AccountProfile::set_profile(RuntimeOrigin::signed(1), item_id),
            Error::<Test>::ItemNotFound
        );
    });
}

#[test]
fn set_profile_fails_wrong_owner() {
    new_test_ext().execute_with(|| {
        // Item published by account 1.
        let item_id = publish_item(1, REVISIONABLE | RETRACTABLE);

        // Account 2 tries to use it as their profile.
        assert_noop!(
            AccountProfile::set_profile(RuntimeOrigin::signed(2), item_id),
            Error::<Test>::NotItemOwner
        );
    });
}

#[test]
fn set_profile_fails_item_retracted() {
    new_test_ext().execute_with(|| {
        let item_id = publish_item(1, REVISIONABLE | RETRACTABLE);

        assert_ok!(Content::<Test>::retract_item(
            RuntimeOrigin::signed(1),
            item_id.clone(),
        ));

        assert_noop!(
            AccountProfile::set_profile(RuntimeOrigin::signed(1), item_id),
            Error::<Test>::ItemRetracted
        );
    });
}

#[test]
fn set_profile_fails_item_not_revisionable() {
    new_test_ext().execute_with(|| {
        // Publish with only RETRACTABLE – not revisionable.
        let item_id = publish_item(1, RETRACTABLE);

        assert_noop!(
            AccountProfile::set_profile(RuntimeOrigin::signed(1), item_id),
            Error::<Test>::ItemNotRevisionable
        );
    });
}

#[test]
fn set_profile_fails_after_set_not_revisionable() {
    new_test_ext().execute_with(|| {
        let item_id = publish_item(1, REVISIONABLE | RETRACTABLE);

        assert_ok!(Content::<Test>::set_not_revisionable(
            RuntimeOrigin::signed(1),
            item_id.clone(),
        ));

        assert_noop!(
            AccountProfile::set_profile(RuntimeOrigin::signed(1), item_id),
            Error::<Test>::ItemNotRevisionable
        );
    });
}

// ── get_profile ───────────────────────────────────────────────────────────────

#[test]
fn get_profile_returns_none_when_unset() {
    new_test_ext().execute_with(|| {
        assert_eq!(AccountProfile::get_profile(&1), None);
    });
}

#[test]
fn get_profile_returns_item_id_when_set() {
    new_test_ext().execute_with(|| {
        let item_id = publish_item(1, REVISIONABLE | RETRACTABLE);

        assert_ok!(AccountProfile::set_profile(
            RuntimeOrigin::signed(1),
            item_id.clone(),
        ));

        assert_eq!(AccountProfile::get_profile(&1), Some(item_id));
    });
}
