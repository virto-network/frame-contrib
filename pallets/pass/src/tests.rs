//! Tests for pass pallet.
use super::{Error, Event};
use crate::mock::*;

use fc_traits_authn::{Challenger, HashedUserId};
use frame_support::{assert_noop, assert_ok, parameter_types, traits::fungible::Mutate};
use sp_core::Hasher;
use sp_runtime::ArithmeticError;

const SIGNER: AccountId = AccountId::new([0u8; 32]);
const OTHER: AccountId = AccountId::new([1u8; 32]);

const THE_DEVICE: fc_traits_authn::DeviceId = [0u8; 32];
const OTHER_DEVICE: fc_traits_authn::DeviceId = [1u8; 32];

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
            let account_id =
                Pass::account_id_for(AccountNameA::get()).expect("account exists; qed");
            assert_ok!(Pass::create_account(&account_id));

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
                Error::<Test>::AccountAlreadyRegistered
            );
        });
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
                ArithmeticError::Underflow,
            );
        });
    }

    #[test]
    fn fail_if_attestation_is_invalid() {
        new_test_ext().execute_with(|| {
            assert_ok!(Balances::mint_into(&SIGNER, 2));

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
            assert_ok!(Balances::mint_into(&SIGNER, 2));

            let account_id =
                Pass::account_id_for(AccountNameA::get()).expect("account exists; qed");

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
                Event::<Test>::AddedDevice {
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
        assert_ok!(Balances::mint_into(&SIGNER, 2));
        assert_ok!(Pass::register(
            RuntimeOrigin::signed(SIGNER),
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

    #[test]
    fn fail_if_cannot_find_account() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_noop!(
                Pass::authenticate(
                    RuntimeOrigin::signed(OTHER),
                    THE_DEVICE,
                    PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                        user_id: AccountNameB::get(),
                        challenge: authenticator_a::Authenticator::generate(&()),
                    }),
                    Some(DURATION),
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
                    RuntimeOrigin::signed(OTHER),
                    OTHER_DEVICE,
                    PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                        user_id: AccountNameA::get(),
                        challenge: authenticator_a::Authenticator::generate(&()),
                    }),
                    Some(DURATION),
                ),
                Error::<Test>::DeviceNotFound
            );
        });
    }

    #[test]
    fn fail_if_attestation_is_invalid() {
        new_test_ext().execute_with(|| {
            assert_ok!(Balances::mint_into(&SIGNER, 2));

            assert_ok!(Pass::register(
                RuntimeOrigin::signed(SIGNER),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: THE_DEVICE,
                    challenge: AuthenticatorB::generate(&THE_DEVICE),
                }),
            ));

            assert_noop!(
                Pass::authenticate(
                    RuntimeOrigin::signed(OTHER),
                    THE_DEVICE,
                    PassCredential::AuthenticatorB(authenticator_b::Credential::new(
                        AccountNameA::get(),
                        AuthenticatorB::generate(&OTHER_DEVICE)
                    )),
                    Some(DURATION),
                ),
                Error::<Test>::CredentialInvalid
            );
        });
    }

    #[test]
    fn fail_if_attested_with_credentials_from_a_different_device() {
        new_test_ext().execute_with(|| {
            assert_ok!(Balances::mint_into(&SIGNER, 2));

            assert_ok!(Pass::register(
                RuntimeOrigin::signed(SIGNER),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: THE_DEVICE,
                    challenge: AuthenticatorB::generate(&THE_DEVICE),
                }),
            ));
            assert_ok!(Pass::authenticate(
                RuntimeOrigin::signed(SIGNER),
                THE_DEVICE,
                PassCredential::AuthenticatorB(
                    authenticator_b::Credential::new(
                        AccountNameA::get(),
                        AuthenticatorB::generate(&AccountNameA::get()),
                    )
                    .sign(&THE_DEVICE)
                ),
                None,
            ));
            assert_ok!(Pass::add_device(
                RuntimeOrigin::signed(SIGNER),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: OTHER_DEVICE,
                    challenge: AuthenticatorB::generate(&OTHER_DEVICE),
                }),
            ));

            assert_noop!(
                Pass::authenticate(
                    RuntimeOrigin::signed(OTHER),
                    THE_DEVICE,
                    PassCredential::AuthenticatorB(
                        authenticator_b::Credential::new(
                            AccountNameA::get(),
                            AuthenticatorB::generate(&AccountNameA::get())
                        )
                        .sign(&OTHER_DEVICE)
                    ),
                    Some(DURATION),
                ),
                Error::<Test>::CredentialInvalid
            );
        });
    }

    #[test]
    fn fail_if_attested_with_credentials_from_a_different_user_device() {
        new_test_ext().execute_with(|| {
            assert_ok!(Balances::mint_into(&SIGNER, 2));

            assert_ok!(Pass::register(
                RuntimeOrigin::signed(SIGNER),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: THE_DEVICE,
                    challenge: AuthenticatorB::generate(&THE_DEVICE),
                }),
            ));
            assert_ok!(Pass::register(
                RuntimeOrigin::signed(SIGNER),
                AccountNameB::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: OTHER_DEVICE,
                    challenge: AuthenticatorB::generate(&OTHER_DEVICE),
                }),
            ));

            assert_noop!(
                Pass::authenticate(
                    RuntimeOrigin::signed(OTHER),
                    THE_DEVICE,
                    PassCredential::AuthenticatorB(
                        authenticator_b::Credential::new(
                            AccountNameA::get(),
                            AuthenticatorB::generate(&AccountNameA::get()),
                        )
                        .sign(&OTHER_DEVICE)
                    ),
                    Some(DURATION),
                ),
                Error::<Test>::CredentialInvalid
            );
        });
    }

    #[test]
    fn it_works() {
        prepare(AccountNameA::get()).execute_with(|| {
            let block_number = System::block_number();

            assert_ok!(Pass::authenticate(
                RuntimeOrigin::signed(OTHER),
                THE_DEVICE,
                PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                    user_id: AccountNameA::get(),
                    challenge: authenticator_a::Authenticator::generate(&()),
                }),
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

    #[test]
    fn verify_credential_works() {
        new_test_ext().execute_with(|| {
            assert_ok!(Balances::mint_into(&SIGNER, 2));

            assert_ok!(Pass::register(
                RuntimeOrigin::signed(SIGNER),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: THE_DEVICE,
                    challenge: AuthenticatorB::generate(&THE_DEVICE),
                }),
            ));

            assert_ok!(Pass::authenticate(
                RuntimeOrigin::signed(SIGNER),
                THE_DEVICE,
                PassCredential::AuthenticatorB(
                    authenticator_b::Credential::new(
                        AccountNameA::get(),
                        AuthenticatorB::generate(&AccountNameA::get())
                    )
                    .sign(&THE_DEVICE)
                ),
                Some(DURATION),
            ));

            let block_number = System::block_number();
            System::assert_has_event(
                Event::<Test>::SessionCreated {
                    session_key: SIGNER,
                    until: block_number + DURATION,
                }
                .into(),
            );
        });
    }
}

