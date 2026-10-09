//! Weights for `fc-pallet-spaces`.
//!
//! TODO: placeholders, not generated. Replace them with `frame-omni-bencher` output on reference
//! hardware (see `benchmarking.rs`). The proof verifier's own weight is not here: it is
//! [`ProofVerifier::weight`](crate::ProofVerifier::weight), added to `anchor` by the call.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for `fc-pallet-spaces`.
pub trait WeightInfo {
	fn register() -> Weight;
	/// `q`: the public input's length.
	fn anchor(q: u32) -> Weight;
	fn anchor_replay() -> Weight;
	fn refound() -> Weight;
	fn set_current_head() -> Weight;
	fn set_program() -> Weight;
	fn set_authority() -> Weight;
}

/// Placeholder weights for a runtime. TODO: generate.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: `Spaces::NextSpaceId` (r:1 w:1), `Spaces::Spaces` (w:1), `Spaces::Programs` (w:1), `Spaces::Heads` (w:1), `Spaces::Epochs` (w:1)
	fn register() -> Weight {
		Weight::from_parts(25_000_000, 3_600)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(5_u64))
	}
	/// Storage: `Spaces::Spaces` (r:1), `Spaces::Heads` (r:1 w:1), `Spaces::Anchors` (r:1 w:1).
	/// Hashing the public input (twice, for the io-hash) grows with `q`.
	fn anchor(q: u32) -> Weight {
		Weight::from_parts(40_000_000, 3_600)
			.saturating_add(Weight::from_parts(2_000, 0).saturating_mul(q.into()))
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: `Spaces::Spaces` (r:1), `Spaces::Heads` (r:1), `Spaces::Anchors` (r:1)
	fn anchor_replay() -> Weight {
		Weight::from_parts(20_000_000, 3_600)
			.saturating_add(T::DbWeight::get().reads(3_u64))
	}
	/// Storage: `Spaces::Spaces` (r:1), `Spaces::Heads` (r:1 w:1), `Spaces::Epochs` (r:0 w:1)
	fn refound() -> Weight {
		Weight::from_parts(25_000_000, 3_600)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: `Spaces::Spaces` (r:1), `Spaces::Heads` (r:1 w:1), `Spaces::Epochs` (r:0 w:1)
	fn set_current_head() -> Weight {
		Weight::from_parts(25_000_000, 3_600)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: `Spaces::Spaces` (r:1 w:1), `Spaces::Heads` (r:1), `Spaces::Programs` (r:0 w:1)
	fn set_program() -> Weight {
		Weight::from_parts(25_000_000, 3_600)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: `Spaces::Spaces` (r:1 w:1)
	fn set_authority() -> Weight {
		Weight::from_parts(20_000_000, 3_600)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

// For backwards compatibility and tests.
impl WeightInfo for () {
	fn register() -> Weight {
		Weight::from_parts(25_000_000, 3_600)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	fn anchor(q: u32) -> Weight {
		Weight::from_parts(40_000_000, 3_600)
			.saturating_add(Weight::from_parts(2_000, 0).saturating_mul(q.into()))
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	fn anchor_replay() -> Weight {
		Weight::from_parts(20_000_000, 3_600)
			.saturating_add(RocksDbWeight::get().reads(3_u64))
	}
	fn refound() -> Weight {
		Weight::from_parts(25_000_000, 3_600)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	fn set_current_head() -> Weight {
		Weight::from_parts(25_000_000, 3_600)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: `Spaces::Spaces` (r:1 w:1), `Spaces::Heads` (r:1), `Spaces::Programs` (r:0 w:1)
	fn set_program() -> Weight {
		Weight::from_parts(25_000_000, 3_600)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: `Spaces::Spaces` (r:1 w:1)
	fn set_authority() -> Weight {
		Weight::from_parts(20_000_000, 3_600)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
