//! Tests for pass pallet.
use super::{Account, AccountStatus, Accounts, Error, Event};
use crate::mock::*;
use frame_support::{assert_noop, assert_ok, parameter_types, traits::Randomness, BoundedVec};
use sp_core::ConstU32;

const SIGNER: AccountId = AccountId::new([0u8; 32]);

parameter_types! {
    pub AccountName: BoundedVec<u8, ConstU32<64>> =
        BoundedVec::truncate_from((*b"@account:example.org").to_vec());
}

mod register {
    use super::*;

    #[test]
    fn fails_if_already_registered() {
        new_test_ext().execute_with(|| {
            Accounts::<Test>::insert(
                AccountName::get(),
                Account::new(AccountId::new([0u8; 32]), crate::AccountStatus::Active),
            );

            assert_noop!(
                Pass::register(
                    RuntimeOrigin::signed(SIGNER),
                    AccountName::get(),
                    MockAuthenticators::DummyAuthenticator,
                    BoundedVec::new(),
                    (*b"challeng").to_vec()
                ),
                Error::<Test>::AlreadyRegistered
            );
        });
    }

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
            let challenge_response = RandomessFromBlockNumber::random_seed()
                .0
                .as_bytes()
                .to_vec();

            System::set_block_number(2);

            assert_noop!(
                Pass::register(
                    RuntimeOrigin::signed(SIGNER),
                    AccountName::get(),
                    MockAuthenticators::DummyAuthenticator,
                    BoundedVec::new(),
                    challenge_response,
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
                RandomessFromBlockNumber::random_seed()
                    .0
                    .as_bytes()
                    .to_vec()
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

    #[test]
    fn unreserving_if_uninitialized_works() {
        env_logger::init();
        // Test for uninitialized account that unreserves if not activated after timeout
        new_test_ext().execute_with(|| {
            assert_ok!(Pass::register(
                RuntimeOrigin::signed(SIGNER),
                AccountName::get(),
                MockAuthenticators::DummyAuthenticator,
                BoundedVec::new(),
                RandomessFromBlockNumber::random_seed()
                    .0
                    .as_bytes()
                    .to_vec()
            ));

            assert_eq!(
                Accounts::<Test>::get(AccountName::get()),
                Some(Account::new(
                    Pass::account_id_for(&AccountName::get()),
                    AccountStatus::Uninitialized
                ))
            );

            run_to(12);
            assert_eq!(Accounts::<Test>::get(AccountName::get()), None);
        });

        // Test for uninitialized account that is initialized before timeout
        new_test_ext().execute_with(|| {
            assert_ok!(Pass::register(
                RuntimeOrigin::signed(SIGNER),
                AccountName::get(),
                MockAuthenticators::DummyAuthenticator,
                BoundedVec::new(),
                RandomessFromBlockNumber::random_seed()
                    .0
                    .as_bytes()
                    .to_vec()
            ));

            let account_id = Pass::account_id_for(&AccountName::get());

            assert_eq!(
                Accounts::<Test>::get(AccountName::get()),
                Some(Account::new(
                    account_id.clone(),
                    AccountStatus::Uninitialized
                ))
            );

            run_to(11);
            System::inc_providers(&account_id);

            run_to(12);
            assert_eq!(
                Accounts::<Test>::get(AccountName::get()),
                Some(Account::new(account_id.clone(), AccountStatus::Active))
            );
        });
    }
}

mod claim {
    use super::*;

    #[test]
    fn claim_works_with_dummy_registrar() {
        new_test_ext().execute_with(|| {
            Pass::claim(
                RuntimeOrigin::signed(SIGNER),
                AccountName::get(),
                MockAuthenticators::DummyAuthenticator,
                BoundedVec::new(),
                RandomessFromBlockNumber::random_seed()
                    .0
                    .as_bytes()
                    .to_vec()
            );
            
            System::assert_has_event(Event::<Test>::Claimed {
                account_name: AccountName::get(),
            }.into());
        });
    }
}