mod add_device {
    use super::*;

    fn prepare() -> sp_io::TestExternalities {
        let mut t = super::prepare(AccountNameA::get());
        t.execute_with(|| {
            assert_ok!(Pass::authenticate(
                RuntimeOrigin::signed(SIGNER),
                THE_DEVICE,
                PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                    user_id: AccountNameA::get(),
                    challenge: authenticator_a::Authenticator::generate(&()),
                }),
                Some(DURATION),
            ));
        });
        t
    }

    #[test]
    fn fail_if_not_signed_by_session_key() {
        prepare().execute_with(|| {
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
                Error::<Test>::SessionNotFound
            );
        });
    }

    #[test]
    fn it_works() {
        prepare().execute_with(|| {
            let who = Pass::account_id_for(AccountNameA::get()).expect("account exists; qed");

            assert_ok!(Pass::add_device(
                RuntimeOrigin::signed(SIGNER),
                PassDeviceAttestation::AuthenticatorAAuthenticator(
                    authenticator_a::DeviceAttestation {
                        device_id: OTHER_DEVICE,
                        challenge: authenticator_a::Authenticator::generate(&()),
                    }
                ),
            ),);

            System::assert_has_event(
                Event::<Test>::AddedDevice {
                    who,
                    device_id: OTHER_DEVICE,
                }
                .into(),
            );
        });
    }
}

mod dispatch {
    use super::*;

