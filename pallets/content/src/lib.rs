#![cfg_attr(not(feature = "std"), no_std)]

//! # Template Pallet
//!
//! This is the place where you'd put the documentation of the pallet

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use scale_info::prelude::vec::Vec;
use sp_io::hashing::blake2_256;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

pub use pallet::*;

pub const REVISIONABLE: u8 = 1 << 0;
pub const RETRACTABLE: u8 = 1 << 1;
pub const RETRACTED: u8 = 1 << 2;

#[derive(PartialEq, Clone, Debug, TypeInfo, Encode, Decode, DecodeWithMemTracking, Default)]
pub struct Nonce(pub [u8; 32]);

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[derive(PartialEq, Clone, Debug, TypeInfo, Encode, Decode, MaxEncodedLen)]
    pub struct Item<AccountId> {
        pub owner: AccountId, // Owner of the item
        pub revision_id: u32, // Latest revision_id
        pub flags: u8,
    }

    #[derive(
        PartialEq,
        Clone,
        Debug,
        TypeInfo,
        Default,
        Encode,
        Decode,
        DecodeWithMemTracking,
        MaxEncodedLen,
    )]
    pub struct ItemId(pub [u8; 32]);

    #[derive(PartialEq, Clone, Debug, Encode, Decode, TypeInfo, DecodeWithMemTracking, Default)]
    pub struct IpfsHash(pub [u8; 32]);

    // #[pallet::config]
    // pub trait Config: pallet_balances::Config + frame_system::Config {}

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        type WeightInfo: WeightInfo;
    }

    // Simple declaration of the `Pallet` type. It is placeholder we use to implement traits and
    // method.
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // #[pallet::call(weight(<T as Config>::WeightInfo))]
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::publish_item())]
        pub fn publish_item(
            origin: OriginFor<T>,
            nonce: Nonce,
            parents: Vec<ItemId>,
            flags: u8,
            links: Vec<ItemId>,
            ipfs_hash: IpfsHash,
        ) -> DispatchResult {
            let account = ensure_signed(origin)?;
            // Get item_id for the new item.
            let item_id = Self::get_item_id(account.clone(), nonce);
            // Ensure the item does not already exist.
            if <ItemState<T>>::contains_key(&item_id) {
                return Err(Error::<T>::ItemAlreadyExists.into());
            }
            // Store item in state.
            let item = Item {
                owner: account.clone(),
                revision_id: 0,
                flags,
            };
            <ItemState<T>>::insert(&item_id, item);
            // Emit event to log.
            Self::deposit_event(Event::PublishItem {
                item_id: item_id.clone(),
                owner: account.clone(),
                parents,
                flags,
            });
            Self::deposit_event(Event::PublishRevision {
                item_id: item_id.clone(),
                owner: account,
                revision_id: 0,
                links,
                ipfs_hash,
            });

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::publish_revision())]
        pub fn publish_revision(
            origin: OriginFor<T>,
            item_id: ItemId,
            links: Vec<ItemId>,
            ipfs_hash: IpfsHash,
        ) -> DispatchResult {
            let account = ensure_signed(origin)?;

            let mut item = <ItemState<T>>::get(&item_id).ok_or(Error::<T>::ItemNotFound)?;

            if item.owner != account.clone() {
                return Err(Error::<T>::WrongAccount.into());
            }

            if item.flags & RETRACTED != 0 {
                return Err(Error::<T>::ItemRetracted.into());
            }

            if item.flags & REVISIONABLE == 0 {
                return Err(Error::<T>::ItemNotRevisionable.into());
            }

            let revision_id = item.revision_id + 1;
            item.revision_id = revision_id;

            <ItemState<T>>::insert(&item_id, item);

            Self::deposit_event(Event::PublishRevision {
                item_id,
                owner: account,
                revision_id,
                links,
                ipfs_hash,
            });

            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::retract_item())]
        pub fn retract_item(origin: OriginFor<T>, item_id: ItemId) -> DispatchResult {
            let account = ensure_signed(origin)?;
            let mut item = <ItemState<T>>::get(&item_id).ok_or(Error::<T>::ItemNotFound)?;

            if item.owner != account.clone() {
                return Err(Error::<T>::WrongAccount.into());
            }

            if item.flags & RETRACTED != 0 {
                return Err(Error::<T>::ItemRetracted.into());
            }

            if item.flags & RETRACTABLE == 0 {
                return Err(Error::<T>::ItemNotRetractable.into());
            }

            item.flags = RETRACTED;
            <ItemState<T>>::insert(&item_id, item);
            Self::deposit_event(Event::RetractItem {
                item_id,
                owner: account,
            });

            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::set_not_revisionable())]
        pub fn set_not_revisionable(origin: OriginFor<T>, item_id: ItemId) -> DispatchResult {
            let account = ensure_signed(origin)?;
            let mut item = <ItemState<T>>::get(&item_id).ok_or(Error::<T>::ItemNotFound)?;

            if item.owner != account.clone() {
                return Err(Error::<T>::WrongAccount.into());
            }

            if item.flags & REVISIONABLE == 0 {
                return Err(Error::<T>::ItemNotRevisionable.into());
            }

            item.flags &= !REVISIONABLE;
            <ItemState<T>>::insert(&item_id, item);
            Self::deposit_event(Event::SetNotRevisionable {
                item_id,
                owner: account,
            });

            Ok(())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::set_not_retractable())]
        pub fn set_not_retractable(origin: OriginFor<T>, item_id: ItemId) -> DispatchResult {
            let account = ensure_signed(origin)?;
            let mut item = <ItemState<T>>::get(&item_id).ok_or(Error::<T>::ItemNotFound)?;

            if item.owner != account.clone() {
                return Err(Error::<T>::WrongAccount.into());
            }

            if item.flags & RETRACTABLE == 0 {
                return Err(Error::<T>::ItemNotRetractable.into());
            }

            item.flags &= !RETRACTABLE;
            <ItemState<T>>::insert(&item_id, item);
            Self::deposit_event(Event::SetNotRetractable {
                item_id,
                owner: account,
            });

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn get_item_id(account: T::AccountId, nonce: Nonce) -> ItemId {
            let mut item_id = ItemId::default();
            item_id
                .0
                .copy_from_slice(&blake2_256(&[account.encode(), nonce.encode()].concat()));
            item_id
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        // Success,
        PublishItem {
            item_id: ItemId,
            owner: T::AccountId,
            parents: Vec<ItemId>,
            flags: u8,
        },
        PublishRevision {
            item_id: ItemId,
            owner: T::AccountId,
            revision_id: u32,
            links: Vec<ItemId>,
            ipfs_hash: IpfsHash,
        },
        RetractItem {
            item_id: ItemId,
            owner: T::AccountId,
        },
        SetNotRevisionable {
            item_id: ItemId,
            owner: T::AccountId,
        },
        SetNotRetractable {
            item_id: ItemId,
            owner: T::AccountId,
        },
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        // Error,
        /// The item already exists.
        ItemAlreadyExists,
        /// The item could not be found.
        ItemNotFound,
        /// The item has been retracted.
        ItemRetracted,
        /// The item is not revisionable.
        ItemNotRevisionable,
        /// The item is not retractable.
        ItemNotRetractable,
        /// Wrong account.
        WrongAccount,
    }

    #[pallet::storage]
    #[pallet::getter(fn item)]
    pub type ItemState<T: Config> =
        StorageMap<_, Blake2_128Concat, ItemId, Item<T::AccountId>, OptionQuery>;
}

pub trait ItemInspect<AccountId> {
    fn get_owner(item_id: &pallet::ItemId) -> Option<AccountId>;
    fn is_revisionable(item_id: &pallet::ItemId) -> bool;
    fn is_retracted(item_id: &pallet::ItemId) -> bool;
}

impl<T: pallet::Config> ItemInspect<T::AccountId> for pallet::Pallet<T> {
    fn get_owner(item_id: &pallet::ItemId) -> Option<T::AccountId> {
        pallet::ItemState::<T>::get(item_id).map(|item| item.owner)
    }

    fn is_revisionable(item_id: &pallet::ItemId) -> bool {
        pallet::ItemState::<T>::get(item_id)
            .map(|item| item.flags & REVISIONABLE != 0)
            .unwrap_or(false)
    }

    fn is_retracted(item_id: &pallet::ItemId) -> bool {
        pallet::ItemState::<T>::get(item_id)
            .map(|item| item.flags & RETRACTED != 0)
            .unwrap_or(false)
    }
}
