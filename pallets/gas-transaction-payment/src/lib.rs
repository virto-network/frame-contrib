#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Gas Burn
//!
//! This pallet exposes a mechanism to allow transactions payment using a prepaid fees mechanism.

use frame_contrib_traits::gas_tank::{GasBurner, GasFueler};
use frame_support::pallet_prelude::*;
use sp_runtime::traits::TransactionExtension;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod extensions;
mod weights;

pub use extensions::*;
pub use pallet::*;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_contrib_traits::gas_tank::GasFueler;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching runtime event type
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The Weight info
        type WeightInfo: WeightInfo;
        /// A type that handles gas tanks
        type GasTank: GasFueler<Gas = Weight> + GasBurner<AccountId = Self::AccountId, Gas = Weight>;
        /// A helper to prepare benchmarking tests
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: BenchmarkHelper<Self>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        GasBurned {
            who: T::AccountId,
            remaining: Weight,
        },
    }
}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<T: Config> {
    type Ext: TransactionExtension<T::RuntimeCall>;

    /// An instance of the extension, ready to be used.
    fn ext() -> ChargeTransactionPayment<T, Self::Ext>;
}
