#![cfg_attr(not(feature = "std"), no_std)]

//! # Pallet Pass
//!
//! > TODO: Update with [spec](https://hackmd.io/@pandres95/pallet-pass) document once complete

extern crate alloc;
extern crate core;

use core::fmt::Debug;
use fc_traits_authn::{
    util::AuthorityFromPalletId, Authenticator, DeviceChallengeResponse, DeviceId,
    ExtrinsicContext, HashedUserId, UserAuthenticator, UserChallengeResponse,
};
use frame_support::{
    pallet_prelude::*,
    storage::StorageDoubleMap as _,
    traits::{
        fungible::{Inspect, Mutate},
        schedule::{
            v3::{Named, TaskName},
            DispatchTime,
        },
        Bounded, Consideration, EnsureOriginWithArg, Footprint,
    },
    PalletId,
};
use frame_system::pallet_prelude::*;
use sp_runtime::{
    traits::{Dispatchable, StaticLookup},
    DispatchResult,
};

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod extensions;
mod types;

pub mod weights;
pub use extensions::*;
pub use pallet::*;
pub use types::*;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        // Primitives: Some overarching types that come from the system (or the system depends on).

        /// The overarching event type.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The caller origin, overarching type of all pallets origins.
        type PalletsOrigin: From<frame_system::Origin<Self>>;
        /// The overarching call type.
        type RuntimeCall: Parameter
            + Dispatchable<RuntimeOrigin = Self::RuntimeOrigin>
            + Debug
            + From<Call<Self, I>>
            + From<frame_system::Call<Self>>;
        /// The weight information for this pallet.
        type WeightInfo: WeightInfo;

        // Origins: Types that manage authorization rules to allow or deny some caller origins to
        // execute a method.

        /// The origin to register an account. Returns an [`AccountId`] that identifies an account
        /// that holds the origin.
        type RegisterOrigin: EnsureOriginWithArg<
            Self::RuntimeOrigin,
            HashedUserId,
            Success = Self::AccountId,
        >;

        // Dependencies: The external components this pallet depends on.

        /// A structure to generate addresses.
        type AddressGenerator: AddressGenerator<Self, I>;
        /// The native fungible system of a runtime.
        type Balances: Inspect<Self::AccountId> + Mutate<Self::AccountId>;
        /// A single or composite authenticator that allows the pallet to handle the actions
        /// regarding assertion to register devices and authenticating with credentials.
        type Authenticator: Authenticator<Authority = AuthorityFromPalletId<Self::PalletId>>;
        /// The `Scheduler` system.
        type Scheduler: Named<
            BlockNumberFor<Self>,
            <Self as Config<I>>::RuntimeCall,
            Self::PalletsOrigin,
        >;

        // Considerations: Costs that are "taken from [the caller's] account temporarily in order to
        // offset the cost to the chain of holding some data Footprint in state".

        /// A `Consideration` helper to handle the deposits for registering an account. The account
        /// registrar would cover for the consideration.
        type RegistrarConsideration: Consideration<Self::AccountId, Footprint>;
        /// A `Consideration` helper to handle the deposits for storing devices.
        type DeviceConsideration: Consideration<Self::AccountId, Footprint>;
        /// A `Consideration` helper to handle the deposits for storing session keys.
        type SessionKeyConsideration: Consideration<Self::AccountId, Footprint>;

        // Parameters: A set of constant parameters to configure limits.

        /// A unique identification for the pallet.
        #[pallet::constant]
        type PalletId: Get<PalletId>;
        /// The maximum duration of a session
        #[pallet::constant]
        type MaxSessionDuration: Get<BlockNumberFor<Self>>;

        // Benchmarking: Types to handle benchmarks.

        /// A helper trait to set up benchmark tests.
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: BenchmarkHelper<Self, I>;
    }

    #[pallet::composite_enum]
    pub enum HoldReason {
        /// Holds an amount for registering accounts. Can be released when killing an account.
        AccountRegistration,
        /// Holds an amount for adding devices. Can be released by removing devices.
        AccountDevices,
        /// Holds an amount for storing session keys. Gets released automatically once the keys
        /// expire and get removed.
        SessionKeys,
    }

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    /// A map of the pass accounts registered by a system account, with a mapping of the
    /// [`HashedUserId`] used to register it.
    #[pallet::storage]
    pub type RegisteredAccounts<T: Config<I>, I: 'static = ()> =
        StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, HashedUserId, ()>;

    /// Counts how many pass accounts a system account has registered and holds an amount as
    /// registrar.
    #[pallet::storage]
    pub type RegistrarConsiderations<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (T::RegistrarConsideration, u64)>;

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

    /// Counts how many devices a pass account has registered and holds an amount to the account.
    #[pallet::storage]
    pub type DeviceConsiderations<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (T::DeviceConsideration, u64)>;

    #[pallet::storage]
    pub type SessionKeys<T: Config<I>, I: 'static = ()> =
        CountedStorageMap<_, Blake2_128Concat, T::AccountId, (T::AccountId, BlockNumberFor<T>)>;

    /// Counts how many sessions a pass account has registered and holds an amount to the account.
    #[pallet::storage]
    pub type SessionKeyConsiderations<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (T::SessionKeyConsideration, u64)>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        Registered {
            who: T::AccountId,
        },
        DeviceAdded {
            who: T::AccountId,
            device_id: DeviceId,
        },
        DeviceRemoved {
            who: T::AccountId,
            device_id: DeviceId,
        },
        SessionCreated {
            session_key: T::AccountId,
            until: BlockNumberFor<T>,
        },
        SessionRemoved {
            session_key: T::AccountId,
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
        /// There is an existing account for the given session key. Thus it cannot be used to
        /// register a new session.
        AccountForSessionKeyAlreadyExists,
        InvalidConsideration,
    }

    #[pallet::call(weight(<T as Config<I>>::WeightInfo))]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Registers an account. Takes a deposit to provision the account.
        #[pallet::call_index(0)]
        pub fn register(
            origin: OriginFor<T>,
            user: HashedUserId,
            attestation: DeviceAttestationOf<T, I>,
        ) -> DispatchResult {
            let registrar = &T::RegisterOrigin::ensure_origin(origin, &user)?;
            let address = &T::AddressGenerator::generate_address(user);

            // Handles the deposit of storage for the account
            ConsiderationHandler::<
                T::AccountId,
                RegistrarConsiderations<T, I>,
                T::RegistrarConsideration,
                HashedUserId,
            >::increment(registrar)?;

            Self::create_account(address)?;
            Self::try_add_device(address, attestation)
        }

        #[pallet::call_index(1)]
        pub fn add_device(
            origin: OriginFor<T>,
            attestation: DeviceAttestationOf<T, I>,
        ) -> DispatchResult {
            let address = &Self::ensure_signer_is_pass_account(origin)?;
            Self::try_add_device(address, attestation)
        }

        #[pallet::call_index(2)]
        pub fn remove_device(origin: OriginFor<T>, device_id: DeviceId) -> DispatchResult {
            let address = Self::ensure_signer_is_pass_account(origin)?;
            Self::try_remove_device(&address, &device_id)
        }

        #[pallet::call_index(3)]
        pub fn add_session_key(
            origin: OriginFor<T>,
            session: AccountIdLookupOf<T>,
            duration: Option<BlockNumberFor<T>>,
        ) -> DispatchResult {
            let address = &Self::ensure_signer_is_pass_account(origin)?;
            let session_key = &T::Lookup::lookup(session)?;

            // We must ensure that there is no account registered for that session key.
            //
            // Session keys are meant to be ephemeral, therefore they should never be tied to an
            // existing account.
            ensure!(
                !frame_system::Pallet::<T>::account_exists(session_key),
                Error::<T, I>::AccountForSessionKeyAlreadyExists
            );

            ConsiderationHandler::<
                T::AccountId,
                SessionKeyConsiderations<T, I>,
                T::SessionKeyConsideration,
                T::AccountId,
            >::increment(address)?;

            // Let's try to remove an existing session that uses the same session key (if any). This is
            // so we ensure we decrease the provider counter correctly.
            Self::try_remove_session_key(session_key)?;

            let until = duration
                .unwrap_or(T::MaxSessionDuration::get())
                .min(T::MaxSessionDuration::get());
            SessionKeys::<T, I>::insert(session_key.clone(), (address.clone(), until));
            Self::schedule_next_removal(session_key, duration)?;

            Self::deposit_event(Event::<T, I>::SessionCreated {
                session_key: session_key.clone(),
                until,
            });

            Ok(())
        }

        #[pallet::call_index(4)]
        pub fn remove_session_key(
            origin: OriginFor<T>,
            session_key: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::try_remove_session_key(&session_key)
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    pub fn address_for(user: HashedUserId) -> T::AccountId {
        T::AddressGenerator::generate_address(user)
    }

    /// Extracts the pass account from a session key.
    pub(crate) fn pass_account_from_session_key(who: &T::AccountId) -> Option<T::AccountId> {
        SessionKeys::<T, I>::get(who).map(|(s, _)| s)
    }

    /// Ensure that the signed origin maps onto an already existing pass account.
    pub(crate) fn ensure_signer_is_pass_account(
        origin: OriginFor<T>,
    ) -> Result<T::AccountId, DispatchError> {
        let who = ensure_signed(origin)?;
        ensure!(
            Devices::<T, I>::contains_prefix(&who),
            DispatchError::BadOrigin
        );
        Ok(who)
    }

    /// Provisions a new account.
    pub(crate) fn create_account(address: &T::AccountId) -> DispatchResult {
        // Fail to register if a system account already exists with the same address. Otherwise,
        // we have a new account!
        ensure!(
            frame_system::Pallet::<T>::inc_providers(address)
                == frame_system::IncRefStatus::Created,
            Error::<T, I>::AccountAlreadyRegistered
        );

        Self::deposit_event(Event::<T, I>::Registered {
            who: address.clone(),
        });

        Ok(())
    }

    pub(crate) fn authenticate(
        device_id: &DeviceId,
        credential: &CredentialOf<T, I>,
        extrinsic_context: &impl ExtrinsicContext,
    ) -> Result<T::AccountId, DispatchError> {
        let address = T::AddressGenerator::generate_address(credential.user_id());
        ensure!(
            Devices::<T, I>::contains_prefix(&address),
            Error::<T, I>::AccountNotFound
        );
        let device =
            Devices::<T, I>::get(&address, device_id).ok_or(Error::<T, I>::DeviceNotFound)?;
        device
            .verify_user(credential, extrinsic_context)
            .ok_or(Error::<T, I>::CredentialInvalid)?;

        Ok(address)
    }

    pub(crate) fn try_add_device(
        address: &T::AccountId,
        attestation: DeviceAttestationOf<T, I>,
    ) -> DispatchResult {
        let device_id = attestation.device_id();
        // Device attestations are considered "to be trusted", thus they don't require and extrinsic context.
        let device = T::Authenticator::verify_device(attestation.clone(), &[])
            .ok_or(Error::<T, I>::DeviceAttestationInvalid)?;

        ConsiderationHandler::<
            T::AccountId,
            DeviceConsiderations<T, I>,
            T::DeviceConsideration,
            DeviceOf<T, I>,
        >::increment(address)?;

        Devices::<T, I>::insert(address, device_id, device);

        Self::deposit_event(Event::<T, I>::DeviceAdded {
            who: address.clone(),
            device_id: *device_id,
        });

        Ok(())
    }

    pub(crate) fn try_remove_device(address: &T::AccountId, id: &DeviceId) -> DispatchResult {
        ensure!(
            Devices::<T, I>::contains_key(address, id),
            Error::<T, I>::DeviceNotFound
        );

        ConsiderationHandler::<
            T::AccountId,
            DeviceConsiderations<T, I>,
            T::DeviceConsideration,
            DeviceOf<T, I>,
        >::decrement(address)?;

        Devices::<T, I>::remove(address, id);

        Self::deposit_event(Event::<T, I>::DeviceRemoved {
            who: address.clone(),
            device_id: *id,
        });

        Ok(())
    }

    /// Removes a previously existing session. This is infallible.
    fn try_remove_session_key(session_key: &T::AccountId) -> DispatchResult {
        Self::cancel_scheduled_session_key_removal(session_key);

        if let Some(address) = &Self::pass_account_from_session_key(session_key) {
            ConsiderationHandler::<
                T::AccountId,
                SessionKeyConsiderations<T, I>,
                T::SessionKeyConsideration,
                T::AccountId,
            >::decrement(address)?;

            SessionKeys::<T, I>::remove(session_key);

            Self::deposit_event(Event::<T, I>::SessionRemoved {
                session_key: session_key.clone(),
            })
        }

        Ok(())
    }

    #[inline]
    fn task_name_from_session_key(session_key: &T::AccountId) -> TaskName {
        sp_core::blake2_256(&("remove_session_key", session_key).encode())
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

    /// Infallibly cancels an already scheduled session key removal
    fn cancel_scheduled_session_key_removal(session_key: &T::AccountId) {
        let _ = T::Scheduler::cancel_named(Self::task_name_from_session_key(session_key));
    }
}
