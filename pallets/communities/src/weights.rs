//! Autogenerated weights for `pallet_communities`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2024-04-08, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
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
// pallet_communities
// --extrinsic
// *
// --steps
// 50
// --repeat
// 20
// --output
// runtime/kreivo/src/weights/pallet_communities.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::{constants::RocksDbWeight, Weight}};

/// Weight functions needed for pallet_communities.
pub trait WeightInfo {
	fn create() -> Weight;
	fn add_member() -> Weight;
	fn set_admin_origin() -> Weight;
	fn set_decision_method() -> Weight;
	fn promote() -> Weight;
	fn demote() -> Weight;
	fn remove_member() -> Weight;
	fn vote() -> Weight;
	fn remove_vote() -> Weight;
	fn unlock() -> Weight;
	fn dispatch_as_account() -> Weight;
}

/// Weights for pallet_communities using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: `Communities::Info` (r:1 w:1)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityIdFor` (r:0 w:1)
	/// Proof: `Communities::CommunityIdFor` (`max_values`: None, `max_size`: Some(622), added: 3097, mode: `MaxEncodedLen`)
	fn create() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `56`
		//  Estimated: `3593`
		// Minimum execution time: 39_254_000 picoseconds.
		Weight::from_parts(44_989_000, 0)
			.saturating_add(Weight::from_parts(0, 3593))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `Communities::CommunityIdFor` (r:1 w:2)
	/// Proof: `Communities::CommunityIdFor` (`max_values`: None, `max_size`: Some(622), added: 3097, mode: `MaxEncodedLen`)
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	fn set_admin_origin() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `122`
		//  Estimated: `4087`
		// Minimum execution time: 39_264_000 picoseconds.
		Weight::from_parts(59_388_000, 0)
			.saturating_add(Weight::from_parts(0, 4087))
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityDecisionMethod` (r:0 w:1)
	/// Proof: `Communities::CommunityDecisionMethod` (`max_values`: None, `max_size`: Some(37), added: 2512, mode: `MaxEncodedLen`)
	fn set_decision_method() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `115`
		//  Estimated: `3484`
		// Minimum execution time: 24_894_000 picoseconds.
		Weight::from_parts(36_775_000, 0)
			.saturating_add(Weight::from_parts(0, 3484))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Account` (r:1 w:2)
	/// Proof: `CommunityMemberships::Account` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Attribute` (r:3 w:2)
	/// Proof: `CommunityMemberships::Attribute` (`max_values`: None, `max_size`: Some(477), added: 2952, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemConfigOf` (r:2 w:2)
	/// Proof: `CommunityMemberships::ItemConfigOf` (`max_values`: None, `max_size`: Some(46), added: 2521, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Collection` (r:2 w:2)
	/// Proof: `CommunityMemberships::Collection` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Item` (r:2 w:2)
	/// Proof: `CommunityMemberships::Item` (`max_values`: None, `max_size`: Some(859), added: 3334, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemMetadataOf` (r:1 w:0)
	/// Proof: `CommunityMemberships::ItemMetadataOf` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::CollectionConfigOf` (r:1 w:0)
	/// Proof: `CommunityMemberships::CollectionConfigOf` (`max_values`: None, `max_size`: Some(69), added: 2544, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemPriceOf` (r:0 w:1)
	/// Proof: `CommunityMemberships::ItemPriceOf` (`max_values`: None, `max_size`: Some(87), added: 2562, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemAttributesApprovalsOf` (r:0 w:1)
	/// Proof: `CommunityMemberships::ItemAttributesApprovalsOf` (`max_values`: None, `max_size`: Some(999), added: 3474, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::PendingSwapOf` (r:0 w:1)
	/// Proof: `CommunityMemberships::PendingSwapOf` (`max_values`: None, `max_size`: Some(67), added: 2542, mode: `MaxEncodedLen`)
	fn add_member() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `880`
		//  Estimated: `9846`
		// Minimum execution time: 245_334_000 picoseconds.
		Weight::from_parts(360_487_000, 0)
			.saturating_add(Weight::from_parts(0, 9846))
			.saturating_add(T::DbWeight::get().reads(13))
			.saturating_add(T::DbWeight::get().writes(13))
	}
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Account` (r:1 w:1)
	/// Proof: `CommunityMemberships::Account` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Attribute` (r:4 w:3)
	/// Proof: `CommunityMemberships::Attribute` (`max_values`: None, `max_size`: Some(477), added: 2952, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Collection` (r:1 w:1)
	/// Proof: `CommunityMemberships::Collection` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemConfigOf` (r:1 w:1)
	/// Proof: `CommunityMemberships::ItemConfigOf` (`max_values`: None, `max_size`: Some(46), added: 2521, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Item` (r:1 w:1)
	/// Proof: `CommunityMemberships::Item` (`max_values`: None, `max_size`: Some(859), added: 3334, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemMetadataOf` (r:1 w:0)
	/// Proof: `CommunityMemberships::ItemMetadataOf` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemPriceOf` (r:0 w:1)
	/// Proof: `CommunityMemberships::ItemPriceOf` (`max_values`: None, `max_size`: Some(87), added: 2562, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemAttributesApprovalsOf` (r:0 w:1)
	/// Proof: `CommunityMemberships::ItemAttributesApprovalsOf` (`max_values`: None, `max_size`: Some(999), added: 3474, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::PendingSwapOf` (r:0 w:1)
	/// Proof: `CommunityMemberships::PendingSwapOf` (`max_values`: None, `max_size`: Some(67), added: 2542, mode: `MaxEncodedLen`)
	fn remove_member() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1006`
		//  Estimated: `12798`
		// Minimum execution time: 317_361_000 picoseconds.
		Weight::from_parts(465_610_000, 0)
			.saturating_add(Weight::from_parts(0, 12798))
			.saturating_add(T::DbWeight::get().reads(10))
			.saturating_add(T::DbWeight::get().writes(10))
	}
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Attribute` (r:2 w:2)
	/// Proof: `CommunityMemberships::Attribute` (`max_values`: None, `max_size`: Some(477), added: 2952, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Collection` (r:1 w:1)
	/// Proof: `CommunityMemberships::Collection` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	fn promote() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `717`
		//  Estimated: `6894`
		// Minimum execution time: 134_868_000 picoseconds.
		Weight::from_parts(158_292_000, 0)
			.saturating_add(Weight::from_parts(0, 6894))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Attribute` (r:2 w:2)
	/// Proof: `CommunityMemberships::Attribute` (`max_values`: None, `max_size`: Some(477), added: 2952, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Collection` (r:1 w:1)
	/// Proof: `CommunityMemberships::Collection` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	fn demote() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `755`
		//  Estimated: `6894`
		// Minimum execution time: 198_275_000 picoseconds.
		Weight::from_parts(202_790_000, 0)
			.saturating_add(Weight::from_parts(0, 6894))
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `CommunityMemberships::Account` (r:1 w:0)
	/// Proof: `CommunityMemberships::Account` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityDecisionMethod` (r:1 w:0)
	/// Proof: `Communities::CommunityDecisionMethod` (`max_values`: None, `max_size`: Some(37), added: 2512, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityVotes` (r:1 w:1)
	/// Proof: `Communities::CommunityVotes` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `CommunityReferenda::ReferendumInfoFor` (r:1 w:1)
	/// Proof: `CommunityReferenda::ReferendumInfoFor` (`max_values`: None, `max_size`: Some(900), added: 3375, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityVoteLocks` (r:2 w:1)
	/// Proof: `Communities::CommunityVoteLocks` (`max_values`: None, `max_size`: Some(104), added: 2579, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Holds` (r:1 w:1)
	/// Proof: `Assets::Holds` (`max_values`: None, `max_size`: Some(983), added: 3458, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(224), added: 2699, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:1 w:1)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(148), added: 2623, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn vote() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3035`
		//  Estimated: `6148`
		// Minimum execution time: 393_947_000 picoseconds.
		Weight::from_parts(476_186_000, 0)
			.saturating_add(Weight::from_parts(0, 6148))
			.saturating_add(T::DbWeight::get().reads(10))
			.saturating_add(T::DbWeight::get().writes(6))
	}
	/// Storage: `CommunityMemberships::Account` (r:1 w:0)
	/// Proof: `CommunityMemberships::Account` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityDecisionMethod` (r:1 w:0)
	/// Proof: `Communities::CommunityDecisionMethod` (`max_values`: None, `max_size`: Some(37), added: 2512, mode: `MaxEncodedLen`)
	/// Storage: `CommunityReferenda::ReferendumInfoFor` (r:1 w:1)
	/// Proof: `CommunityReferenda::ReferendumInfoFor` (`max_values`: None, `max_size`: Some(900), added: 3375, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityVotes` (r:1 w:1)
	/// Proof: `Communities::CommunityVotes` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Attribute` (r:1 w:0)
	/// Proof: `CommunityMemberships::Attribute` (`max_values`: None, `max_size`: Some(477), added: 2952, mode: `MaxEncodedLen`)
	fn remove_vote() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2898`
		//  Estimated: `4365`
		// Minimum execution time: 178_506_000 picoseconds.
		Weight::from_parts(241_482_000, 0)
			.saturating_add(Weight::from_parts(0, 4365))
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	/// Storage: `CommunityReferenda::ReferendumInfoFor` (r:1 w:0)
	/// Proof: `CommunityReferenda::ReferendumInfoFor` (`max_values`: None, `max_size`: Some(900), added: 3375, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityVoteLocks` (r:2 w:1)
	/// Proof: `Communities::CommunityVoteLocks` (`max_values`: None, `max_size`: Some(104), added: 2579, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:1)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(4658), added: 7133, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:0)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	fn unlock() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1082`
		//  Estimated: `8123`
		// Minimum execution time: 164_095_000 picoseconds.
		Weight::from_parts(219_358_000, 0)
			.saturating_add(Weight::from_parts(0, 8123))
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	fn dispatch_as_account() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `115`
		//  Estimated: `3484`
		// Minimum execution time: 33_857_000 picoseconds.
		Weight::from_parts(47_890_000, 0)
			.saturating_add(Weight::from_parts(0, 3484))
			.saturating_add(T::DbWeight::get().reads(1))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// Storage: `Communities::Info` (r:1 w:1)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityIdFor` (r:0 w:1)
	/// Proof: `Communities::CommunityIdFor` (`max_values`: None, `max_size`: Some(622), added: 3097, mode: `MaxEncodedLen`)
	fn create() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `56`
		//  Estimated: `3593`
		// Minimum execution time: 39_254_000 picoseconds.
		Weight::from_parts(44_989_000, 0)
			.saturating_add(Weight::from_parts(0, 3593))
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(3))
	}
	/// Storage: `Communities::CommunityIdFor` (r:1 w:2)
	/// Proof: `Communities::CommunityIdFor` (`max_values`: None, `max_size`: Some(622), added: 3097, mode: `MaxEncodedLen`)
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	fn set_admin_origin() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `122`
		//  Estimated: `4087`
		// Minimum execution time: 39_264_000 picoseconds.
		Weight::from_parts(59_388_000, 0)
			.saturating_add(Weight::from_parts(0, 4087))
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityDecisionMethod` (r:0 w:1)
	/// Proof: `Communities::CommunityDecisionMethod` (`max_values`: None, `max_size`: Some(37), added: 2512, mode: `MaxEncodedLen`)
	fn set_decision_method() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `115`
		//  Estimated: `3484`
		// Minimum execution time: 24_894_000 picoseconds.
		Weight::from_parts(36_775_000, 0)
			.saturating_add(Weight::from_parts(0, 3484))
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Account` (r:1 w:2)
	/// Proof: `CommunityMemberships::Account` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Attribute` (r:3 w:2)
	/// Proof: `CommunityMemberships::Attribute` (`max_values`: None, `max_size`: Some(477), added: 2952, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemConfigOf` (r:2 w:2)
	/// Proof: `CommunityMemberships::ItemConfigOf` (`max_values`: None, `max_size`: Some(46), added: 2521, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Collection` (r:2 w:2)
	/// Proof: `CommunityMemberships::Collection` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Item` (r:2 w:2)
	/// Proof: `CommunityMemberships::Item` (`max_values`: None, `max_size`: Some(859), added: 3334, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemMetadataOf` (r:1 w:0)
	/// Proof: `CommunityMemberships::ItemMetadataOf` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::CollectionConfigOf` (r:1 w:0)
	/// Proof: `CommunityMemberships::CollectionConfigOf` (`max_values`: None, `max_size`: Some(69), added: 2544, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemPriceOf` (r:0 w:1)
	/// Proof: `CommunityMemberships::ItemPriceOf` (`max_values`: None, `max_size`: Some(87), added: 2562, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemAttributesApprovalsOf` (r:0 w:1)
	/// Proof: `CommunityMemberships::ItemAttributesApprovalsOf` (`max_values`: None, `max_size`: Some(999), added: 3474, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::PendingSwapOf` (r:0 w:1)
	/// Proof: `CommunityMemberships::PendingSwapOf` (`max_values`: None, `max_size`: Some(67), added: 2542, mode: `MaxEncodedLen`)
	fn add_member() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `880`
		//  Estimated: `9846`
		// Minimum execution time: 245_334_000 picoseconds.
		Weight::from_parts(360_487_000, 0)
			.saturating_add(Weight::from_parts(0, 9846))
			.saturating_add(RocksDbWeight::get().reads(13))
			.saturating_add(RocksDbWeight::get().writes(13))
	}
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Account` (r:1 w:1)
	/// Proof: `CommunityMemberships::Account` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Attribute` (r:4 w:3)
	/// Proof: `CommunityMemberships::Attribute` (`max_values`: None, `max_size`: Some(477), added: 2952, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Collection` (r:1 w:1)
	/// Proof: `CommunityMemberships::Collection` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemConfigOf` (r:1 w:1)
	/// Proof: `CommunityMemberships::ItemConfigOf` (`max_values`: None, `max_size`: Some(46), added: 2521, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Item` (r:1 w:1)
	/// Proof: `CommunityMemberships::Item` (`max_values`: None, `max_size`: Some(859), added: 3334, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemMetadataOf` (r:1 w:0)
	/// Proof: `CommunityMemberships::ItemMetadataOf` (`max_values`: None, `max_size`: Some(345), added: 2820, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemPriceOf` (r:0 w:1)
	/// Proof: `CommunityMemberships::ItemPriceOf` (`max_values`: None, `max_size`: Some(87), added: 2562, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::ItemAttributesApprovalsOf` (r:0 w:1)
	/// Proof: `CommunityMemberships::ItemAttributesApprovalsOf` (`max_values`: None, `max_size`: Some(999), added: 3474, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::PendingSwapOf` (r:0 w:1)
	/// Proof: `CommunityMemberships::PendingSwapOf` (`max_values`: None, `max_size`: Some(67), added: 2542, mode: `MaxEncodedLen`)
	fn remove_member() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1006`
		//  Estimated: `12798`
		// Minimum execution time: 317_361_000 picoseconds.
		Weight::from_parts(465_610_000, 0)
			.saturating_add(Weight::from_parts(0, 12798))
			.saturating_add(RocksDbWeight::get().reads(10))
			.saturating_add(RocksDbWeight::get().writes(10))
	}
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Attribute` (r:2 w:2)
	/// Proof: `CommunityMemberships::Attribute` (`max_values`: None, `max_size`: Some(477), added: 2952, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Collection` (r:1 w:1)
	/// Proof: `CommunityMemberships::Collection` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	fn promote() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `717`
		//  Estimated: `6894`
		// Minimum execution time: 134_868_000 picoseconds.
		Weight::from_parts(158_292_000, 0)
			.saturating_add(Weight::from_parts(0, 6894))
			.saturating_add(RocksDbWeight::get().reads(4))
			.saturating_add(RocksDbWeight::get().writes(3))
	}
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Attribute` (r:2 w:2)
	/// Proof: `CommunityMemberships::Attribute` (`max_values`: None, `max_size`: Some(477), added: 2952, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Collection` (r:1 w:1)
	/// Proof: `CommunityMemberships::Collection` (`max_values`: None, `max_size`: Some(82), added: 2557, mode: `MaxEncodedLen`)
	fn demote() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `755`
		//  Estimated: `6894`
		// Minimum execution time: 198_275_000 picoseconds.
		Weight::from_parts(202_790_000, 0)
			.saturating_add(Weight::from_parts(0, 6894))
			.saturating_add(RocksDbWeight::get().reads(4))
			.saturating_add(RocksDbWeight::get().writes(3))
	}
	/// Storage: `CommunityMemberships::Account` (r:1 w:0)
	/// Proof: `CommunityMemberships::Account` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityDecisionMethod` (r:1 w:0)
	/// Proof: `Communities::CommunityDecisionMethod` (`max_values`: None, `max_size`: Some(37), added: 2512, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityVotes` (r:1 w:1)
	/// Proof: `Communities::CommunityVotes` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `CommunityReferenda::ReferendumInfoFor` (r:1 w:1)
	/// Proof: `CommunityReferenda::ReferendumInfoFor` (`max_values`: None, `max_size`: Some(900), added: 3375, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityVoteLocks` (r:2 w:1)
	/// Proof: `Communities::CommunityVoteLocks` (`max_values`: None, `max_size`: Some(104), added: 2579, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Holds` (r:1 w:1)
	/// Proof: `Assets::Holds` (`max_values`: None, `max_size`: Some(983), added: 3458, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Asset` (r:1 w:1)
	/// Proof: `Assets::Asset` (`max_values`: None, `max_size`: Some(224), added: 2699, mode: `MaxEncodedLen`)
	/// Storage: `Assets::Account` (r:1 w:1)
	/// Proof: `Assets::Account` (`max_values`: None, `max_size`: Some(148), added: 2623, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn vote() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3035`
		//  Estimated: `6148`
		// Minimum execution time: 393_947_000 picoseconds.
		Weight::from_parts(476_186_000, 0)
			.saturating_add(Weight::from_parts(0, 6148))
			.saturating_add(RocksDbWeight::get().reads(10))
			.saturating_add(RocksDbWeight::get().writes(6))
	}
	/// Storage: `CommunityMemberships::Account` (r:1 w:0)
	/// Proof: `CommunityMemberships::Account` (`max_values`: None, `max_size`: Some(86), added: 2561, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityDecisionMethod` (r:1 w:0)
	/// Proof: `Communities::CommunityDecisionMethod` (`max_values`: None, `max_size`: Some(37), added: 2512, mode: `MaxEncodedLen`)
	/// Storage: `CommunityReferenda::ReferendumInfoFor` (r:1 w:1)
	/// Proof: `CommunityReferenda::ReferendumInfoFor` (`max_values`: None, `max_size`: Some(900), added: 3375, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityVotes` (r:1 w:1)
	/// Proof: `Communities::CommunityVotes` (`max_values`: None, `max_size`: Some(108), added: 2583, mode: `MaxEncodedLen`)
	/// Storage: `CommunityMemberships::Attribute` (r:1 w:0)
	/// Proof: `CommunityMemberships::Attribute` (`max_values`: None, `max_size`: Some(477), added: 2952, mode: `MaxEncodedLen`)
	fn remove_vote() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2898`
		//  Estimated: `4365`
		// Minimum execution time: 178_506_000 picoseconds.
		Weight::from_parts(241_482_000, 0)
			.saturating_add(Weight::from_parts(0, 4365))
			.saturating_add(RocksDbWeight::get().reads(5))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
	/// Storage: `CommunityReferenda::ReferendumInfoFor` (r:1 w:0)
	/// Proof: `CommunityReferenda::ReferendumInfoFor` (`max_values`: None, `max_size`: Some(900), added: 3375, mode: `MaxEncodedLen`)
	/// Storage: `Communities::CommunityVoteLocks` (r:2 w:1)
	/// Proof: `Communities::CommunityVoteLocks` (`max_values`: None, `max_size`: Some(104), added: 2579, mode: `MaxEncodedLen`)
	/// Storage: `System::Account` (r:1 w:1)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Freezes` (r:1 w:1)
	/// Proof: `Balances::Freezes` (`max_values`: None, `max_size`: Some(4658), added: 7133, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Locks` (r:1 w:0)
	/// Proof: `Balances::Locks` (`max_values`: None, `max_size`: Some(1299), added: 3774, mode: `MaxEncodedLen`)
	fn unlock() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1082`
		//  Estimated: `8123`
		// Minimum execution time: 164_095_000 picoseconds.
		Weight::from_parts(219_358_000, 0)
			.saturating_add(Weight::from_parts(0, 8123))
			.saturating_add(RocksDbWeight::get().reads(6))
			.saturating_add(RocksDbWeight::get().writes(3))
	}
	/// Storage: `Communities::Info` (r:1 w:0)
	/// Proof: `Communities::Info` (`max_values`: None, `max_size`: Some(19), added: 2494, mode: `MaxEncodedLen`)
	fn dispatch_as_account() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `115`
		//  Estimated: `3484`
		// Minimum execution time: 33_857_000 picoseconds.
		Weight::from_parts(47_890_000, 0)
			.saturating_add(Weight::from_parts(0, 3484))
			.saturating_add(RocksDbWeight::get().reads(1))
	}
}
