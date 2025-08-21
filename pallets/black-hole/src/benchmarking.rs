use super::*;
use alloc::vec;
use frame::benchmarking::prelude::*;

fn assert_last_event<T: Config>(generic_event: T::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

#[benchmarks]
mod benchmarks {
    use super::*;
    use crate::frame_system::Call as SystemCall;

    #[benchmark]
    pub fn burn() -> Result<(), BenchmarkError> {
        // Setup code
        T::Balances::mint_into(
            &Pallet::<T>::event_horizon(),
            T::Balances::minimum_balance(),
        )?;

        #[block]
        {
            Pallet::<T>::burn();
        }

        // Verify code
        assert_last_event::<T>(Event::<T>::BalanceBurned.into());

        Ok(())
    }

    #[benchmark]
    pub fn dispatch_as_event_horizon() -> Result<(), BenchmarkError> {
        // Setup code
        let origin = T::EventHorizonDispatchOrigin::try_successful_origin()
            .map_err(|_| DispatchError::BadOrigin)?;
        let call: Box<T::RuntimeCall> =
            Box::new(SystemCall::remark_with_event { remark: vec![] }.into());

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, call);

        // Verify code
        frame_system::Pallet::<T>::assert_last_event(
            frame_system::Event::<T>::Remarked {
                sender: Pallet::<T>::event_horizon(),
                hash: T::Hashing::hash(&[]),
            }
            .into(),
        );

        Ok(())
    }

    impl_benchmark_test_suite!(
        Pallet,
        frame::deps::sp_io::TestExternalities::default(),
        crate::mock::Test
    );
}
