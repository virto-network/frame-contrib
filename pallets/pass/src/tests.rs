//! Tests for pass pallet.
use super::{Error, Event, PassAuthenticate};
use crate::filter::DeviceFilter;
use crate::mock::*;

use codec::{Encode, MaxEncodedLen};
use fc_traits_authn::{Challenger, DeviceId, HashedUserId};
use frame_support::{
    assert_noop, assert_ok,
    dispatch::{GetDispatchInfo, PostDispatchInfo},
    parameter_types,
    traits::{fungible::Mutate, Footprint},
};
use sp_core::Hasher;
use sp_runtime::{
    generic::ExtrinsicFormat,
    traits::{Applyable, Convert, ExtensionVariant},
    ApplyExtrinsicResultWithInfo, DispatchError, TokenError,
};

const SIGNER: AccountId = AccountId::new([1u8; 32]);
const OTHER: AccountId = AccountId::new([2u8; 32]);
const CHARLIE: AccountId = AccountId::new([3u8; 32]);

const THE_DEVICE: DeviceId = [10u8; 32];
const OTHER_DEVICE: DeviceId = [11u8; 32];
const THIRD_DEVICE: DeviceId = [12u8; 32];

fn remark_only_filter(
) -> DeviceFilter<(), u128, frame_support::traits::ConstU32<10>, frame_support::traits::ConstU32<5>>
{
    DeviceFilter::Calls(
        [(0u8, 7u8)]
            .into_iter()
            .collect::<alloc::collections::BTreeSet<_>>()
            .try_into()
            .unwrap(),
    )
}

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
                            challenge: authenticator_a::Authenticator::generate(&(), &[]),
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
                        challenge: authenticator_a::Authenticator::generate(&(), &[]),
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
                            challenge: authenticator_a::Authenticator::generate(&(), &[]),
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
                        context: System::block_number(),
                        challenge: LastThreeBlocksChallenger::generate(
                            &System::block_number(),
                            &[1]
                        ),
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
                        challenge: authenticator_a::Authenticator::generate(&(), &[]),
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
                    challenge: authenticator_a::Authenticator::generate(&(), &[]),
                }
            ),
        ));
    });
    t
}

const DURATION: u64 = 10;

/// Applies `call` as an extrinsic authenticated with a device credential.
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
        format: ExtrinsicFormat::General(ExtensionVariant::V0(extensions)),
        function: call.clone(),
    };

    xt.apply::<Test>(&call.get_dispatch_info(), call.encoded_size())
}

/// Applies `call` as an extrinsic signed by `session_key`.
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
                        challenge: authenticator_a::Authenticator::generate(&(), &[]),
                    }),
                    &[]
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
                        challenge: authenticator_a::Authenticator::generate(&(), &[]),
                    }),
                    &[]
                ),
                Error::<Test>::DeviceNotFound
            );
        });
    }

    #[test]
    fn fail_if_challenge_validation_fails() {
        new_test_ext().execute_with(|| {
            assert_ok!(Pass::register(
                RuntimeOrigin::root(),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: THE_DEVICE,
                    context: System::block_number(),
                    challenge: LastThreeBlocksChallenger::generate(
                        &System::block_number(),
                        &Address::get(),
                    ),
                }),
            ));

            assert_noop!(
                Pass::authenticate(
                    &THE_DEVICE,
                    &PassCredential::AuthenticatorB(
                        authenticator_b::Credential::new(
                            AccountNameA::get(),
                            System::block_number(),
                            0,
                            LastThreeBlocksChallenger::generate(&System::block_number(), &[])
                        )
                        .sign(&THE_DEVICE)
                    ),
                    &[1]
                ),
                Error::<Test>::CredentialInvalid
            );
        });
    }

    #[test]
    fn fail_if_credentials_cannot_be_verified() {
        new_test_ext().execute_with(|| {
            assert_ok!(Pass::register(
                RuntimeOrigin::root(),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: THE_DEVICE,
                    context: System::block_number(),
                    challenge: LastThreeBlocksChallenger::generate(
                        &System::block_number(),
                        &Address::get(),
                    ),
                }),
            ));

            assert_noop!(
                Pass::authenticate(
                    &THE_DEVICE,
                    // The credentials are not signed, therefore, are invalid
                    &PassCredential::AuthenticatorB(authenticator_b::Credential::new(
                        AccountNameA::get(),
                        System::block_number(),
                        0,
                        LastThreeBlocksChallenger::generate(&System::block_number(), &[])
                    )),
                    &[]
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
                    challenge: authenticator_a::Authenticator::generate(&(), &[]),
                }),
                &[],
            ));

            Pass::try_add_device(
                &address,
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: OTHER_DEVICE,
                    context: System::block_number(),
                    challenge: LastThreeBlocksChallenger::generate(
                        &System::block_number(),
                        &address,
                    ),
                }),
                DeviceFilter::Admin,
            )
            .expect("adding device on an existing account works; qed");

            assert_noop!(
                Pass::authenticate(
                    &THE_DEVICE,
                    &PassCredential::AuthenticatorB(
                        authenticator_b::Credential::new(
                            AccountNameA::get(),
                            System::block_number(),
                            0,
                            LastThreeBlocksChallenger::generate(&System::block_number(), &[]),
                        )
                        .sign(&OTHER_DEVICE)
                    ),
                    &[],
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
                    context: System::block_number(),
                    challenge: LastThreeBlocksChallenger::generate(
                        &System::block_number(),
                        &Address::get(),
                    ),
                }),
            ));
            assert_ok!(Pass::register(
                RuntimeOrigin::root(),
                AccountNameB::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: OTHER_DEVICE,
                    context: System::block_number(),
                    challenge: LastThreeBlocksChallenger::generate(
                        &System::block_number(),
                        &AddressB::get(),
                    ),
                }),
            ));

            assert_noop!(
                Pass::authenticate(
                    &THE_DEVICE,
                    &PassCredential::AuthenticatorB(
                        authenticator_b::Credential::new(
                            AccountNameA::get(),
                            System::block_number(),
                            0,
                            LastThreeBlocksChallenger::generate(&System::block_number(), &[]),
                        )
                        .sign(&OTHER_DEVICE)
                    ),
                    &[]
                ),
                Error::<Test>::CredentialInvalid
            );
        });
    }

    /// This must ensure credentials are unique and cannot be reused.
    #[test]
    fn fails_if_authenticated_with_already_reused_credentials_if_challenge_is_still_valid() {
        new_test_ext().execute_with(|| {
            assert_ok!(Pass::register(
                RuntimeOrigin::root(),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: THE_DEVICE,
                    context: System::block_number(),
                    challenge: LastThreeBlocksChallenger::generate(
                        &System::block_number(),
                        &Address::get(),
                    ),
                }),
            ));

            let credentials = &PassCredential::AuthenticatorB(
                authenticator_b::Credential::new(
                    AccountNameA::get(),
                    System::block_number(),
                    0,
                    LastThreeBlocksChallenger::generate(&System::block_number(), &[1]),
                )
                .sign(&THE_DEVICE),
            );

            // Nonce: 1
            assert_ok!(Pass::authenticate(&THE_DEVICE, credentials, &[1]));

            // Nonce: 2
            assert_noop!(
                Pass::authenticate(&THE_DEVICE, credentials, &[2]),
                Error::<Test>::CredentialInvalid,
            );
        })
    }

    #[test]
    fn it_works() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Pass::authenticate(
                &THE_DEVICE,
                &PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                    user_id: AccountNameA::get(),
                    challenge: authenticator_a::Authenticator::generate(&(), &[]),
                }),
                &[]
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
                    context: System::block_number(),
                    challenge: LastThreeBlocksChallenger::generate(
                        &System::block_number(),
                        &Address::get(),
                    ),
                }),
            ));

            assert_ok!(Pass::authenticate(
                &THE_DEVICE,
                &PassCredential::AuthenticatorB(
                    authenticator_b::Credential::new(
                        AccountNameA::get(),
                        System::block_number(),
                        0,
                        LastThreeBlocksChallenger::generate(&System::block_number(), &[]),
                    )
                    .sign(&THE_DEVICE)
                ),
                &[],
            ));
        });
    }

    #[test]
    fn updating_device_after_verify_credential_works() {
        new_test_ext().execute_with(|| {
            assert_ok!(Pass::register(
                RuntimeOrigin::root(),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: THE_DEVICE,
                    context: System::block_number(),
                    challenge: LastThreeBlocksChallenger::generate(
                        &System::block_number(),
                        &Address::get(),
                    ),
                }),
            ));

            assert_ok!(Pass::authenticate(
                &THE_DEVICE,
                &PassCredential::AuthenticatorB(
                    authenticator_b::Credential::new(
                        AccountNameA::get(),
                        System::block_number(),
                        0,
                        LastThreeBlocksChallenger::generate(&System::block_number(), &[]),
                    )
                    .sign(&THE_DEVICE)
                ),
                &[],
            ));

            assert_noop!(
                Pass::authenticate(
                    &THE_DEVICE,
                    &PassCredential::AuthenticatorB(
                        authenticator_b::Credential::new(
                            AccountNameA::get(),
                            System::block_number(),
                            0,
                            LastThreeBlocksChallenger::generate(&System::block_number(), &[]),
                        )
                        .sign(&THE_DEVICE)
                    ),
                    &[]
                ),
                Error::<Test>::CredentialInvalid
            );

            assert_ok!(Pass::authenticate(
                &THE_DEVICE,
                &PassCredential::AuthenticatorB(
                    authenticator_b::Credential::new(
                        AccountNameA::get(),
                        System::block_number(),
                        1,
                        LastThreeBlocksChallenger::generate(&System::block_number(), &[]),
                    )
                    .sign(&THE_DEVICE)
                ),
                &[],
            ));
        });
    }
}

