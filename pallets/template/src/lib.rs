#![cfg_attr(not(feature = "std"), no_std)]

//! # Template Pallet
//!
//! This is the place where you'd put the documentation of the pallet

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        Success,
    }

    #[pallet::error]
    pub enum Error<T> {
        Error,
    }

    #[pallet::call(weight(<T as Config>::WeightInfo))]
    impl<T: Config> Pallet<T> {
        /// Successful call
        #[pallet::call_index(0)]
        pub fn success(origin: OriginFor<T>) -> DispatchResult {
            ensure_signed_or_root(origin)?;
            Self::deposit_event(Event::<T>::Success);
            Ok(())
        }

        /// Call that errors
        #[pallet::call_index(1)]
        pub fn error(origin: OriginFor<T>) -> DispatchResult {
            ensure_signed_or_root(origin)?;
            Err(Error::<T>::Error.into())
        }
    }
}
