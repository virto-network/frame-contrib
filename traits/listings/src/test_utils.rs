use super::*;
use crate::item::ItemPrice;
use core::marker::PhantomData;
use frame_support::{
    pallet_prelude::*, sp_runtime::traits::TrailingZeroInput, traits::tokens::Balance,
};

pub trait Config<I: 'static = ()>: frame_system::Config {
    type MerchantId: Parameter + Copy + MaybeSerializeDeserialize + MaxEncodedLen;
    type InventoryId: Parameter + Copy + MaybeSerializeDeserialize + MaxEncodedLen;
    type ItemId: Parameter + Copy + MaybeSerializeDeserialize + MaxEncodedLen;

    type AssetId: Parameter + MaxEncodedLen;
    type Balance: Balance;

    type MaxKeyLen: Get<u32>;
    type MaxValueLen: Get<u32>;
}

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type MerchantIdOf<T, I> = <T as Config<I>>::MerchantId;
type InventoryIdOf<T, I> = <T as Config<I>>::InventoryId;
type ItemIdOf<T, I> = <T as Config<I>>::ItemId;
type ItemOf<T, I> =
    item::Item<AccountIdOf<T>, <T as Config<I>>::AssetId, <T as Config<I>>::Balance>;

/// A map of storing attributes.
///
/// Each map supports up to 256 attributes. This should be more than enough for test purposes.
type AttributesMapOf<T, I> = BoundedBTreeMap<
    BoundedVec<u8, <T as Config<I>>::MaxKeyLen>,
    BoundedVec<u8, <T as Config<I>>::MaxValueLen>,
    ConstU32<256>,
>;

/// A test inventory. This is the simplest structure we can handle without incurring in multiple
/// storage types.
#[derive(DebugNoBound, Encode, Decode, PartialEqNoBound, MaxEncodedLen, TypeInfo)]
#[codec(mel_bound(T: Config))]
struct Inventory<T: Config<I>, I: 'static = ()> {
    /// The account that owns the inventory.
    owner: T::AccountId,
    /// Whether an inventory is active or not.
    active: bool,
    /// The limit of items
    items_limit: Option<u32>,
    /// The map of attributes for the inventory.
    attributes: AttributesMapOf<T, I>,
}

/// A test item. This is the simplest structure we can handle without incurring in multiple
/// storage types.
#[derive(DebugNoBound, Encode, Decode, PartialEqNoBound, MaxEncodedLen, TypeInfo)]
#[codec(mel_bound(T: Config))]
struct Item<T: Config<I>, I: 'static = ()> {
    /// The account that created the item.
    creator: T::AccountId,
    /// The account that owns the item.
    owner: T::AccountId,
    /// The item name.
    name: BoundedVec<u8, <T as Config<I>>::MaxValueLen>,
    /// The item price.
    price: Option<ItemPrice<<T as Config<I>>::AssetId, <T as Config<I>>::Balance>>,
    /// Whether the item can be transferred to another account.
    transferable: bool,
    /// The item can be resold.
    can_resell: bool,
    /// The map of attributes for the item.
    attributes: AttributesMapOf<T, I>,
}

/// The inventories, grouped by [`MerchantId`][T::MerchantId]
#[frame_support::storage_alias(dynamic)]
pub type Inventories<T: Config<I>, I: 'static> = StorageDoubleMap<
    MockListings<T, I>,
    Twox64Concat,
    MerchantIdOf<T, I>,
    Twox64Concat,
    InventoryIdOf<T, I>,
    Inventory<T, I>,
>;
/// The items within an inventory.
#[frame_support::storage_alias(dynamic)]
pub type Items<T: Config<I>, I: 'static> = StorageDoubleMap<
    MockListings<T, I>,
    Twox64Concat,
    (MerchantIdOf<T, I>, InventoryIdOf<T, I>),
    Twox64Concat,
    ItemIdOf<T, I>,
    Item<T, I>,
>;

pub struct MockListings<T, I = ()>(PhantomData<(T, I)>);

impl<T, I> Get<&'static str> for MockListings<T, I> {
    fn get() -> &'static str {
        "MockListings"
    }
}

impl<T: Config<I>, I: 'static> InspectInventory for MockListings<T, I> {
    type MerchantId = T::MerchantId;
    type InventoryId = T::InventoryId;

    fn is_active((merchant_id, inventory_id): &(Self::MerchantId, Self::InventoryId)) -> bool {
        Inventories::<T, I>::get(merchant_id, inventory_id)
            .map(|Inventory { active, .. }| active)
            .unwrap_or_default()
    }

    fn exists((merchant_id, inventory_id): &(Self::MerchantId, Self::InventoryId)) -> bool {
        Inventories::<T, I>::contains_key(merchant_id, inventory_id)
    }

    fn inventory_attribute<K: Encode, V: Decode>(
        (merchant_id, inventory_id): &(Self::MerchantId, Self::InventoryId),
        key: &K,
    ) -> Option<V> {
        let Inventory { attributes, .. } = Inventories::<T, I>::get(merchant_id, inventory_id)?;
        attributes
            .get(&key.using_encoded(|b| BoundedVec::truncate_from(b.to_vec())))
            .and_then(|v| Decode::decode(&mut TrailingZeroInput::new(v)).ok())
    }
}

