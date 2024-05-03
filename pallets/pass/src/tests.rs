//! Tests for pass pallet.
use super::{Error, Event};
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use frame_support::{parameter_types, BoundedVec};
use sp_core::ConstU32;

const SIGNER: AccountId = AccountId::new([0u8; 32]);

parameter_types! {
    pub AccountName: BoundedVec<u8, ConstU32<64>> =
        BoundedVec::truncate_from((*b"@account:example.org").to_vec());
}

mod register {
    use super::*;

    #[test]
    fn fails_if_cannot_resolve_device() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Pass::register(
                    RuntimeOrigin::signed(SIGNER),
                    AccountName::get(),
                    MockAuthenticators::InvalidAuthenticator,
                    BoundedVec::new(),
                    (*b"challeng").to_vec()
                ),
                Error::<Test>::InvalidDeviceForAuthenticator
            );
        });
    }

    #[test]
    fn fails_if_cannot_fulfill_challenge() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Pass::register(
                    RuntimeOrigin::signed(SIGNER),
                    AccountName::get(),
                    MockAuthenticators::DummyAuthenticator,
                    BoundedVec::new(),
                    (*b"challeng").to_vec()
                ),
                Error::<Test>::ChallengeFailed
            );
        });
    }

    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            let account_id = Pass::account_id_for(&AccountName::get());

            assert_ok!(Pass::register(
                RuntimeOrigin::signed(SIGNER),
                AccountName::get(),
                MockAuthenticators::DummyAuthenticator,
                BoundedVec::new(),
                (*b"challenge").to_vec()
            ));

            System::assert_has_event(
                Event::<Test>::Registered {
                    account_name: AccountName::get(),
                    account_id,
                }
                .into(),
            );
            System::assert_has_event(
                Event::<Test>::AddedDevice {
                    account_name: AccountName::get(),
                    device_id: [1u8; 32],
                }
                .into(),
            );
        });
    }
}
