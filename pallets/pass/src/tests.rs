//! Tests for pass pallet.
use super::{Error, Event, PassAuthenticate};
use crate::mock::*;

use codec::{Encode, MaxEncodedLen};
use fc_traits_authn::{Challenger, DeviceId, HashedUserId};
use frame_support::{
    assert_noop, assert_ok,
    dispatch::PostDispatchInfo,
    parameter_types,
    traits::{fungible::Mutate, Footprint},
};
use sp_core::Hasher;
use sp_runtime::{
    generic::ExtrinsicFormat,
    traits::{Applyable, Convert},
    ApplyExtrinsicResultWithInfo, DispatchError, TokenError,
};

const SIGNER: AccountId = AccountId::new([1u8; 32]);
const OTHER: AccountId = AccountId::new([2u8; 32]);
const CHARLIE: AccountId = AccountId::new([3u8; 32]);

const THE_DEVICE: DeviceId = [0u8; 32];
const OTHER_DEVICE: DeviceId = [1u8; 32];

parameter_types! {
    pub AccountNameA: HashedUserId = <Test as frame_system::Config>::Hashing::hash(
        &*b"@account.a:example.org"
    ).0;
    pub AccountNameB: HashedUserId = <Test as frame_system::Config>::Hashing::hash(
        &*b"@account.b:example.org"
    ).0;
}

mod register {
    use super::*;

    #[test]
    fn fail_if_already_registered() {
        new_test_ext().execute_with(|| {
            let account_id: AccountId = Pass::address_for(AccountNameA::get());
            assert_ok!(Pass::create_account(&account_id));

            assert_noop!(
                Pass::register(
                    RuntimeOrigin::root(),
                    AccountNameA::get(),
                    PassDeviceAttestation::AuthenticatorAAuthenticator(
                        authenticator_a::DeviceAttestation {
                            device_id: THE_DEVICE,
                            challenge: authenticator_a::Authenticator::generate(&()),
                        }
                    ),
                ),
                Error::<Test>::AccountAlreadyRegistered
            );
        })
    }

    #[test]
    fn register_deposit_logic_works() {
        new_test_ext().execute_with(|| {
            assert_ok!(Pass::register(
                RuntimeOrigin::root(),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorAAuthenticator(
                    authenticator_a::DeviceAttestation {
                        device_id: THE_DEVICE,
                        challenge: authenticator_a::Authenticator::generate(&()),
                    }
                ),
            ));
        });

        new_test_ext().execute_with(|| {
            assert_noop!(
                Pass::register(
                    RuntimeOrigin::signed(SIGNER),
                    AccountNameA::get(),
                    PassDeviceAttestation::AuthenticatorAAuthenticator(
                        authenticator_a::DeviceAttestation {
                            device_id: THE_DEVICE,
                            challenge: authenticator_a::Authenticator::generate(&()),
                        }
                    ),
                ),
                TokenError::FundsUnavailable,
            );
        });
    }

    #[test]
    fn fail_if_attestation_is_invalid() {
        new_test_ext().execute_with(|| {
            assert_ok!(Balances::mint_into(
                &SIGNER,
                ExistentialDeposit::get()
                    + RegistrationStoragePrice::convert(Footprint::from_parts(1, 32))
            ));

            assert_noop!(
                Pass::register(
                    RuntimeOrigin::signed(SIGNER),
                    AccountNameB::get(),
                    PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                        device_id: THE_DEVICE,
                        challenge: AuthenticatorB::generate(&OTHER_DEVICE),
                    }),
                ),
                Error::<Test>::DeviceAttestationInvalid
            );
        });
    }

    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            assert_ok!(Balances::mint_into(
                &SIGNER,
                ExistentialDeposit::get()
                    + RegistrationStoragePrice::convert(Footprint::from_parts(1, 32))
            ));

            let account_id = Pass::address_for(AccountNameA::get());

            assert_ok!(Pass::register(
                RuntimeOrigin::signed(SIGNER),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorAAuthenticator(
                    authenticator_a::DeviceAttestation {
                        device_id: THE_DEVICE,
                        challenge: authenticator_a::Authenticator::generate(&()),
                    }
                ),
            ));

            System::assert_has_event(
                Event::<Test>::Registered {
                    who: account_id.clone(),
                }
                .into(),
            );
            System::assert_has_event(
                Event::<Test>::DeviceAdded {
                    who: account_id,
                    device_id: THE_DEVICE,
                }
                .into(),
            );
        });
    }
}

fn prepare(user_id: HashedUserId) -> sp_io::TestExternalities {
    let mut t = new_test_ext();
    t.execute_with(|| {
        assert_ok!(Pass::register(
            RuntimeOrigin::root(),
            user_id,
            PassDeviceAttestation::AuthenticatorAAuthenticator(
                authenticator_a::DeviceAttestation {
                    device_id: THE_DEVICE,
                    challenge: authenticator_a::Authenticator::generate(&()),
                }
            ),
        ));
    });
    t
}

