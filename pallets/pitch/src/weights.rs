#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use core::marker::PhantomData;
use frame::deps::{frame_support::weights::Weight, frame_system};

pub trait WeightInfo {
	fn claim() -> Weight;
	fn join() -> Weight;
	fn amend() -> Weight;
	fn dissolve() -> Weight;
	fn grant_disclosure() -> Weight;
	fn reap() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn claim() -> Weight {
		Weight::from_parts(20_000_000, 0)
	}
	fn join() -> Weight {
		Weight::from_parts(10_000_000, 0)
	}
	fn amend() -> Weight {
		Weight::from_parts(15_000_000, 0)
	}
	fn dissolve() -> Weight {
		Weight::from_parts(15_000_000, 0)
	}
	fn grant_disclosure() -> Weight {
		Weight::from_parts(10_000_000, 0)
	}
	fn reap() -> Weight {
		Weight::from_parts(15_000_000, 0)
	}
}

impl WeightInfo for () {
	fn claim() -> Weight {
		Weight::from_parts(20_000_000, 0)
	}
	fn join() -> Weight {
		Weight::from_parts(10_000_000, 0)
	}
	fn amend() -> Weight {
		Weight::from_parts(15_000_000, 0)
	}
	fn dissolve() -> Weight {
		Weight::from_parts(15_000_000, 0)
	}
	fn grant_disclosure() -> Weight {
		Weight::from_parts(10_000_000, 0)
	}
	fn reap() -> Weight {
		Weight::from_parts(15_000_000, 0)
	}
}
