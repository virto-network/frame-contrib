#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for `pallet_listings`.
pub trait WeightInfo {
	fn create_inventory() -> Weight;
	fn archive_inventory() -> Weight;
	fn publish_item(q: u32) -> Weight;
	fn set_item_price() -> Weight;
	fn clear_item_price() -> Weight;
	fn mark_item_can_transfer() -> Weight;
	fn mark_item_not_for_resale() -> Weight;
	fn set_item_attribute(p: u32, q: u32) -> Weight;
	fn clear_item_attribute(p: u32, q: u32) -> Weight;
	fn set_subscription_conditions() -> Weight;
	fn subscribe() -> Weight;
	fn charge_due() -> Weight;
	fn cancel_subscription() -> Weight;
	fn terminate_subscription() -> Weight;
	fn schedule_replacement() -> Weight;
	fn amend_subscription() -> Weight;
	fn cancel_replacement() -> Weight;
	fn amend_item_subscriptions() -> Weight;
	fn process_due(n: u32, ) -> Weight;
	fn process_due_replacement() -> Weight;
	fn skip_empty_bucket() -> Weight;
	fn process_due_overflow() -> Weight;
	fn settle_subscription() -> Weight;
	fn set_subscription_policy() -> Weight;
	fn withdraw_subscription_conditions() -> Weight;
}

