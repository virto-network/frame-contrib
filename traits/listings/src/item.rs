use super::*;

use alloc::vec::Vec;
use codec::DecodeWithMemTracking;
use scale_info::TypeInfo;

#[derive(Encode, Decode, PartialEq, Clone, Debug, TypeInfo)]
pub struct Item<AccountId, Asset, Balance> {
    pub name: Vec<u8>,
    pub owner: AccountId,
    pub price: Option<ItemPrice<Asset, Balance>>,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, PartialEq, Clone, Debug, TypeInfo, MaxEncodedLen,
)]
pub struct ItemPrice<Asset, Balance> {
    pub asset: Asset,
    pub amount: Balance,
}

pub type InventoryIdOf<T, AccountId> = (
    <T as Inspect<AccountId>>::MerchantId,
    <T as Inspect<AccountId>>::InventoryId,
);
pub type ItemOf<T, AccountId> =
    Item<AccountId, <T as Inspect<AccountId>>::Asset, <T as Inspect<AccountId>>::Balance>;

pub use {
    Inspect as InspectItem, InspectEnumerable as ItemInspectEnumerable, Mutate as MutateItem,
};

/// Methods for fetching information about a regular item from an inventory.
pub trait Inspect<AccountId> {
    /// A listings merchant.
    type MerchantId: ListingsIdentifier;
    /// A type to uniquely identify an inventory from the same merchant.
    type InventoryId: ListingsIdentifier;
    /// A type to uniquely identify each item within an inventory.
    type ItemId: ListingsIdentifier;
    /// The type to represent an asset class used to set the price of an item.
    type Asset: Parameter + MaxEncodedLen;
    /// The type to represent the amount in the price of an item.
    type Balance: frame_support::traits::tokens::Balance;

    /// Returns the displayable name for an item.
    fn item(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> Option<Item<AccountId, Self::Asset, Self::Balance>>;

    /// Returns the creator of an item, if it exists.
    fn creator(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> Option<AccountId>;

    /// Returns an attribute associated to the item.
    fn attribute<K: Encode, V: Decode>(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        key: &K,
    ) -> Option<V>;

    /// Returns whether an item can be transferred.
    fn transferable(inventory_id: &InventoryIdOf<Self, AccountId>, id: &Self::ItemId) -> bool;

    /// Returns whether an item is available for resale.
    fn can_resell(inventory_id: &InventoryIdOf<Self, AccountId>, id: &Self::ItemId) -> bool;
}

/// Methods to fetching lists of items.
pub trait InspectEnumerable<AccountId>: Inspect<AccountId> {
    /// Returns an iterable list of the items published in an inventory.
    fn items(
        inventory_id: &InventoryIdOf<Self, AccountId>,
    ) -> impl Iterator<Item = (Self::ItemId, ItemOf<Self, AccountId>)>;

    /// Returns an iterable list of the items owned by an account.
    fn owned(
        owner: &AccountId,
    ) -> impl Iterator<
        Item = (
            impl Into<InventoryIdOf<Self, AccountId>>,
            Self::ItemId,
            ItemOf<Self, AccountId>,
        ),
    >;
}

pub trait Mutate<AccountId>: Inspect<AccountId> {
    /// Publish a new item in an active inventory.
    fn publish(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        name: Vec<u8>,
        maybe_price: Option<ItemPrice<Self::Asset, Self::Balance>>,
    ) -> DispatchResult;

    /// Enables an existing item to be resold.
    fn enable_resell(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult;

    /// Disables an existing item to be resold.
    fn disable_resell(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult;

    /// Marks an existing item as transferable
    fn enable_transfer(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult;

    /// Marks an existing item as non-transferable
    fn disable_transfer(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult;

    /// Forcefully transfers an item, even though is disabled for transfer.
    fn transfer(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        beneficiary: &AccountId,
    ) -> DispatchResult;

    /// Transfers an item, marking the beneficiary as the item creator.
    fn creator_transfer(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        beneficiary: &AccountId,
    ) -> DispatchResult;

    /// Sets the price on an existing item.
    fn set_price(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        price: ItemPrice<Self::Asset, Self::Balance>,
    ) -> DispatchResult;

    /// Clears the price on an existing item.
    fn clear_price(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult;

    /// Sets some metadata to an item.
    fn set_metadata(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        metadata: &[u8],
    ) -> DispatchResult;

    /// Clears the metadata of an inventory.
    fn clear_metadata(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult;

    /// Sets an arbitrary attribute on an existing item.
    fn set_attribute<K: Encode, V: Encode>(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        key: &K,
        value: V,
    ) -> DispatchResult;

    /// Clears an arbitrary attribute on an existing item.
    fn clear_attribute<K: Encode>(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        key: &K,
    ) -> DispatchResult;
}

pub mod subscriptions;
