//! Autogenerated weights for `pallet_payments`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2024-03-18, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `virto-builder`, CPU: `Intel(R) Xeon(R) Silver 4216 CPU @ 2.10GHz`
//! WASM-EXECUTION: `Compiled`, CHAIN: `Some("kreivo-local")`, DB CACHE: 1024

// Executed Command:
// ./target/release/virto-node
// benchmark
// pallet
// --chain
// kreivo-local
// --pallet
// pallet_payments
// --extrinsic
// *
// --steps
// 50
// --repeat
// 20
// --output
// runtime/kreivo/src/weights/pallet_payments.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::{constants::RocksDbWeight, Weight}};

/// Weight functions needed for pallet_payments.
pub trait WeightInfo {
	fn pay(q: u32) -> Weight;
	fn release() -> Weight;
	fn cancel() -> Weight;
	fn request_refund() -> Weight;
	fn dispute_refund() -> Weight;
	fn resolve_dispute() -> Weight;
	fn request_payment() -> Weight;
	fn accept_and_pay() -> Weight;
}

/// Weights for pallet_payments using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Holds` (r:2 w:2)
	/// Proof: `Assets::Holds` (`max_values`: None, `max_size`: Some(982), added: 3457, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:2 w:1)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// Storage: `Payments::PaymentParties` (r:0 w:1)
	/// Proof: `Payments::PaymentParties` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// The range of component `q` is `[1, 50]`.
	fn pay(q: u32) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `473`
		//  Estimated: `8517`
		// Minimum execution time: 161_587_000 picoseconds.
		Weight::from_parts(218_726_681, 0)
			.saturating_add(Weight::from_parts(0, 8517))
			// Standard Error: 78_914
			.saturating_add(Weight::from_parts(900_944, 0).saturating_mul(q.into()))
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(6))
	}
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:3 w:3)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Holds` (r:2 w:2)
	/// Proof: `Assets::Holds` (`max_values`: None, `max_size`: Some(982), added: 3457, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn release() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1082`
		//  Estimated: `8856`
		// Minimum execution time: 398_666_000 picoseconds.
		Weight::from_parts(404_550_000, 0)
			.saturating_add(Weight::from_parts(0, 8856))
			.saturating_add(T::DbWeight::get().reads(8))
			.saturating_add(T::DbWeight::get().writes(8))
	}
	/// Storage: `Payments::PaymentParties` (r:1 w:1)
	/// Proof: `Payments::PaymentParties` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:2 w:2)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Holds` (r:2 w:2)
	/// Proof: `Assets::Holds` (`max_values`: None, `max_size`: Some(982), added: 3457, mode: `MaxEncodedLen`)
	fn cancel() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1049`
		//  Estimated: `8517`
		// Minimum execution time: 300_337_000 picoseconds.
		Weight::from_parts(308_347_000, 0)
			.saturating_add(Weight::from_parts(0, 8517))
			.saturating_add(T::DbWeight::get().reads(7))
			.saturating_add(T::DbWeight::get().writes(7))
	}
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Scheduler::Lookup` (r:1 w:1)
	/// Proof: `Scheduler::Lookup` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Scheduler::Agenda` (r:1 w:1)
	/// Proof: `Scheduler::Agenda` (`max_values`: None, `max_size`: Some(155814), added: 158289, mode: `MaxEncodedLen`)
	fn request_refund() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `343`
		//  Estimated: `159279`
		// Minimum execution time: 80_215_000 picoseconds.
		Weight::from_parts(82_549_000, 0)
			.saturating_add(Weight::from_parts(0, 159279))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `Payments::PaymentParties` (r:1 w:0)
	/// Proof: `Payments::PaymentParties` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Holds` (r:1 w:1)
	/// Proof: `Assets::Holds` (`max_values`: None, `max_size`: Some(982), added: 3457, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:1 w:1)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// Storage: `Scheduler::Lookup` (r:1 w:1)
	/// Proof: `Scheduler::Lookup` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Scheduler::Agenda` (r:1 w:1)
	/// Proof: `Scheduler::Agenda` (`max_values`: None, `max_size`: Some(155814), added: 158289, mode: `MaxEncodedLen`)
	fn dispute_refund() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1150`
		//  Estimated: `159279`
		// Minimum execution time: 213_633_000 picoseconds.
		Weight::from_parts(216_604_000, 0)
			.saturating_add(Weight::from_parts(0, 159279))
			.saturating_add(T::DbWeight::get().reads(7))
			.saturating_add(T::DbWeight::get().writes(6))
	}
	/// Storage: `Payments::PaymentParties` (r:1 w:0)
	/// Proof: `Payments::PaymentParties` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:3 w:3)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Holds` (r:2 w:2)
	/// Proof: `Assets::Holds` (`max_values`: None, `max_size`: Some(982), added: 3457, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn resolve_dispute() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1152`
		//  Estimated: `8856`
		// Minimum execution time: 588_034_000 picoseconds.
		Weight::from_parts(602_119_000, 0)
			.saturating_add(Weight::from_parts(0, 8856))
			.saturating_add(T::DbWeight::get().reads(9))
			.saturating_add(T::DbWeight::get().writes(8))
	}
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:0)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Payments::PaymentParties` (r:0 w:1)
	/// Proof: `Payments::PaymentParties` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	fn request_payment() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `320`
		//  Estimated: `8517`
		// Minimum execution time: 57_887_000 picoseconds.
		Weight::from_parts(58_829_000, 0)
			.saturating_add(Weight::from_parts(0, 8517))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `Payments::PaymentParties` (r:1 w:0)
	/// Proof: `Payments::PaymentParties` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:3 w:3)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn accept_and_pay() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `943`
		//  Estimated: `8856`
		// Minimum execution time: 364_626_000 picoseconds.
		Weight::from_parts(369_330_000, 0)
			.saturating_add(Weight::from_parts(0, 8856))
			.saturating_add(T::DbWeight::get().reads(7))
			.saturating_add(T::DbWeight::get().writes(6))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Holds` (r:2 w:2)
	/// Proof: `Assets::Holds` (`max_values`: None, `max_size`: Some(982), added: 3457, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:2 w:1)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// Storage: `Payments::PaymentParties` (r:0 w:1)
	/// Proof: `Payments::PaymentParties` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// The range of component `q` is `[1, 50]`.
	fn pay(q: u32) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `473`
		//  Estimated: `8517`
		// Minimum execution time: 161_587_000 picoseconds.
		Weight::from_parts(218_726_681, 0)
			.saturating_add(Weight::from_parts(0, 8517))
			// Standard Error: 78_914
			.saturating_add(Weight::from_parts(900_944, 0).saturating_mul(q.into()))
			.saturating_add(RocksDbWeight::get().reads(6))
			.saturating_add(RocksDbWeight::get().writes(6))
	}
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:3 w:3)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Holds` (r:2 w:2)
	/// Proof: `Assets::Holds` (`max_values`: None, `max_size`: Some(982), added: 3457, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn release() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1082`
		//  Estimated: `8856`
		// Minimum execution time: 398_666_000 picoseconds.
		Weight::from_parts(404_550_000, 0)
			.saturating_add(Weight::from_parts(0, 8856))
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().writes(8))
	}
	/// Storage: `Payments::PaymentParties` (r:1 w:1)
	/// Proof: `Payments::PaymentParties` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:2 w:2)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Holds` (r:2 w:2)
	/// Proof: `Assets::Holds` (`max_values`: None, `max_size`: Some(982), added: 3457, mode: `MaxEncodedLen`)
	fn cancel() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1049`
		//  Estimated: `8517`
		// Minimum execution time: 300_337_000 picoseconds.
		Weight::from_parts(308_347_000, 0)
			.saturating_add(Weight::from_parts(0, 8517))
			.saturating_add(RocksDbWeight::get().reads(7))
			.saturating_add(RocksDbWeight::get().writes(7))
	}
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Scheduler::Lookup` (r:1 w:1)
	/// Proof: `Scheduler::Lookup` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Scheduler::Agenda` (r:1 w:1)
	/// Proof: `Scheduler::Agenda` (`max_values`: None, `max_size`: Some(155814), added: 158289, mode: `MaxEncodedLen`)
	fn request_refund() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `343`
		//  Estimated: `159279`
		// Minimum execution time: 80_215_000 picoseconds.
		Weight::from_parts(82_549_000, 0)
			.saturating_add(Weight::from_parts(0, 159279))
			.saturating_add(RocksDbWeight::get().reads(3))
			.saturating_add(RocksDbWeight::get().writes(3))
	}
	/// Storage: `Payments::PaymentParties` (r:1 w:0)
	/// Proof: `Payments::PaymentParties` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Holds` (r:1 w:1)
	/// Proof: `Assets::Holds` (`max_values`: None, `max_size`: Some(982), added: 3457, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:1 w:1)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// Storage: `Scheduler::Lookup` (r:1 w:1)
	/// Proof: `Scheduler::Lookup` (`max_values`: None, `max_size`: Some(48), added: 2523, mode: `MaxEncodedLen`)
	/// Storage: `Scheduler::Agenda` (r:1 w:1)
	/// Proof: `Scheduler::Agenda` (`max_values`: None, `max_size`: Some(155814), added: 158289, mode: `MaxEncodedLen`)
	fn dispute_refund() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1150`
		//  Estimated: `159279`
		// Minimum execution time: 213_633_000 picoseconds.
		Weight::from_parts(216_604_000, 0)
			.saturating_add(Weight::from_parts(0, 159279))
			.saturating_add(RocksDbWeight::get().reads(7))
			.saturating_add(RocksDbWeight::get().writes(6))
	}
	/// Storage: `Payments::PaymentParties` (r:1 w:0)
	/// Proof: `Payments::PaymentParties` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:3 w:3)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Holds` (r:2 w:2)
	/// Proof: `Assets::Holds` (`max_values`: None, `max_size`: Some(982), added: 3457, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn resolve_dispute() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1152`
		//  Estimated: `8856`
		// Minimum execution time: 588_034_000 picoseconds.
		Weight::from_parts(602_119_000, 0)
			.saturating_add(Weight::from_parts(0, 8856))
			.saturating_add(RocksDbWeight::get().reads(9))
			.saturating_add(RocksDbWeight::get().writes(8))
	}
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:0)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Payments::PaymentParties` (r:0 w:1)
	/// Proof: `Payments::PaymentParties` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	fn request_payment() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `320`
		//  Estimated: `8517`
		// Minimum execution time: 57_887_000 picoseconds.
		Weight::from_parts(58_829_000, 0)
			.saturating_add(Weight::from_parts(0, 8517))
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	/// Storage: `Payments::PaymentParties` (r:1 w:0)
	/// Proof: `Payments::PaymentParties` (`max_values`: None, `max_size`: Some(80), added: 2555, mode: `MaxEncodedLen`)
	/// Storage: `Payments::Payment` (r:1 w:1)
	/// Proof: `Payments::Payment` (`max_values`: None, `max_size`: Some(5052), added: 7527, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(223), added: 2698, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:3 w:3)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn accept_and_pay() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `943`
		//  Estimated: `8856`
		// Minimum execution time: 364_626_000 picoseconds.
		Weight::from_parts(369_330_000, 0)
			.saturating_add(Weight::from_parts(0, 8856))
			.saturating_add(RocksDbWeight::get().reads(7))
			.saturating_add(RocksDbWeight::get().writes(6))
	}
}