const DURATION: u64 = 10;

mod authenticate {
    use super::*;
    use crate::DeviceOf;

    #[test]
    fn fail_if_cannot_find_account() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_noop!(
                Pass::authenticate(
                    &THE_DEVICE,
                    &PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                        user_id: AccountNameB::get(),
                        challenge: authenticator_a::Authenticator::generate(&()),
                    }),
                ),
                Error::<Test>::AccountNotFound
            );
        });
    }

    #[test]
    fn fail_if_cannot_find_device() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_noop!(
                Pass::authenticate(
                    &OTHER_DEVICE,
                    &PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                        user_id: AccountNameA::get(),
                        challenge: authenticator_a::Authenticator::generate(&()),
                    }),
                ),
                Error::<Test>::DeviceNotFound
            );
        });
    }

    #[test]
    fn fail_if_attestation_is_invalid() {
        new_test_ext().execute_with(|| {
            assert_ok!(Pass::register(
                RuntimeOrigin::root(),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: THE_DEVICE,
                    challenge: AuthenticatorB::generate(&THE_DEVICE),
                }),
            ));

            assert_noop!(
                Pass::authenticate(
                    &THE_DEVICE,
                    &PassCredential::AuthenticatorB(authenticator_b::Credential::new(
                        AccountNameA::get(),
                        AuthenticatorB::generate(&OTHER_DEVICE)
                    )),
                ),
                Error::<Test>::CredentialInvalid
            );
        });
    }

    #[test]
    fn fail_if_attested_with_credentials_from_a_different_device() {
        prepare(AccountNameA::get()).execute_with(|| {
            let address = Pass::address_for(AccountNameA::get());
            assert_ok!(Balances::mint_into(
                &Address::get(),
                ExistentialDeposit::get()
                    + ItemStoragePrice::convert(Footprint::from_parts(
                        1,
                        DeviceOf::<Test>::max_encoded_len()
                    ))
            ));

            assert_ok!(Pass::authenticate(
                &THE_DEVICE,
                &PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                    user_id: AccountNameA::get(),
                    challenge: authenticator_a::Authenticator::generate(&()),
                }),
            ));

            Pass::try_add_device(
                &address,
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: OTHER_DEVICE,
                    challenge: AuthenticatorB::generate(&OTHER_DEVICE),
                }),
            )
            .expect("adding device on an existing account works; qed");

            assert_noop!(
                Pass::authenticate(
                    &THE_DEVICE,
                    &PassCredential::AuthenticatorB(
                        authenticator_b::Credential::new(
                            AccountNameA::get(),
                            AuthenticatorB::generate(&AccountNameA::get())
                        )
                        .sign(&OTHER_DEVICE)
                    ),
                ),
                Error::<Test>::CredentialInvalid
            );
        });
    }

    #[test]
    fn fail_if_attested_with_credentials_from_a_different_users_device() {
        new_test_ext().execute_with(|| {
            assert_ok!(Pass::register(
                RuntimeOrigin::root(),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: THE_DEVICE,
                    challenge: AuthenticatorB::generate(&THE_DEVICE),
                }),
            ));
            assert_ok!(Pass::register(
                RuntimeOrigin::root(),
                AccountNameB::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: OTHER_DEVICE,
                    challenge: AuthenticatorB::generate(&OTHER_DEVICE),
                }),
            ));

            assert_noop!(
                Pass::authenticate(
                    &THE_DEVICE,
                    &PassCredential::AuthenticatorB(
                        authenticator_b::Credential::new(
                            AccountNameA::get(),
                            AuthenticatorB::generate(&AccountNameA::get()),
                        )
                        .sign(&OTHER_DEVICE)
                    ),
                ),
                Error::<Test>::CredentialInvalid
            );
        });
    }

    #[test]
    fn it_works() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Pass::authenticate(
                &THE_DEVICE,
                &PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                    user_id: AccountNameA::get(),
                    challenge: authenticator_a::Authenticator::generate(&()),
                }),
            ));
        });
    }

    #[test]
    fn verify_credential_works() {
        new_test_ext().execute_with(|| {
            assert_ok!(Pass::register(
                RuntimeOrigin::root(),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: THE_DEVICE,
                    challenge: AuthenticatorB::generate(&THE_DEVICE),
                }),
            ));

            assert_ok!(Pass::authenticate(
                &THE_DEVICE,
                &PassCredential::AuthenticatorB(
                    authenticator_b::Credential::new(
                        AccountNameA::get(),
                        AuthenticatorB::generate(&AccountNameA::get())
                    )
                    .sign(&THE_DEVICE)
                ),
            ));
        });
    }
}

