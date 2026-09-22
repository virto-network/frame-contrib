use super::*;
use crate::{DeviceOf, Pallet, SessionKeys};

use frame_benchmarking::v2::*;
use frame_support::{
    dispatch::{DispatchInfo, GetDispatchInfo},
    traits::OriginTrait,
};
use frame_system::RawOrigin;
use sp_io::hashing::blake2_256;
use sp_runtime::traits::{
    transaction_extension::DispatchTransaction, AsTransactionAuthorizedOrigin, DispatchInfoOf,
    Hash, PostDispatchInfoOf, TxBaseImplication,
};

fn assert_has_event<T: Config<I>, I: 'static>(generic_event: T::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_has_event(generic_event.into());
}

fn prepare_register<T: Config<I>, I: 'static>(
    hashed_user_id: HashedUserId,
) -> Result<T::RuntimeOrigin, BenchmarkError> {
    let origin = T::RegisterOrigin::try_successful_origin(&hashed_user_id)
        .map_err(|_| DispatchError::BadOrigin)?;
    let registrar_account_id = T::RegisterOrigin::ensure_origin(origin.clone(), &hashed_user_id)
        .map_err(|_| DispatchError::BadOrigin)?;

    T::RegistrarConsideration::ensure_successful(
        &registrar_account_id,
        Footprint::from_parts(1, HashedUserId::max_encoded_len()),
    );

    Ok(origin)
}

fn do_register<T: Config<I>, I: 'static>(
    hashed_user_id: HashedUserId,
) -> Result<DeviceId, BenchmarkError>
where
    T::Authenticator: AuthenticatorBenchmarkHelper,
{
    let origin = prepare_register::<T, I>(hashed_user_id)
        .map_err(|_| BenchmarkError::Stop("Cannot prepare origin"))?;
    let account_address = Pallet::<T, I>::address_for(hashed_user_id);
    let attestation =
        <T as Config<I>>::Authenticator::device_attestation(&account_address.encode());
    Pallet::<T, I>::register(origin, hashed_user_id, attestation.clone())
        .map(|_| *(attestation.device_id()))
        .map_err(|_| BenchmarkError::Stop("Cannot register pass account"))
}

#[allow(dead_code)]
fn setup_signers<T: frame_system::Config>() -> (T::AccountId, T::AccountId) {
    (account("signer", 0, 0), account("signer", 1, 0))
}

/// The call dispatched in the `PassAuthenticate` benchmarks. The dispatch
/// itself is substituted, so only its encoding is relevant.
fn benchmark_call<T: frame_system::Config>() -> RuntimeCallFor<T> {
    frame_system::Call::remark {
        remark: b"Hello, world".to_vec(),
    }
    .into()
}

fn hash<T: frame_system::Config>(b: &[u8]) -> HashedUserId
where
    T::Hash: Into<HashedUserId>,
{
    T::Hashing::hash(b).into()
}

#[instance_benchmarks(
where
    T::Hash: Into<HashedUserId>,
    <T as Config<I>>::Authenticator: AuthenticatorBenchmarkHelper,
    DispatchInfoOf<RuntimeCallFor<T>>: From<DispatchInfo>,
    PostDispatchInfoOf<RuntimeCallFor<T>>: From<()>,
    OriginFor<T>: From<frame_system::Origin<T>> + AsTransactionAuthorizedOrigin,
)]
mod benchmarks {
    use super::*;

    #[benchmark]
    pub fn register() -> Result<(), BenchmarkError> {
        // Setup code
        let user_id = hash::<T>(b"my-account");
        let origin = prepare_register::<T, I>(user_id)?;

        let account_id = Pallet::<T, I>::address_for(user_id);
        let attestation = <T as Config<I>>::Authenticator::device_attestation(&account_id.encode());
        let device_id = *(attestation.clone().device_id());

        #[extrinsic_call]
        _(origin.into_caller(), user_id, attestation);

        // Verification code
        assert_has_event::<T, I>(
            Event::Registered {
                who: account_id.clone(),
            }
            .into(),
        );
        assert_has_event::<T, I>(
            Event::DeviceAdded {
                who: account_id,
                device_id,
            }
            .into(),
        );

        Ok(())
    }

    /// `PassAuthenticate` with a credential: the whole extension pipeline
    /// (`validate`, `prepare` and `post_dispatch`).
    #[benchmark]
    pub fn authenticate() -> Result<(), BenchmarkError> {
        // Setup code
        let user_id = hash::<T>(b"my-account");
        let device_id = do_register::<T, I>(user_id)?;

        let call = benchmark_call::<T>();
        let ext = PassAuthenticate::<T, I>::from(
            device_id,
            <T as Config<I>>::Authenticator::credential(
                user_id,
                device_id,
                &TxBaseImplication((0u8, call.clone())).using_encoded(blake2_256),
            ),
        );
        let info = call.get_dispatch_info();
        let len = call.encoded_size();

        #[block]
        {
            assert!(matches!(
                ext.test_run(RawOrigin::None.into(), &call, &info.into(), len, 0, |_| Ok(
                    ().into()
                )),
                Ok(Ok(_))
            ));
        }

        Ok(())
    }

