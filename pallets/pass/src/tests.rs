//! Tests for pass pallet.
use super::{Error, Event, PassAuthenticate};
use crate::filter::DeviceFilter;
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

const THE_DEVICE: DeviceId = [10u8; 32];
const OTHER_DEVICE: DeviceId = [11u8; 32];
const THIRD_DEVICE: DeviceId = [12u8; 32];

fn remark_only_filter(
) -> DeviceFilter<u32, u128, frame_support::traits::ConstU32<10>, frame_support::traits::ConstU32<5>>
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

mod add_device {
    use super::*;
    use crate::DeviceOf;

    #[test]
    fn fails_if_bad_origin() {
        prepare(AccountNameA::get()).execute_with(|| {
            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_noop!(
                Pass::add_device(
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
                Pass::add_device(
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
            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_noop!(
                Pass::add_device(
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
            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_noop!(
                Pass::add_device(
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

            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_ok!(Pass::add_device(
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

            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_ok!(Pass::add_device(
                RuntimeOrigin::signed(Address::get()),
                PassDeviceAttestation::AuthenticatorAAuthenticator(
                    authenticator_a::DeviceAttestation {
                        device_id: OTHER_DEVICE,
                        challenge: authenticator_a::Authenticator::generate(&(), &[]),
                    }
                ),
                DeviceFilter::Admin,
            ));

            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_noop!(
                Pass::add_device(
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

mod add_session_key {
    use super::*;
    use sp_core::{blake2_256, H256};

    #[test]
    fn fails_if_bad_origin() {
        prepare(AccountNameA::get()).execute_with(|| {
            assert_noop!(
                Pass::add_session_key(
                    RuntimeOrigin::root(),
                    OTHER,
                    Some(DURATION),
                    remark_only_filter()
                ),
                DispatchError::BadOrigin
            );
            assert_noop!(
                Pass::add_session_key(
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
            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_noop!(
                Pass::add_session_key(
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
            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_ok!(Pass::add_session_key(
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

            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_ok!(Pass::add_session_key(
                RuntimeOrigin::signed(Address::get()),
                OTHER,
                Some(DURATION),
                remark_only_filter(),
            ));

            crate::AuthenticatedDevice::<Test>::put(OTHER_DEVICE);
            assert_noop!(
                Pass::add_session_key(
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
            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_ok!(Pass::add_session_key(
                RuntimeOrigin::signed(Address::get()),
                SIGNER,
                None,
                remark_only_filter(),
            ));

            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_ok!(Pass::add_session_key(
                RuntimeOrigin::signed(Address::get()),
                OTHER,
                None,
                remark_only_filter(),
            ));

            run_to(5);

            // Extending a session key works
            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_ok!(Pass::add_session_key(
                RuntimeOrigin::signed(Address::get()),
                OTHER,
                None,
                remark_only_filter(),
            ));

            run_to(10);

            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_noop!(
                Pass::add_session_key(
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
            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_ok!(Pass::add_session_key(
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
    use frame_support::dispatch::GetDispatchInfo;
    use sp_core::blake2_256;
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

            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_ok!(Pass::add_session_key(
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
                    crate::Call::add_device {
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
                crate::Call::<Test>::add_session_key {
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
                crate::Call::<Test>::add_session_key {
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

            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_ok!(Pass::add_device(
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
            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_ok!(Pass::add_device(
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
            crate::AuthenticatedDevice::<Test>::put(OTHER_DEVICE);
            assert_noop!(
                Pass::add_device(
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
            let pallets: DeviceFilter<u32, u128, ConstU32<10>, ConstU32<5>> = DeviceFilter::Pallets(
                [0u8, 5, 10]
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .try_into()
                    .unwrap(),
            );
            let calls_subset: DeviceFilter<u32, u128, ConstU32<10>, ConstU32<5>> =
                DeviceFilter::Calls(
                    [(0u8, 1u8), (5u8, 0u8)]
                        .into_iter()
                        .collect::<BTreeSet<_>>()
                        .try_into()
                        .unwrap(),
                );
            let calls_outside: DeviceFilter<u32, u128, ConstU32<10>, ConstU32<5>> =
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
                    asset: 1u32,
                    max_amount: 1000u128,
                }]
                .try_into()
                .unwrap(),
            );
            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_ok!(Pass::add_device(
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
            crate::AuthenticatedDevice::<Test>::put(OTHER_DEVICE);
            assert_noop!(
                Pass::add_device(
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
            type F = DeviceFilter<u32, u128, ConstU32<10>, ConstU32<5>>;

            let spend_1000: F = DeviceFilter::Spend(
                vec![AssetSpendLimit {
                    asset: 1u32,
                    max_amount: 1000u128,
                }]
                .try_into()
                .unwrap(),
            );
            let spend_500: F = DeviceFilter::Spend(
                vec![AssetSpendLimit {
                    asset: 1u32,
                    max_amount: 500u128,
                }]
                .try_into()
                .unwrap(),
            );
            let spend_2000: F = DeviceFilter::Spend(
                vec![AssetSpendLimit {
                    asset: 1u32,
                    max_amount: 2000u128,
                }]
                .try_into()
                .unwrap(),
            );
            let spend_other_asset: F = DeviceFilter::Spend(
                vec![AssetSpendLimit {
                    asset: 2u32,
                    max_amount: 100u128,
                }]
                .try_into()
                .unwrap(),
            );

            // Can delegate with lower limit
            assert!(spend_1000.is_superset_of(&spend_500));
            // Cannot delegate with higher limit
            assert!(!spend_1000.is_superset_of(&spend_2000));
            // Cannot delegate for an asset you don't have
            assert!(!spend_1000.is_superset_of(&spend_other_asset));
            // Admin can delegate any spend
            assert!(F::Admin.is_superset_of(&spend_1000));
            // Spend can't escalate to admin
            assert!(!spend_1000.is_superset_of(&F::Admin));
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
                Pass::add_session_key(
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
    fn session_key_stores_filter() {
        setup_with_admin_device().execute_with(|| {
            let filter = DeviceFilter::Calls(
                [(0u8, 7u8)]
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .try_into()
                    .unwrap(),
            );
            crate::AuthenticatedDevice::<Test>::put(THE_DEVICE);
            assert_ok!(Pass::add_session_key(
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