parameter_types! {
    pub Address: AccountId = Pass::address_for(AccountNameA::get());
    pub AddressB: AccountId = Pass::address_for(AccountNameB::get());
}

mod add_device_with_filter {
    use super::*;
    use crate::DeviceOf;

    #[test]
    fn fails_if_bad_origin() {
        prepare(AccountNameA::get()).execute_with(|| {
            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_noop!(
                Pass::add_device_with_filter(
                    RuntimeOrigin::root(),
                    PassDeviceAttestation::AuthenticatorAAuthenticator(
                        authenticator_a::DeviceAttestation {
                            device_id: OTHER_DEVICE,
                            challenge: authenticator_a::Authenticator::generate(&(), &[]),
                        }
                    ),
                    DeviceFilter::Admin,
                ),
                DispatchError::BadOrigin
            );

            assert_noop!(
                Pass::add_device_with_filter(
                    RuntimeOrigin::signed(OTHER),
                    PassDeviceAttestation::AuthenticatorAAuthenticator(
                        authenticator_a::DeviceAttestation {
                            device_id: OTHER_DEVICE,
                            challenge: authenticator_a::Authenticator::generate(&(), &[]),
                        }
                    ),
                    DeviceFilter::Admin,
                ),
                DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn fails_if_invalid_challenge() {
        prepare(AccountNameA::get()).execute_with(|| {
            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_noop!(
                Pass::add_device_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                        device_id: OTHER_DEVICE,
                        context: System::block_number(),
                        challenge: LastThreeBlocksChallenger::generate(
                            &System::block_number(),
                            &[]
                        ),
                    }),
                    DeviceFilter::Admin,
                ),
                Error::<Test>::DeviceAttestationInvalid
            );
        });
    }

    #[test]
    fn deposit_logic_works() {
        prepare(AccountNameA::get()).execute_with(|| {
            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_noop!(
                Pass::add_device_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    PassDeviceAttestation::AuthenticatorAAuthenticator(
                        authenticator_a::DeviceAttestation {
                            device_id: OTHER_DEVICE,
                            challenge: authenticator_a::Authenticator::generate(&(), &[]),
                        }
                    ),
                    DeviceFilter::Admin,
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

            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_device_with_filter(
                RuntimeOrigin::signed(Address::get()),
                PassDeviceAttestation::AuthenticatorAAuthenticator(
                    authenticator_a::DeviceAttestation {
                        device_id: OTHER_DEVICE,
                        challenge: authenticator_a::Authenticator::generate(&(), &[]),
                    }
                ),
                DeviceFilter::Admin,
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

    #[test]
    fn device_limit_works() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Balances::mint_into(
                &Address::get(),
                ExistentialDeposit::get()
                    + ItemStoragePrice::convert(Footprint::from_parts(
                        1,
                        DeviceOf::<Test>::max_encoded_len(),
                    ))
            ));

            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_device_with_filter(
                RuntimeOrigin::signed(Address::get()),
                PassDeviceAttestation::AuthenticatorAAuthenticator(
                    authenticator_a::DeviceAttestation {
                        device_id: OTHER_DEVICE,
                        challenge: authenticator_a::Authenticator::generate(&(), &[]),
                    }
                ),
                DeviceFilter::Admin,
            ));

            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_noop!(
                Pass::add_device_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    PassDeviceAttestation::AuthenticatorAAuthenticator(
                        authenticator_a::DeviceAttestation {
                            device_id: THIRD_DEVICE,
                            challenge: authenticator_a::Authenticator::generate(&(), &[]),
                        }
                    ),
                    DeviceFilter::Admin,
                ),
                Error::<Test>::MaxDevicesExceeded
            );
        });
    }
}

mod add_session_key_with_filter {
    use super::*;
    use sp_core::H256;
    use sp_io::hashing::blake2_256;

    #[test]
    fn fails_if_bad_origin() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_noop!(
                Pass::add_session_key_with_filter(
                    RuntimeOrigin::root(),
                    OTHER,
                    Some(DURATION),
                    remark_only_filter()
                ),
                DispatchError::BadOrigin
            );
            assert_noop!(
                Pass::add_session_key_with_filter(
                    RuntimeOrigin::signed(OTHER),
                    OTHER,
                    Some(DURATION),
                    remark_only_filter()
                ),
                DispatchError::BadOrigin
            );
        })
    }

    #[test]
    fn fails_if_account_exists() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Balances::mint_into(&CHARLIE, ExistentialDeposit::get()));
            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_noop!(
                Pass::add_session_key_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    CHARLIE,
                    Some(DURATION),
                    remark_only_filter(),
                ),
                Error::<Test>::AccountForSessionKeyAlreadyExists
            );
        })
    }

    #[test]
    fn it_works() {
        prepare(AccountNameA::get()).execute_with(|| {
            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_session_key_with_filter(
                RuntimeOrigin::signed(Address::get()),
                OTHER,
                Some(DURATION),
                remark_only_filter(),
            ));

            System::assert_has_event(
                Event::<Test>::SessionCreated {
                    session_key_hash: H256(blake2_256(&OTHER.encode())),
                    until: DURATION,
                }
                .into(),
            );
        })
    }

    #[test]
    fn cannot_use_a_session_key_in_use_by_another_user() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Pass::register(
                RuntimeOrigin::root(),
                AccountNameB::get(),
                PassDeviceAttestation::AuthenticatorAAuthenticator(
                    authenticator_a::DeviceAttestation {
                        device_id: OTHER_DEVICE,
                        challenge: authenticator_a::Authenticator::generate(&(), &[]),
                    }
                ),
            ));

            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_session_key_with_filter(
                RuntimeOrigin::signed(Address::get()),
                OTHER,
                Some(DURATION),
                remark_only_filter(),
            ));

            crate::AuthenticatedDevice::<Test>::put((AddressB::get(), OTHER_DEVICE));
            assert_noop!(
                Pass::add_session_key_with_filter(
                    RuntimeOrigin::signed(AddressB::get()),
                    OTHER,
                    Some(DURATION),
                    remark_only_filter(),
                ),
                Error::<Test>::SessionKeyInUse
            );
        })
    }

    #[test]
    fn max_sessions_per_account_works() {
        prepare(AccountNameA::get()).execute_with(|| {
            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_session_key_with_filter(
                RuntimeOrigin::signed(Address::get()),
                SIGNER,
                None,
                remark_only_filter(),
            ));

            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_session_key_with_filter(
                RuntimeOrigin::signed(Address::get()),
                OTHER,
                None,
                remark_only_filter(),
            ));

            run_to(5);

            // Extending a session key works
            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_session_key_with_filter(
                RuntimeOrigin::signed(Address::get()),
                OTHER,
                None,
                remark_only_filter(),
            ));

            run_to(10);

            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_noop!(
                Pass::add_session_key_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    CHARLIE,
                    None,
                    remark_only_filter()
                ),
                Error::<Test>::MaxSessionsExceeded,
            );

            run_to(12);

            // Sessions count of an account changes once the session of an already existing session
            // expires.
            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_session_key_with_filter(
                RuntimeOrigin::signed(Address::get()),
                CHARLIE,
                None,
                remark_only_filter(),
            ));
        })
    }
}

