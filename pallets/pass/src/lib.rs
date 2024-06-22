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
use sp_runtime::traits::{Dispatchable, Hash, TrailingZeroInput};
use sp_std::fmt::Debug;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use fc_traits_authn::{AuthenticateError, AuthenticationMethod, Registrar, RegistrarError};

mod types;
pub use types::*;

pub mod weights;
pub use weights::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use fc_traits_authn::DeviceId;
    use frame_support::PalletId;
    use frame_system::RawOrigin;

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

        type AuthenticationMethod: Parameter + Into<Box<dyn AuthenticationMethod>>;

        type Registrar: fc_traits_authn::Registrar<AccountIdOf<Self>, AccountName<Self, I>>;

        type Randomness: Randomness<<Self as frame_system::Config>::Hash, BlockNumberFor<Self>>;

        type Scheduler: Named<
            BlockNumberFor<Self>,
            <Self as Config<I>>::RuntimeCall,
            Self::PalletsOrigin,
        >;

        type PalletsOrigin: From<frame_system::Origin<Self>>;

        #[pallet::constant]
        type PalletId: Get<PalletId>;

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

        // /// Constant maximum number of blocks allowed to keep alive a session
        // #[pallet::constant]
        // type MaxDuration: Get<BlockNumberFor<Self>>;
        #[pallet::constant]
        type ModForBlockNumber: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    // Storage
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
    #[pallet::storage]
    pub type Sessions<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, AccountIdOf<T>, (AccountName<T, I>, BlockNumberFor<T>)>; // Maybe could be a good idea modifying AccountIdOf to have extra padding so no other account can match with this

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
        SessionCreated {
            session_key: AccountIdOf<T>,
            until: BlockNumberFor<T>,
        },
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        AlreadyRegistered,
        CannotClaim,
        RegistrarCannotInitialize,
        InvalidDeviceForAuthenticationMethod,
        ChallengeFailed,
        ExceedsMaxDevices,
        AccountNotFound,
        Uninitialized,
        DeviceNotFound,
    }

    #[pallet::call(weight(<T as Config<I>>::WeightInfo))]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Successful call
        #[pallet::call_index(0)]
        pub fn register(
            origin: OriginFor<T>,
            account_name: AccountName<T, I>,
            authenticator: T::AuthenticationMethod,
            device: DeviceDescriptor<T, I>,
            challenge_response: Vec<u8>,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            let authenticator: Box<dyn AuthenticationMethod> = authenticator.into();
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

            if account.is_uninitialized() {
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
                .ok_or(Error::<T, I>::InvalidDeviceForAuthenticationMethod)?;
            authenticator
                .authenticate(
                    device.clone().to_vec(),
                    T::Randomness::random(&Encode::encode(&T::PalletId::get()))
                        .0
                        .as_ref(),
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
            authenticator: T::AuthenticationMethod,
            device: DeviceDescriptor<T, I>,
            challenge_payload: Vec<u8>,
        ) -> DispatchResult {
            // Ensures that the function is called by a signed origin
            let who = ensure_signed(origin)?;

            // Attempt to claim the account with the provided name and caller as the claimer
            T::Registrar::claim(&account_name, &who).map_err(|e| match e {
                RegistrarError::CannotClaim => Error::<T, I>::CannotClaim,
                RegistrarError::CannotInitialize => Error::<T, I>::RegistrarCannotInitialize,
                RegistrarError::AlreadyRegistered => Error::<T, I>::AlreadyRegistered,
            })?;
            Self::create_account(&account_name.clone().into())?;

            // Simulate device authentication
            let authenticator = Box::new(authenticator.into());
            let device_id = authenticator
                .get_device_id(device.to_vec())
                .ok_or(Error::<T, I>::InvalidDeviceForAuthenticationMethod)?;

            authenticator
                .authenticate(
                    device.to_vec(),
                    T::Randomness::random(&Encode::encode(&T::PalletId::get()))
                        .0
                        .as_ref(),
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

        #[pallet::call_index(3)]
        pub fn authenticate(
            origin: OriginFor<T>,
            account_name: AccountName<T, I>,
            authentication_method: T::AuthenticationMethod,
            device_id: DeviceId,
            authentication_proof: Vec<u8>,
            new_session_key: AccountIdOf<T>,
            maybe_duration: Option<BlockNumberFor<T>>,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            // Check account name exist
            ensure!(
                Accounts::<T, I>::contains_key(account_name.clone()),
                Error::<T, I>::AccountNotFound
            );

            let block_number = frame_system::Pallet::<T>::block_number();
            // Get the device from storage
            let (ac_name_from_dev_id, dev_descript_from_dev_id) =
                Devices::<T, I>::get(&device_id).ok_or(Error::<T, I>::DeviceNotFound)?;
            ensure!(
                ac_name_from_dev_id == account_name,
                Error::<T, I>::AccountNotFound
            );

            authentication_method
                .into()
                .authenticate(
                    dev_descript_from_dev_id.to_vec(),
                    T::Randomness::random(&Encode::encode(&T::PalletId::get()))
                        .0
                        .as_ref(),
                    &authentication_proof,
                )
                .map_err(|_| Error::<T, I>::ChallengeFailed)?;

            // Create the new session
            let session_duration = maybe_duration
                .unwrap_or(T::MaxSessionDuration::get())
                .max(T::MaxSessionDuration::get());

            Sessions::<T, I>::insert(
                new_session_key.clone(),
                (account_name, block_number + session_duration),
            );

            // Event
            Self::deposit_event(
                Event::<T, I>::SessionCreated {
                    session_key: new_session_key.clone(),
                    until: session_duration.clone(),
                }
                .into(),
            );

            // Finish
            Ok(())
        }

        /// Call to claim an Account
        #[pallet::call_index(4)]
        // #[pallet::feeless_if()]
        pub fn add_device(
            origin: OriginFor<T>,
            account_name: AccountName<T, I>,
            authentication_method: T::AuthenticationMethod,
            device_id: DeviceId,
            authentication_proof: Vec<u8>,
        ) -> DispatchResult {
            // Ensures that the function is called by a signed origin
            let _who = ensure_signed(origin)?;

            // Check account name exist
            ensure!(
                Accounts::<T, I>::contains_key(account_name.clone()),
                Error::<T, I>::AccountNotFound
            );

            // <Validate device>
            let auth_method: Box<dyn AuthenticationMethod> = authentication_method.into();

            // Verify signature of device
            auth_method
                .authenticate(
                    device_id.to_vec(),
                    T::Randomness::random(&Encode::encode(&T::PalletId::get()))
                        .0
                        .as_ref(),
                    &authentication_proof[..],
                )
                .map_err(|_| Error::<T, I>::ChallengeFailed)?;

            // <Add device>
            AccountDevices::<T, I>::try_append(account_name.clone(), device_id)
                .map_err(|_| Error::<T, I>::ExceedsMaxDevices)?;
            // </Add device>

            Ok(())
        }

        #[pallet::call_index(5)]
        pub fn dispatch(
            origin: OriginFor<T>,
            call: Box<RuntimeCallFor<T>>,
            maybe_authentication: Option<(AccountName<T, I>, T::AuthenticationMethod, DeviceId)>,
            _maybe_next_session_key: Option<AccountIdOf<T>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Authentication logic (if provided)
            if let Some((_account_name, authenticator, _device_id)) = maybe_authentication {
                // Commented while add_device is not implemented.
                // let (_, device) = Devices::<T, I>::get(device_id)
                //     .ok_or(Error::<T, I>::InvalidDeviceForAuthenticationMethod)?;
                let device = DeviceDescriptor::<T, I>::default();

                let authenticator = Box::new(authenticator.into());

                // This has to be rethought, what would a real challenge look like? Do we pass a challenge instead?
                let challenge = T::Randomness::random(&Encode::encode(&T::PalletId::get()))
                    .0
                    .as_ref()
                    .to_vec();

                // Same as above, what would a real payload look like?
                let payload = challenge.clone();

                authenticator
                    .authenticate(device.to_vec(), &challenge, &payload)
                    .map_err(|_| Error::<T, I>::ChallengeFailed)?;
            }

            // Re-dispatch the call on behalf of the caller.
            let res = call.dispatch(RawOrigin::Signed(who).into());

            // Turn the result from the `dispatch` into our expected `DispatchResult` type.
            res.map(|_| ()).map_err(|e| e.error)
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    pub fn account_id_for(account_name: &AccountName<T, I>) -> AccountIdOf<T> {
        let hashed = <T as frame_system::Config>::Hashing::hash(&account_name);
        Decode::decode(&mut TrailingZeroInput::new(hashed.as_ref()))
            .expect("All byte sequences are valid `AccountIds`; qed")
    }

    pub(crate) fn create_account(account_name: &AccountName<T, I>) -> DispatchResult {
        Accounts::<T, I>::try_mutate(account_name.clone(), |maybe_account| {
            if maybe_account.is_none() {
                *maybe_account = Some(Account {
                    account_id: Self::account_id_for(account_name),
                    status: AccountStatus::Active,
                });
                Ok(())
            } else {
                Err(Error::<T, I>::AlreadyRegistered.into())
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
