#![cfg_attr(not(feature = "std"), no_std)]

//! # Black Hole Pallet
//!
//! This pallet owns an account, which receives transfers from other accounts. Then, periodically
//! it burns the balance the pallet account owns.

extern crate alloc;

use frame::prelude::*;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod address_mapper;
pub mod weights;

pub use weights::*;

pub use pallet::*;

#[frame::pallet]
pub mod pallet {
    use super::*;
    use frame::traits::{Block, Header};
    use fungible::{Inspect, Mutate};

    pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    pub(crate) type BalanceOf<T> = <<T as Config>::Balances as Inspect<AccountIdOf<T>>>::Balance;
    pub(crate) type SystemBlockNumberFor<T> =
        <<<T as frame_system::Config>::Block as Block>::Header as Header>::Number;
    pub(crate) type BlockNumberFor<T> =
        <<T as Config>::BlockNumberProvider as BlockNumberProvider>::BlockNumber;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        // Primitives: Some overarching types that come from the system (or the system depends on).

        /// The overarching runtime event type
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The Weight info
        type WeightInfo: WeightInfo;

        // Origins: Types that manage authorization rules to allow or deny some caller origins to
        // execute a method.

        /// The origin allowed dispatching a call on behalf of the pallet account (a.k.a. the event
        /// horizon).
        type EventHorizonDispatchOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        // Dependencies: The external components this pallet depends on.

        /// The native balance system.
        type Balances: Mutate<Self::AccountId>;
        /// The provider of the block number.
        type BlockNumberProvider: BlockNumberProvider;

        // Parameters: A set of constant parameters to configure limits.

        /// An id for this pallet.
        #[pallet::constant]
        type PalletId: Get<PalletId>;
        /// The burn period. After at least the given number of blocks since the last burn elapsed,
        /// the burn mechanism will take place.
        #[pallet::constant]
        type BurnPeriod: Get<BlockNumberFor<Self>>;
    }

    /// The last time a burn happened (0 if never).
    #[pallet::storage]
    pub type LastBurn<T> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;
    /// Counts the accumulated balance that's been burned so far.
    #[pallet::storage]
    pub type BlackHoleMass<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        BalanceBurned,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<SystemBlockNumberFor<T>> for Pallet<T> {
        fn on_idle(_: SystemBlockNumberFor<T>, remaining_weight: Weight) -> Weight {
            if remaining_weight.all_lt(T::WeightInfo::burn()) {
                return Zero::zero();
            }
            Self::burn()
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight({
            let di = call.get_dispatch_info();
			let weight = T::WeightInfo::dispatch_as_event_horizon()
				.saturating_add(T::DbWeight::get().reads_writes(1, 1))
				.saturating_add(di.call_weight);
			(weight, di.class)
        })]
        pub fn dispatch_as_event_horizon(
            origin: OriginFor<T>,
            call: Box<T::RuntimeCall>,
        ) -> DispatchResult {
            T::EventHorizonDispatchOrigin::ensure_origin(origin)?;
            Self::do_initialize();

            call.dispatch(frame_system::RawOrigin::Signed(Self::event_horizon()).into())
                .map(|_| ())
                .map_err(|e| e.error)
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn event_horizon() -> T::AccountId {
            T::PalletId::get().into_account_truncating()
        }

        #[inline]
        fn do_initialize() {
            if !frame_system::Pallet::<T>::account_exists(&Self::event_horizon()) {
                frame_system::Pallet::<T>::inc_providers(&Self::event_horizon());
            }
        }

        pub(crate) fn burn() -> Weight {
            if LastBurn::<T>::get().le(&T::BlockNumberProvider::current_block_number()
                .saturating_sub(T::BurnPeriod::get()))
            {
                let burn_account = Self::event_horizon();
                let burn_balance = T::Balances::total_balance(&burn_account);

                // Just burn it.
                let _ = T::Balances::burn_from(
                    &burn_account,
                    burn_balance,
                    Preservation::Expendable,
                    Precision::Exact,
                    Fortitude::Force,
                );

                BlackHoleMass::<T>::set(BlackHoleMass::<T>::get().saturating_add(burn_balance));
                LastBurn::<T>::set(T::BlockNumberProvider::current_block_number());
                Self::deposit_event(Event::<T>::BalanceBurned);
            }

            T::WeightInfo::burn()
        }
    }
}