mod dispatch {
    use super::*;
    use crate::SessionKeys;
    use sp_io::hashing::blake2_256;
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

            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_session_key_with_filter(
                RuntimeOrigin::signed(Address::get()),
                OTHER,
                None,
                remark_only_filter(),
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
                        challenge: authenticator_a::Authenticator::generate(&(), &[])
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
                        challenge: authenticator_a::Authenticator::generate(&(), &[])
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
            let challenge = authenticator_a::Authenticator::generate(&(), &[]);

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

    /// This must ensure credentials are unique and cannot be reused.
    ///
    /// In this case we assert that it should not be possible for someone to take some credentials
    /// that were successful in the past, and use them to impersonate another account (maybe adding
    /// a device from the attacker).
    #[test]
    fn fail_if_credentials_are_reused() {
        new_test_ext().execute_with(|| {
            assert_ok!(Pass::register(
                RuntimeOrigin::root(),
                AccountNameA::get(),
                PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                    device_id: THE_DEVICE,
                    context: System::block_number(),
                    challenge: LastThreeBlocksChallenger::generate(
                        &System::block_number(),
                        &Address::get(),
                    ),
                }),
            ));
            assert_ok!(Balances::mint_into(&Address::get(), Balance::MAX));

            let extrinsic_context = (
                0u8,                                                                   // extension_version_byte
                Call::get(),                                                           // call
                pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0), // transaction_extensions
                (),                                                                    // implicit
            )
                .using_encoded(blake2_256);

            let credentials = PassCredential::AuthenticatorB(
                authenticator_b::Credential::new(
                    AccountNameA::get(),
                    System::block_number(),
                    0,
                    LastThreeBlocksChallenger::generate(
                        &System::block_number(),
                        &extrinsic_context,
                    ),
                )
                .sign(&THE_DEVICE),
            );

            assert_ok!(authenticate(THE_DEVICE, credentials.clone(), Call::get()));

            assert_noop!(
                authenticate(
                    THE_DEVICE,
                    credentials,
                    crate::Call::add_device_with_filter {
                        attestation: PassDeviceAttestation::AuthenticatorAAuthenticator(
                            authenticator_a::DeviceAttestation {
                                device_id: OTHER_DEVICE,
                                challenge: authenticator_a::Authenticator::generate(&(), &[])
                            }
                        ),
                        filter: DeviceFilter::Admin,
                    }
                    .into()
                ),
                InvalidTransaction::BadSigner,
            );
        })
    }

    #[test]
    fn with_credentials_it_works() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Balances::mint_into(&Address::get(), Balance::MAX));

            assert_ok!(authenticate(
                THE_DEVICE,
                PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                    user_id: AccountNameA::get(),
                    challenge: authenticator_a::Authenticator::generate(&(), &[]),
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
                    challenge: authenticator_a::Authenticator::generate(&(), &[]),
                }),
                crate::Call::<Test>::add_session_key_with_filter {
                    session: SIGNER,
                    duration: None,
                    filter: remark_only_filter(),
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
                    challenge: authenticator_a::Authenticator::generate(&(), &[]),
                }),
                crate::Call::<Test>::add_session_key_with_filter {
                    session: OTHER,
                    duration: Some(7),
                    filter: remark_only_filter(),
                }
                .into(),
            ));

            run_to(20);

            assert!(!SessionKeys::<Test>::contains_key(OTHER));
        });
    }
}

mod device_filters {
    use super::*;
    use crate::filter::AssetSpendLimit;
    use crate::{DeviceFilters, DeviceOf, SessionKeys};
    use alloc::collections::BTreeSet;
    use frame_support::traits::ConstU32;

    fn setup_with_admin_device() -> sp_io::TestExternalities {
        prepare(AccountNameA::get())
    }

    #[test]
    fn first_device_gets_admin_filter() {
        setup_with_admin_device().execute_with(|| {
            let filter = DeviceFilters::<Test>::get(Address::get(), THE_DEVICE);
            assert_eq!(filter, Some(DeviceFilter::Admin));
        })
    }

    #[test]
    fn admin_can_add_device_with_any_filter() {
        setup_with_admin_device().execute_with(|| {
            assert_ok!(Balances::mint_into(
                &Address::get(),
                ExistentialDeposit::get()
                    + ItemStoragePrice::convert(Footprint::from_parts(
                        1,
                        DeviceOf::<Test>::max_encoded_len(),
                    ))
            ));

            let calls_filter = DeviceFilter::Calls(
                [(0u8, 1u8)]
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .try_into()
                    .unwrap(),
            );

            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_device_with_filter(
                RuntimeOrigin::signed(Address::get()),
                PassDeviceAttestation::AuthenticatorAAuthenticator(
                    authenticator_a::DeviceAttestation {
                        device_id: OTHER_DEVICE,
                        challenge: authenticator_a::Authenticator::generate(&(), &[]),
                    }
                ),
                calls_filter.clone(),
            ));

            assert_eq!(
                DeviceFilters::<Test>::get(Address::get(), OTHER_DEVICE),
                Some(calls_filter)
            );
        })
    }

    #[test]
    fn restricted_device_cannot_escalate_to_admin() {
        setup_with_admin_device().execute_with(|| {
            assert_ok!(Balances::mint_into(
                &Address::get(),
                ExistentialDeposit::get()
                    + ItemStoragePrice::convert(Footprint::from_parts(
                        2,
                        DeviceOf::<Test>::max_encoded_len(),
                    ))
            ));

            // Add a Calls-restricted device
            let calls_filter = DeviceFilter::Calls(
                [(0u8, 1u8)]
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .try_into()
                    .unwrap(),
            );
            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_device_with_filter(
                RuntimeOrigin::signed(Address::get()),
                PassDeviceAttestation::AuthenticatorAAuthenticator(
                    authenticator_a::DeviceAttestation {
                        device_id: OTHER_DEVICE,
                        challenge: authenticator_a::Authenticator::generate(&(), &[]),
                    }
                ),
                calls_filter,
            ));

            // Now try to use that restricted device to add an Admin device — must fail
            crate::AuthenticatedDevice::<Test>::put((Address::get(), OTHER_DEVICE));
            assert_noop!(
                Pass::add_device_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    PassDeviceAttestation::AuthenticatorAAuthenticator(
                        authenticator_a::DeviceAttestation {
                            device_id: THIRD_DEVICE,
                            challenge: authenticator_a::Authenticator::generate(&(), &[]),
                        }
                    ),
                    DeviceFilter::Admin, // trying to escalate
                ),
                Error::<Test>::PermissionEscalation,
            );
        })
    }

    #[test]
    fn restricted_device_can_add_subset_filter() {
        // Needs 3 devices total, but MaxDevicesPerAccount=2 in mock.
        // Use a fresh ext with higher limit or just test is_superset_of directly.
        setup_with_admin_device().execute_with(|| {
            // Test the superset logic directly since we can't add 3 devices in mock
            let pallets: DeviceFilter<(), u128, ConstU32<10>, ConstU32<5>> = DeviceFilter::Pallets(
                [0u8, 5, 10]
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .try_into()
                    .unwrap(),
            );
            let calls_subset: DeviceFilter<(), u128, ConstU32<10>, ConstU32<5>> =
                DeviceFilter::Calls(
                    [(0u8, 1u8), (5u8, 0u8)]
                        .into_iter()
                        .collect::<BTreeSet<_>>()
                        .try_into()
                        .unwrap(),
                );
            let calls_outside: DeviceFilter<(), u128, ConstU32<10>, ConstU32<5>> =
                DeviceFilter::Calls(
                    [(99u8, 0u8)]
                        .into_iter()
                        .collect::<BTreeSet<_>>()
                        .try_into()
                        .unwrap(),
                );

            assert!(pallets.is_superset_of(&calls_subset));
            assert!(!pallets.is_superset_of(&calls_outside));
            assert!(!pallets.is_superset_of(&DeviceFilter::Admin));
        })
    }

    #[test]
    fn spend_device_cannot_escalate_to_calls() {
        setup_with_admin_device().execute_with(|| {
            assert_ok!(Balances::mint_into(
                &Address::get(),
                ExistentialDeposit::get()
                    + ItemStoragePrice::convert(Footprint::from_parts(
                        2,
                        DeviceOf::<Test>::max_encoded_len(),
                    ))
            ));

            // Add a Spend-only device
            let spend_filter = DeviceFilter::Spend(
                vec![AssetSpendLimit {
                    asset: (),
                    max_amount: 1000u128,
                }]
                .try_into()
                .unwrap(),
            );
            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_device_with_filter(
                RuntimeOrigin::signed(Address::get()),
                PassDeviceAttestation::AuthenticatorAAuthenticator(
                    authenticator_a::DeviceAttestation {
                        device_id: OTHER_DEVICE,
                        challenge: authenticator_a::Authenticator::generate(&(), &[]),
                    }
                ),
                spend_filter,
            ));

            // Spend device tries to add a Calls device — must fail
            let calls_filter = DeviceFilter::Calls(
                [(0u8, 0u8)]
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .try_into()
                    .unwrap(),
            );
            crate::AuthenticatedDevice::<Test>::put((Address::get(), OTHER_DEVICE));
            assert_noop!(
                Pass::add_device_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    PassDeviceAttestation::AuthenticatorAAuthenticator(
                        authenticator_a::DeviceAttestation {
                            device_id: THIRD_DEVICE,
                            challenge: authenticator_a::Authenticator::generate(&(), &[]),
                        }
                    ),
                    calls_filter,
                ),
                Error::<Test>::PermissionEscalation,
            );
        })
    }

    #[test]
    fn spend_filter_superset_logic() {
        new_test_ext().execute_with(|| {
            type F = DeviceFilter<(), u128, ConstU32<10>, ConstU32<5>>;

            let spend_1000: F = DeviceFilter::Spend(
                vec![AssetSpendLimit {
                    asset: (),
                    max_amount: 1000u128,
                }]
                .try_into()
                .unwrap(),
            );
            let spend_500: F = DeviceFilter::Spend(
                vec![AssetSpendLimit {
                    asset: (),
                    max_amount: 500u128,
                }]
                .try_into()
                .unwrap(),
            );
            let spend_2000: F = DeviceFilter::Spend(
                vec![AssetSpendLimit {
                    asset: (),
                    max_amount: 2000u128,
                }]
                .try_into()
                .unwrap(),
            );
            // Can delegate with lower limit
            assert!(spend_1000.is_superset_of(&spend_500));
            // Cannot delegate with higher limit
            assert!(!spend_1000.is_superset_of(&spend_2000));
            // Admin can delegate any spend
            assert!(F::Admin.is_superset_of(&spend_1000));
            // Spend can't escalate to admin
            assert!(!spend_1000.is_superset_of(&F::Admin));
            // Calls cannot delegate to Spend (security fix)
            let calls: F = DeviceFilter::Calls(
                [(0u8, 1u8)]
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .try_into()
                    .unwrap(),
            );
            assert!(!calls.is_superset_of(&spend_1000));
        })
    }

    #[test]
    fn filter_is_removed_with_device() {
        setup_with_admin_device().execute_with(|| {
            assert!(DeviceFilters::<Test>::get(Address::get(), THE_DEVICE).is_some());

            assert_ok!(Pass::remove_device(
                RuntimeOrigin::signed(Address::get()),
                THE_DEVICE
            ));

            assert!(DeviceFilters::<Test>::get(Address::get(), THE_DEVICE).is_none());
        })
    }

    #[test]
    fn session_key_cannot_have_admin_filter() {
        setup_with_admin_device().execute_with(|| {
            assert_noop!(
                Pass::add_session_key_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    SIGNER,
                    Some(5),
                    DeviceFilter::Admin,
                ),
                Error::<Test>::PermissionEscalation,
            );
        })
    }

    #[test]
    fn stale_auth_from_different_account_is_ignored() {
        // Security test: if AuthenticatedDevice storage contains a stale
        // entry for account A, but the current call is from account B,
        // the escalation check must reject it (treat as not authenticated).
        setup_with_admin_device().execute_with(|| {
            // Simulate stale auth context from a DIFFERENT account
            let other_account: AccountId = AccountId::new([99u8; 32]);
            crate::AuthenticatedDevice::<Test>::put((other_account, THE_DEVICE));

            assert_noop!(
                Pass::add_session_key_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    SIGNER,
                    Some(5),
                    DeviceFilter::Calls(
                        [(0u8, 7u8)]
                            .into_iter()
                            .collect::<BTreeSet<_>>()
                            .try_into()
                            .unwrap(),
                    ),
                ),
                Error::<Test>::NotAuthenticatedByDevice,
            );
        })
    }

    #[test]
    fn session_key_stores_filter() {
        setup_with_admin_device().execute_with(|| {
            let filter = DeviceFilter::Calls(
                [(0u8, 7u8)]
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .try_into()
                    .unwrap(),
            );
            crate::AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_session_key_with_filter(
                RuntimeOrigin::signed(Address::get()),
                SIGNER,
                Some(5),
                filter.clone(),
            ));

            let (account, _, stored_filter) =
                SessionKeys::<Test>::get(SIGNER).expect("session exists");
            assert_eq!(account, Address::get());
            assert_eq!(stored_filter, filter);
        })
    }
}

