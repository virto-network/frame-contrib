#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Pass
//!
//! > TODO: Update with [spec](https://hackmd.io/@pandres95/pallet-pass) document once complete

use fc_traits_authn::{
    util::AuthorityFromPalletId, Authenticator, DeviceChallengeResponse, DeviceId, HashedUserId,
    UserAuthenticator, UserChallengeResponse, HASHED_USER_ID_LEN,
};
use frame_support::traits::schedule::DispatchTime;
use frame_support::traits::Bounded;
use frame_support::{
    pallet_prelude::*,
    traits::{
        fungible::{Inspect, Mutate},
        schedule::v3::{Named, TaskName},
        EnsureOriginWithArg,
    },
    PalletId,
};
use frame_system::{pallet_prelude::*, RawOrigin};
use sp_core::blake2_256;
use sp_runtime::{
    traits::{Dispatchable, TrailingZeroInput},
    DispatchResult,
};
use sp_std::{boxed::Box, fmt::Debug};

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod extension;
mod types;

pub mod weights;
pub use extension::*;
pub use pallet::*;
pub use types::*;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
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

        type Currency: Inspect<Self::AccountId> + Mutate<Self::AccountId>;

        type WeightInfo: WeightInfo;

        type Authenticator: Authenticator<Authority = AuthorityFromPalletId<Self::PalletId>>;

        type PalletsOrigin: From<frame_system::Origin<Self>>;

        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// The maximum duration of a session
        #[pallet::constant]
        type MaxSessionDuration: Get<BlockNumberFor<Self>>;

        type RegisterOrigin: EnsureOriginWithArg<
            Self::RuntimeOrigin,
            HashedUserId,
            Success = Option<DepositInformation<Self, I>>,
        >;

        type Scheduler: Named<
            BlockNumberFor<Self>,
            <Self as Config<I>>::RuntimeCall,
            Self::PalletsOrigin,
        >;

        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: BenchmarkHelper<Self, I>;
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
            let maybe_deposit_info = T::RegisterOrigin::ensure_origin(origin, &user)?;
            let account_id = Self::account_id_for(user)?;
            ensure!(
                !Self::account_exists(&account_id),
                Error::<T, I>::AccountAlreadyRegistered
            );

            if let Some(deposit_info) = maybe_deposit_info {
                Self::charge_register_deposit(deposit_info)?;
            }
            Self::create_account(&account_id)?;
            Self::deposit_event(Event::<T, I>::Registered {
                who: account_id.clone(),
            });

            Self::do_add_device(&account_id, attestation)
        }

        #[pallet::feeless_if(
            |_: &OriginFor<T>, device_id: &DeviceId, credential: &CredentialOf<T, I>, _: &Option<BlockNumberFor<T>>| -> bool {
                Pallet::<T, I>::try_authenticate(device_id, credential).is_ok()
            }
		)]
        #[pallet::call_index(3)]
        pub fn authenticate(
            origin: OriginFor<T>,
            device_id: DeviceId,
            credential: CredentialOf<T, I>,
            duration: Option<BlockNumberFor<T>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let account_id = Self::try_authenticate(&device_id, &credential)?;
            Self::try_add_session(&who, &account_id, duration)?;
            Ok(())
        }

        /// Call to claim an Account: It assumes the account is initialized
        /// (because an active account is required to authenticate first of all).
        #[pallet::call_index(4)]
        pub fn add_device(
            origin: OriginFor<T>,
            attestation: DeviceAttestationOf<T, I>,
        ) -> DispatchResult {
            let who = Self::ensure_signer_is_valid_session(origin)?;
            Self::do_add_device(&who, attestation)
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
                Self::try_add_session(
                    &next_session_key,
                    &account_id,
                    Some(T::MaxSessionDuration::get()),
                )?;
            }

            // Re-dispatch the call on behalf of the caller.
            let res = call.dispatch(RawOrigin::Signed(account_id).into());
            // Turn the result from the `dispatch` into our expected `DispatchResult` type.
            res.map(|_| ()).map_err(|e| e.error)
        }

        #[pallet::call_index(6)]
        pub fn remove_session_key(
            origin: OriginFor<T>,
            session_key: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::do_remove_session(&session_key);
            Ok(())
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    pub fn account_id_for(user: HashedUserId) -> Result<T::AccountId, DispatchError> {
        // we know the length of HashedUserId
        let mut input = [0u8; 2 * HASHED_USER_ID_LEN];
        input[HASHED_USER_ID_LEN..].copy_from_slice(&user);
        let account_id: T::AccountId =
            T::AccountId::decode(&mut TrailingZeroInput::new(&blake2_256(&input)))
                .map_err(|_| Error::<T, I>::AccountNotFound)?;
        Ok(account_id)
    }

    pub fn account_exists(who: &T::AccountId) -> bool {
        frame_system::Pallet::<T>::account_exists(who)
    }

    #[allow(dead_code)]
    pub(crate) fn charge_register_deposit(
        (source, amount, dest): DepositInformation<T, I>,
    ) -> DispatchResult {
        T::Currency::transfer(
            &source,
            &dest,
            amount,
            frame_support::traits::tokens::Preservation::Expendable,
        )
        .map(|_| ())
    }

    pub(crate) fn create_account(who: &T::AccountId) -> DispatchResult {
        ensure!(
            frame_system::Pallet::<T>::inc_providers(who) == frame_system::IncRefStatus::Created,
            Error::<T, I>::AccountAlreadyRegistered
        );
        Ok(())
    }

    pub(crate) fn try_authenticate(
        device_id: &DeviceId,
        credential: &CredentialOf<T, I>,
    ) -> Result<T::AccountId, DispatchError> {
        let account_id = Self::account_id_for(credential.user_id())?;
        ensure!(
            Self::account_exists(&account_id),
            Error::<T, I>::AccountNotFound
        );
        let device =
            Devices::<T, I>::get(&account_id, device_id).ok_or(Error::<T, I>::DeviceNotFound)?;
        device
            .verify_user(credential)
            .ok_or(Error::<T, I>::CredentialInvalid)?;

        Ok(account_id)
    }

    pub(crate) fn do_add_device(
        who: &T::AccountId,
        attestation: DeviceAttestationOf<T, I>,
    ) -> DispatchResult {
        let device_id = attestation.device_id();
        let device = T::Authenticator::verify_device(attestation.clone())
            .ok_or(Error::<T, I>::DeviceAttestationInvalid)?;

        Devices::<T, I>::insert(who, device_id, device);
        Self::deposit_event(Event::<T, I>::AddedDevice {
            who: who.clone(),
            device_id: *device_id,
        });

        Ok(())
    }

    pub(crate) fn ensure_signer_is_valid_session(
        origin: OriginFor<T>,
    ) -> Result<T::AccountId, DispatchError> {
        let who = ensure_signed(origin)?;

        let (account_id, until) =
            Sessions::<T, I>::get(&who).ok_or(Error::<T, I>::SessionNotFound)?;

        // Failsafe: In the extreme case the scheduler needs to defer the cleanup call for a
        // certain
        if frame_system::Pallet::<T>::block_number() > until {
            Self::do_remove_session(&who);
            return Err(Error::<T, I>::SessionExpired.into());
        }

        Ok(account_id)
    }

    pub(crate) fn signer_from_session_key(who: &T::AccountId) -> Option<T::AccountId> {
        let (account_id, until) = Sessions::<T, I>::get(who)?;
        if frame_system::Pallet::<T>::block_number() <= until {
            Some(account_id)
        } else {
            None
        }
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

    fn do_remove_session(session_key: &T::AccountId) {
        Self::cancel_scheduled_session_key_removal(session_key);
        // Decrements the provider reference of this `Session` key account once it's expired.
        let _ = frame_system::Pallet::<T>::dec_providers(session_key).inspect_err(|_| {
			log::warn!(
                "Failed to remove the last provider for {session_key:?}, which has an active consumer"
            )
		});
        Sessions::<T, I>::remove(session_key);
    }

    pub(crate) fn try_add_session(
        session_key: &T::AccountId,
        account_id: &T::AccountId,
        duration: Option<BlockNumberFor<T>>,
    ) -> DispatchResult {
        // Let's try to remove an existing session that uses the same session key (if any). This is
        // so we ensure we decrease the provider counter correctly.
        if Sessions::<T, I>::contains_key(session_key) {
            Self::do_remove_session(session_key);
        }

        let block_number = frame_system::Pallet::<T>::block_number();

        // Add a consumer reference to this account, since we'll be using
        // it meanwhile it stays active as a Session.
        frame_system::Pallet::<T>::inc_providers(session_key);

        let session_duration = duration
            .unwrap_or(T::MaxSessionDuration::get())
            .min(T::MaxSessionDuration::get());
        let until = block_number + session_duration;

        Sessions::<T, I>::insert(session_key.clone(), (account_id.clone(), until));
        Self::schedule_next_removal(session_key, duration)?;

        Self::deposit_event(Event::<T, I>::SessionCreated {
            session_key: session_key.clone(),
            until,
        });

        Ok(())
    }

    fn task_name_from_session_key(session_key: &T::AccountId) -> TaskName {
        blake2_256(&("remove_session_key", session_key).encode())
    }

    /// Infallibly cancels an already scheduled session key removal
    fn cancel_scheduled_session_key_removal(session_key: &T::AccountId) {
        T::Scheduler::cancel_named(Self::task_name_from_session_key(session_key))
            .map_or_else(|_| (), |_| ());
    }

    fn schedule_next_removal(
        session_key: &T::AccountId,
        duration: Option<BlockNumberFor<T>>,
    ) -> DispatchResult {
        Self::cancel_scheduled_session_key_removal(session_key);

        let duration = duration
            .unwrap_or(T::MaxSessionDuration::get())
            .min(T::MaxSessionDuration::get());
        let call: <T as Config<I>>::RuntimeCall = Call::remove_session_key {
            session_key: session_key.clone(),
        }
        .into();

        T::Scheduler::schedule_named(
            Self::task_name_from_session_key(session_key),
            DispatchTime::After(duration),
            None,
            0,
            frame_system::RawOrigin::Root.into(),
            Bounded::Inline(BoundedVec::truncate_from(call.encode())),
        )?;

        Ok(())
    }
}
