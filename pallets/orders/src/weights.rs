#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use core::marker::PhantomData;
use frame_support::weights::Weight;

/// Weight functions needed for fc_pallet_orders.
pub trait WeightInfo {
	fn create_cart() -> Weight;
	fn set_cart_items() -> Weight;
	fn checkout() -> Weight;

	fn cancel() -> Weight;
	fn pay() -> Weight;
}

/// Weights for pallet_remark using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// The range of component `l` is `[1, 1048576]`.
	fn create_cart() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn set_cart_items() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn checkout() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn cancel() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn pay() -> Weight {
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
	fn create_cart() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn set_cart_items() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn checkout() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn cancel() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}


	/// The range of component `l` is `[1, 1048576]`.
	fn pay() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
}