    parameter_types! {
        pub Call: Box<RuntimeCall> = Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
            remark: b"Hello, world".to_vec()
        }));
        pub CallEvent: RuntimeEvent = frame_system::Event::Remarked {
            sender: Pass::account_id_for(AccountNameA::get()).expect("account exists; qed"),
            hash: <Test as frame_system::Config>::Hashing::hash(&*b"Hello, world"),
        }.into();
    }

    fn prepare() -> sp_io::TestExternalities {
        let mut t = super::prepare(AccountNameA::get());
        t.execute_with(|| {
            assert_ok!(Pass::authenticate(
                RuntimeOrigin::signed(SIGNER),
                THE_DEVICE,
                PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                    user_id: AccountNameA::get(),
                    challenge: authenticator_a::Authenticator::generate(&()),
                }),
                Some(DURATION),
            ));
        });
        t
    }

    #[test]
    fn fail_without_credentials_if_not_signed_by_session_key() {
        prepare().execute_with(|| {
            assert_noop!(
                Pass::dispatch(RuntimeOrigin::signed(OTHER), Call::get(), None, None),
                Error::<Test>::SessionNotFound
            );
        });
    }

    #[test]
    fn without_credentials_it_works() {
        prepare().execute_with(|| {
            assert_ok!(Pass::dispatch(
                RuntimeOrigin::signed(SIGNER),
                Call::get(),
                None,
                None
            ));

            System::assert_has_event(CallEvent::get());
        });
    }

    #[test]
    fn fail_with_credentials_if_account_not_found() {
        prepare().execute_with(|| {
            assert_noop!(
                Pass::dispatch(
                    RuntimeOrigin::signed(OTHER),
                    Call::get(),
                    Some((
                        OTHER_DEVICE,
                        PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                            user_id: AccountNameB::get(),
                            challenge: authenticator_a::Authenticator::generate(&())
                        })
                    )),
                    None
                ),
                Error::<Test>::AccountNotFound
            );
        });
    }

    #[test]
    fn fail_with_credentials_if_device_not_found() {
        prepare().execute_with(|| {
            assert_noop!(
                Pass::dispatch(
                    RuntimeOrigin::signed(OTHER),
                    Call::get(),
                    Some((
                        OTHER_DEVICE,
                        PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                            user_id: AccountNameA::get(),
                            challenge: authenticator_a::Authenticator::generate(&())
                        })
                    )),
                    None
                ),
                Error::<Test>::DeviceNotFound
            );
        });
    }

    #[test]
    fn fail_with_credentials_if_credential_invalid() {
        prepare().execute_with(|| {
            // On block 1
            let challenge = authenticator_a::Authenticator::generate(&());

            // On block 3
            run_to(3);

            assert_noop!(
                Pass::dispatch(
                    RuntimeOrigin::signed(OTHER),
                    Call::get(),
                    Some((
                        THE_DEVICE,
                        PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                            user_id: AccountNameA::get(),
                            challenge,
                        })
                    )),
                    None,
                ),
                Error::<Test>::CredentialInvalid
            );

            let challenge = authenticator_a::Authenticator::generate(&());
            assert_ok!(Pass::dispatch(
                RuntimeOrigin::signed(OTHER),
                Call::get(),
                Some((
                    THE_DEVICE,
                    PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                        user_id: AccountNameA::get(),
                        challenge,
                    })
                )),
                None,
            ));
        });
    }

    #[test]
    fn with_credentials_it_works() {
        prepare().execute_with(|| {
            assert_ok!(Pass::dispatch(
                RuntimeOrigin::signed(OTHER),
                Call::get(),
                Some((
                    THE_DEVICE,
                    PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                        user_id: AccountNameA::get(),
                        challenge: authenticator_a::Authenticator::generate(&())
                    })
                )),
                None
            ));

            System::assert_has_event(CallEvent::get());
        });
    }

    #[test]
    fn with_new_session_key_it_creates_a_session() {
        prepare().execute_with(|| {
            let block_number = System::block_number();

            assert_ok!(Pass::dispatch(
                RuntimeOrigin::signed(OTHER),
                Call::get(),
                Some((
                    THE_DEVICE,
                    PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                        user_id: AccountNameA::get(),
                        challenge: authenticator_a::Authenticator::generate(&())
                    })
                )),
                Some(OTHER)
            ));

            System::assert_has_event(
                Event::SessionCreated {
                    session_key: OTHER,
                    until: block_number + DURATION,
                }
                .into(),
            );
        });
    }

    #[test]
    fn session_duration_is_met() {
        prepare().execute_with(|| {
            assert_ok!(Pass::dispatch(
                RuntimeOrigin::signed(SIGNER),
                Call::get(),
                None,
                None,
            ));

            run_to(9);

            assert_ok!(Pass::dispatch(
                RuntimeOrigin::signed(SIGNER),
                Call::get(),
                None,
                Some(OTHER),
            ));

            run_to(12);

            assert_noop!(
                Pass::dispatch(RuntimeOrigin::signed(SIGNER), Call::get(), None, None,),
                Error::<Test>::SessionExpired
            );

            assert_ok!(Pass::dispatch(
                RuntimeOrigin::signed(OTHER),
                Call::get(),
                None,
                None,
            ));

            run_to(20);

            assert_noop!(
                Pass::dispatch(RuntimeOrigin::signed(OTHER), Call::get(), None, None,),
                Error::<Test>::SessionExpired
            );
        });
    }
}