impl<T: Config<I>, I: 'static> InventoryInspectEnumerable for MockListings<T, I> {
    fn all() -> impl Iterator<Item = impl Into<(Self::MerchantId, Self::InventoryId)>> {
        Inventories::<T, I>::iter().map(|(m, i, _)| (m, i))
    }

    fn owned(
        who: &Self::MerchantId,
    ) -> impl Iterator<Item = impl Into<(Self::MerchantId, Self::InventoryId)>> {
        Inventories::<T, I>::iter_prefix(who).map(|(i, _)| (*who, i))
    }
}

impl<T: Config<I>, I: 'static> InventoryLifecycle<T::AccountId> for MockListings<T, I> {
    fn create(
        (m, i): (Self::MerchantId, Self::InventoryId),
        owner: &T::AccountId,
    ) -> DispatchResult {
        Inventories::<T, I>::insert(
            m,
            i,
            Inventory {
                owner: owner.clone(),
                active: true,
                items_limit: None,
                attributes: BoundedBTreeMap::new(),
            },
        );
        Ok(())
    }

    fn archive((m, i): &(Self::MerchantId, Self::InventoryId)) -> DispatchResult {
        Inventories::<T, I>::try_mutate(m, i, |maybe_inventory| {
            let Some(Inventory { active, .. }) = maybe_inventory else {
                Err(DispatchError::Other("UnknownInventory"))?
            };
            *active = false;
            Ok(())
        })
    }
}

impl<T: Config<I>, I: 'static> MutateInventory for MockListings<T, I> {
    fn set_items_limit(
        (m, i): &(Self::MerchantId, Self::InventoryId),
        limit: Option<usize>,
    ) -> DispatchResult {
        Inventories::<T, I>::try_mutate(m, i, |maybe_inventory| {
            let Some(Inventory { items_limit, .. }) = maybe_inventory else {
                Err(DispatchError::Other("UnknownInventory"))?
            };
            *items_limit = limit.map(|l| l as u32);
            Ok(())
        })
    }

    fn set_inventory_attribute<K: Encode, V: Encode>(
        (m, i): &(Self::MerchantId, Self::InventoryId),
        key: &K,
        value: &V,
    ) -> DispatchResult {
        Inventories::<T, I>::try_mutate(m, i, |maybe_inventory| {
            let Some(Inventory { attributes, .. }) = maybe_inventory else {
                Err(DispatchError::Other("UnknownInventory"))?
            };

            let key = key.using_encoded(|v| {
                BoundedVec::try_from(v.to_vec())
                    .map_err(|_| DispatchError::Other("MaxKeyLenExceeded"))
            })?;
            let value = value.using_encoded(|v| {
                BoundedVec::try_from(v.to_vec())
                    .map_err(|_| DispatchError::Other("MaxValueLenExceeded"))
            })?;
            attributes
                .try_insert(key, value)
                .map_err(|_| DispatchError::Other("MaxAttributesExceeded"))?;
            Ok(())
        })
    }

    fn clear_inventory_attribute<K: Encode>(
        (m, i): &(Self::MerchantId, Self::InventoryId),
        key: &K,
    ) -> DispatchResult {
        Inventories::<T, I>::try_mutate(m, i, |maybe_inventory| {
            let Some(Inventory { attributes, .. }) = maybe_inventory else {
                Err(DispatchError::Unavailable)?
            };

            let key = key.using_encoded(|v| {
                BoundedVec::try_from(v.to_vec())
                    .map_err(|_| DispatchError::Other("MaxKeyLenExceeded"))
            })?;
            attributes.remove(&key);
            Ok(())
        })
    }
}

impl<T: Config<I>, I: 'static> InspectItem<T::AccountId> for MockListings<T, I> {
    type MerchantId = T::MerchantId;
    type InventoryId = T::InventoryId;
    type ItemId = T::ItemId;
    type Asset = T::AssetId;
    type Balance = T::Balance;

    fn item(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
    ) -> Option<ItemOf<T, I>> {
        Items::<T, I>::get(inventory_id, id).map(
            |Item {
                 name, owner, price, ..
             }| item::Item {
                name: name.into_inner(),
                owner,
                price,
            },
        )
    }

    fn creator(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
    ) -> Option<T::AccountId> {
        Items::<T, I>::get(inventory_id, id).map(|Item { creator, .. }| creator)
    }

    fn attribute<K: Encode, V: Decode>(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
        key: &K,
    ) -> Option<V> {
        let Item { attributes, .. } = Items::<T, I>::get(inventory_id, id)?;
        attributes
            .get(&key.using_encoded(|b| BoundedVec::truncate_from(b.to_vec())))
            .and_then(|v| Decode::decode(&mut TrailingZeroInput::new(v)).ok())
    }

    fn transferable(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
    ) -> bool {
        Items::<T, I>::get(inventory_id, id)
            .map(|Item { transferable, .. }| transferable)
            .unwrap_or_default()
    }

    fn can_resell(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
    ) -> bool {
        Items::<T, I>::get(inventory_id, id)
            .map(|Item { can_resell, .. }| can_resell)
            .unwrap_or_default()
    }
}

