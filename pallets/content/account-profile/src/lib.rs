#![cfg_attr(not(feature = "std"), no_std)]

//! # Account Profile Pallet
//!
//! Enables each account to associate itself with a single profile content item
//! from `fc-pallet-content`. The profile item must be owned by the sender,
//! must not be retracted, and must be revisionable (updatable).

use fc_pallet_content::{pallet::ItemId, ItemInspect};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Something that can inspect items in `fc-pallet-content`.
        type ContentStore: ItemInspect<Self::AccountId>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Mapping from account to their profile item ID.
    #[pallet::storage]
    #[pallet::getter(fn profile_of)]
    pub type Profiles<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, ItemId, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// An account has set its profile item.
        ProfileSet {
            account: T::AccountId,
            item_id: ItemId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The content item could not be found.
        ItemNotFound,
        /// The content item is not owned by the sender.
        NotItemOwner,
        /// The content item has been retracted.
        ItemRetracted,
        /// The content item is not revisionable (updatable).
        ItemNotRevisionable,
        /// The account does not have a profile set.
        NoProfile,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Set the profile item for the sender.
        ///
        /// The item must exist, be owned by the sender, not be retracted,
        /// and must be revisionable.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::set_profile())]
        pub fn set_profile(origin: OriginFor<T>, item_id: ItemId) -> DispatchResult {
            let account = ensure_signed(origin)?;

            // Ensure item exists and retrieve its owner.
            let owner = T::ContentStore::get_owner(&item_id).ok_or(Error::<T>::ItemNotFound)?;

            // Ensure sender owns the item.
            ensure!(owner == account, Error::<T>::NotItemOwner);

            // Ensure the item has not been retracted.
            ensure!(
                !T::ContentStore::is_retracted(&item_id),
                Error::<T>::ItemRetracted
            );

            // Ensure the item is still revisionable (updatable).
            ensure!(
                T::ContentStore::is_revisionable(&item_id),
                Error::<T>::ItemNotRevisionable
            );

            // Store the profile mapping.
            <Profiles<T>>::insert(&account, &item_id);

            Self::deposit_event(Event::ProfileSet { account, item_id });

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Returns the profile item ID for the given account, if set.
        pub fn get_profile(account: &T::AccountId) -> Option<ItemId> {
            <Profiles<T>>::get(account)
        }
    }
}
