use super::*;

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
pub type InventoryIdOf<T, I = ()> =
    <<T as Config<I>>::Listings as InspectItem<AccountIdOf<T>>>::InventoryId;
pub type ItemIdOf<T, I = ()> = <<T as Config<I>>::Listings as InspectItem<AccountIdOf<T>>>::Id;
pub type PaymentIdOf<T, I = ()> =
    <<T as Config<I>>::Payments as PaymentInspect<AccountIdOf<T>>>::Id;
pub type PaymentAssetIdOf<T, I = ()> =
    <<T as Config<I>>::Payments as PaymentInspect<AccountIdOf<T>>>::AssetId;
pub type PaymentBalanceOf<T, I = ()> =
    <<T as Config<I>>::Payments as PaymentInspect<AccountIdOf<T>>>::Balance;
pub type OrderDetailsOf<T, I = ()> = OrderDetails<
    AccountIdOf<T>,
    InventoryIdOf<T, I>,
    ItemIdOf<T, I>,
    PaymentIdOf<T, I>,
    <T as Config<I>>::MaxItemLen,
>;

#[derive(Clone, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(MaxItemLen))]
pub struct OrderDetails<AccountId, InventoryId, ItemId, PaymentId, MaxItemLen: Get<u32>> {
    pub(crate) status: OrderStatus,
    pub(crate) items: BoundedVec<OrderItem<AccountId, InventoryId, ItemId, PaymentId>, MaxItemLen>,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct OrderItem<AccountId, InventoryId, ItemId, PaymentId> {
    pub(crate) id: ItemId,
    pub(crate) inventory_id: InventoryId,
    pub(crate) seller: AccountId,
    pub(crate) beneficiary: Option<AccountId>,
    pub(crate) payment_id: Option<PaymentId>,
    pub(crate) delivery: Option<DeliveryStatus>,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub enum OrderStatus {
    /// The list of items is editable. A buyer can add or remove items, and they'll be locked once
    /// added to the list, meaning there are some restrictions for the item (i.e. cannot transfer or
    /// resell the item).
    Cart,
    /// The order is now ready to be paid. The list of items is no longer editable.
    Checkout,
    /// The order is cancelled. The list of items is now empty, and items are unlocked, ready to be
    /// acquired by another buyer.
    Cancelled,
    /// The order is paid. One or more items in the order haven't been fully processed (i.e. not yet
    /// transferred to the beneficiary, or the funds haven't been released by the seller).
    ///
    /// In this state, items are owned by the buyer (or beneficiaries if set), and are still locked,
    /// meaning the funds need to be released by the seller, or some time needs to be elapsed,
    /// before the items can be unlocked.
    InProgress,
    /// In this state, every item has been processed, and the order is now complete.
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub enum DeliveryStatus {
    Cancelled,
    Delivered,
}