impl<T: Config<I>, I: 'static> MutateItem<T::AccountId> for MockListings<T, I> {
    fn publish(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
        name: Vec<u8>,
        price: Option<ItemPrice<Self::Asset, Self::Balance>>,
    ) -> DispatchResult {
        let Inventory { owner: creator, .. } =
            Inventories::<T, I>::get(inventory_id.0, inventory_id.1)
                .ok_or(DispatchError::Other("UnknownInventory"))?;
        Items::<T, I>::insert(
            inventory_id,
            id,
            Item {
                creator: creator.clone(),
                owner: creator,
                name: BoundedVec::try_from(name)
                    .map_err(|_| DispatchError::Other("MaxValueLenExceeded"))?,
                price,
                transferable: true,
                can_resell: true,
                attributes: Default::default(),
            },
        );
        Ok(())
    }

    fn enable_resell(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult {
        Items::<T, I>::try_mutate(inventory_id, id, |maybe_item| {
            let Some(Item { can_resell, .. }) = maybe_item else {
                Err(DispatchError::Other("UnknownItem"))?
            };
            *can_resell = true;
            Ok(())
        })
    }

    fn disable_resell(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult {
        Items::<T, I>::try_mutate(inventory_id, id, |maybe_item| {
            let Some(Item { can_resell, .. }) = maybe_item else {
                Err(DispatchError::Other("UnknownItem"))?
            };
            *can_resell = false;
            Ok(())
        })
    }

    fn enable_transfer(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult {
        Items::<T, I>::try_mutate(inventory_id, id, |maybe_item| {
            let Some(Item { transferable, .. }) = maybe_item else {
                Err(DispatchError::Other("UnknownItem"))?
            };
            *transferable = true;
            Ok(())
        })
    }

    fn disable_transfer(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult {
        Items::<T, I>::try_mutate(inventory_id, id, |maybe_item| {
            let Some(Item { transferable, .. }) = maybe_item else {
                Err(DispatchError::Other("UnknownItem"))?
            };
            *transferable = false;
            Ok(())
        })
    }

    fn transfer(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
        beneficiary: &T::AccountId,
    ) -> DispatchResult {
        Items::<T, I>::try_mutate(inventory_id, id, |maybe_item| {
            let Some(Item { owner, .. }) = maybe_item else {
                Err(DispatchError::Other("UnknownItem"))?
            };
            *owner = beneficiary.clone();
            Ok(())
        })
    }

    fn set_price(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
        new_price: ItemPrice<Self::Asset, Self::Balance>,
    ) -> DispatchResult {
        Items::<T, I>::try_mutate(inventory_id, id, |maybe_item| {
            let Some(Item { price, .. }) = maybe_item else {
                Err(DispatchError::Other("UnknownItem"))?
            };
            *price = Some(new_price);
            Ok(())
        })
    }

    fn set_attribute<K: Encode, V: Encode>(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
        key: &K,
        value: V,
    ) -> DispatchResult {
        Items::<T, I>::try_mutate(inventory_id, id, |maybe_item| {
            let Some(Item { attributes, .. }) = maybe_item else {
                Err(DispatchError::Other("UnknownItem"))?
            };
            let key = key.using_encoded(|v| {
                BoundedVec::try_from(v.to_vec())
                    .map_err(|_| DispatchError::Other("MaxKeyLenExceeded"))
            })?;
            let value = value.using_encoded(|v| {
                BoundedVec::try_from(v.to_vec())
                    .map_err(|_| DispatchError::Other("MaxValueLenExceeded"))
            })?;
            attributes
                .try_insert(key, value)
                .map_err(|_| DispatchError::Other("MaxAttributesExceeded"))?;
            Ok(())
        })
    }

    fn clear_attribute<K: Encode>(
        inventory_id: &item::InventoryIdOf<Self, T::AccountId>,
        id: &Self::ItemId,
        key: &K,
    ) -> DispatchResult {
        Items::<T, I>::try_mutate(inventory_id, id, |maybe_item| {
            let Some(Item { attributes, .. }) = maybe_item else {
                Err(DispatchError::Other("UnknownItem"))?
            };
            let key = key.using_encoded(|v| {
                BoundedVec::try_from(v.to_vec())
                    .map_err(|_| DispatchError::Other("MaxKeyLenExceeded"))
            })?;
            attributes.remove(&key);
            Ok(())
        })
    }
}