parameter_types! {
    pub Address: AccountId = Pass::address_for(AccountNameA::get());
}

mod add_device {
    use super::*;
    use crate::DeviceOf;

    #[test]
    fn fails_if_bad_origin() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_noop!(
                Pass::add_device(
                    RuntimeOrigin::root(),
                    PassDeviceAttestation::AuthenticatorAAuthenticator(
                        authenticator_a::DeviceAttestation {
                            device_id: OTHER_DEVICE,
                            challenge: authenticator_a::Authenticator::generate(&()),
                        }
                    ),
                ),
                DispatchError::BadOrigin
            );

            assert_noop!(
                Pass::add_device(
                    RuntimeOrigin::signed(OTHER),
                    PassDeviceAttestation::AuthenticatorAAuthenticator(
                        authenticator_a::DeviceAttestation {
                            device_id: OTHER_DEVICE,
                            challenge: authenticator_a::Authenticator::generate(&()),
                        }
                    ),
                ),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn deposit_logic_works() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_noop!(
                Pass::add_device(
                    RuntimeOrigin::signed(Address::get()),
                    PassDeviceAttestation::AuthenticatorAAuthenticator(
                        authenticator_a::DeviceAttestation {
                            device_id: OTHER_DEVICE,
                            challenge: authenticator_a::Authenticator::generate(&()),
                        }
                    ),
                ),
                TokenError::FundsUnavailable
            );
        })
    }

    #[test]
    fn it_works() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Balances::mint_into(
                &Address::get(),
                ExistentialDeposit::get()
                    + ItemStoragePrice::convert(Footprint::from_parts(
                        1,
                        DeviceOf::<Test>::max_encoded_len(),
                    ))
            ));

            assert_ok!(Pass::add_device(
                RuntimeOrigin::signed(Address::get()),
                PassDeviceAttestation::AuthenticatorAAuthenticator(
                    authenticator_a::DeviceAttestation {
                        device_id: OTHER_DEVICE,
                        challenge: authenticator_a::Authenticator::generate(&()),
                    }
                ),
            ));

            System::assert_has_event(
                Event::<Test>::DeviceAdded {
                    who: Address::get(),
                    device_id: OTHER_DEVICE,
                }
                .into(),
            );
        });
    }
}

mod add_session_key {
    use super::*;

    #[test]
    fn fails_if_bad_origin() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_noop!(
                Pass::add_session_key(RuntimeOrigin::root(), OTHER, Some(DURATION)),
                DispatchError::BadOrigin
            );
            assert_noop!(
                Pass::add_session_key(RuntimeOrigin::signed(OTHER), OTHER, Some(DURATION)),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn fails_if_account_exists() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Balances::mint_into(&CHARLIE, ExistentialDeposit::get()));
            assert_noop!(
                Pass::add_session_key(
                    RuntimeOrigin::signed(Address::get()),
                    CHARLIE,
                    Some(DURATION)
                ),
                Error::<Test>::AccountForSessionKeyAlreadyExists
            );
        })
    }

    #[test]
    fn it_works() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Pass::add_session_key(
                RuntimeOrigin::signed(Address::get()),
                OTHER,
                Some(DURATION)
            ));

            System::assert_has_event(
                Event::<Test>::SessionCreated {
                    session_key: OTHER,
                    until: DURATION,
                }
                .into(),
            );
        })
    }

    #[test]
    fn deposit_logic_works() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Pass::add_session_key(
                RuntimeOrigin::signed(Address::get()),
                SIGNER,
                None,
            ));

            assert_noop!(
                Pass::add_session_key(RuntimeOrigin::signed(Address::get()), OTHER, None),
                TokenError::FundsUnavailable
            );

            assert_ok!(Balances::mint_into(
                &Address::get(),
                ExistentialDeposit::get()
                    + ItemStoragePrice::convert(Footprint::from_parts(
                        1,
                        AccountId::max_encoded_len()
                    ))
            ));

            assert_ok!(Pass::add_session_key(
                RuntimeOrigin::signed(Address::get()),
                OTHER,
                None,
            ));
        })
    }
}

mod dispatch {
    use super::*;
    use crate::SessionKeys;
    use frame_support::dispatch::GetDispatchInfo;
    use sp_runtime::transaction_validity::InvalidTransaction;

    parameter_types! {
        pub Call: RuntimeCall = RuntimeCall::System(frame_system::Call::remark_with_event {
            remark: b"Hello, world".to_vec()
        });
        pub CallEvent: RuntimeEvent = frame_system::Event::Remarked {
            sender: Address::get(),
            hash: <Test as frame_system::Config>::Hashing::hash(&*b"Hello, world"),
        }.into();
    }

