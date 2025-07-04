#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame::weights_prelude::*;

/// Weight functions needed for fc_pallet_black_hole.
pub trait WeightInfo {
	fn dispatch_as_event_horizon() -> Weight;
	fn burn() -> Weight;
}

/// Weights for fc_pallet_black_hole using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn dispatch_as_event_horizon() -> Weight {
		Weight::from_parts(8_586_000, 0)
	}
	fn burn() -> Weight {
		Weight::from_parts(1_359, 0)
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn dispatch_as_event_horizon() -> Weight {
		Weight::from_parts(8_586_000, 0)
	}
	fn burn() -> Weight {
		Weight::from_parts(1_359, 0)
	}
}