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
	fn publish_item() -> Weight;
	fn set_item_price() -> Weight;
	fn mark_item_can_transfer() -> Weight;
	fn mark_item_not_for_resale() -> Weight;
	fn set_item_attribute() -> Weight;
	fn clear_item_attribute() -> Weight;
}

/// Weights for `pallet_listings` using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn create_inventory() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn archive_inventory() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn publish_item() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn mark_item_can_transfer() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn mark_item_not_for_resale() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn set_item_price() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn set_item_attribute() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn clear_item_attribute() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn create_inventory() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn archive_inventory() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn publish_item() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn mark_item_can_transfer() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn mark_item_not_for_resale() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn set_item_price() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn set_item_attribute() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
	fn clear_item_attribute() -> Weight {
		Weight::from_parts(8_586_000, 0)
			// Standard Error: 0
			.saturating_add(Weight::from_parts(1_359, 0))
	}
}
