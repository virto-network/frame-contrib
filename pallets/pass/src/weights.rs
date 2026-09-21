#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::{constants::RocksDbWeight, Weight}};

/// Weight functions needed for `fc_pallet_pass`.
pub trait WeightInfo {
	/// Does not include verifying the attestation, which the authenticator reports separately.
	fn register() -> Weight;
	/// Weight of `PassAuthenticate` when it carries a credential: authenticates the device,
	/// checks its call filter, and sets/clears the transient `AuthenticatedDevice` context.
	///
	/// Does not include the cost of verifying the credential itself, which the authenticator
	/// reports separately.
	fn authenticate() -> Weight;
	/// Weight of `PassAuthenticate` when it carries no credential (the path taken by every
	/// extrinsic that does not authenticate with a Pass device): at most one `SessionKeys`
	/// lookup, plus the session key's call filter check.
	fn authenticate_none() -> Weight;
	/// Does not include verifying the attestation, which the authenticator reports separately.
	fn add_device() -> Weight;
	fn remove_device() -> Weight;
	fn add_session_key() -> Weight;
	fn remove_session_key() -> Weight;
}

/// PLACEHOLDER weights for `fc_pallet_pass`: none of these come from a benchmark run on reference
/// hardware. They count the DB accesses of each code path, with conservative estimates for
/// execution time and proof size. Runtimes must generate their own weights with
/// `frame-omni-bencher` rather than rely on these.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// PLACEHOLDER: not produced by a benchmark run. Execution time and proof size are
	/// conservative estimates; DB accesses are counted from the code path, excluding
	/// `RegisterOrigin` and `RegistrarConsideration`/`DeviceConsideration` internals beyond one
	/// hold each:
	/// Storage: `Pass::RegistrarConsiderations` (r:1 w:1)
	/// Storage: `System::Account` (r:2 w:2)
	/// Storage: `Balances::Holds` (r:2 w:2)
	/// Storage: `Pass::DeviceIds` (r:1 w:1)
	/// Storage: `Pass::DeviceConsiderations` (r:1 w:1)
	/// Storage: `Pass::Devices` (r:0 w:1)
	/// Storage: `Pass::DeviceFilters` (r:0 w:1)
	///
	/// Does not include verifying the attestation, which the authenticator reports separately.
	fn register() -> Weight {
		Weight::from_parts(50_000_000, 20_000)
			.saturating_add(T::DbWeight::get().reads(7_u64))
			.saturating_add(T::DbWeight::get().writes(9_u64))
	}

	/// PLACEHOLDER: not produced by a benchmark run. Execution time and proof size are
	/// conservative estimates; DB accesses are counted from the code path:
	/// Storage: `Pass::Devices` (r:2 w:1)
	/// Storage: `Pass::DeviceFilters` (r:1 w:0)
	/// Storage: `Pass::AuthenticatedDevice` (r:0 w:2)
	fn authenticate() -> Weight {
		Weight::from_parts(20_000_000, 10_000)
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}

	/// PLACEHOLDER: not produced by a benchmark run. Execution time and proof size are
	/// conservative estimates; DB accesses are counted from the code path:
	/// Storage: `Pass::SessionKeys` (r:1 w:0)
	fn authenticate_none() -> Weight {
		Weight::from_parts(10_000_000, 4_000)
			.saturating_add(T::DbWeight::get().reads(1_u64))
	}

	/// PLACEHOLDER: not produced by a benchmark run. Execution time and proof size are
	/// conservative estimates; DB accesses are counted from the code path, excluding
	/// `DeviceConsideration` internals beyond one hold:
	/// Storage: `Pass::Devices` (r:2 w:1)
	/// Storage: `Pass::AuthenticatedDevice` (r:1 w:0)
	/// Storage: `Pass::DeviceFilters` (r:1 w:1)
	/// Storage: `Pass::DeviceIds` (r:1 w:1)
	/// Storage: `Pass::DeviceConsiderations` (r:1 w:1)
	/// Storage: `System::Account` (r:1 w:1)
	/// Storage: `Balances::Holds` (r:1 w:1)
	///
	/// Does not include verifying the attestation, which the authenticator reports separately.
	fn add_device() -> Weight {
		Weight::from_parts(100_000_000, 20_000)
			.saturating_add(T::DbWeight::get().reads(8_u64))
			.saturating_add(T::DbWeight::get().writes(6_u64))
	}

	/// PLACEHOLDER: not produced by a benchmark run. Execution time and proof size are
	/// conservative estimates; DB accesses are counted from the code path, excluding
	/// `DeviceConsideration` internals beyond one hold:
	/// Storage: `Pass::Devices` (r:2 w:1)
	/// Storage: `Pass::DeviceConsiderations` (r:1 w:1)
	/// Storage: `System::Account` (r:1 w:1)
	/// Storage: `Balances::Holds` (r:1 w:1)
	/// Storage: `Pass::DeviceIds` (r:0 w:1)
	/// Storage: `Pass::DeviceFilters` (r:0 w:1)
	fn remove_device() -> Weight {
		Weight::from_parts(100_000_000, 20_000)
			.saturating_add(T::DbWeight::get().reads(5_u64))
			.saturating_add(T::DbWeight::get().writes(6_u64))
	}

	/// PLACEHOLDER: not produced by a benchmark run. Execution time and proof size are
	/// conservative estimates (the proof size is dominated by `Scheduler::Agenda`, whose bound
	/// depends on the runtime); DB accesses are counted from the code path, for the worst case
	/// where the session key replaces an existing one:
	/// Storage: `Pass::Devices` (r:2 w:0)
	/// Storage: `Pass::AuthenticatedDevice` (r:1 w:0)
	/// Storage: `Pass::DeviceFilters` (r:1 w:0)
	/// Storage: `System::Account` (r:1 w:0)
	/// Storage: `Pass::SessionKeys` (r:2 w:2)
	/// Storage: `Pass::CounterForSessionKeys` (r:2 w:2)
	/// Storage: `Pass::AccountSessionsCount` (r:3 w:2)
	/// Storage: `Scheduler::Lookup` (r:3 w:3)
	/// Storage: `Scheduler::Agenda` (r:3 w:3)
	fn add_session_key() -> Weight {
		Weight::from_parts(100_000_000, 200_000)
			.saturating_add(T::DbWeight::get().reads(18_u64))
			.saturating_add(T::DbWeight::get().writes(12_u64))
	}

	/// PLACEHOLDER: not produced by a benchmark run. Execution time and proof size are
	/// conservative estimates (the proof size is dominated by `Scheduler::Agenda`, whose bound
	/// depends on the runtime); DB accesses are counted from the code path:
	/// Storage: `Scheduler::Lookup` (r:1 w:1)
	/// Storage: `Scheduler::Agenda` (r:1 w:1)
	/// Storage: `Pass::SessionKeys` (r:1 w:1)
	/// Storage: `Pass::CounterForSessionKeys` (r:1 w:1)
	/// Storage: `Pass::AccountSessionsCount` (r:1 w:1)
	fn remove_session_key() -> Weight {
		Weight::from_parts(100_000_000, 200_000)
			.saturating_add(T::DbWeight::get().reads(5_u64))
			.saturating_add(T::DbWeight::get().writes(5_u64))
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
		Weight::from_parts(20_000_000, 10_000)
			.saturating_add(RocksDbWeight::get().reads(3_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
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
