#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Pass
//!
//! > TODO: Update with [spec](https://hackmd.io/@pandres95/pallet-pass) document once complete

use fc_traits_authn::{DeviceId, HashedUserId, UserAuthenticator, UserChallengeResponse};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use sp_runtime::{
    traits::{Dispatchable, StaticLookup},
    DispatchResult,
};
use sp_std::fmt::Debug;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod types;

pub mod weights;
pub use pallet::*;
pub use types::*;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use fc_traits_authn::{Authenticator, DeviceChallengeResponse, DeviceId, HashedUserId};
    use frame_support::{traits::EnsureOriginWithArg, PalletId};
    use frame_system::RawOrigin;

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

        type Authenticator: Authenticator;

        type PalletsOrigin: From<frame_system::Origin<Self>>;

        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// The maximum duration of a session
        #[pallet::constant]
        type MaxSessionDuration: Get<BlockNumberFor<Self>>;

        type RegisterOrigin: EnsureOriginWithArg<Self::RuntimeOrigin, HashedUserId>;
    }

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    // Storage
    #[pallet::storage]
    pub type Devices<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        DeviceId,
        DeviceOf<T, I>,
    >;

    #[pallet::storage]
    pub type Sessions<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (T::AccountId, BlockNumberFor<T>)>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        Registered {
            who: T::AccountId,
        },
        AddedDevice {
            who: T::AccountId,
            device_id: DeviceId,
        },
        SessionCreated {
            session_key: T::AccountId,
            until: BlockNumberFor<T>,
        },
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        AccountAlreadyRegistered,
        AccountNotFound,
        CredentialInvalid,
        DeviceAttestationInvalid,
        DeviceNotFound,
        SessionExpired,
        SessionNotFound,
    }

    #[pallet::call(weight(<T as Config<I>>::WeightInfo))]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Register an account
        #[pallet::call_index(0)]
        pub fn register(
            origin: OriginFor<T>,
            user: HashedUserId,
            attestation: DeviceAttestationOf<T, I>,
        ) -> DispatchResult {
            T::RegisterOrigin::ensure_origin(origin, &user)?;
            let account_id = Self::account_id_for(user)?;
            ensure!(
                Self::account_exists(&account_id),
                Error::<T, I>::AccountAlreadyRegistered
            );

            let device = T::Authenticator::verify_device(&attestation)
                .ok_or(Error::<T, I>::DeviceAttestationInvalid)?;

            Self::create_account(&account_id)?;
            Self::deposit_event(Event::<T, I>::Registered {
                who: account_id.clone(),
            });

            let device_id = device.device_id();
            Devices::<T, I>::insert(&account_id, device_id, device);
            Self::deposit_event(Event::<T, I>::AddedDevice {
                who: account_id,
                device_id,
            });

            Ok(())
        }

        #[pallet::call_index(3)]
        pub fn authenticate(
            origin: OriginFor<T>,
            device_id: DeviceId,
            credential: CredentialOf<T, I>,
            duration: Option<BlockNumberFor<T>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let account_id = T::Lookup::lookup(credential.user_id());
            Self::account_exists(&account_id)?;

            let device = Devices::<T, I>::get(&account_id, &device_id)
                .ok_or(Error::<T, I>::DeviceNotFound)?;
            device.verify_user(&credential).then_some(()).ok_or();

            Self::do_add_session(&who, &account_id, duration);
            Ok(())
        }

        /// Call to claim an Account: It assumes the account is initialized
        /// (because an active account is required to authenticate first of all).
        #[pallet::call_index(4)]
        // #[pallet::feeless_if()]
        pub fn add_device(
            origin: OriginFor<T>,
            attestation: DeviceAttestationOf<T, I>,
        ) -> DispatchResult {
            let who = Self::ensure_signer_is_valid_session(origin)?;

            let device_id = attestation.device_id();
            let device = T::Authenticator::verify_device(&attestation).ok_or(())?;

            Devices::<T, I>::insert(who, device_id, device);
            Self::deposit_event(Event::<T, I>::AddedDevice { who, device_id }.into());

            Ok(())
        }

        #[pallet::call_index(5)]
        pub fn dispatch(
            origin: OriginFor<T>,
            call: Box<RuntimeCallFor<T>>,
            maybe_credential: Option<(DeviceId, CredentialOf<T, I>)>,
            maybe_next_session_key: Option<T::AccountId>,
        ) -> DispatchResult {
            let account_id = if let Some((device_id, credential)) = maybe_credential {
                let account_id = Self::account_id_for(credential.user_id())?;
                Self::do_authenticate(credential, device_id)?;
                account_id
            } else {
                Self::ensure_signer_is_valid_session(origin)?
            };

            if let Some(next_session_key) = maybe_next_session_key {
                Self::do_add_session(
                    &next_session_key,
                    &account_id,
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
    pub fn account_id_for(user: HashedUserId) -> Result<T::AccountId, DispatchError> {
        // TODO it will be nice if we can use the lookup sytem to convert
        // to accept hashes or other types of data that fit the MultiAddress
        let pass = T::Lookup::lookup(user)?;
        Ok(pass)
    }

    pub(crate) fn account_exists(who: &T::AccountId) -> bool {
        frame_system::Pallet::<T>::account_exists(who)
    }

    pub(crate) fn create_account(who: &T::AccountId) -> DispatchResult {
        ensure!(
            frame_system::Pallet::<T>::inc_providers(who) == frame_system::IncRefStatus::Created,
            Error::<T, I>::AccountAlreadyRegistered
        );
        Ok(())
    }

    pub(crate) fn ensure_signer_is_valid_session(
        origin: OriginFor<T>,
    ) -> Result<T::AccountId, DispatchError> {
        let who = ensure_signed(origin)?;

        let (account_id, until) =
            Sessions::<T, I>::get(&who).ok_or(Error::<T, I>::SessionNotFound)?;
        if frame_system::Pallet::<T>::block_number() > until {
            Sessions::<T, I>::remove(who);
            return Err(Error::<T, I>::SessionExpired.into());
        }

        Ok(account_id)
    }

    pub(crate) fn do_authenticate(
        credential: CredentialOf<T, I>,
        device_id: DeviceId,
    ) -> Result<T::AccountId, DispatchError> {
        let account_id = Self::account_id_for(credential.user_id())?;
        ensure!(
            Self::account_exists(&account_id),
            Error::<T, I>::AccountNotFound
        );
        let device =
            Devices::<T, I>::get(&account_id, device_id).ok_or(Error::<T, I>::DeviceNotFound)?;
        device
            .verify_user(&credential)
            .ok_or(Error::<T, I>::CredentialInvalid)?;
        Ok(account_id)
    }

    pub(crate) fn do_add_session(
        session_key: &T::AccountId,
        account_id: &T::AccountId,
        duration: Option<BlockNumberFor<T>>,
    ) {
        let block_number = frame_system::Pallet::<T>::block_number();
        let session_duration = duration
            .unwrap_or(T::MaxSessionDuration::get())
            .min(T::MaxSessionDuration::get());
        let until = block_number + session_duration;

        Sessions::<T, I>::insert(session_key.clone(), (account_id.clone(), until));

        Self::deposit_event(
            Event::<T, I>::SessionCreated {
                session_key: session_key.clone(),
                until,
            }
            .into(),
        );
    }
}
