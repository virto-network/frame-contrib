//! Tests for pass pallet.
use super::{Account, AccountStatus, Accounts, Error, Event};
use crate::mock::*;
use codec::Encode;
use frame_support::{assert_noop, assert_ok, parameter_types, traits::Randomness, BoundedVec};
use sp_core::ConstU32;

const SIGNER: AccountId = AccountId::new([0u8; 32]);
const OTHER: AccountId = AccountId::new([1u8; 32]);

const THE_DEVICE: fc_traits_authn::DeviceId = [1u8; 32];

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
                    MockAuthenticationMethods::DummyAuthenticationMethod,
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
                    MockAuthenticationMethods::InvalidAuthenticationMethod,
                    BoundedVec::new(),
                    (*b"challeng").to_vec()
                ),
                Error::<Test>::InvalidDeviceForAuthenticationMethod
            );
        });
    }

    #[test]
    fn fails_if_cannot_fulfill_challenge() {
        new_test_ext().execute_with(|| {
            let challenge_response =
                RandomnessFromBlockNumber::random(&Encode::encode(&PassPalletId::get()))
                    .0
                    .as_bytes()
                    .to_vec();

            System::set_block_number(2);

            assert_noop!(
                Pass::register(
                    RuntimeOrigin::signed(SIGNER),
                    AccountName::get(),
                    MockAuthenticationMethods::DummyAuthenticationMethod,
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
                MockAuthenticationMethods::DummyAuthenticationMethod,
                BoundedVec::new(),
                RandomnessFromBlockNumber::random(&Encode::encode(&PassPalletId::get()))
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
        // Test for uninitialized account that unreserves if not activated after timeout
        new_test_ext().execute_with(|| {
            assert_ok!(Pass::register(
                RuntimeOrigin::signed(SIGNER),
                AccountName::get(),
                MockAuthenticationMethods::DummyAuthenticationMethod,
                BoundedVec::new(),
                RandomnessFromBlockNumber::random(&Encode::encode(&PassPalletId::get()))
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
                MockAuthenticationMethods::DummyAuthenticationMethod,
                BoundedVec::new(),
                RandomnessFromBlockNumber::random(&Encode::encode(&PassPalletId::get()))
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
    fn claim_works_with_evenodd_registrar() {
        new_test_ext().execute_with(|| {
            // Setup: Register and prepare an account for claiming
            let account_name = AccountName::get();
            // Device ID given by DummyAuthenticationMethod
            let device_id = [1u8; 32];

            // Attempt to claim the account
            assert_ok!(Pass::claim(
                RuntimeOrigin::signed(SIGNER),
                account_name.clone(),
                MockAuthenticationMethods::DummyAuthenticationMethod,
                BoundedVec::new(),
                RandomnessFromBlockNumber::random(&Encode::encode(&PassPalletId::get()))
                    .0
                    .as_bytes()
                    .to_vec()
            ));

            // Verify the account status is now Active
            let updated_account =
                Accounts::<Test>::get(account_name.clone()).expect("Account should exist.");
            assert_eq!(
                updated_account.status,
                AccountStatus::Active,
                "Account should be active after claiming."
            );

            // Check for the expected events
            System::assert_has_event(
                Event::<Test>::Claimed {
                    account_name: account_name.clone(),
                }
                .into(),
            );
            System::assert_has_event(
                Event::<Test>::AddedDevice {
                    account_name,
                    device_id,
                }
                .into(),
            );
        });
    }

    #[test]
    fn claim_fails_with_evenodd_registrar() {
        new_test_ext().execute_with(|| {
            const BADSIGNER: AccountId = AccountId::new([1u8; 32]);

            assert_noop!(
                Pass::claim(
                    RuntimeOrigin::signed(BADSIGNER),
                    AccountName::get(),
                    MockAuthenticationMethods::DummyAuthenticationMethod,
                    BoundedVec::new(),
                    RandomnessFromBlockNumber::random(&Encode::encode(&PassPalletId::get()))
                        .0
                        .as_bytes()
                        .to_vec()
                ),
                Error::<Test>::CannotClaim
            );
        });
    }
}

const DURATION: u64 = 10;
parameter_types! {
    pub ChallengeResponse: Vec<u8> =
            RandomnessFromBlockNumber::random(&Encode::encode(&PassPalletId::get()))
                .0
                .as_bytes()
                .to_vec();
}

fn prepare() -> sp_io::TestExternalities {
    let mut t = new_test_ext();
    t.execute_with(|| {
        assert_ok!(Pass::register(
            RuntimeOrigin::signed(SIGNER),
            AccountName::get(),
            MockAuthenticationMethods::DummyAuthenticationMethod,
            BoundedVec::new(),
            ChallengeResponse::get(),
        ));
    });
    t
}

mod authenticate {
    use super::*;

    #[test]
    fn fails_if_cannot_resolve_device() {
        prepare().execute_with(|| {
            let device = [2u8; 32];

            assert_noop!(
                Pass::authenticate(
                    RuntimeOrigin::signed(OTHER),
                    AccountName::get(),
                    MockAuthenticationMethods::DummyAuthenticationMethod,
                    device,
                    ChallengeResponse::get(),
                    Some(DURATION),
                ),
                Error::<Test>::DeviceNotFound
            );
        });
    }

    #[test]
    fn it_works() {
        prepare().execute_with(|| {
            let block_number = System::block_number();

            assert_ok!(Pass::authenticate(
                RuntimeOrigin::signed(OTHER),
                AccountName::get(),
                MockAuthenticationMethods::DummyAuthenticationMethod,
                THE_DEVICE,
                ChallengeResponse::get(),
                Some(DURATION),
            ));

            System::assert_has_event(
                Event::<Test>::SessionCreated {
                    session_key: OTHER,
                    until: block_number + DURATION,
                }
                .into(),
            );
        });
    }
}

mod add_device {
    use super::*;
    const NEW_DEVICE_ID: fc_traits_authn::DeviceId = [2u8; 32];

    #[test]
    fn fails_if_not_signed_by_session_key() {
        prepare().execute_with(|| {
            assert_noop!(
                Pass::add_device(
                    RuntimeOrigin::signed(OTHER),
                    AccountName::get(),
                    MockAuthenticationMethods::DummyAuthenticationMethod,
                    BoundedVec::truncate_from(vec![1u8]),
                    ChallengeResponse::get()
                ),
                Error::<Test>::SessionNotFound
            );
        });
    }

    #[test]
    fn it_works() {
        prepare().execute_with(|| {
            assert_ok!(Pass::authenticate(
                RuntimeOrigin::signed(OTHER),
                AccountName::get(),
                MockAuthenticationMethods::DummyAuthenticationMethod,
                THE_DEVICE,
                ChallengeResponse::get(),
                Some(DURATION),
            ));

            assert_ok!(Pass::add_device(
                RuntimeOrigin::signed(OTHER),
                AccountName::get(),
                MockAuthenticationMethods::DummyAuthenticationMethod,
                BoundedVec::truncate_from(vec![1u8]),
                ChallengeResponse::get()
            ));

            System::assert_has_event(
                Event::<Test>::AddedDevice {
                    account_name: AccountName::get(),
                    device_id: NEW_DEVICE_ID,
                }
                .into(),
            );
        });
    }
}

mod dispatch {
    use super::*;

    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            let account_id = Pass::account_id_for(&AccountName::get());

            assert_ok!(Pass::register(
                RuntimeOrigin::signed(SIGNER),
                AccountName::get(),
                MockAuthenticationMethods::DummyAuthenticationMethod,
                BoundedVec::new(),
                RandomnessFromBlockNumber::random(&Encode::encode(&PassPalletId::get()))
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
}