    fn authenticate(
        device_id: DeviceId,
        credentials: PassCredential,
        call: RuntimeCall,
    ) -> ApplyExtrinsicResultWithInfo<PostDispatchInfo> {
        let extensions: TxExtensions = (
            PassAuthenticate::<Test>::from(device_id, credentials),
            pallet_transaction_payment::ChargeTransactionPayment::from(0),
        );

        let xt = CheckedExtrinsic {
            format: ExtrinsicFormat::General(0, extensions),
            function: call.clone(),
        };

        xt.apply::<Test>(&call.get_dispatch_info(), call.encoded_size())
    }

    fn signed(
        session_key: AccountId,
        call: RuntimeCall,
    ) -> ApplyExtrinsicResultWithInfo<PostDispatchInfo> {
        let extensions: TxExtensions = (
            PassAuthenticate::<Test>::default(),
            pallet_transaction_payment::ChargeTransactionPayment::from(0),
        );

        let xt = CheckedExtrinsic {
            format: ExtrinsicFormat::Signed(session_key, extensions),
            function: call.clone(),
        };

        xt.apply::<Test>(&call.get_dispatch_info(), call.encoded_size())
    }

    #[test]
    fn bypasses_if_not_signed_by_a_session_key() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Balances::mint_into(&CHARLIE, Balance::MAX));
            assert_ok!(signed(CHARLIE, Call::get()));

            System::assert_has_event(
                frame_system::Event::Remarked {
                    sender: CHARLIE,
                    hash: <Test as frame_system::Config>::Hashing::hash(&*b"Hello, world"),
                }
                .into(),
            )
        });
    }

    #[test]
    fn it_works_if_signed_by_a_session_key() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Balances::mint_into(&Address::get(), Balance::MAX));

            assert_ok!(Pass::add_session_key(
                RuntimeOrigin::signed(Address::get()),
                OTHER,
                None,
            ));

            assert_ok!(signed(OTHER, Call::get()));

            System::assert_has_event(CallEvent::get());
        });
    }

    #[test]
    fn fail_with_credentials_if_account_not_found() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_noop!(
                authenticate(
                    OTHER_DEVICE,
                    PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                        user_id: AccountNameB::get(),
                        challenge: authenticator_a::Authenticator::generate(&())
                    }),
                    Call::get(),
                ),
                InvalidTransaction::BadSigner
            );
        });
    }

    #[test]
    fn fail_with_credentials_if_device_not_found() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_noop!(
                authenticate(
                    OTHER_DEVICE,
                    PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                        user_id: AccountNameA::get(),
                        challenge: authenticator_a::Authenticator::generate(&())
                    }),
                    Call::get(),
                ),
                InvalidTransaction::BadSigner
            );
        })
    }

    #[test]
    fn fail_with_credentials_if_credential_invalid() {
        prepare(AccountNameA::get()).execute_with(|| {
            // On block 1
            let challenge = authenticator_a::Authenticator::generate(&());

            // On block 3
            run_to(3);

            assert_noop!(
                authenticate(
                    THE_DEVICE,
                    PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                        user_id: AccountNameA::get(),
                        challenge,
                    }),
                    Call::get(),
                ),
                InvalidTransaction::BadSigner
            );
        });
    }

    #[test]
    fn with_credentials_it_works() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Balances::mint_into(&Address::get(), Balance::MAX));

            assert_ok!(authenticate(
                THE_DEVICE,
                PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                    user_id: AccountNameA::get(),
                    challenge: authenticator_a::Authenticator::generate(&()),
                }),
                Call::get(),
            ));

            System::assert_has_event(CallEvent::get());
        });
    }

    #[test]
    fn session_duration_is_met() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Balances::mint_into(&Address::get(), Balance::MAX));

            assert_ok!(authenticate(
                THE_DEVICE,
                PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                    user_id: AccountNameA::get(),
                    challenge: authenticator_a::Authenticator::generate(&()),
                }),
                crate::Call::<Test>::add_session_key {
                    session: SIGNER,
                    duration: None,
                }
                .into(),
            ));

            run_to(9);

            assert_ok!(signed(SIGNER, Call::get()));

            run_to(12);

            assert!(!SessionKeys::<Test>::contains_key(SIGNER));

            assert_ok!(authenticate(
                THE_DEVICE,
                PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                    user_id: AccountNameA::get(),
                    challenge: authenticator_a::Authenticator::generate(&()),
                }),
                crate::Call::<Test>::add_session_key {
                    session: OTHER,
                    duration: Some(7),
                }
                .into(),
            ));

            run_to(20);

            assert!(!SessionKeys::<Test>::contains_key(OTHER));
        });
    }
}