mod extension_weights {
    use super::*;
    use crate::{AuthenticatedDevice, WeightInfo};
    use frame_support::{
        dispatch::{DispatchInfo, GetDispatchInfo},
        traits::OnInitialize,
        weights::Weight,
    };
    use sp_runtime::{
        traits::{DispatchTransaction, TransactionExtension, TxBaseImplication},
        transaction_validity::TransactionSource,
    };

    type Weights = <Test as crate::Config>::WeightInfo;

    parameter_types! {
        pub Call: RuntimeCall = RuntimeCall::System(frame_system::Call::remark {
            remark: b"Hello, world".to_vec()
        });
    }

    fn credential() -> PassCredential {
        PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
            user_id: AccountNameA::get(),
            challenge: authenticator_a::Authenticator::generate(&(), &[]),
        })
    }

    fn info_for(ext: &PassAuthenticate<Test>) -> DispatchInfo {
        let mut info = Call::get().get_dispatch_info();
        info.extension_weight = ext.weight(&Call::get());
        info
    }

    /// Runs `ext` through `validate`, `prepare` and `post_dispatch` (the call
    /// itself is not dispatched), and returns the resulting actual weight.
    fn actual_weight(ext: PassAuthenticate<Test>, origin: RuntimeOrigin) -> (Weight, Weight) {
        let info = info_for(&ext);
        let post_info = ext
            .test_run(origin, &Call::get(), &info, 0, 0, |_| Ok(().into()))
            .expect("valid transaction")
            .expect("successful dispatch");
        (
            info.total_weight(),
            post_info.actual_weight.expect("set by the extension"),
        )
    }

    #[test]
    fn weight_depends_on_whether_a_credential_is_given() {
        new_test_ext().execute_with(|| {
            let none = PassAuthenticate::<Test>::default().weight(&Call::get());
            let some =
                PassAuthenticate::<Test>::from(THE_DEVICE, credential()).weight(&Call::get());

            assert_eq!(none, Weights::authenticate_none());
            assert_eq!(
                some,
                Weights::authenticate().saturating_add(authenticator_a::CREDENTIAL_WEIGHT)
            );
            assert!(none.all_lt(some));
        })
    }

    #[test]
    fn signed_without_credential_charges_authenticate_none_without_refund() {
        prepare(AccountNameA::get()).execute_with(|| {
            let (charged, actual) =
                actual_weight(PassAuthenticate::default(), RuntimeOrigin::signed(CHARLIE));
            assert_eq!(
                charged,
                Call::get()
                    .get_dispatch_info()
                    .call_weight
                    .saturating_add(Weights::authenticate_none())
            );
            assert_eq!(actual, charged);
        })
    }

    #[test]
    fn unsigned_without_credential_refunds_the_session_key_lookup() {
        // An unsigned origin that reaches `PassAuthenticate` without a
        // credential never looks up `SessionKeys`. Such a transaction is only
        // valid if a later extension authorizes it, so the extension is driven
        // step by step here, rather than through `test_run`.
        prepare(AccountNameA::get()).execute_with(|| {
            let ext = PassAuthenticate::<Test>::default();
            let info = info_for(&ext);
            let (_, val, origin) = ext
                .validate(
                    RuntimeOrigin::none(),
                    &Call::get(),
                    &info,
                    0,
                    (),
                    &TxBaseImplication(()),
                    TransactionSource::External,
                )
                .expect("origin is passed through");
            let pre = ext
                .prepare(val, &origin, &Call::get(), &info, 0)
                .expect("prepare is infallible");

            let mut post_info = PostDispatchInfo {
                actual_weight: Some(info.total_weight()),
                ..Default::default()
            };
            assert_ok!(PassAuthenticate::<Test>::post_dispatch(
                pre,
                &info,
                &mut post_info,
                0,
                &Ok(())
            ));

            let refund = Weight::from_parts(
                <Test as frame_system::Config>::DbWeight::get()
                    .reads(1)
                    .ref_time(),
                Weights::authenticate_none().proof_size(),
            );
            assert!(refund.any_gt(Weight::zero()));
            assert_eq!(
                post_info.actual_weight,
                Some(info.total_weight().saturating_sub(refund))
            );
        })
    }

    #[test]
    fn with_credential_charges_authenticate_and_clears_the_context() {
        prepare(AccountNameA::get()).execute_with(|| {
            let (charged, actual) = actual_weight(
                PassAuthenticate::from(THE_DEVICE, credential()),
                RuntimeOrigin::none(),
            );
            assert_eq!(
                charged,
                Call::get()
                    .get_dispatch_info()
                    .call_weight
                    .saturating_add(Weights::authenticate())
                    .saturating_add(authenticator_a::CREDENTIAL_WEIGHT)
            );
            assert_eq!(actual, charged);
            assert_eq!(AuthenticatedDevice::<Test>::get(), None);
        })
    }

    #[test]
    fn without_credential_does_not_touch_the_authenticated_device() {
        prepare(AccountNameA::get()).execute_with(|| {
            // Any value there is left alone: the path without a credential
            // performs no writes.
            AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            actual_weight(PassAuthenticate::default(), RuntimeOrigin::signed(CHARLIE));
            assert_eq!(
                AuthenticatedDevice::<Test>::get(),
                Some((Address::get(), THE_DEVICE))
            );
        })
    }

    #[test]
    fn on_initialize_clears_a_leaked_authenticated_device() {
        prepare(AccountNameA::get()).execute_with(|| {
            AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));

            let weight = Pass::on_initialize(System::block_number() + 1);

            assert_eq!(AuthenticatedDevice::<Test>::get(), None);
            assert_eq!(
                weight,
                <Test as frame_system::Config>::DbWeight::get().writes(1)
            );
        })
    }

    #[test]
    fn a_leaked_context_cannot_be_used_by_a_session_key_in_a_later_block() {
        // Transactions without a credential don't clear `AuthenticatedDevice`.
        // Should a value ever leak past its transaction, a later transaction
        // from a session key of the same account would pass the account check
        // in `check_no_escalation`, and could add an `Admin` device. The
        // per-block clear in `on_initialize` is what stops that.
        prepare(AccountNameA::get()).execute_with(|| {
            assert_ok!(Balances::mint_into(&Address::get(), Balance::MAX / 2));

            // A session key allowed to call anything in `pallet-pass`.
            AuthenticatedDevice::<Test>::put((Address::get(), THE_DEVICE));
            assert_ok!(Pass::add_session_key_with_filter(
                RuntimeOrigin::signed(Address::get()),
                OTHER,
                None,
                DeviceFilter::Pallets(
                    [11u8]
                        .into_iter()
                        .collect::<alloc::collections::BTreeSet<_>>()
                        .try_into()
                        .unwrap()
                ),
            ));
            // ...and the admin device's context left behind.
            assert_eq!(
                AuthenticatedDevice::<Test>::get(),
                Some((Address::get(), THE_DEVICE))
            );

            let n = System::block_number() + 1;
            System::set_block_number(n);
            Pass::on_initialize(n);

            let call: RuntimeCall = crate::Call::<Test>::add_device_with_filter {
                attestation: PassDeviceAttestation::AuthenticatorAAuthenticator(
                    authenticator_a::DeviceAttestation {
                        device_id: OTHER_DEVICE,
                        challenge: authenticator_a::Authenticator::generate(&(), &[]),
                    },
                ),
                filter: DeviceFilter::Admin,
            }
            .into();
            let xt = CheckedExtrinsic {
                format: ExtrinsicFormat::Signed(
                    OTHER,
                    (
                        PassAuthenticate::<Test>::default(),
                        pallet_transaction_payment::ChargeTransactionPayment::from(0),
                    ),
                ),
                function: call.clone(),
            };
            let result = xt
                .apply::<Test>(&call.get_dispatch_info(), call.encoded_size())
                .expect("the session key's filter allows the call");

            assert_eq!(
                result.map_err(|e| e.error),
                Err(Error::<Test>::NotAuthenticatedByDevice.into())
            );
            assert!(!crate::Devices::<Test>::contains_key(
                Address::get(),
                OTHER_DEVICE
            ));
        })
    }
}

