#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Pass
//!
//! > TODO: Update with [spec](https://hackmd.io/@pandres95/pallet-pass) document once complete

use frame_support::{pallet_prelude::*, traits::Randomness};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::{Hash, TrailingZeroInput};

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod traits;
use traits::{AuthenticateError, Authenticator, Registrar};

mod types;
pub use types::*;

pub mod weights;
pub use weights::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use traits::ClaimError;

    use super::*;

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type WeightInfo: WeightInfo;

        type Authenticator: Parameter + Into<Box<dyn Authenticator>>;

        type Registrar: Registrar<AccountIdOf<Self>, AccountName<Self, I>>;

        type Randomness: Randomness<<Self as frame_system::Config>::Hash, BlockNumberFor<Self>>;

        /// The maximum lenght for an account name
        #[pallet::constant]
        type MaxAccountNameLen: Get<u32>;

        /// The maximum size a device descriptor can contain
        #[pallet::constant]
        type MaxDeviceDescriptorLen: Get<u32>;

        /// The maximum duration of a session
        #[pallet::constant]
        type MaxSessionDuration: Get<BlockNumberFor<Self>>;
    }

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::storage]
    pub type Accounts<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, AccountName<T, I>, AccountOf<T>>;
    #[pallet::storage]
    pub type Devices<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, DeviceId, (AccountName<T, I>, DeviceDescriptor<T, I>)>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        Registered {
            account_name: AccountName<T, I>,
            account_id: AccountIdOf<T>,
        },
        AddedDevice {
            account_name: AccountName<T, I>,
            device_id: DeviceId,
        },
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        AlreadyRegistered,
        CannotClaim,
        InvalidDeviceForAuthenticator,
        ChallengeFailed,
    }

    #[pallet::call(weight(<T as Config<I>>::WeightInfo))]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Successful call
        #[pallet::call_index(0)]
        pub fn register(
            origin: OriginFor<T>,
            account_name: AccountName<T, I>,
            authenticator: T::Authenticator,
            device: DeviceDescriptor<T, I>,
            challenge_response: Vec<u8>,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            let authenticator: Box<dyn Authenticator> = authenticator.into();
            let account_id = Self::account_id_for(&account_name);

            ensure!(
                !Accounts::<T, I>::contains_key(account_name.clone()),
                Error::<T, I>::AlreadyRegistered
            );
            let account = Account::new(
                account_id.clone(),
                if frame_system::Pallet::<T>::account_exists(&account_id) {
                    AccountStatus::Active
                } else {
                    AccountStatus::Uninitialized
                },
            );

            // TODO: if account.is_unitialized()
            //  Schedule::register(crate::call::<T, I>::UnreserveAccount { account_name }, when: T::UninitializedTimeout);

            Accounts::<T, I>::insert(account_name.clone(), account.clone());

            Self::deposit_event(
                Event::<T, I>::Registered {
                    account_name: account_name.clone(),
                    account_id: account_id.clone(),
                }
                .into(),
            );

            let device_id = authenticator
                .get_device_id(device.to_vec())
                .ok_or(Error::<T, I>::InvalidDeviceForAuthenticator)?;
            authenticator
                .authenticate(device.clone().to_vec(), &*b"challenge", &challenge_response)
                .map_err(|e| match e {
                    AuthenticateError::ChallengeFailed => Error::<T, I>::ChallengeFailed,
                })?;

            Devices::<T, I>::insert(device_id, (account_name.clone(), device));

            Self::deposit_event(
                Event::<T, I>::AddedDevice {
                    account_name,
                    device_id,
                }
                .into(),
            );

            Ok(())
        }

        /// Call that errors
        #[pallet::call_index(1)]
        // #[pallet::feeless_if()]
        pub fn claim(
            origin: OriginFor<T>,
            account_name: AccountName<T, I>,
            authenticator: T::Authenticator,
            device: DeviceDescriptor<T, I>,
            challenge_payload: Vec<u8>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::Registrar::claim(account_name, who).map_err(|e| match e {
                ClaimError::CannotClaim => Error::<T, I>::CannotClaim,
            })?;

            // TODO: Finish registering the account

            Err(Error::<T, I>::CannotClaim.into())
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    pub fn account_id_for(account_name: &AccountName<T, I>) -> AccountIdOf<T> {
        let hashed = <T as frame_system::Config>::Hashing::hash(&account_name);
        Decode::decode(&mut TrailingZeroInput::new(hashed.as_ref()))
            .expect("All byte sequences are valid `AccountIds`; qed")
    }
}
