use super::*;
use crate::Pallet;
use frame_benchmarking::v2::*;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	pub fn success() {
		// Setup code

		#[extrinsic_call]
		_(frame_system::RawOrigin::Root);

		// Verification code
		assert_last_event::<T>(Event::Success.into());
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
