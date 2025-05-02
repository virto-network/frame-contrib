#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]
#![allow(dead_code)]

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::{constants::RocksDbWeight, Weight}};

/// Weight functions needed for `pallet_transaction_payment`.
pub trait WeightInfo {
	fn charge_transaction_payment() -> Weight;
}

/// Weights for `pallet_transaction_payment` using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn charge_transaction_payment() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `52`
		//  Estimated: `3593`
		// Minimum execution time: 35_425_000 picoseconds.
		Weight::from_parts(35_979_000, 3593)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

// For backwards compatibility and tests.
impl WeightInfo for () {
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn charge_transaction_payment() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `52`
		//  Estimated: `3593`
		// Minimum execution time: 35_425_000 picoseconds.
		Weight::from_parts(35_979_000, 3593)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}