mod authenticator_weights {
    use super::*;
    use crate::WeightInfo;
    use frame_support::dispatch::GetDispatchInfo;
    use sp_runtime::traits::TransactionExtension;

    type Weights = <Test as crate::Config>::WeightInfo;

    fn attestation_a() -> PassDeviceAttestation {
        PassDeviceAttestation::AuthenticatorAAuthenticator(authenticator_a::DeviceAttestation {
            device_id: THE_DEVICE,
            challenge: [0; 32],
        })
    }

    fn attestation_b() -> PassDeviceAttestation {
        PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
            device_id: THE_DEVICE,
            context: 0,
            challenge: [0; 32],
        })
    }

    fn call_weight(call: crate::Call<Test>) -> frame_support::weights::Weight {
        RuntimeCall::from(call).get_dispatch_info().call_weight
    }

    #[test]
    fn register_charges_what_the_authenticator_reports() {
        new_test_ext().execute_with(|| {
            assert_eq!(
                call_weight(crate::Call::register {
                    user: AccountNameA::get(),
                    attestation: attestation_a(),
                }),
                Weights::register().saturating_add(authenticator_a::ATTESTATION_WEIGHT)
            );
            // Authenticator B does not report a weight: it defaults to zero.
            assert_eq!(
                call_weight(crate::Call::register {
                    user: AccountNameA::get(),
                    attestation: attestation_b(),
                }),
                Weights::register()
            );
        })
    }

    #[test]
    fn add_device_with_filter_charges_what_the_authenticator_reports() {
        new_test_ext().execute_with(|| {
            assert_eq!(
                call_weight(crate::Call::add_device_with_filter {
                    attestation: attestation_a(),
                    filter: DeviceFilter::Admin,
                }),
                Weights::add_device_with_filter()
                    .saturating_add(authenticator_a::ATTESTATION_WEIGHT)
            );
            assert_eq!(
                call_weight(crate::Call::add_device_with_filter {
                    attestation: attestation_b(),
                    filter: DeviceFilter::Admin,
                }),
                Weights::add_device_with_filter()
            );
        })
    }

    #[test]
    fn pass_authenticate_charges_what_the_authenticator_reports() {
        new_test_ext().execute_with(|| {
            let call: RuntimeCall = frame_system::Call::remark { remark: vec![] }.into();

            let with_a = PassAuthenticate::<Test>::from(
                THE_DEVICE,
                PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
                    user_id: AccountNameA::get(),
                    challenge: [0; 32],
                }),
            );
            assert_eq!(
                with_a.weight(&call),
                Weights::authenticate().saturating_add(authenticator_a::CREDENTIAL_WEIGHT)
            );

            let with_b = PassAuthenticate::<Test>::from(
                THE_DEVICE,
                PassCredential::AuthenticatorB(authenticator_b::Credential::new(
                    AccountNameA::get(),
                    0,
                    0,
                    [0; 32],
                )),
            );
            assert_eq!(with_b.weight(&call), Weights::authenticate());
        })
    }
}

fn attestation(device_id: DeviceId) -> PassDeviceAttestation {
    PassDeviceAttestation::AuthenticatorAAuthenticator(authenticator_a::DeviceAttestation {
        device_id,
        challenge: authenticator_a::Authenticator::generate(&(), &[]),
    })
}

fn credential(user_id: HashedUserId) -> PassCredential {
    PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
        user_id,
        challenge: authenticator_a::Authenticator::generate(&(), &[]),
    })
}

fn pallets_filter(pallets: &[u8]) -> crate::DeviceFilterOf<Test> {
    DeviceFilter::Pallets(
        pallets
            .iter()
            .copied()
            .collect::<alloc::collections::BTreeSet<_>>()
            .try_into()
            .unwrap(),
    )
}

/// The index of the Pass pallet in the mock runtime.
const PASS_PALLET: u8 = 11;

/// Gives `Address` enough to pay fees and the deposit of the devices it adds.
fn fund_address() {
    assert_ok!(Balances::mint_into(&Address::get(), 1_000_000_000));
}

fn remark() -> RuntimeCall {
    RuntimeCall::System(frame_system::Call::remark_with_event {
        remark: b"Hello, world".to_vec(),
    })
}

/// Makes `device_id` the device that authenticated the current transaction, as
/// `PassAuthenticate` does.
fn authenticated_as(device_id: DeviceId) {
    crate::AuthenticatedDevice::<Test>::put((Address::get(), device_id));
}

/// The amendment that brings back the unfiltered calls, with the same indices they had in 2.x.
#[allow(deprecated)]
mod legacy_calls {
    use super::*;
    use crate::{DeviceFilters, SessionKeys};

    // Call indices keep their 2.x meaning: 1 add_device(attestation), 3 add_session_key(session,
    // duration). The filtered ones are 5 and 6.
    #[test]
    fn call_indices_and_arguments_are_those_of_2x() {
        new_test_ext().execute_with(|| {
            let encoded = |call: crate::Call<Test>| RuntimeCall::from(call).encode();

            assert_eq!(
                encoded(crate::Call::add_device {
                    attestation: attestation(OTHER_DEVICE)
                }),
                [&[PASS_PALLET, 1][..], &attestation(OTHER_DEVICE).encode()].concat()
            );
            assert_eq!(
                encoded(crate::Call::add_session_key {
                    session: OTHER,
                    duration: Some(3)
                }),
                [&[PASS_PALLET, 3][..], &OTHER.encode(), &Some(3u64).encode()].concat()
            );
            assert_eq!(
                encoded(crate::Call::add_device_with_filter {
                    attestation: attestation(OTHER_DEVICE),
                    filter: DeviceFilter::Admin
                })[..2],
                [PASS_PALLET, 5]
            );
            assert_eq!(
                encoded(crate::Call::add_session_key_with_filter {
                    session: OTHER,
                    duration: None,
                    filter: DeviceFilter::Admin
                })[..2],
                [PASS_PALLET, 6]
            );
        })
    }

    #[test]
    fn add_session_key_is_marked_deprecated_in_the_metadata() {
        let calls = Pass::call_functions().deprecation_info;

        assert!(calls.is_variant_deprecated(3));
        for index in [0, 1, 2, 4, 5, 6] {
            assert!(!calls.is_variant_deprecated(index), "call {index}");
        }
    }

    mod add_device {
        use super::*;

        #[test]
        fn stores_an_admin_filter() {
            prepare(AccountNameA::get()).execute_with(|| {
                fund_address();
                authenticated_as(THE_DEVICE);
                assert_ok!(Pass::add_device(
                    RuntimeOrigin::signed(Address::get()),
                    attestation(OTHER_DEVICE)
                ));

                assert_eq!(
                    DeviceFilters::<Test>::get(Address::get(), OTHER_DEVICE),
                    Some(DeviceFilter::Admin)
                );
                System::assert_has_event(
                    Event::<Test>::DeviceAdded {
                        who: Address::get(),
                        device_id: OTHER_DEVICE,
                    }
                    .into(),
                );
            })
        }

        #[test]
        fn fails_if_not_a_pass_account() {
            prepare(AccountNameA::get()).execute_with(|| {
                authenticated_as(THE_DEVICE);
                for origin in [RuntimeOrigin::root(), RuntimeOrigin::signed(OTHER)] {
                    assert_noop!(
                        Pass::add_device(origin, attestation(OTHER_DEVICE)),
                        DispatchError::BadOrigin
                    );
                }
            })
        }

