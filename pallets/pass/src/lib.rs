#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Pass
//!
//! > TODO: Update with [spec](https://hackmd.io/@pandres95/pallet-pass) document once complete

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod types;
pub use types::*;

pub mod weights;
pub use weights::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type WeightInfo: WeightInfo;

        /// The maximum lenght for an account name
        #[pallet::constant]
        type MaxAccountNameLen: Get<u32>;

        /// The maximum duration of a session
        #[pallet::constant]
        type MaxSessionDuration: Get<BlockNumberFor<Self>>;
    }

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::storage]
    pub type Accounts<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, BoundedVec<u8, T::MaxAccountNameLen>, AccountIdOf<T>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        Success,
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        Error,
    }

    #[pallet::call(weight(<T as Config<I>>::WeightInfo))]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Successful call
        #[pallet::call_index(0)]
        pub fn register(origin: OriginFor<T>) -> DispatchResult {
            ensure_signed_or_root(origin)?;
            Self::deposit_event(Event::<T, I>::Success);
            Ok(())
        }

        /// Call that errors
        #[pallet::call_index(1)]
        pub fn error(origin: OriginFor<T>) -> DispatchResult {
            ensure_signed_or_root(origin)?;
            Err(Error::<T, I>::Error.into())
        }
    }
}