/// Weights for `pallet_listings` using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::CollectionRoleOf` (r:0 w:1)
	/// Proof: `ListingsCatalog::CollectionRoleOf` (`max_values`: None, `max_size`: Some(71), added: 2546, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::CollectionConfigOf` (r:0 w:1)
	/// Proof: `ListingsCatalog::CollectionConfigOf` (`max_values`: None, `max_size`: Some(77), added: 2552, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::CollectionAccount` (r:0 w:1)
	/// Proof: `ListingsCatalog::CollectionAccount` (`max_values`: None, `max_size`: Some(70), added: 2545, mode: `MaxEncodedLen`)
	fn create_inventory() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `6`
		//  Estimated: `3551`
		// Minimum execution time: 27_359_000 picoseconds.
		Weight::from_parts(28_328_000, 0)
			.saturating_add(Weight::from_parts(0, 3551))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(4))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:1 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	fn archive_inventory() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `219`
		//  Estimated: `3950`
		// Minimum execution time: 40_324_000 picoseconds.
		Weight::from_parts(42_295_000, 0)
			.saturating_add(Weight::from_parts(0, 3950))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:3 w:2)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:1)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::CollectionConfigOf` (r:1 w:0)
	/// Proof: `ListingsCatalog::CollectionConfigOf` (`max_values`: None, `max_size`: Some(77), added: 2552, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::ItemConfigOf` (r:1 w:1)
	/// Proof: `ListingsCatalog::ItemConfigOf` (`max_values`: None, `max_size`: Some(54), added: 2529, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Account` (r:0 w:1)
	/// Proof: `ListingsCatalog::Account` (`max_values`: None, `max_size`: Some(94), added: 2569, mode: `MaxEncodedLen`)
	/// The range of component `q` is `[1, 216]`.
	fn publish_item(q: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `410`
		//  Estimated: `9870`
		// Minimum execution time: 116_758_000 picoseconds.
		Weight::from_parts(120_516_367, 0)
			.saturating_add(Weight::from_parts(0, 9870))
			// Standard Error: 1_501
			.saturating_add(Weight::from_parts(5_289, 0).saturating_mul(q.into()))
			.saturating_add(T::DbWeight::get().reads(8))
			.saturating_add(T::DbWeight::get().writes(7))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:2 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:0)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	fn set_item_price() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `533`
		//  Estimated: `6910`
		// Minimum execution time: 68_375_000 picoseconds.
		Weight::from_parts(70_371_000, 0)
			.saturating_add(Weight::from_parts(0, 6910))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:2 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:0)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	fn clear_item_price() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `533`
		//  Estimated: `6910`
		// Minimum execution time: 68_375_000 picoseconds.
		Weight::from_parts(70_371_000, 0)
			.saturating_add(Weight::from_parts(0, 6910))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:2 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:0)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	fn mark_item_can_transfer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `524`
		//  Estimated: `6910`
		// Minimum execution time: 52_472_000 picoseconds.
		Weight::from_parts(53_629_000, 0)
			.saturating_add(Weight::from_parts(0, 6910))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:3 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:0)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	fn mark_item_not_for_resale() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `533`
		//  Estimated: `9870`
		// Minimum execution time: 61_894_000 picoseconds.
		Weight::from_parts(63_441_000, 0)
			.saturating_add(Weight::from_parts(0, 9870))
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:3 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:0)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	/// The range of component `p` is `[1, 59]`.
	/// The range of component `q` is `[1, 251]`.
	fn set_item_attribute(p: u32, q: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `533`
		//  Estimated: `9870`
		// Minimum execution time: 58_392_000 picoseconds.
		Weight::from_parts(59_944_382, 0)
			.saturating_add(Weight::from_parts(0, 9870))
			// Standard Error: 1_215
			.saturating_add(Weight::from_parts(12_741, 0).saturating_mul(p.into()))
			// Standard Error: 284
			.saturating_add(Weight::from_parts(2_035, 0).saturating_mul(q.into()))
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:3 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:0)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	/// The range of component `p` is `[1, 59]`.
	/// The range of component `q` is `[1, 251]`.
	fn clear_item_attribute(p: u32, q: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `579 + p * (1 ±0) + q * (1 ±0)`
		//  Estimated: `9870`
		// Minimum execution time: 54_111_000 picoseconds.
		Weight::from_parts(55_681_786, 0)
			.saturating_add(Weight::from_parts(0, 9870))
			// Standard Error: 1_226
			.saturating_add(Weight::from_parts(22_803, 0).saturating_mul(p.into()))
			// Standard Error: 286
			.saturating_add(Weight::from_parts(1_275, 0).saturating_mul(q.into()))
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn set_subscription_conditions() -> Weight {
		Weight::from_parts(60_000_000, 12000)
			.saturating_add(T::DbWeight::get().reads(6_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn subscribe() -> Weight {
		Weight::from_parts(350_000_000, 70000)
			.saturating_add(T::DbWeight::get().reads(22_u64))
			.saturating_add(T::DbWeight::get().writes(13_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn charge_due() -> Weight {
		Weight::from_parts(250_000_000, 40000)
			.saturating_add(T::DbWeight::get().reads(14_u64))
			.saturating_add(T::DbWeight::get().writes(8_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn cancel_subscription() -> Weight {
		Weight::from_parts(120_000_000, 30000)
			.saturating_add(T::DbWeight::get().reads(8_u64))
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn terminate_subscription() -> Weight {
		Weight::from_parts(100_000_000, 30000)
			.saturating_add(T::DbWeight::get().reads(6_u64))
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn schedule_replacement() -> Weight {
		Weight::from_parts(200_000_000, 45000)
			.saturating_add(T::DbWeight::get().reads(13_u64))
			.saturating_add(T::DbWeight::get().writes(7_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn amend_subscription() -> Weight {
		Weight::from_parts(120_000_000, 30000)
			.saturating_add(T::DbWeight::get().reads(10_u64))
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn cancel_replacement() -> Weight {
		Weight::from_parts(150_000_000, 45000)
			.saturating_add(T::DbWeight::get().reads(12_u64))
			.saturating_add(T::DbWeight::get().writes(6_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn amend_item_subscriptions() -> Weight {
		Weight::from_parts(70_000_000, 14000)
			.saturating_add(T::DbWeight::get().reads(7_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn process_due_replacement() -> Weight {
		Weight::from_parts(450_000_000, 75000)
			.saturating_add(T::DbWeight::get().reads(24_u64))
			.saturating_add(T::DbWeight::get().writes(15_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn skip_empty_bucket() -> Weight {
		Weight::from_parts(10_000_000, 20000)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(0_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn process_due_overflow() -> Weight {
		Weight::from_parts(300_000_000, 60000)
			.saturating_add(T::DbWeight::get().reads(18_u64))
			.saturating_add(T::DbWeight::get().writes(11_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn settle_subscription() -> Weight {
		Weight::from_parts(180_000_000, 45000)
			.saturating_add(T::DbWeight::get().reads(12_u64))
			.saturating_add(T::DbWeight::get().writes(7_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	/// The range of component `n` is `[1, 64]`.
	fn process_due(n: u32, ) -> Weight {
		Weight::from_parts(20_000_000, 20000)
			.saturating_add(Weight::from_parts(250_000_000, 40000).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().reads((14_u64).saturating_mul(n.into())))
			.saturating_add(T::DbWeight::get().writes(1_u64))
			.saturating_add(T::DbWeight::get().writes((9_u64).saturating_mul(n.into())))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn set_subscription_policy() -> Weight {
		Weight::from_parts(40_000_000, 8000)
			.saturating_add(T::DbWeight::get().reads(4_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn withdraw_subscription_conditions() -> Weight {
		Weight::from_parts(40_000_000, 8000)
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::CollectionRoleOf` (r:0 w:1)
	/// Proof: `ListingsCatalog::CollectionRoleOf` (`max_values`: None, `max_size`: Some(71), added: 2546, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::CollectionConfigOf` (r:0 w:1)
	/// Proof: `ListingsCatalog::CollectionConfigOf` (`max_values`: None, `max_size`: Some(77), added: 2552, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::CollectionAccount` (r:0 w:1)
	/// Proof: `ListingsCatalog::CollectionAccount` (`max_values`: None, `max_size`: Some(70), added: 2545, mode: `MaxEncodedLen`)
	fn create_inventory() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `6`
		//  Estimated: `3551`
		// Minimum execution time: 27_359_000 picoseconds.
		Weight::from_parts(28_328_000, 0)
			.saturating_add(Weight::from_parts(0, 3551))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(4))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:1 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	fn archive_inventory() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `219`
		//  Estimated: `3950`
		// Minimum execution time: 40_324_000 picoseconds.
		Weight::from_parts(42_295_000, 0)
			.saturating_add(Weight::from_parts(0, 3950))
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:3 w:2)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:1)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::CollectionConfigOf` (r:1 w:0)
	/// Proof: `ListingsCatalog::CollectionConfigOf` (`max_values`: None, `max_size`: Some(77), added: 2552, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::ItemConfigOf` (r:1 w:1)
	/// Proof: `ListingsCatalog::ItemConfigOf` (`max_values`: None, `max_size`: Some(54), added: 2529, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Account` (r:0 w:1)
	/// Proof: `ListingsCatalog::Account` (`max_values`: None, `max_size`: Some(94), added: 2569, mode: `MaxEncodedLen`)
	/// The range of component `q` is `[1, 216]`.
	fn publish_item(q: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `410`
		//  Estimated: `9870`
		// Minimum execution time: 116_758_000 picoseconds.
		Weight::from_parts(120_516_367, 0)
			.saturating_add(Weight::from_parts(0, 9870))
			// Standard Error: 1_501
			.saturating_add(Weight::from_parts(5_289, 0).saturating_mul(q.into()))
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().writes(7))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:2 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:0)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	fn set_item_price() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `533`
		//  Estimated: `6910`
		// Minimum execution time: 68_375_000 picoseconds.
		Weight::from_parts(70_371_000, 0)
			.saturating_add(Weight::from_parts(0, 6910))
			.saturating_add(RocksDbWeight::get().reads(4))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:2 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:0)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	fn clear_item_price() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `533`
		//  Estimated: `6910`
		// Minimum execution time: 68_375_000 picoseconds.
		Weight::from_parts(70_371_000, 0)
			.saturating_add(Weight::from_parts(0, 6910))
			.saturating_add(RocksDbWeight::get().reads(4))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:2 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:0)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	fn mark_item_can_transfer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `524`
		//  Estimated: `6910`
		// Minimum execution time: 52_472_000 picoseconds.
		Weight::from_parts(53_629_000, 0)
			.saturating_add(Weight::from_parts(0, 6910))
			.saturating_add(RocksDbWeight::get().reads(4))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:3 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:0)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	fn mark_item_not_for_resale() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `533`
		//  Estimated: `9870`
		// Minimum execution time: 61_894_000 picoseconds.
		Weight::from_parts(63_441_000, 0)
			.saturating_add(Weight::from_parts(0, 9870))
			.saturating_add(RocksDbWeight::get().reads(5))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:3 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:0)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	/// The range of component `p` is `[1, 59]`.
	/// The range of component `q` is `[1, 251]`.
	fn set_item_attribute(p: u32, q: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `533`
		//  Estimated: `9870`
		// Minimum execution time: 58_392_000 picoseconds.
		Weight::from_parts(59_944_382, 0)
			.saturating_add(Weight::from_parts(0, 9870))
			// Standard Error: 1_215
			.saturating_add(Weight::from_parts(12_741, 0).saturating_mul(p.into()))
			// Standard Error: 284
			.saturating_add(Weight::from_parts(2_035, 0).saturating_mul(q.into()))
			.saturating_add(RocksDbWeight::get().reads(5))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	/// Storage: `ListingsCatalog::Collection` (r:1 w:1)
	/// Proof: `ListingsCatalog::Collection` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Attribute` (r:3 w:1)
	/// Proof: `ListingsCatalog::Attribute` (`max_values`: None, `max_size`: Some(485), added: 2960, mode: `MaxEncodedLen`)
	/// Storage: `ListingsCatalog::Item` (r:1 w:0)
	/// Proof: `ListingsCatalog::Item` (`max_values`: None, `max_size`: Some(164), added: 2639, mode: `MaxEncodedLen`)
	/// The range of component `p` is `[1, 59]`.
	/// The range of component `q` is `[1, 251]`.
	fn clear_item_attribute(p: u32, q: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `579 + p * (1 ±0) + q * (1 ±0)`
		//  Estimated: `9870`
		// Minimum execution time: 54_111_000 picoseconds.
		Weight::from_parts(55_681_786, 0)
			.saturating_add(Weight::from_parts(0, 9870))
			// Standard Error: 1_226
			.saturating_add(Weight::from_parts(22_803, 0).saturating_mul(p.into()))
			// Standard Error: 286
			.saturating_add(Weight::from_parts(1_275, 0).saturating_mul(q.into()))
			.saturating_add(RocksDbWeight::get().reads(5))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn set_subscription_conditions() -> Weight {
		Weight::from_parts(60_000_000, 12000)
			.saturating_add(RocksDbWeight::get().reads(6_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn subscribe() -> Weight {
		Weight::from_parts(350_000_000, 70000)
			.saturating_add(RocksDbWeight::get().reads(22_u64))
			.saturating_add(RocksDbWeight::get().writes(13_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn charge_due() -> Weight {
		Weight::from_parts(250_000_000, 40000)
			.saturating_add(RocksDbWeight::get().reads(14_u64))
			.saturating_add(RocksDbWeight::get().writes(8_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn cancel_subscription() -> Weight {
		Weight::from_parts(120_000_000, 30000)
			.saturating_add(RocksDbWeight::get().reads(8_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn terminate_subscription() -> Weight {
		Weight::from_parts(100_000_000, 30000)
			.saturating_add(RocksDbWeight::get().reads(6_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn schedule_replacement() -> Weight {
		Weight::from_parts(200_000_000, 45000)
			.saturating_add(RocksDbWeight::get().reads(13_u64))
			.saturating_add(RocksDbWeight::get().writes(7_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn amend_subscription() -> Weight {
		Weight::from_parts(120_000_000, 30000)
			.saturating_add(RocksDbWeight::get().reads(10_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn cancel_replacement() -> Weight {
		Weight::from_parts(150_000_000, 45000)
			.saturating_add(RocksDbWeight::get().reads(12_u64))
			.saturating_add(RocksDbWeight::get().writes(6_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn amend_item_subscriptions() -> Weight {
		Weight::from_parts(70_000_000, 14000)
			.saturating_add(RocksDbWeight::get().reads(7_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn process_due_replacement() -> Weight {
		Weight::from_parts(450_000_000, 75000)
			.saturating_add(RocksDbWeight::get().reads(24_u64))
			.saturating_add(RocksDbWeight::get().writes(15_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn skip_empty_bucket() -> Weight {
		Weight::from_parts(10_000_000, 20000)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(0_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn process_due_overflow() -> Weight {
		Weight::from_parts(300_000_000, 60000)
			.saturating_add(RocksDbWeight::get().reads(18_u64))
			.saturating_add(RocksDbWeight::get().writes(11_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn settle_subscription() -> Weight {
		Weight::from_parts(180_000_000, 45000)
			.saturating_add(RocksDbWeight::get().reads(12_u64))
			.saturating_add(RocksDbWeight::get().writes(7_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	/// The range of component `n` is `[1, 64]`.
	fn process_due(n: u32, ) -> Weight {
		Weight::from_parts(20_000_000, 20000)
			.saturating_add(Weight::from_parts(250_000_000, 40000).saturating_mul(n.into()))
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().reads((14_u64).saturating_mul(n.into())))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
			.saturating_add(RocksDbWeight::get().writes((9_u64).saturating_mul(n.into())))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn set_subscription_policy() -> Weight {
		Weight::from_parts(40_000_000, 8000)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	// PLACEHOLDER: regenerated by /cmd bench on the PR
	fn withdraw_subscription_conditions() -> Weight {
		Weight::from_parts(40_000_000, 8000)
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
