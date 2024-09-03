#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Gas Burn
//!
//! This pallet exposes a mechanism to allow transactions payment using a prepaid fees mechanism.

use fc_traits_gas_tank::GasBurner;
use frame_support::pallet_prelude::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod extensions;
pub use extensions::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching runtime event type
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// A method for gas handling
        type GasBurner: GasBurner<AccountId = Self::AccountId, Gas = Weight>;
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