    /// `PassAuthenticate` without a credential, signed by a session key: the
    /// most expensive branch the extension takes when not authenticating (a
    /// `SessionKeys` hit).
    #[benchmark]
    pub fn authenticate_none() -> Result<(), BenchmarkError> {
        // Setup code
        let user_id = hash::<T>(b"my-account");
        do_register::<T, I>(user_id)?;
        let address = Pallet::<T, I>::address_for(user_id);

        let call = benchmark_call::<T>();
        let session_key: T::AccountId = account("session-key", 0, 0);
        SessionKeys::<T, I>::insert(&session_key, (address, T::MaxSessionDuration::get()));

        let ext = PassAuthenticate::<T, I>::default();
        let info = call.get_dispatch_info();
        let len = call.encoded_size();

        #[block]
        {
            assert!(matches!(
                ext.test_run(
                    RawOrigin::Signed(session_key).into(),
                    &call,
                    &info.into(),
                    len,
                    0,
                    |_| Ok(().into())
                ),
                Ok(Ok(_))
            ));
        }

        Ok(())
    }

    #[benchmark]
    pub fn add_device() -> Result<(), BenchmarkError> {
        // Setup code
        let user_id = hash::<T>(b"my-account");
        do_register::<T, I>(user_id)?;

        let address = Pallet::<T, I>::address_for(user_id);
        let attestation = <T as Config<I>>::Authenticator::device_attestation(&address.encode());
        let new_device_id = *(attestation.clone().device_id());
        T::DeviceConsideration::ensure_successful(
            &address,
            Footprint::from_parts(2, DeviceOf::<T, I>::max_encoded_len()),
        );

        #[extrinsic_call]
        _(RawOrigin::Signed(address.clone()), attestation);

        // Verification code
        assert_has_event::<T, I>(
            Event::DeviceAdded {
                who: address,
                device_id: new_device_id,
            }
            .into(),
        );

        Ok(())
    }

    #[benchmark]
    pub fn remove_device() -> Result<(), BenchmarkError> {
        // Setup code
        let user_id = hash::<T>(b"my-account");
        do_register::<T, I>(user_id)?;

        let address = Pallet::<T, I>::address_for(user_id);
        let attestation = <T as Config<I>>::Authenticator::device_attestation(&address.encode());
        let new_device_id = *(attestation.clone().device_id());
        T::DeviceConsideration::ensure_successful(
            &address,
            Footprint::from_parts(2, DeviceOf::<T, I>::max_encoded_len()),
        );

        Pallet::<T, I>::add_device(RawOrigin::Signed(address.clone()).into(), attestation)?;

        #[extrinsic_call]
        _(RawOrigin::Signed(address.clone()), new_device_id);

        // Verification code
        assert_has_event::<T, I>(
            Event::DeviceRemoved {
                who: address,
                device_id: new_device_id,
            }
            .into(),
        );

        Ok(())
    }

    #[benchmark]
    pub fn add_session_key() -> Result<(), BenchmarkError> {
        // Setup code
        let user_id = hash::<T>(b"my-account");
        do_register::<T, I>(user_id)?;

        let address = Pallet::<T, I>::address_for(user_id);
        let new_session_key: T::AccountId = account("session-key", 0, 0);
        T::DeviceConsideration::ensure_successful(
            &address,
            Footprint::from_parts(2, T::AccountId::max_encoded_len()),
        );

        // Worst case: the key is already in use by this account, so the
        // existing session (and its scheduled removal) is torn down before the
        // new one is created and scheduled.
        Pallet::<T, I>::add_session_key(
            RawOrigin::Signed(address.clone()).into(),
            T::Lookup::unlookup(new_session_key.clone()),
            None,
        )?;

        #[extrinsic_call]
        _(
            RawOrigin::Signed(address.clone()),
            T::Lookup::unlookup(new_session_key.clone()),
            None,
        );

        // Verification code
        assert_has_event::<T, I>(
            Event::SessionRemoved {
                session_key: new_session_key.clone(),
            }
            .into(),
        );
        assert_has_event::<T, I>(
            Event::SessionCreated {
                session_key_hash: T::Hashing::hash(&new_session_key.encode()),
                until: T::MaxSessionDuration::get(),
            }
            .into(),
        );

        Ok(())
    }

    #[benchmark]
    pub fn remove_session_key() -> Result<(), BenchmarkError> {
        // Setup code
        let user_id = hash::<T>(b"my-account");
        do_register::<T, I>(user_id)?;

        let address = Pallet::<T, I>::address_for(user_id);
        let session_key: T::AccountId = account("session-key", 0, 0);
        T::DeviceConsideration::ensure_successful(
            &address,
            Footprint::from_parts(2, T::AccountId::max_encoded_len()),
        );

        Pallet::<T, I>::add_session_key(
            RawOrigin::Signed(address.clone()).into(),
            T::Lookup::unlookup(session_key.clone()),
            None,
        )?;

        #[extrinsic_call]
        _(RawOrigin::Root, session_key.clone());

        // Verification code
        assert_has_event::<T, I>(Event::SessionRemoved { session_key }.into());

        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, mock::new_test_ext(), mock::Test);
}
