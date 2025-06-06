use super::*;

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
pub type BlockNumberFor<T, I> =
    <<T as Config<I>>::BlockNumberProvider as BlockNumberProvider>::BlockNumber;
pub(crate) type MerchantIdOf<T, I = ()> =
    <<T as Config<I>>::Listings as InspectItem<AccountIdOf<T>>>::MerchantId;
pub type InventoryIdOf<T, I = ()> =
    <<T as Config<I>>::Listings as InspectItem<AccountIdOf<T>>>::InventoryId;
pub type ItemIdOf<T, I = ()> = <<T as Config<I>>::Listings as InspectItem<AccountIdOf<T>>>::ItemId;
pub type ItemFullIdOf<T, I = ()> =
    ItemFullId<MerchantIdOf<T, I>, InventoryIdOf<T, I>, ItemIdOf<T, I>>;
pub type PaymentIdOf<T, I = ()> =
    <<T as Config<I>>::Payments as PaymentInspect<AccountIdOf<T>>>::Id;
pub type PaymentAssetIdOf<T, I = ()> =
    <<T as Config<I>>::Payments as PaymentInspect<AccountIdOf<T>>>::AssetId;
pub type PaymentBalanceOf<T, I = ()> =
    <<T as Config<I>>::Payments as PaymentInspect<AccountIdOf<T>>>::Balance;
pub type OrderDetailsOf<T, I = ()> = OrderDetails<
    AccountIdOf<T>,
    MerchantIdOf<T, I>,
    InventoryIdOf<T, I>,
    ItemIdOf<T, I>,
    PaymentIdOf<T, I>,
    <T as Config<I>>::MaxItemLen,
>;
pub(crate) type CartItemParameterOf<T, I = ()> = (ItemFullIdOf<T, I>, Option<AccountIdOf<T>>);

type ItemFullId<MerchantId, InventoryId, ItemId> = ((MerchantId, InventoryId), ItemId);

#[derive(Clone, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(MaxItemLen))]
pub struct OrderDetails<AccountId, MerchantId, InventoryId, ItemId, PaymentId, MaxItemLen: Get<u32>>
{
    pub(crate) status: OrderStatus,
    pub(crate) items:
        BoundedVec<OrderItem<AccountId, MerchantId, InventoryId, ItemId, PaymentId>, MaxItemLen>,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct OrderItem<AccountId, MerchantId, InventoryId, ItemId, PaymentId> {
    pub(crate) id: ItemFullId<MerchantId, InventoryId, ItemId>,
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

#[cfg(feature = "runtime-benchmarks")]
pub use benchmarks::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks {
    use super::*;
    use frame_support::traits::{
        fungible::{Inspect, Mutate as FunMutate},
        fungibles::{Create, Mutate},
    };

    pub(crate) type BalanceOf<T, I> = <<<T as Config<I>>::BenchmarkHelper as BenchmarkHelper<
        T,
        I,
    >>::Balances as Inspect<AccountIdOf<T>>>::Balance;

    pub trait BenchmarkHelper<T: Config<I>, I: 'static = ()> {
        /// The native `Balances` system.
        type Balances: FunMutate<T::AccountId>;
        /// The `Assets` system bound to the configuration `Listings` system.
        type Assets: Create<T::AccountId, AssetId = PaymentAssetIdOf<T, I>, Balance = PaymentBalanceOf<T, I>>
            + Mutate<T::AccountId, AssetId = PaymentAssetIdOf<T, I>, Balance = PaymentBalanceOf<T, I>>;
        type InventoryDeposit: Get<BalanceOf<T, I>>;
        type ItemDeposit: Get<BalanceOf<T, I>>;

        /// The identifier of the inventory created to gather the items for the order.
        fn inventory_id() -> (MerchantIdOf<T, I>, InventoryIdOf<T, I>);

        /// An iterator for getting multiple `item_id`s. Used when trying to build many items to
        /// set up a benchmark test.
        fn item_id(i: usize) -> ItemIdOf<T, I>;
    }
}