        // 2.x only asked for a Pass account as signer. Doing the same now would let any device
        // whose filter allows the Pass pallet create an Admin device, so the call needs an
        // Admin device.
        #[test]
        fn needs_an_admin_device() {
            prepare(AccountNameA::get()).execute_with(|| {
                fund_address();

                // No device authenticated the transaction.
                assert_noop!(
                    Pass::add_device(
                        RuntimeOrigin::signed(Address::get()),
                        attestation(OTHER_DEVICE)
                    ),
                    Error::<Test>::NotAuthenticatedByDevice
                );

                // A device that is allowed to call the pallet, but is not Admin.
                authenticated_as(THE_DEVICE);
                assert_ok!(Pass::add_device_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    attestation(OTHER_DEVICE),
                    pallets_filter(&[PASS_PALLET]),
                ));
                authenticated_as(OTHER_DEVICE);
                assert_noop!(
                    Pass::add_device(
                        RuntimeOrigin::signed(Address::get()),
                        attestation(THIRD_DEVICE)
                    ),
                    Error::<Test>::PermissionEscalation
                );
            })
        }

        #[test]
        fn an_admin_device_can_call_it_through_the_extension() {
            prepare(AccountNameA::get()).execute_with(|| {
                fund_address();
                let call = crate::Call::<Test>::add_device {
                    attestation: attestation(OTHER_DEVICE),
                };

                assert_ok!(
                    authenticate(THE_DEVICE, credential(AccountNameA::get()), call.into()).unwrap()
                );
                assert_eq!(
                    DeviceFilters::<Test>::get(Address::get(), OTHER_DEVICE),
                    Some(DeviceFilter::Admin)
                );
            })
        }

        #[test]
        fn a_session_key_cannot_call_it() {
            prepare(AccountNameA::get()).execute_with(|| {
                fund_address();
                // The session key is allowed to call the Pass pallet.
                authenticated_as(THE_DEVICE);
                assert_ok!(Pass::add_session_key_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    OTHER,
                    None,
                    pallets_filter(&[PASS_PALLET]),
                ));
                crate::AuthenticatedDevice::<Test>::kill();

                let call = crate::Call::<Test>::add_device {
                    attestation: attestation(OTHER_DEVICE),
                };
                assert_eq!(
                    signed(OTHER, call.into()).unwrap().unwrap_err().error,
                    Error::<Test>::NotAuthenticatedByDevice.into()
                );
                assert!(DeviceFilters::<Test>::get(Address::get(), OTHER_DEVICE).is_none());
            })
        }

        #[test]
        fn a_restricted_device_cannot_call_it_through_the_extension() {
            prepare(AccountNameA::get()).execute_with(|| {
                fund_address();
                authenticated_as(THE_DEVICE);
                assert_ok!(Pass::add_device_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    attestation(OTHER_DEVICE),
                    pallets_filter(&[PASS_PALLET]),
                ));
                crate::AuthenticatedDevice::<Test>::kill();

                let call = crate::Call::<Test>::add_device {
                    attestation: attestation(THIRD_DEVICE),
                };
                assert_eq!(
                    authenticate(OTHER_DEVICE, credential(AccountNameA::get()), call.into())
                        .unwrap()
                        .unwrap_err()
                        .error,
                    Error::<Test>::PermissionEscalation.into()
                );
            })
        }

        #[test]
        fn charges_what_the_authenticator_reports() {
            new_test_ext().execute_with(|| {
                assert_eq!(
                    call_weight(crate::Call::<Test>::add_device {
                        attestation: attestation(OTHER_DEVICE),
                    }),
                    <() as crate::WeightInfo>::add_device()
                        .saturating_add(authenticator_a::ATTESTATION_WEIGHT)
                );
            })
        }

        fn call_weight(call: crate::Call<Test>) -> frame_support::weights::Weight {
            RuntimeCall::from(call).get_dispatch_info().call_weight
        }
    }

    mod add_session_key {
        use super::*;

        #[test]
        fn stores_an_admin_filter() {
            prepare(AccountNameA::get()).execute_with(|| {
                authenticated_as(THE_DEVICE);
                assert_ok!(Pass::add_session_key(
                    RuntimeOrigin::signed(Address::get()),
                    OTHER,
                    Some(DURATION),
                ));

                assert_eq!(
                    SessionKeys::<Test>::get(OTHER),
                    Some((Address::get(), DURATION, DeviceFilter::Admin))
                );
                assert_eq!(SessionKeys::<Test>::count(), 1);
                System::assert_has_event(
                    Event::<Test>::SessionCreated {
                        session_key_hash: sp_core::H256(sp_io::hashing::blake2_256(
                            &OTHER.encode(),
                        )),
                        until: DURATION,
                    }
                    .into(),
                );
            })
        }

        #[test]
        fn the_session_key_is_unrestricted() {
            prepare(AccountNameA::get()).execute_with(|| {
                fund_address();
                authenticated_as(THE_DEVICE);
                assert_ok!(Pass::add_session_key(
                    RuntimeOrigin::signed(Address::get()),
                    OTHER,
                    None,
                ));
                crate::AuthenticatedDevice::<Test>::kill();

                assert_ok!(signed(OTHER, remark()).unwrap());
            })
        }

        #[test]
        fn keeps_the_rules_of_any_other_session() {
            prepare(AccountNameA::get()).execute_with(|| {
                authenticated_as(THE_DEVICE);
                assert_ok!(Balances::mint_into(&CHARLIE, ExistentialDeposit::get()));
                assert_noop!(
                    Pass::add_session_key(RuntimeOrigin::signed(Address::get()), CHARLIE, None),
                    Error::<Test>::AccountForSessionKeyAlreadyExists
                );

                assert_ok!(Pass::add_session_key(
                    RuntimeOrigin::signed(Address::get()),
                    SIGNER,
                    None
                ));
                assert_ok!(Pass::add_session_key(
                    RuntimeOrigin::signed(Address::get()),
                    OTHER,
                    None
                ));
                assert_noop!(
                    Pass::add_session_key(RuntimeOrigin::signed(Address::get()), THIRD_KEY, None),
                    Error::<Test>::MaxSessionsExceeded
                );
            })
        }

        #[test]
        fn fails_if_not_a_pass_account() {
            prepare(AccountNameA::get()).execute_with(|| {
                authenticated_as(THE_DEVICE);
                for origin in [RuntimeOrigin::root(), RuntimeOrigin::signed(OTHER)] {
                    assert_noop!(
                        Pass::add_session_key(origin, OTHER, None),
                        DispatchError::BadOrigin
                    );
                }
            })
        }

        // See `add_device::needs_an_admin_device`.
        #[test]
        fn needs_an_admin_device() {
            prepare(AccountNameA::get()).execute_with(|| {
                fund_address();
                assert_noop!(
                    Pass::add_session_key(RuntimeOrigin::signed(Address::get()), OTHER, None),
                    Error::<Test>::NotAuthenticatedByDevice
                );

                authenticated_as(THE_DEVICE);
                assert_ok!(Pass::add_device_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    attestation(OTHER_DEVICE),
                    pallets_filter(&[PASS_PALLET]),
                ));
                authenticated_as(OTHER_DEVICE);
                assert_noop!(
                    Pass::add_session_key(RuntimeOrigin::signed(Address::get()), OTHER, None),
                    Error::<Test>::PermissionEscalation
                );
            })
        }

        #[test]
        fn a_session_key_cannot_call_it() {
            prepare(AccountNameA::get()).execute_with(|| {
                fund_address();
                authenticated_as(THE_DEVICE);
                assert_ok!(Pass::add_session_key_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    OTHER,
                    None,
                    pallets_filter(&[PASS_PALLET]),
                ));
                crate::AuthenticatedDevice::<Test>::kill();

                let call = crate::Call::<Test>::add_session_key {
                    session: SIGNER,
                    duration: None,
                };
                assert_eq!(
                    signed(OTHER, call.into()).unwrap().unwrap_err().error,
                    Error::<Test>::NotAuthenticatedByDevice.into()
                );
                assert!(SessionKeys::<Test>::get(SIGNER).is_none());
            })
        }

        #[test]
        fn can_be_disabled_by_the_runtime() {
            prepare(AccountNameA::get()).execute_with(|| {
                authenticated_as(THE_DEVICE);
                UnrestrictedSessionKeysEnabled::set(false);

                assert_noop!(
                    Pass::add_session_key(RuntimeOrigin::signed(Address::get()), OTHER, None),
                    Error::<Test>::UnrestrictedSessionKeysDisabled
                );

                // Whatever the origin: the call is off.
                assert_noop!(
                    Pass::add_session_key(RuntimeOrigin::root(), OTHER, None),
                    Error::<Test>::UnrestrictedSessionKeysDisabled
                );

                // Sessions with a filter are not affected.
                assert_ok!(Pass::add_session_key_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    OTHER,
                    None,
                    remark_only_filter(),
                ));

                UnrestrictedSessionKeysEnabled::set(true);
                assert_ok!(Pass::add_session_key(
                    RuntimeOrigin::signed(Address::get()),
                    SIGNER,
                    None
                ));
            })
        }
    }

    const THIRD_KEY: AccountId = AccountId::new([13u8; 32]);
}

