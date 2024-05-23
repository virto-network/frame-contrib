#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Pass
//!
//! > TODO: Update with [spec](https://hackmd.io/@pandres95/pallet-pass) document once complete

use frame_support::{
    pallet_prelude::*,
    traits::{
        schedule::{v3::Named, DispatchTime},
        Randomness,
    },
};
use frame_system::pallet_prelude::*;
use sp_core::blake2_256;
use sp_runtime::traits::{Dispatchable, Hash, TrailingZeroInput, TryMorph};
use sp_std::fmt::Debug;

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
    // use types::RegistrarResult;

    use super::*;

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type RuntimeCall: Parameter
            + Dispatchable<RuntimeOrigin = Self::RuntimeOrigin>
            + Debug
            + From<Call<Self, I>>
            + From<frame_system::Call<Self>>;

        type WeightInfo: WeightInfo;

        type Authenticator: Parameter + Into<Box<dyn Authenticator>>;

        // type Registrar: Registrar<AccountIdOf<Self>, AccountName<Self, I>>;
        type Registrar: TryMorph<(AccountName<Self, I>, AccountIdOf<Self>), Outcome = RegistrarResult>;

        type Randomness: Randomness<<Self as frame_system::Config>::Hash, BlockNumberFor<Self>>;

        type Scheduler: Named<
            BlockNumberFor<Self>,
            <Self as Config<I>>::RuntimeCall,
            Self::PalletsOrigin,
        >;

        type PalletsOrigin: From<frame_system::Origin<Self>>;

        /// The maximum lenght for an account name
        #[pallet::constant]
        type MaxAccountNameLen: Get<u32>;

        /// The maximum size a device descriptor can contain
        #[pallet::constant]
        type MaxDeviceDescriptorLen: Get<u32>;

        /// The maximum amount of devices per account
        #[pallet::constant]
        type MaxDevicesPerAccount: Get<u32>;

        /// The maximum duration of a session
        #[pallet::constant]
        type MaxSessionDuration: Get<BlockNumberFor<Self>>;

        /// The maximum duration after an uninitialized account is unreserved
        #[pallet::constant]
        type UninitializedTimeout: Get<BlockNumberFor<Self>>;
    }

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::storage]
    pub type Accounts<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, AccountName<T, I>, AccountOf<T>>;
    #[pallet::storage]
    pub type Devices<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, DeviceId, (AccountName<T, I>, DeviceDescriptor<T, I>)>;
    #[pallet::storage]
    pub type AccountDevices<T: Config<I>, I: 'static = ()> = StorageMap<
        _,
        Blake2_128Concat,
        AccountName<T, I>,
        BoundedVec<DeviceId, T::MaxDevicesPerAccount>,
        ValueQuery,
    >;

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
        Claimed {
            account_name: AccountName<T, I>,
        },
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        AlreadyRegistered,
        CannotClaim,
        RegistrarCannotInitialize,
        InvalidDeviceForAuthenticator,
        ChallengeFailed,
        ExceedsMaxDevices,
        AccountNotFound,
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

            if account.is_unitialized() {
                Self::schedule_unreserve(&account_name)?;
            }

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
                .authenticate(
                    device.clone().to_vec(),
                    T::Randomness::random(&[][..]).0.as_ref(),
                    &challenge_response,
                )
                .map_err(|e| match e {
                    AuthenticateError::ChallengeFailed => Error::<T, I>::ChallengeFailed,
                })?;

            AccountDevices::<T, I>::try_append(account_name.clone(), device_id)
                .map_err(|_| Error::<T, I>::ExceedsMaxDevices)?;
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

        /// Call to claim an Account
        #[pallet::call_index(1)]
        // #[pallet::feeless_if()]
        pub fn claim(
            origin: OriginFor<T>,
            account_name: AccountName<T, I>,
            authenticator: T::Authenticator,
            device: DeviceDescriptor<T, I>,
            challenge_payload: Vec<u8>,
        ) -> DispatchResult {
            // Ensures that the function is called by a signed origin
            let who = ensure_signed(origin)?;

            // Attempt to claim the account with the provided name and caller as the claimer
            T::Registrar::claim(&account_name, &who).map_err(|e| match e {
                RegistrarResult::AlreadyRegistered => Error::<T, I>::AlreadyRegistered,
                RegistrarResult::CannotClaim => Error::<T, I>::CannotClaim,
                RegistrarResult::CannotInitialize => Error::<T, I>::RegistrarCannotInitialize,
            })?;

            // Simulate device authentication
            let authenticator = Box::new(authenticator.into());
            let device_id = authenticator
                .get_device_id(device.to_vec())
                .ok_or(Error::<T, I>::InvalidDeviceForAuthenticator)?;

            authenticator
                .authenticate(
                    device.to_vec(),
                    T::Randomness::random(&[][..]).0.as_ref(),
                    &challenge_payload,
                )
                .map_err(|_| Error::<T, I>::ChallengeFailed)?;

            // Register the device with the account
            AccountDevices::<T, I>::try_append(account_name.clone(), device_id)
                .map_err(|_| Error::<T, I>::ExceedsMaxDevices)?;
            Devices::<T, I>::insert(device_id, (account_name.clone(), device));

            // Emit events
            Self::deposit_event(
                Event::<T, I>::Claimed {
                    account_name: account_name.clone(),
                }
                .into(),
            );
            Self::deposit_event(
                Event::<T, I>::AddedDevice {
                    account_name,
                    device_id,
                }
                .into(),
            );

            Ok(())
        }

        #[pallet::call_index(2)]
        pub fn unreserve_uninitialized_account(
            origin: OriginFor<T>,
            account_name: AccountName<T, I>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Accounts::<T, I>::try_mutate(account_name.clone(), |maybe_account| {
                log::trace!("Maybe Account for {account_name:?}? {maybe_account:?}");
                if let Some(Account { account_id, status }) = maybe_account {
                    if frame_system::Pallet::<T>::account_exists(account_id) {
                        log::trace!("Removing exists, not removing account");
                        *status = AccountStatus::Active;
                        return Ok(());
                    }

                    log::trace!("Removing account");
                    for device_id in AccountDevices::<T, I>::get(account_name.clone()) {
                        Devices::<T, I>::remove(device_id);
                    }
                    AccountDevices::<T, I>::remove(account_name.clone());
                    *maybe_account = None;

                    return Ok(());
                }

                Ok(())
            })
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    pub fn account_id_for(account_name: &AccountName<T, I>) -> AccountIdOf<T> {
        let hashed = <T as frame_system::Config>::Hashing::hash(&account_name);
        Decode::decode(&mut TrailingZeroInput::new(hashed.as_ref()))
            .expect("All byte sequences are valid `AccountIds`; qed")
    }

    pub fn create_account(account_name: &AccountName<T, I>) -> Result<(), RegistrarResult> {
        Accounts::<T, I>::try_mutate(account_name.clone(), |maybe_account| {
            if maybe_account.is_none() {
                *maybe_account = Some(Account {
                    account_id: Self::account_id_for(account_name),
                    status: AccountStatus::Active,
                });
                Ok(())
            } else {
                Err(RegistrarResult::AlreadyRegistered)
            }
        })
    }

    pub(crate) fn schedule_name_from_account_id(account_name: &AccountName<T, I>) -> [u8; 32] {
        blake2_256(&account_name.to_vec())
    }

    pub(crate) fn schedule_unreserve(account_name: &AccountName<T, I>) -> DispatchResult {
        T::Scheduler::schedule_named(
            Self::schedule_name_from_account_id(account_name),
            DispatchTime::After(T::UninitializedTimeout::get()),
            None,
            0u8,
            frame_system::Origin::<T>::Root.into(),
            frame_support::traits::Bounded::Inline(BoundedVec::truncate_from(
                <T as Config<I>>::RuntimeCall::from(
                    Call::<T, I>::unreserve_uninitialized_account {
                        account_name: account_name.clone(),
                    },
                )
                .encode(),
            )),
        )?;

        Ok(())
    }
}
