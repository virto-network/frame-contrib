#![cfg_attr(not(feature = "std"), no_std)]

//! # Template Pallet
//!
//! This is the place where you'd put the documentation of the pallet

use frame_support::{pallet_prelude::*, dispatch::DispatchResult};
use frame_system::{pallet_prelude::*, ensure_signed};
use sp_std::prelude::*;

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
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The maximum size of the payload that can be accepted.
        type MaxPayloadSize: Get<u32>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn devices)]
    pub type Devices<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, DeviceId>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A device was added.
        AddedDevice { account: T::AccountId, device_id: DeviceId },
        /// Template event.
        Success,
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Account has not been initialized.
        Uninitialized,
        /// The challenge failed.
        ChallengeFailed,
        /// Template event.
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

        /// Registers a new device.
        #[pallet::call_index(2)]
        pub fn add_device(
            origin: OriginFor<T>,
            device: Vec<u8>,
            challenge_payload: BoundedVec<u8, T::MaxPayloadSize>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(Self::validate_device(&device, &challenge_payload), Error::<T>::ChallengeFailed);

            let device_id = Self::generate_device_id(&device);

            Devices::<T>::insert(&who, device_id.clone());

            Self::deposit_event(Event::AddedDevice { account: who, device_id });

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Example validation function for devices and challenges.
        fn validate_device(device: &[u8], challenge: &[u8]) -> bool {
            // Insert validation logic here
            true
        }

        /// Example function to generate a device ID from a device descriptor.
        fn generate_device_id(device: &[u8]) -> DeviceId {
            // Insert ID generation logic based on device descriptor
            [1;32]
        }
    }
}
