#![cfg_attr(not(feature = "std"), no_std)]

//! # Listings Pallet
//!
//! This pallet allows commerces to publish listings of items that can be exchanged using the `Pay`
//! trait.

extern crate alloc;
extern crate core;

use fc_traits_listings::*;
use frame_support::{
    pallet_prelude::*,
    traits::{
        fungible,
        nonfungibles_v2::{self, Inspect as _},
        EnsureOriginWithArg,
    },
};
use frame_system::pallet_prelude::*;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod impls;
mod types;
pub mod weights;

pub use pallet::*;
pub use types::*;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        /// The overarching type for events.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// A type that defines the weights different calls and methods benchmark.
        type WeightInfo: WeightInfo;

        /// A type that handles the native token and system balances.
        #[cfg(not(feature = "runtime-benchmarks"))]
        type Balances: frame_support::traits::fungible::Inspect<Self::AccountId>;

        #[cfg(feature = "runtime-benchmarks")]
        /// A type that handles the native token and system balances.
        type Balances: fungible::Inspect<Self::AccountId> + fungible::Mutate<Self::AccountId>;

        /// An associated type of assets system. This system must be the same one
        /// that `Payment` uses.
        type Assets: frame_support::traits::fungibles::Inspect<Self::AccountId>;

        /// An associated type of nonfungibles system. This is used to store the inventories and
        /// items.
        type Nonfungibles: nonfungibles_v2::Inspect<
                Self::AccountId,
                CollectionId = InventoryIdOf<Self, I>,
                ItemId = ItemType<Self::ItemSKU>,
            > + nonfungibles_v2::InspectEnumerable<
                Self::AccountId,
                CollectionId = InventoryIdOf<Self, I>,
                ItemId = ItemType<Self::ItemSKU>,
            > + nonfungibles_v2::Create<
                Self::AccountId,
                pallet_nfts::CollectionConfig<
                    NativeBalanceOf<Self, I>,
                    BlockNumberFor<Self>,
                    InventoryIdOf<Self, I>,
                >,
                CollectionId = InventoryIdOf<Self, I>,
                ItemId = ItemType<Self::ItemSKU>,
            > + nonfungibles_v2::Mutate<
                Self::AccountId,
                pallet_nfts::ItemConfig,
                CollectionId = InventoryIdOf<Self, I>,
                ItemId = ItemType<Self::ItemSKU>,
            > + nonfungibles_v2::Transfer<
                Self::AccountId,
                CollectionId = InventoryIdOf<Self, I>,
                ItemId = ItemType<Self::ItemSKU>,
            >;

        /// Limit size for attribute keys on the `Nonfungibles` system.
        type NonfungiblesKeyLimit: Get<u32>;

        /// Limit size for attribute values on the `Nonfungibles` system.
        type NonfungiblesValueLimit: Get<u32>;

        /// An origin authorized to create an inventory.
        type CreateInventoryOrigin: EnsureOriginWithArg<
            Self::RuntimeOrigin,
            InventoryIdOf<Self, I>,
            Success = Self::AccountId,
        >;

        /// An origin authorized to manage a specific inventory.
        type InventoryAdminOrigin: EnsureOriginWithArg<Self::RuntimeOrigin, InventoryIdOf<Self, I>>;

        /// A type that represents the identification of a merchant.
        type MerchantId: Parameter + Copy;

        /// A type that represents the unique identification of an inventory from a merchant.
        type InventoryId: Parameter + Copy;

        /// A type that represents the SKU of an item.
        type ItemSKU: Parameter + Copy;

        #[cfg(feature = "runtime-benchmarks")]
        /// Helper for executing pallet benchmarks
        type BenchmarkHelper: BenchmarkHelper<Self, I>;
    }

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// An inventory was created
        InventoryCreated {
            merchant: T::MerchantId,
            id: T::InventoryId,
            owner: T::AccountId,
        },
        /// An inventory was archived.
        ///
        /// This action is final.
        InventoryArchived {
            merchant: T::MerchantId,
            id: T::InventoryId,
        },
        /// A new item has been published.
        ItemPublished {
            inventory_id: InventoryIdOf<T, I>,
            id: ItemIdOf<T, I>,
        },
        /// The price for an item has been set.
        ItemPriceSet {
            inventory_id: InventoryIdOf<T, I>,
            id: ItemIdOf<T, I>,
            price: ItemPriceOf<T, I>,
        },
        /// An item has been marked either as _"not for resale"_ or not.
        MarkNotForResale {
            inventory_id: InventoryIdOf<T, I>,
            id: ItemIdOf<T, I>,
            not_for_resale: bool,
        },
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// The specified inventory does not exist.
        UnknownInventory,
        /// The specified inventory already exists.
        AlreadyExistingInventory,
        /// The specified inventory is archived and cannot be mutated.
        ArchivedInventory,
        /// The specified item does not exist.
        UnknownItem,
        /// The caller does not have permissions to mutate an item.
        NoPermission,
        /// The specified item is not enabled for resale.
        NotForResale,
        /// The specified item is not transferable.
        ItemNonTransferable,
    }

    #[pallet::call(weight(<T as Config<I>>::WeightInfo))]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Creates an inventory from an authorized origin. An inventory is a collection of items
        /// with a unique identification. The `CreateInventoryOrigin` is a valid origin that can
        /// administer origins, and must resolve to an `AccountId` which will be used to define
        /// the owner account of the inventory.
        ///
        /// - `origin`: The origin used to create the inventory.
        /// - `inventory_id`: A composite value which contains the `MerchantId` and an internal `Id`.
        #[pallet::call_index(0)]
        pub fn create_inventory(
            origin: OriginFor<T>,
            inventory_id: InventoryIdOf<T, I>,
        ) -> DispatchResult {
            let owner = T::CreateInventoryOrigin::ensure_origin(origin, &inventory_id)?;
            let InventoryId(merchant, id) = &inventory_id;

            ensure!(
                !Self::exists(merchant, id),
                Error::<T, I>::AlreadyExistingInventory,
            );
            Self::create(merchant, id, &owner)?;

            Self::deposit_event(Event::<T, I>::InventoryCreated {
                merchant: *merchant,
                id: *id,
                owner: owner.clone(),
            });
            Ok(())
        }

        /// Archives an inventory, meaning the inventory cannot be longer mutated. This action is
        /// final. The caller must be an authorized `InventoryAdminOrigin` for the given inventory.
        ///
        /// - `inventory_id`: The ID of the inventory to be archived.
        #[pallet::call_index(1)]
        pub fn archive_inventory(
            origin: OriginFor<T>,
            inventory_id: InventoryIdOf<T, I>,
        ) -> DispatchResult {
            Self::ensure_active_inventory(&inventory_id)?;
            let InventoryId(ref merchant, ref id) = inventory_id;
            T::InventoryAdminOrigin::ensure_origin(origin, &inventory_id)?;

            Self::archive(merchant, id)?;

            Self::deposit_event(Event::<T, I>::InventoryArchived {
                merchant: *merchant,
                id: *id,
            });
            Ok(())
        }

        /// Publishes an item in an existing inventory. The caller must be a valid
        /// [`InventoryAdminOrigin`][T::InventoryAdminOrigin] for the given inventory.
        ///
        /// - `inventory_id`: The identification of the inventory under which the item will be
        ///    published.
        /// - `item_id`: An unique identification for the item that will be published in the
        ///    inventory.
        /// - `name`: A valid name for the item.
        /// - `maybe_price`: Optionally, include the price of the item.
        #[pallet::call_index(2)]
        pub fn publish_item(
            origin: OriginFor<T>,
            inventory_id: InventoryIdOf<T, I>,
            id: ItemIdOf<T, I>,
            name: ItemValueOf<T, I>,
            maybe_price: Option<ItemPriceOf<T, I>>,
        ) -> DispatchResult {
            Self::ensure_active_inventory(&inventory_id)?;
            T::InventoryAdminOrigin::ensure_origin(origin, &inventory_id)?;

            Self::publish(&inventory_id, &id, name.to_vec(), maybe_price.clone())?;

            Self::deposit_event(Event::ItemPublished { inventory_id, id });
            if let Some(price) = maybe_price {
                Self::deposit_event(Event::ItemPriceSet {
                    inventory_id,
                    id,
                    price,
                });
            }
            Ok(())
        }

        /// Sets the price of an existing item. This places a published item as _"for sale"_ and
        /// enables it to be purchased by an external system.
        ///
        /// - `origin`: can be either
        ///   - A valid [`InventoryAdminOrigin`][T::InventoryAdminOrigin] for the given inventory,
        ///     after which the owner of the item must be the same owner of the inventory, or
        ///   - A signed origin, where the caller must be the owner of the item, and the item must be
        ///     transferable and enabled for resale.
        /// - `inventory_id`: The identification of the inventory under which the item will be
        ///    published.
        /// - `item_id`: An unique identification for the item that will be published in the
        ///    inventory.
        /// - `price`: The new price of the item.
        #[pallet::call_index(3)]
        pub fn set_item_price(
            origin: OriginFor<T>,
            inventory_id: InventoryIdOf<T, I>,
            id: ItemIdOf<T, I>,
            price: ItemPriceOf<T, I>,
        ) -> DispatchResult {
            Self::ensure_active_inventory(&inventory_id)?;
            match T::InventoryAdminOrigin::ensure_origin(origin.clone(), &inventory_id) {
                Ok(_) => Self::ensure_item_owned_by_creator(&inventory_id, &id)?,
                Err(_) => {
                    let item = Self::item(&inventory_id, &id).ok_or(Error::<T, I>::UnknownItem)?;
                    let who = ensure_signed(origin)?;

                    // The owner of an item can set a price for an item, and the item must be
                    // transferable and enabled for resale.
                    ensure!(item.owner == who, Error::<T, I>::NoPermission);
                    ensure!(
                        Self::transferable(&inventory_id, &id),
                        Error::<T, I>::ItemNonTransferable,
                    );
                    ensure!(
                        Self::can_resell(&inventory_id, &id),
                        Error::<T, I>::NotForResale
                    );
                }
            }

            Self::set_price(&inventory_id, &id, price.clone())?;

            Self::deposit_event(Event::ItemPriceSet {
                inventory_id,
                id,
                price,
            });
            Ok(())
        }

        /// Marks whether an item can be transferred or not. The caller must be a valid
        /// [`InventoryAdminOrigin`][T::InventoryAdminOrigin] for the given inventory.
        ///
        /// - `inventory_id`: The identification of the inventory under which the item will be
        ///    marked as transferable or not.
        /// - `item_id`: An unique identification for the item that will be marked as transferable
        ///   or not.
        /// - `can_transfer`: Whether an item will be transferable or not.
        #[pallet::call_index(4)]
        pub fn mark_item_can_transfer(
            origin: OriginFor<T>,
            inventory_id: InventoryIdOf<T, I>,
            id: ItemIdOf<T, I>,
            can_transfer: bool,
        ) -> DispatchResult {
            Self::ensure_active_inventory(&inventory_id)?;
            T::Nonfungibles::owner(&inventory_id, &id).ok_or(Error::<T, I>::UnknownItem)?;
            T::InventoryAdminOrigin::ensure_origin(origin, &inventory_id)?;

            Self::mark_can_transfer(&inventory_id, &id, can_transfer)
        }

        /// Marks whether an item is marked as _"not for resale"_ or not. The caller must be a valid
        /// [`InventoryAdminOrigin`][T::InventoryAdminOrigin] for the given inventory.
        ///
        /// The item must be in possession of the inventory owner to be mutate (i.e. it's not
        /// possible to mark an item as _"not for sale"_ once you sold it.
        ///
        /// - `inventory_id`: The identification of the inventory under which the item will be
        ///    marked as transferable or not.
        /// - `item_id`: An unique identification for the item that will be marked as transferable
        ///   or not.
        /// - `not_for_resale`: Whether an item cannot be resold, or not.
        #[pallet::call_index(5)]
        pub fn mark_item_not_for_resale(
            origin: OriginFor<T>,
            inventory_id: InventoryIdOf<T, I>,
            id: ItemIdOf<T, I>,
            not_for_resale: bool,
        ) -> DispatchResult {
            Self::ensure_active_inventory(&inventory_id)?;
            Self::ensure_item_owned_by_creator(&inventory_id, &id)?;
            T::InventoryAdminOrigin::ensure_origin(origin, &inventory_id)?;

            Self::mark_not_for_resale(&inventory_id, &id, not_for_resale)?;

            Self::deposit_event(Event::MarkNotForResale {
                inventory_id,
                id,
                not_for_resale,
            });
            Ok(())
        }

        /// Sets an attribute on an item. The `origin` must be a valid
        /// [`InventoryAdminOrigin`][T::InventoryAdminOrigin] for the given inventory.
        ///
        /// - `inventory_id`: The identification of the inventory under which the item will be
        ///    mutated.
        /// - `item_id`: An unique identification for the item that will be set or cleared an
        ///    attribute for.
        /// - `key`: The key of the attribute.
        /// - `maybe_value`: If `Some`, sets an attribute. Clears an attribute otherwise.
        #[pallet::call_index(6)]
        #[pallet::weight(
            if maybe_value.is_some() {
                <T as Config<I>>::WeightInfo::set_item_attribute()
            } else {
                <T as Config<I>>::WeightInfo::clear_item_attribute()
            }
		)]
        pub fn set_item_attribute(
            origin: OriginFor<T>,
            inventory_id: InventoryIdOf<T, I>,
            id: ItemIdOf<T, I>,
            key: ItemKeyOf<T, I>,
            maybe_value: Option<ItemValueOf<T, I>>,
        ) -> DispatchResult {
            Self::ensure_active_inventory(&inventory_id)?;
            ensure!(
                Self::item(&inventory_id, &id).is_some(),
                Error::<T, I>::UnknownItem
            );
            T::InventoryAdminOrigin::ensure_origin(origin, &inventory_id)?;

            if let Some(value) = maybe_value {
                Self::set_attribute(&inventory_id, &id, &key, value.to_vec())
            } else {
                Self::clear_attribute(&inventory_id, &id, &key)
            }
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    fn ensure_item_owned_by_creator(
        inventory_id: &InventoryIdOf<T, I>,
        id: &ItemIdOf<T, I>,
    ) -> DispatchResult {
        let creator = T::Nonfungibles::collection_owner(inventory_id)
            .ok_or(Error::<T, I>::UnknownInventory)?;
        let item::Item { owner, .. } =
            Self::item(inventory_id, id).ok_or(Error::<T, I>::UnknownItem)?;

        ensure!(owner == creator, Error::<T, I>::NoPermission);
        Ok(())
    }

    fn ensure_active_inventory(inventory_id: &InventoryIdOf<T, I>) -> DispatchResult {
        let InventoryId(merchant, id) = &inventory_id;
        T::Nonfungibles::collection_owner(inventory_id).ok_or(Error::<T, I>::UnknownInventory)?;
        ensure!(
            Self::is_active(merchant, id),
            Error::<T, I>::ArchivedInventory
        );
        Ok(())
    }
}
