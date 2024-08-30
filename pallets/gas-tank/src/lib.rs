#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Gas Tank
//!
//! This pallet exposes a mechanism to allow transactions payment using a prepaid fees mechanism.

use frame_support::pallet_prelude::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod extensions;
pub use extensions::*;

mod traits;
pub use traits::*;

pub use pallet::*;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching runtime event type
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// A method for gas handling
        type GasHandler: GasBurner<AccountId = Self::AccountId, Gas = Weight>;
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
