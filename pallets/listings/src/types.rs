use super::*;
use frame_support::traits::{fungibles::Inspect, Incrementable};
pub use item::ItemPrice;

use serde::{Deserialize, Serialize};

/// The AssetId type bound to the pallet instance.
pub(crate) type AssetIdOf<T, I> = <<T as Config<I>>::Assets as Inspect<AccountIdOf<T>>>::AssetId;

/// The asset Balance type bound to the pallet instance.
pub(crate) type AssetBalanceOf<T, I> =
    <<T as Config<I>>::Assets as Inspect<AccountIdOf<T>>>::Balance;

/// The `MerchantId` configuration parameter.
pub(crate) type MerchantIdOf<T, I = ()> = <T as Config<I>>::MerchantId;

/// The `InventoryId` configuration parameter.
pub(crate) type InternalInventoryIdOf<T, I = ()> = <T as Config<I>>::InventoryId;

/// The composite `InventoryId` bound to the pallet instance.
pub type InventoryIdOf<T, I = ()> =
    InventoryId<<T as Config<I>>::MerchantId, <T as Config<I>>::InventoryId>;

/// The overarching `AccountId` type.
pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

/// The [`Item`][item::Item] type bound to the pallet instance.
pub(crate) type ItemOf<T, I = ()> =
    item::Item<AccountIdOf<T>, AssetIdOf<T, I>, AssetBalanceOf<T, I>>;

/// The [`ItemPrice`] type bound to the pallet instance.
pub type ItemPriceOf<T, I = ()> = ItemPrice<AssetIdOf<T, I>, AssetBalanceOf<T, I>>;

/// The ID of every item inside the inventory.
pub type ItemIdOf<T, I = ()> = ItemType<<T as Config<I>>::ItemSKU>;

/// A `BoundedVec` limited by the overarching `KeyLimit`.
pub(crate) type ItemKeyOf<T, I = ()> = BoundedVec<u8, <T as Config<I>>::NonfungiblesKeyLimit>;

/// A `BoundedVec` limited by the overarching `ValueLimit`.
pub(crate) type ItemValueOf<T, I = ()> = BoundedVec<u8, <T as Config<I>>::NonfungiblesValueLimit>;

pub(crate) type NativeBalanceOf<T, I = ()> = <
<T as Config<I>>::Balances as frame_support::traits::fungible::Inspect<AccountIdOf<T>>
>::Balance;

/// A set of attributes associated to an inventory.
#[derive(Encode)]
pub enum InventoryAttribute {
    /// Indicates if the inventory is archived.
    Archived,
}

/// A set of attributes associated to an item.
#[derive(Encode)]
pub enum ItemAttribute {
    /// The item basic info (name and price).
    #[codec(index = 10)]
    Info,
    /// Whether an item cannot be resold.
    #[codec(index = 11)]
    NotForResale,
}

/// The item's basic information
pub type ItemInfo<Name, Price> = (Name, Option<Price>);

/// The internal representation of a listings inventory ID.
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct InventoryId<MerchantId, Id>(pub MerchantId, pub Id);

impl<MerchantId, Id> From<(MerchantId, Id)> for InventoryId<MerchantId, Id> {
    fn from(value: (MerchantId, Id)) -> Self {
        Self(value.0, value.1)
    }
}

impl<MerchantId: Clone, Id: Clone + Incrementable> Incrementable for InventoryId<MerchantId, Id> {
    fn increment(&self) -> Option<Self> {
        // Increment shouldn't happen for inventory, but
        // we'll implement it anyway.
        self.1
            .increment()
            .map(|new_id| Self(self.0.clone(), new_id))
    }

    fn initial_value() -> Option<Self> {
        None
    }
}

impl<MerchantId, Id> From<InventoryId<MerchantId, Id>> for (MerchantId, Id) {
    fn from(value: InventoryId<MerchantId, Id>) -> Self {
        (value.0, value.1)
    }
}

/// The type an item can be, part of its unique identification.
#[derive(
    Serialize,
    Deserialize,
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    MaxEncodedLen,
    TypeInfo,
)]
pub enum ItemType<Id> {
    Unit(Id),
    Subscription(Id),
}

impl<T: Default> Default for ItemType<T> {
    fn default() -> Self {
        ItemType::Unit(Default::default())
    }
}

impl<T: Incrementable> Incrementable for ItemType<T> {
    fn increment(&self) -> Option<Self> {
        match self {
            ItemType::Unit(v) => v.increment().map(ItemType::Unit),
            ItemType::Subscription(v) => v.increment().map(ItemType::Subscription),
        }
    }

    fn initial_value() -> Option<Self> {
        T::initial_value().map(ItemType::Unit)
    }
}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<InventoryId, ItemId> {
    fn inventory_id() -> InventoryId;
    fn item_id() -> ItemId;
}
