use super::*;

pub use {
    Inspect as InspectInventory, InspectEnumerable as InventoryInspectEnumerable,
    Lifecycle as InventoryLifecycle, Mutate as MutateInventory,
};

/// Methods to fetch information about a single inventory
pub trait Inspect {
    /// A listings merchant.
    type MerchantId: ListingsIdentifier;
    /// A type to uniquely identify an inventory from the same merchant.
    type InventoryId: ListingsIdentifier;

    /// Returns `false` if the inventory is archived, `true` otherwise.
    fn is_active(id: &(Self::MerchantId, Self::InventoryId)) -> bool;

    /// Returns whether an inventory exists given its id.
    fn exists(id: &(Self::MerchantId, Self::InventoryId)) -> bool;

    /// Retrieves an attribute of an inventory.
    fn inventory_attribute<K: Encode, V: Decode>(
        id: &(Self::MerchantId, Self::InventoryId),
        key: &K,
    ) -> Option<V>;
}

/// Methods to get iterable lists of inventories.
pub trait InspectEnumerable: Inspect {
    /// Returns an iterable list of all the existing inventories.
    fn all() -> impl Iterator<Item = impl Into<(Self::MerchantId, Self::InventoryId)>>;

    /// Returns an iterable list of the inventories owned by a merchant.
    fn owned(
        who: &Self::MerchantId,
    ) -> impl Iterator<Item = impl Into<(Self::MerchantId, Self::InventoryId)>>;
}

/// Methods to manage the lifecycle of an inventory.
pub trait Lifecycle<AccountId>: Inspect {
    /// Creates a new inventory with a given identification
    fn create(id: (Self::MerchantId, Self::InventoryId), owner: &AccountId) -> DispatchResult;

    /// Archives an existing inventory with a given identification.
    ///
    /// When an inventory is archived, it's not possible to publish new items, and subscriptions
    /// associated to existing items cannot be renewed.
    ///
    /// The purchase of items via payments gets disabled, though transfers are still enabled.
    ///
    /// After archiving an inventory, it's not possible to return the inventory to an
    /// active state again.
    fn archive(id: &(Self::MerchantId, Self::InventoryId)) -> DispatchResult;
}

/// Methods to mutate the state of an inventory.
pub trait Mutate: Inspect {
    /// Sets or clears the maximum amount of items an inventory can have.
    fn set_items_limit(
        id: &(Self::MerchantId, Self::InventoryId),
        limit: Option<usize>,
    ) -> DispatchResult;

    /// Sets an attribute on an inventory.
    fn set_inventory_attribute<K: Encode, V: Encode>(
        id: &(Self::MerchantId, Self::InventoryId),
        key: &K,
        value: &V,
    ) -> DispatchResult;

    /// Clears an attribute on an inventory, if the attribute exists.
    fn clear_inventory_attribute<K: Encode>(
        id: &(Self::MerchantId, Self::InventoryId),
        key: &K,
    ) -> DispatchResult;
}
