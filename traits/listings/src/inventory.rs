use frame_support::pallet_prelude::DispatchResult;
use frame_support::Parameter;

/// Methods to fetch the list of inventories.
pub trait Inspect<MerchantId> {
    /// The internal ID uniquely identifying two inventories from the same merchant.
    type Id: Parameter;

    /// Returns an iterable list of all the existing inventories.
    fn all() -> impl Iterator<Item = (MerchantId, Self::Id)>;

    /// Returns an iterable list of the inventories owned by a merchant.
    fn owned(merchant_id: &MerchantId) -> impl Iterator<Item = (MerchantId, Self::Id)>;

    /// Returns `false` if the inventory is archived, `true` otherwise.
    fn is_active(merchant_id: &MerchantId, id: &Self::Id) -> bool;

    /// Returns whether an inventory exists given its id.
    fn exists(merchant_id: &MerchantId, id: &Self::Id) -> bool {
        Self::owned(merchant_id).any(|(_, ref inventory_id)| inventory_id == id)
    }
}

/// Methods to manage the lifecycle of an inventory.
pub trait Lifecycle<MerchantId>: Inspect<MerchantId> {
    type AccountId;

    /// Creates a new inventory with a given identification
    fn create(merchant_id: &MerchantId, id: &Self::Id, owner: &Self::AccountId) -> DispatchResult;

    /// Archives an existing inventory with a given identification.
    ///
    /// When an inventory is archived, it's not possible to publish new items, and subscriptions
    /// associated to existing items cannot be renewed.
    ///
    /// The purchase of items via payments gets disabled, though transfers are still enabled.
    ///
    /// After archiving an inventory, it's not possible to return the inventory to an
    /// active state again.
    fn archive(merchant_id: &MerchantId, id: &Self::Id) -> DispatchResult;
}
