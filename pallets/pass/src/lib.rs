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
        DeviceNotFound,
        InvalidDeviceForAuthenticationMethod,
        ChallengeFailed,
        ExceedsMaxDevices,
        AccountNotFound,
        SessionNotFound,
        ExpiredSession,
        Uninitialized,
    }

    #[pallet::call(weight(<T as Config<I>>::WeightInfo))]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Register an account
        #[pallet::call_index(0)]
        pub fn register(
            origin: OriginFor<T>,
            account_name: AccountName<T, I>,
            authentication_method: T::AuthenticationMethod,
            device: DeviceDescriptor<T, I>,
            challenge_response: Vec<u8>,
        ) -> DispatchResult {
            ensure_signed(origin)?;
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

            let authentication_method: Box<dyn AuthenticationMethod> = authentication_method.into();
            let device_id = authentication_method
                .get_device_id(device.to_vec())
                .ok_or(Error::<T, I>::InvalidDeviceForAuthenticationMethod)?;

            Self::do_authenticate(&authentication_method, &device, &challenge_response)?;
            Self::do_add_device(&account_name, device_id, device)?;

            Ok(())
        }

        /// Call to claim an Account
        #[pallet::call_index(1)]
        // #[pallet::feeless_if()]
        pub fn claim(
            origin: OriginFor<T>,
            account_name: AccountName<T, I>,
            authentication_method: T::AuthenticationMethod,
            device: DeviceDescriptor<T, I>,
            challenge_response: Vec<u8>,
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

            Self::deposit_event(
                Event::<T, I>::Claimed {
                    account_name: account_name.clone(),
                }
                .into(),
            );

            let authentication_method: Box<dyn AuthenticationMethod> = authentication_method.into();
            let device_id = authentication_method
                .get_device_id(device.to_vec())
                .ok_or(Error::<T, I>::InvalidDeviceForAuthenticationMethod)?;

            Self::do_authenticate(&authentication_method, &device, &challenge_response)?;
            Self::do_add_device(&account_name, device_id, device)?;

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
            challenge_response: Vec<u8>,
            duration: Option<BlockNumberFor<T>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Check account name exist
            ensure!(
                Accounts::<T, I>::contains_key(account_name.clone()),
                Error::<T, I>::AccountNotFound
            );

            let authentication_method: Box<dyn AuthenticationMethod> = authentication_method.into();
            let (device_account_name, device) =
                Devices::<T, I>::get(device_id.clone()).ok_or(Error::<T, I>::DeviceNotFound)?;

            ensure!(
                account_name == device_account_name,
                Error::<T, I>::AccountNotFound
            );
            Self::do_authenticate(&authentication_method, &device, &challenge_response)?;
            Self::do_add_session(&who, &account_name, duration);

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
            device: DeviceDescriptor<T, I>,
            challenge_response: Vec<u8>,
        ) -> DispatchResult {
            let session_account_name = Self::ensure_signer_is_valid_session(origin)?;
            ensure!(
                account_name == session_account_name,
                Error::<T, I>::AccountNotFound,
            );

            let authentication_method: Box<dyn AuthenticationMethod> = authentication_method.into();
            let device_id = authentication_method
                .get_device_id(device.clone().to_vec())
                .ok_or(Error::<T, I>::InvalidDeviceForAuthenticationMethod)?;

            Self::do_authenticate(&authentication_method, &device, &challenge_response)?;
            Self::do_add_device(&account_name, device_id, device)?;

            Ok(())
        }

        #[pallet::call_index(5)]
        pub fn dispatch(
            origin: OriginFor<T>,
            call: Box<RuntimeCallFor<T>>,
            maybe_authentication: Option<(
                AccountName<T, I>,
                T::AuthenticationMethod,
                DeviceId,
                Vec<u8>,
            )>,
            maybe_next_session_key: Option<AccountIdOf<T>>,
        ) -> DispatchResult {
            let account_name =
                if let Some((account_name, authentication_method, device_id, payload_challenge)) =
                    maybe_authentication
                {
                    let (device_account_name, device) =
                        Devices::<T, I>::get(device_id).ok_or(Error::<T, I>::DeviceNotFound)?;
                    ensure!(
                        device_account_name == account_name,
                        Error::<T, I>::AccountNotFound
                    );

                    let authentication_method: Box<dyn AuthenticationMethod> =
                        authentication_method.into();

                    Self::do_authenticate(&authentication_method, &device, &payload_challenge)?;
                    account_name
                } else {
                    Self::ensure_signer_is_valid_session(origin)?
                };

            let Account { account_id, status } = Accounts::<T, I>::get(account_name.clone())
                .ok_or(Error::<T, I>::AccountNotFound)?;
            ensure!(
                status == AccountStatus::Active,
                Error::<T, I>::Uninitialized
            );

            if let Some(next_session_key) = maybe_next_session_key {
                Self::do_add_session(
                    &next_session_key,
                    &account_name,
                    Some(T::MaxSessionDuration::get()),
                );
            }

            // Re-dispatch the call on behalf of the caller.
            let res = call.dispatch(RawOrigin::Signed(account_id).into());
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

    pub(crate) fn ensure_signer_is_valid_session(
        origin: OriginFor<T>,
    ) -> Result<AccountName<T, I>, DispatchError> {
        let who = ensure_signed(origin)?;

        let (account_name, until) =
            Sessions::<T, I>::get(&who).ok_or(Error::<T, I>::SessionNotFound)?;
        if frame_system::Pallet::<T>::block_number() > until {
            // Clean the expired session logic here
            // Error… just because
            return Err(Error::<T, I>::ExpiredSession.into());
        }

        Ok(account_name)
    }

    pub(crate) fn do_authenticate(
        authentication_method: &Box<dyn AuthenticationMethod>,
        device: &DeviceDescriptor<T, I>,
        challenge_response: &Vec<u8>,
    ) -> DispatchResult {
        authentication_method
            .authenticate(
                device.clone().to_vec(),
                T::Randomness::random(&Encode::encode(&T::PalletId::get()))
                    .0
                    .as_ref(),
                challenge_response,
            )
            .map_err(|e| match e {
                AuthenticateError::ChallengeFailed => Error::<T, I>::ChallengeFailed.into(),
            })
    }

    pub(crate) fn do_add_device(
        account_name: &AccountName<T, I>,
        device_id: DeviceId,
        device: DeviceDescriptor<T, I>,
    ) -> DispatchResult {
        AccountDevices::<T, I>::try_append(account_name.clone(), device_id)
            .map_err(|_| Error::<T, I>::ExceedsMaxDevices)?;
        Devices::<T, I>::insert(device_id, (account_name.clone(), device));

        Self::deposit_event(
            Event::<T, I>::AddedDevice {
                account_name: account_name.clone(),
                device_id,
            }
            .into(),
        );
        Ok(())
    }

    pub(crate) fn do_add_session(
        session_key: &AccountIdOf<T>,
        account_name: &AccountName<T, I>,
        duration: Option<BlockNumberFor<T>>,
    ) {
        let block_number = frame_system::Pallet::<T>::block_number();
        let session_duration = duration
            .unwrap_or(T::MaxSessionDuration::get())
            .max(T::MaxSessionDuration::get());
        let until = block_number + session_duration;

        Sessions::<T, I>::insert(session_key.clone(), (account_name.clone(), until));

        Self::deposit_event(
            Event::<T, I>::SessionCreated {
                session_key: session_key.clone(),
                until,
            }
            .into(),
        );
    }
}