/// The upgrade from storage version 0.
mod upgrade {
    use super::*;
    use crate::{
        migrations::{Cursor, CursorKey, MigrateV0ToV1, Phase},
        DeviceFilters, Devices, SessionKeys, WeightInfo,
    };
    use frame_support::{
        migrations::{SteppedMigration, SteppedMigrationError},
        storage::unhashed,
        traits::{GetStorageVersion, StorageVersion},
        weights::{Weight, WeightMeter},
    };

    fn device_unit() -> Weight {
        <() as WeightInfo>::migrate_device_filter()
    }

    fn session_key_unit() -> Weight {
        <() as WeightInfo>::migrate_session_key()
    }

    /// A meter that fits `units` of `unit`, and nothing else.
    fn fits(units: u64, unit: Weight) -> WeightMeter {
        WeightMeter::with_limit(unit.saturating_mul(units))
    }

    fn step(cursor: Option<Cursor>, mut meter: WeightMeter) -> Option<Cursor> {
        MigrateV0ToV1::<Test>::step(cursor, &mut meter).expect("the unit fits")
    }

    /// Runs the whole migration at once.
    fn migrate() {
        assert_eq!(step(None, WeightMeter::new()), None);
    }

    /// Storage as it was before this upgrade: devices have no `DeviceFilters` entry.
    fn forget_filters() {
        let _ = DeviceFilters::<Test>::clear(u32::MAX, None);
        StorageVersion::new(0).put::<Pass>();
    }

    /// Rewrites a session key as it was stored before this upgrade: `(account, until)`.
    fn make_old_session_key(key: &AccountId) {
        let (account, until, _) = SessionKeys::<Test>::get(key).expect("the session exists");
        unhashed::put_raw(
            &SessionKeys::<Test>::hashed_key_for(key),
            &(account, until).encode(),
        );
    }

    fn is_old_shape(key: &AccountId) -> bool {
        SessionKeys::<Test>::get(key).is_none()
            && unhashed::get_raw(&SessionKeys::<Test>::hashed_key_for(key)).is_some()
    }

    fn session_key_names(n: u8) -> Vec<AccountId> {
        (0..n).map(|i| AccountId::new([50 + i; 32])).collect()
    }

    /// Registers `n` accounts, each with one `Admin` device.
    fn register_accounts(n: u8) -> Vec<(AccountId, DeviceId)> {
        (0..n)
            .map(|i| {
                let user = <Test as frame_system::Config>::Hashing::hash(&[i]).0;
                let device_id = [100 + i; 32];
                assert_ok!(Pass::register(
                    RuntimeOrigin::root(),
                    user,
                    attestation(device_id)
                ));
                (Pass::address_for(user), device_id)
            })
            .collect()
    }

    // The lockout that the migration exists to fix: a device created before the upgrade has
    // no filter, and the extension denies a device without one.
    #[test]
    fn a_device_created_before_the_upgrade_cannot_sign_until_it_is_migrated() {
        prepare(AccountNameA::get()).execute_with(|| {
            fund_address();
            forget_filters();
            assert!(Devices::<Test>::contains_key(Address::get(), THE_DEVICE));
            assert!(DeviceFilters::<Test>::get(Address::get(), THE_DEVICE).is_none());

            let result = authenticate(THE_DEVICE, credential(AccountNameA::get()), remark());
            assert!(
                matches!(
                    result,
                    Err(
                        sp_runtime::transaction_validity::TransactionValidityError::Invalid(
                            sp_runtime::transaction_validity::InvalidTransaction::Call
                        )
                    )
                ),
                "{result:?}"
            );

            migrate();

            assert_ok!(
                authenticate(THE_DEVICE, credential(AccountNameA::get()), remark()).unwrap()
            );
            System::assert_has_event(
                frame_system::Event::Remarked {
                    sender: Address::get(),
                    hash: <Test as frame_system::Config>::Hashing::hash(b"Hello, world"),
                }
                .into(),
            );
        })
    }

    #[test]
    fn a_migrated_device_is_admin_and_can_manage_devices() {
        prepare(AccountNameA::get()).execute_with(|| {
            fund_address();
            forget_filters();
            migrate();

            assert_eq!(
                DeviceFilters::<Test>::get(Address::get(), THE_DEVICE),
                Some(DeviceFilter::Admin)
            );
            // It can add a restricted device through the extension.
            let call = crate::Call::<Test>::add_device_with_filter {
                attestation: attestation(OTHER_DEVICE),
                filter: remark_only_filter(),
            };
            assert_ok!(
                authenticate(THE_DEVICE, credential(AccountNameA::get()), call.into()).unwrap()
            );
        })
    }

    #[test]
    fn backfills_devices_without_a_filter_and_keeps_the_others() {
        new_test_ext().execute_with(|| {
            let accounts = register_accounts(2);

            // Account A has a second device, which has a filter.
            let (a, a_device) = accounts[0].clone();
            assert_ok!(Balances::mint_into(&a, 1_000_000));
            crate::AuthenticatedDevice::<Test>::put((a.clone(), a_device));
            assert_ok!(Pass::add_device_with_filter(
                RuntimeOrigin::signed(a.clone()),
                attestation([7u8; 32]),
                remark_only_filter(),
            ));

            // Before the upgrade, there were no filters at all, except this one.
            let restricted = DeviceFilters::<Test>::get(&a, [7u8; 32]);
            DeviceFilters::<Test>::remove(&a, a_device);
            DeviceFilters::<Test>::remove(accounts[1].0.clone(), accounts[1].1);
            StorageVersion::new(0).put::<Pass>();

            migrate();

            assert_eq!(
                DeviceFilters::<Test>::get(&a, a_device),
                Some(DeviceFilter::Admin)
            );
            assert_eq!(
                DeviceFilters::<Test>::get(&accounts[1].0, accounts[1].1),
                Some(DeviceFilter::Admin)
            );
            assert_eq!(DeviceFilters::<Test>::get(&a, [7u8; 32]), restricted);
            assert_eq!(DeviceFilters::<Test>::iter().count(), 3);
            assert_eq!(Devices::<Test>::iter().count(), 3);
            assert_eq!(Pass::on_chain_storage_version(), StorageVersion::new(1));
        })
    }

