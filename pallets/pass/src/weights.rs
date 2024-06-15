#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for pallet_remark.
pub trait WeightInfo {
	fn register() -> Weight;
	fn claim() -> Weight;
	fn unreserve_uninitialized_account() -> Weight;
	fn dispatch() -> Weight;
}

/// Weights for pallet_remark using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// The range of component `l` is `[1, 1048576]`.
	fn register() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn claim() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn unreserve_uninitialized_account() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn dispatch() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}	
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// The range of component `l` is `[1, 1048576]`.
	fn register() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn claim() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn unreserve_uninitialized_account() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(0, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(0, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn dispatch() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
}
