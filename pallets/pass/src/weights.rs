#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::{constants::RocksDbWeight, Weight}};

/// Weight functions needed for pallet_remark.
pub trait WeightInfo {
	fn register() -> Weight;
	/// Weight of `PassAuthenticate` when it carries a credential: authenticates the device
	/// (updating its stored state).
	///
	/// Does not include the cost of verifying the credential itself, which the authenticator
	/// reports separately.
	fn authenticate() -> Weight;
	/// Weight of `PassAuthenticate` when it carries no credential (the path taken by every
	/// extrinsic that does not authenticate with a Pass device): at most one `SessionKeys`
	/// lookup.
	fn authenticate_none() -> Weight;
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

	/// PLACEHOLDER: not produced by a benchmark run. Execution time and proof size are
	/// conservative estimates; DB accesses are counted from the code path:
	/// Storage: `Pass::Devices` (r:2 w:1)
	fn authenticate() -> Weight {
		Weight::from_parts(20_000_000, 7_000)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}

	/// PLACEHOLDER: not produced by a benchmark run. Execution time and proof size are
	/// conservative estimates; DB accesses are counted from the code path:
	/// Storage: `Pass::SessionKeys` (r:1 w:0)
	fn authenticate_none() -> Weight {
		Weight::from_parts(10_000_000, 4_000)
			.saturating_add(T::DbWeight::get().reads(1_u64))
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

	/// PLACEHOLDER: mirrors [`SubstrateWeight::authenticate`] with `RocksDbWeight`.
	fn authenticate() -> Weight {
		Weight::from_parts(20_000_000, 7_000)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}

	/// PLACEHOLDER: mirrors [`SubstrateWeight::authenticate_none`] with `RocksDbWeight`.
	fn authenticate_none() -> Weight {
		Weight::from_parts(10_000_000, 4_000)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
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