    #[test]
    fn converts_session_keys_to_admin_and_keeps_the_new_ones() {
        prepare(AccountNameA::get()).execute_with(|| {
            let [old, new] = [OTHER, SIGNER];
            authenticated_as(THE_DEVICE);
            for key in [old.clone(), new.clone()] {
                assert_ok!(Pass::add_session_key_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    key,
                    Some(DURATION),
                    remark_only_filter(),
                ));
            }
            // Only `old` was created before the upgrade.
            make_old_session_key(&old);
            StorageVersion::new(0).put::<Pass>();
            let counter = SessionKeys::<Test>::count();
            let sessions = crate::AccountSessionsCount::<Test>::get(Address::get());

            migrate();

            assert_eq!(
                SessionKeys::<Test>::get(&old),
                Some((Address::get(), DURATION, DeviceFilter::Admin))
            );
            // A value in the new shape starts with the old one: it must keep its filter.
            assert_eq!(
                SessionKeys::<Test>::get(&new),
                Some((Address::get(), DURATION, remark_only_filter()))
            );
            assert_eq!(SessionKeys::<Test>::count(), counter);
            assert_eq!(SessionKeys::<Test>::iter().count() as u32, counter);
            assert_eq!(
                crate::AccountSessionsCount::<Test>::get(Address::get()),
                sessions
            );
        })
    }

    #[test]
    fn a_session_key_that_cannot_be_read_is_skipped() {
        prepare(AccountNameA::get()).execute_with(|| {
            authenticated_as(THE_DEVICE);
            for key in [OTHER, SIGNER] {
                assert_ok!(Pass::add_session_key_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    key.clone(),
                    Some(DURATION),
                    remark_only_filter(),
                ));
                make_old_session_key(&key);
            }
            let garbage = vec![1u8, 2, 3];
            unhashed::put_raw(&SessionKeys::<Test>::hashed_key_for(OTHER), &garbage);

            // Neither an error nor a stop.
            migrate();

            assert_eq!(
                unhashed::get_raw(&SessionKeys::<Test>::hashed_key_for(OTHER)),
                Some(garbage)
            );
            assert_eq!(
                SessionKeys::<Test>::get(SIGNER),
                Some((Address::get(), DURATION, DeviceFilter::Admin))
            );
            assert_eq!(Pass::on_chain_storage_version(), StorageVersion::new(1));
        })
    }

    #[test]
    fn a_step_that_does_not_fit_yields_the_cursor_and_resumes() {
        new_test_ext().execute_with(|| {
            register_accounts(3);
            forget_filters();
            let keys = Devices::<Test>::iter_keys()
                .map(|(account, device)| Devices::<Test>::hashed_key_for(account, device))
                .collect::<Vec<_>>();
            assert_eq!(keys.len(), 3);

            // Two devices fit: it stops at the second one.
            let cursor = step(None, fits(2, device_unit()));
            assert_eq!(
                cursor,
                Some((
                    Phase::Devices,
                    CursorKey::try_from(keys[1].clone()).unwrap()
                ))
            );
            assert_eq!(DeviceFilters::<Test>::iter().count(), 2);
            assert_eq!(Pass::on_chain_storage_version(), StorageVersion::new(0));

            // It resumes from the cursor, with the third one, and finishes.
            assert_eq!(step(cursor, WeightMeter::new()), None);
            assert_eq!(DeviceFilters::<Test>::iter().count(), 3);
            assert!(DeviceFilters::<Test>::iter().all(|(_, _, f)| f == DeviceFilter::Admin));
            assert_eq!(Pass::on_chain_storage_version(), StorageVersion::new(1));
        })
    }

    #[test]
    fn it_can_be_stopped_after_every_single_unit() {
        new_test_ext().execute_with(|| {
            register_accounts(3);
            forget_filters();
            authenticated_as_nobody();
            for key in session_key_names(3) {
                SessionKeys::<Test>::insert(&key, (Address::get(), 5u64, DeviceFilter::Admin));
                make_old_session_key(&key);
            }

            // One unit per step, the worst case for a migration that loses its progress.
            let (mut cursor, mut steps) = (None, 0);
            loop {
                let unit = match cursor {
                    Some((Phase::SessionKeys, _)) => session_key_unit(),
                    _ => device_unit(),
                };
                cursor = step(cursor, fits(1, unit));
                steps += 1;
                assert!(steps < 100, "it does not finish");
                if cursor.is_none() {
                    break;
                }
            }

            assert_eq!(DeviceFilters::<Test>::iter().count(), 3);
            assert!(session_key_names(3).iter().all(|k| !is_old_shape(k)));
            assert_eq!(SessionKeys::<Test>::count(), 3);
            // 3 devices + the end of that phase, 3 session keys + the end of that phase.
            assert_eq!(steps, 8);
        })
    }

    fn authenticated_as_nobody() {
        crate::AuthenticatedDevice::<Test>::kill();
    }

    #[test]
    fn a_phase_can_end_exactly_where_a_step_does() {
        new_test_ext().execute_with(|| {
            register_accounts(3);
            forget_filters();

            // 3 devices, and the unit that finds there are no more.
            let cursor = step(None, fits(4, device_unit()));
            assert_eq!(cursor, Some((Phase::SessionKeys, CursorKey::default())));
            assert_eq!(DeviceFilters::<Test>::iter().count(), 3);

            assert_eq!(step(cursor, WeightMeter::new()), None);
        })
    }

    #[test]
    fn session_keys_resume_from_the_cursor() {
        prepare(AccountNameA::get()).execute_with(|| {
            let names = session_key_names(3);
            for key in &names {
                SessionKeys::<Test>::insert(key, (Address::get(), 5u64, DeviceFilter::Admin));
                make_old_session_key(key);
            }
            let raw = |key: &AccountId| {
                CursorKey::try_from(SessionKeys::<Test>::hashed_key_for(key)).unwrap()
            };
            let in_order = SessionKeys::<Test>::iter_keys().collect::<Vec<_>>();

            let start = Some((Phase::SessionKeys, CursorKey::default()));
            let cursor = step(start, fits(2, session_key_unit()));

            assert_eq!(cursor, Some((Phase::SessionKeys, raw(&in_order[1]))));
            assert!(!is_old_shape(&in_order[0]) && !is_old_shape(&in_order[1]));
            assert!(is_old_shape(&in_order[2]));

            assert_eq!(step(cursor, WeightMeter::new()), None);
            assert!(!is_old_shape(&in_order[2]));
            assert_eq!(SessionKeys::<Test>::count(), 3);
        })
    }

    #[test]
    fn fails_only_if_not_even_one_unit_fits_an_empty_meter() {
        new_test_ext().execute_with(|| {
            register_accounts(1);
            forget_filters();
            let unit = device_unit();

            // An empty meter that is too small for one unit: waiting will not help.
            let mut too_small = WeightMeter::with_limit(Weight::from_parts(
                unit.ref_time() - 1,
                unit.proof_size(),
            ));
            assert_eq!(
                MigrateV0ToV1::<Test>::step(None, &mut too_small),
                Err(SteppedMigrationError::InsufficientWeight { required: unit })
            );
            let mut too_small = WeightMeter::with_limit(Weight::from_parts(
                unit.ref_time(),
                unit.proof_size() - 1,
            ));
            assert_eq!(
                MigrateV0ToV1::<Test>::step(None, &mut too_small),
                Err(SteppedMigrationError::InsufficientWeight { required: unit })
            );
            assert_eq!(DeviceFilters::<Test>::iter().count(), 0);

            // A meter that other migrations have used up is not an error: it yields, and the
            // next block starts with an empty one.
            let mut used = fits(10, unit);
            assert_ok!(used.try_consume(unit.saturating_mul(10)));
            assert_eq!(
                MigrateV0ToV1::<Test>::step(None, &mut used),
                Ok(Some((Phase::Devices, CursorKey::default())))
            );
            let mut partly_used = fits(10, unit);
            assert_ok!(partly_used.try_consume(unit.saturating_mul(10).saturating_sub(unit.div(2))));
            assert_eq!(
                MigrateV0ToV1::<Test>::step(None, &mut partly_used),
                Ok(Some((Phase::Devices, CursorKey::default())))
            );
            assert_eq!(DeviceFilters::<Test>::iter().count(), 0);

            // It does the same from the middle.
            let cursor = step(None, fits(1, unit));
            let mut used = fits(10, unit);
            assert_ok!(used.try_consume(unit.saturating_mul(10)));
            assert_eq!(
                MigrateV0ToV1::<Test>::step(cursor.clone(), &mut used),
                Ok(cursor)
            );
        })
    }

    #[test]
    fn running_it_again_changes_nothing() {
        prepare(AccountNameA::get()).execute_with(|| {
            fund_address();
            authenticated_as(THE_DEVICE);
            assert_ok!(Pass::add_device_with_filter(
                RuntimeOrigin::signed(Address::get()),
                attestation(OTHER_DEVICE),
                remark_only_filter(),
            ));
            for key in [OTHER, SIGNER] {
                assert_ok!(Pass::add_session_key_with_filter(
                    RuntimeOrigin::signed(Address::get()),
                    key,
                    Some(DURATION),
                    remark_only_filter(),
                ));
            }
            make_old_session_key(&OTHER);
            DeviceFilters::<Test>::remove(Address::get(), THE_DEVICE);
            StorageVersion::new(0).put::<Pass>();

            migrate();
            let once = sp_io::storage::root(sp_runtime::StateVersion::V1);

            StorageVersion::new(0).put::<Pass>();
            migrate();
            migrate();
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), once);
            // And what it did not touch is still there.
            assert_eq!(
                SessionKeys::<Test>::get(SIGNER),
                Some((Address::get(), DURATION, remark_only_filter()))
            );
            assert_eq!(
                DeviceFilters::<Test>::get(Address::get(), OTHER_DEVICE),
                Some(remark_only_filter())
            );
        })
    }

    #[test]
    fn an_empty_chain_just_gets_the_new_version() {
        new_test_ext().execute_with(|| {
            StorageVersion::new(0).put::<Pass>();
            migrate();
            assert_eq!(Pass::on_chain_storage_version(), StorageVersion::new(1));
        })
    }

    #[test]
    fn has_an_id_that_names_the_pallet_and_the_versions() {
        use frame_support::traits::PalletInfoAccess;

        let id = <MigrateV0ToV1<Test> as SteppedMigration>::id();
        assert_eq!(id.pallet_id, Pass::name_hash());
        assert_eq!((id.version_from, id.version_to), (0, 1));
    }

    // The checks that run in try-runtime must pass after the migration, and notice it was not
    // done.
    #[cfg(feature = "try-runtime")]
    #[test]
    fn try_runtime_checks() {
        prepare(AccountNameA::get()).execute_with(|| {
            authenticated_as(THE_DEVICE);
            assert_ok!(Pass::add_session_key_with_filter(
                RuntimeOrigin::signed(Address::get()),
                OTHER,
                Some(DURATION),
                remark_only_filter(),
            ));
            make_old_session_key(&OTHER);
            forget_filters();

            let state = <MigrateV0ToV1<Test> as SteppedMigration>::pre_upgrade().unwrap();
            assert!(
                <MigrateV0ToV1<Test> as SteppedMigration>::post_upgrade(state.clone()).is_err()
            );

            migrate();
            assert_ok!(<MigrateV0ToV1<Test> as SteppedMigration>::post_upgrade(
                state.clone()
            ));

            // A migration that damaged something would be noticed.
            DeviceFilters::<Test>::insert(
                Address::get(),
                [9u8; 32],
                DeviceFilter::<(), u128, _, _>::Admin,
            );
            assert!(<MigrateV0ToV1<Test> as SteppedMigration>::post_upgrade(state).is_err());
        })
    }
}
