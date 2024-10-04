#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Pallet;
use frame_benchmarking::v2::*;
use frame_support::traits::OriginTrait;
use sp_runtime::traits::Hash;

type RuntimeEventFor<T, I> = <T as Config<I>>::RuntimeEvent;

fn assert_has_event<T: Config<I>, I: 'static>(generic_event: RuntimeEventFor<T, I>) {
    frame_system::Pallet::<T>::assert_has_event(generic_event.into());
}

#[allow(dead_code)]
fn setup_signers<T: frame_system::Config>() -> (T::AccountId, T::AccountId) {
    (
        frame_benchmarking::account("signer", 0, 0),
        frame_benchmarking::account("signer", 1, 0),
    )
}

fn hash<T: frame_system::Config>(b: &[u8]) -> HashedUserId
where
    T::Hash: Into<HashedUserId>,
{
    T::Hashing::hash(b).into()
}

#[instance_benchmarks(
where
    T: frame_system::Config + crate::Config<I>,
    OriginFor<T>: From<frame_system::Origin<T>>,
    T::Hash: Into<HashedUserId>,
    RuntimeEventFor<T, I>: From<frame_system::Event<T>>,
)]
mod benchmarks {
    use super::*;

    #[benchmark]
    pub fn register() -> Result<(), BenchmarkError> {
        // Setup code
        let origin = T::BenchmarkHelper::register_origin();
        let user_id = hash::<T>(&*b"my-account");
        let account_id = Pallet::<T, I>::account_id_for(user_id)?;
        let device_id = [0u8; 32];

        #[extrinsic_call]
        _(
            origin.into_caller(),
            user_id,
            T::BenchmarkHelper::device_attestation(device_id),
        );

        // Verification code
        assert_has_event::<T, I>(Event::Registered { who: account_id }.into());

        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
