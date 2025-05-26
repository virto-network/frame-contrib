#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::{constants::RocksDbWeight, Weight}};

/// Weight functions needed for fc_pallet_orders.
pub trait WeightInfo {
	fn create_cart(p: u32, q: u32) -> Weight;
	fn set_cart_items(q: u32) -> Weight;
	fn checkout() -> Weight;
	fn cancel() -> Weight;
	fn pay() -> Weight;
}

/// Weights for pallet_remark using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// The range of component `p` is `[1, 5]`.
	/// The range of component `q` is `[1, 64]`.
	fn create_cart(p: u32, q: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `479 + q * (260 ±0)`
		//  Estimated: `3554 + q * (2960 ±0)`
		// Minimum execution time: 48_289_000 picoseconds.
		Weight::from_parts(39_755_946, 0)
			.saturating_add(Weight::from_parts(0, 3554))
			// Standard Error: 92_670
			.saturating_add(Weight::from_parts(51_674, 0).saturating_mul(p.into()))
			// Standard Error: 6_687
			.saturating_add(Weight::from_parts(13_691_754, 0).saturating_mul(q.into()))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().reads((2_u64).saturating_mul(q.into())))
			.saturating_add(T::DbWeight::get().writes(3))
			.saturating_add(Weight::from_parts(0, 2960).saturating_mul(q.into()))
	}
	/// The range of component `q` is `[1, 64]`.
	fn set_cart_items(q: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `596 + q * (260 ±0)`
		//  Estimated: `9284 + q * (2960 ±0)`
		// Minimum execution time: 36_150_000 picoseconds.
		Weight::from_parts(27_660_112, 0)
			.saturating_add(Weight::from_parts(0, 9284))
			// Standard Error: 9_456
			.saturating_add(Weight::from_parts(13_602_505, 0).saturating_mul(q.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().reads((2_u64).saturating_mul(q.into())))
			.saturating_add(T::DbWeight::get().writes(1))
			.saturating_add(Weight::from_parts(0, 2960).saturating_mul(q.into()))
	}
	fn checkout() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `23076`
		//  Estimated: `379870`
		// Minimum execution time: 3_267_966_000 picoseconds.
		Weight::from_parts(3_280_710_000, 0)
			.saturating_add(Weight::from_parts(0, 379870))
			.saturating_add(T::DbWeight::get().reads(264))
			.saturating_add(T::DbWeight::get().writes(69))
	}
	fn cancel() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `13928`
		//  Estimated: `190430`
		// Minimum execution time: 1_392_284_000 picoseconds.
		Weight::from_parts(1_403_799_000, 0)
			.saturating_add(Weight::from_parts(0, 190430))
			.saturating_add(T::DbWeight::get().reads(68))
			.saturating_add(T::DbWeight::get().writes(68))
	}
	fn pay() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `28521`
		//  Estimated: `482782`
		// Minimum execution time: 13_331_609_000 picoseconds.
		Weight::from_parts(13_373_345_000, 0)
			.saturating_add(Weight::from_parts(0, 482782))
			.saturating_add(T::DbWeight::get().reads(335))
			.saturating_add(T::DbWeight::get().writes(587))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// The range of component `p` is `[1, 5]`.
	/// The range of component `q` is `[1, 64]`.
	fn create_cart(p: u32, q: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `479 + q * (260 ±0)`
		//  Estimated: `3554 + q * (2960 ±0)`
		// Minimum execution time: 48_289_000 picoseconds.
		Weight::from_parts(39_755_946, 0)
			.saturating_add(Weight::from_parts(0, 3554))
			// Standard Error: 92_670
			.saturating_add(Weight::from_parts(51_674, 0).saturating_mul(p.into()))
			// Standard Error: 6_687
			.saturating_add(Weight::from_parts(13_691_754, 0).saturating_mul(q.into()))
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().reads((2_u64).saturating_mul(q.into())))
			.saturating_add(RocksDbWeight::get().writes(3))
			.saturating_add(Weight::from_parts(0, 2960).saturating_mul(q.into()))
	}
	/// The range of component `q` is `[1, 64]`.
	fn set_cart_items(q: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `596 + q * (260 ±0)`
		//  Estimated: `9284 + q * (2960 ±0)`
		// Minimum execution time: 36_150_000 picoseconds.
		Weight::from_parts(27_660_112, 0)
			.saturating_add(Weight::from_parts(0, 9284))
			// Standard Error: 9_456
			.saturating_add(Weight::from_parts(13_602_505, 0).saturating_mul(q.into()))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().reads((2_u64).saturating_mul(q.into())))
			.saturating_add(RocksDbWeight::get().writes(1))
			.saturating_add(Weight::from_parts(0, 2960).saturating_mul(q.into()))
	}
	fn checkout() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `23076`
		//  Estimated: `379870`
		// Minimum execution time: 3_267_966_000 picoseconds.
		Weight::from_parts(3_280_710_000, 0)
			.saturating_add(Weight::from_parts(0, 379870))
			.saturating_add(RocksDbWeight::get().reads(264))
			.saturating_add(RocksDbWeight::get().writes(69))
	}
	fn cancel() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `13928`
		//  Estimated: `190430`
		// Minimum execution time: 1_392_284_000 picoseconds.
		Weight::from_parts(1_403_799_000, 0)
			.saturating_add(Weight::from_parts(0, 190430))
			.saturating_add(RocksDbWeight::get().reads(68))
			.saturating_add(RocksDbWeight::get().writes(68))
	}
	fn pay() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `28521`
		//  Estimated: `482782`
		// Minimum execution time: 13_331_609_000 picoseconds.
		Weight::from_parts(13_373_345_000, 0)
			.saturating_add(Weight::from_parts(0, 482782))
			.saturating_add(RocksDbWeight::get().reads(335))
			.saturating_add(RocksDbWeight::get().writes(587))
	}
}
