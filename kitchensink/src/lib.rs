//! # FRAME Contrib kitchensink runtime
//!
//! A runtime that includes every FRAME Contrib pallet, plus the Polkadot SDK pallets they depend
//! on, with a plain and generic configuration. Its only purpose is to generate the default
//! `SubstrateWeight`s of the pallets on reference hardware with `frame-omni-bencher`, the same way
//! the Polkadot SDK generates the FRAME defaults from its own `kitchensink-runtime`:
//!
//! ```sh
//! cargo build --release -p fc-kitchensink-runtime --features runtime-benchmarks
//! frame-omni-bencher v1 benchmark pallet \
//!   --runtime target/release/wbuild/fc-kitchensink-runtime/fc_kitchensink_runtime.compact.compressed.wasm \
//!   --pallet fc_pallet_pass --extrinsic "*" \
//!   --steps 50 --repeat 20 --wasm-execution compiled --heap-pages 4096 \
//!   --template .maintain/frame-weight-template.hbs --header .maintain/file_header.txt \
//!   --output pallets/pass/src/weights.rs
//! ```
//!
//! Recent `rustc`s need `WASM_BUILD_RUSTFLAGS="-C link-arg=--allow-undefined"` to link the
//! runtime's WASM blob, which imports its host functions.
//!
//! It is not meant to run a chain: there's no consensus, no governance beyond `Root`, and no
//! migrations.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit.
#![recursion_limit = "512"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

extern crate alloc;

use alloc::vec::Vec;

mod apis;
mod configs;
pub mod genesis_config_presets;

use sp_runtime::{
    generic,
    traits::{BlakeTwo256, IdentifyAccount, Verify},
    MultiAddress, MultiSignature,
};
use sp_version::RuntimeVersion;

pub use configs::*;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;
/// Some way of identifying an account on the chain.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
/// Balance of an account.
pub type Balance = u128;
/// Index of a transaction in the chain.
pub type Nonce = u32;
/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;
/// An index to a block.
pub type BlockNumber = u32;
/// The address format for describing accounts.
pub type Address = MultiAddress<AccountId, ()>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// The transaction extensions that are added to the runtime.
pub type TxExtension = (
    frame_system::AuthorizeCall<Runtime>,
    fc_pallet_pass::PassAuthenticate<Runtime>,
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    fc_pallet_gas_transaction_payment::ChargeTransactionPayment<
        Runtime,
        pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
    >,
    fc_pallet_fees::ChargeFees<Runtime>,
    frame_system::WeightReclaim<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, TxExtension>;

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
>;

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: alloc::borrow::Cow::Borrowed("fc-kitchensink"),
    impl_name: alloc::borrow::Cow::Borrowed("fc-kitchensink"),
    authoring_version: 1,
    spec_version: 1,
    impl_version: 1,
    apis: apis::RUNTIME_API_VERSIONS,
    transaction_version: 1,
    system_version: 1,
};

#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask,
        RuntimeViewFunction
    )]
    pub struct Runtime;

    // System
    #[runtime::pallet_index(0)]
    pub type System = frame_system;
    #[runtime::pallet_index(1)]
    pub type Timestamp = pallet_timestamp;
    #[runtime::pallet_index(2)]
    pub type Scheduler = pallet_scheduler;
    #[runtime::pallet_index(3)]
    pub type Preimage = pallet_preimage;

    // Monetary
    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type TransactionPayment = pallet_transaction_payment;
    #[runtime::pallet_index(12)]
    pub type Assets = pallet_assets;
    #[runtime::pallet_index(13)]
    pub type AssetsFreezer = pallet_assets_freezer;
    #[runtime::pallet_index(14)]
    pub type AssetsHolder = pallet_assets_holder;
    #[runtime::pallet_index(15)]
    pub type GasTransactionPayment = fc_pallet_gas_transaction_payment;
    #[runtime::pallet_index(16)]
    pub type BlackHole = fc_pallet_black_hole;
    #[runtime::pallet_index(17)]
    pub type Fees = fc_pallet_fees;

    // Accounts
    #[runtime::pallet_index(20)]
    pub type Pass = fc_pallet_pass;

    // Communities
    #[runtime::pallet_index(30)]
    pub type Communities = fc_pallet_communities;
    #[runtime::pallet_index(31)]
    pub type Memberships = pallet_nfts<Instance1>;
    #[runtime::pallet_index(32)]
    pub type CommunityReferenda = pallet_referenda;
    #[runtime::pallet_index(33)]
    pub type CommunityTracks = fc_pallet_referenda_tracks;

    // Marketplace
    #[runtime::pallet_index(40)]
    pub type Listings = fc_pallet_listings;
    #[runtime::pallet_index(41)]
    pub type ListingsCatalog = pallet_nfts<Instance2>;
    #[runtime::pallet_index(42)]
    pub type Payments = fc_pallet_payments;
    #[runtime::pallet_index(43)]
    pub type Orders = fc_pallet_orders;
}

#[cfg(feature = "runtime-benchmarks")]
mod benches {
    frame_benchmarking::define_benchmarks!(
        [fc_pallet_black_hole, BlackHole]
        [fc_pallet_communities, Communities]
        [fc_pallet_gas_transaction_payment, GasTransactionPayment]
        [fc_pallet_listings, Listings]
        [fc_pallet_orders, Orders]
        [fc_pallet_pass, Pass]
        [fc_pallet_payments, Payments]
        [fc_pallet_referenda_tracks, CommunityTracks]
    );
}
