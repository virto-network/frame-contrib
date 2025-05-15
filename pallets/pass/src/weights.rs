#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use core::marker::PhantomData;
use frame_support::weights::Weight;

/// Weight functions needed for pallet_remark.
pub trait WeightInfo {
	fn register() -> Weight;
	fn authenticate() -> Weight;
	fn add_device() -> Weight;
	fn remove_device() -> Weight;
	fn add_session_key() -> Weight;
	fn remove_session_key() -> Weight;
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
	fn authenticate() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn add_device() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn remove_device() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn add_session_key() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn remove_session_key() -> Weight {
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
	fn authenticate() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(0, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(0, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn add_device() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(0, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(0, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn remove_device() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(0, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(0, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn add_session_key() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(0, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(0, 0))
	}

	/// The range of component `l` is `[1, 1048576]`.
	fn remove_session_key() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_471_000 picoseconds.
		Weight::from_parts(0, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(0, 0))
	}
}
