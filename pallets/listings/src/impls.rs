use super::*;
use alloc::vec::Vec;

mod inventory {
    use super::*;
    use frame_support::traits::nonfungibles_v2::{Create, Inspect as _, InspectEnumerable, Mutate};
    use pallet_nfts::{CollectionConfig, CollectionSettings};

    impl<T: Config<I>, I: 'static> InspectInventory<T::MerchantId> for Pallet<T, I> {
        type Id = T::InventoryId;

        fn all() -> impl Iterator<Item = (T::MerchantId, Self::Id)> {
            NftsPallet::<T, I>::collections().map(|c| c.into())
        }

        fn owned(merchant_id: &T::MerchantId) -> impl Iterator<Item = (T::MerchantId, Self::Id)> {
            NftsPallet::<T, I>::collections()
                .map(|c| c.into())
                .filter(move |(merchant, _)| merchant == merchant_id)
        }

        fn is_active(merchant_id: &T::MerchantId, id: &Self::Id) -> bool {
            NftsPallet::<T, I>::typed_system_attribute::<InventoryAttribute, bool>(
                &(*merchant_id, *id).into(),
                None,
                &InventoryAttribute::Archived,
            )
            .is_none()
        }
    }

    impl<T: Config<I>, I: 'static> InventoryLifecycle<T::MerchantId> for Pallet<T, I> {
        type AccountId = T::AccountId;

        fn create(
            merchant_id: &T::MerchantId,
            id: &Self::Id,
            owner: &Self::AccountId,
        ) -> DispatchResult {
            NftsPallet::<T, I>::create_collection_with_id(
                (*merchant_id, *id).into(),
                owner,
                owner,
                &CollectionConfig {
                    settings: CollectionSettings::all_enabled(),
                    max_supply: None,
                    mint_settings: Default::default(),
                },
            )
        }

        fn archive(merchant_id: &T::MerchantId, id: &Self::Id) -> DispatchResult {
            NftsPallet::<T, I>::set_typed_collection_attribute(
                &(*merchant_id, *id).into(),
                &InventoryAttribute::Archived,
                &true,
            )
        }
    }
}

mod item {
    use super::*;
    use fc_traits_listings::item::{IdItemOf, Item};
    use frame_support::traits::nonfungibles_v2::{Inspect, InspectEnumerable, Mutate, Transfer};
    use pallet_nfts::{ItemConfig, ItemSettings};

    impl<T: Config<I>, I: 'static> InspectItem<AccountIdOf<T>> for Pallet<T, I> {
        type InventoryId = InventoryIdOf<T, I>;
        type Id = ItemType<T::ItemSKU>;
        type Price = ItemPriceOf<T, I>;

        fn items(
            inventory_id: &Self::InventoryId,
        ) -> impl Iterator<Item = IdItemOf<Self, AccountIdOf<T>>> {
            NftsPallet::<T, I>::items(inventory_id).map(|item| {
                (
                    (*inventory_id, item),
                    Self::item(inventory_id, &item)
                        .expect("item exists, given it's being iterated; qed"),
                )
            })
        }

        fn owned(owner: &AccountIdOf<T>) -> impl Iterator<Item = IdItemOf<Self, AccountIdOf<T>>> {
            NftsPallet::<T, I>::owned(owner).map(|(inventory_id, item)| {
                (
                    (inventory_id, item),
                    Self::item(&inventory_id, &item)
                        .expect("item exists, given it's being iterated; qed"),
                )
            })
        }

        fn item(inventory_id: &Self::InventoryId, id: &Self::Id) -> Option<ItemOf<T, I>> {
            let owner = NftsPallet::<T, I>::owner(*inventory_id, *id)?;

            Some(Item {
                name: NftsPallet::<T, I>::typed_system_attribute(
                    inventory_id,
                    Some(id),
                    &ItemAttribute::Name,
                )?,
                price: NftsPallet::<T, I>::typed_system_attribute(
                    inventory_id,
                    Some(id),
                    &ItemAttribute::Price,
                ),
                owner,
            })
        }

        fn attribute<K: Encode, V: Decode>(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            key: &K,
        ) -> Option<V> {
            NftsPallet::<T, I>::typed_system_attribute(inventory_id, Some(id), key)
        }

        fn transferable(inventory_id: &Self::InventoryId, id: &Self::Id) -> bool {
            NftsPallet::<T, I>::can_transfer(inventory_id, id)
        }

        fn can_resell(inventory_id: &Self::InventoryId, id: &Self::Id) -> bool {
            NftsPallet::<T, I>::typed_system_attribute::<ItemAttribute, ()>(
                inventory_id,
                Some(id),
                &ItemAttribute::NotForResale,
            )
            .is_none()
        }
    }

    impl<T: Config<I>, I: 'static> MutateItem<AccountIdOf<T>> for Pallet<T, I> {
        fn publish(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            name: Vec<u8>,
            maybe_price: Option<Self::Price>,
        ) -> DispatchResult {
            let inventory_owner = NftsPallet::<T, I>::collection_owner(*inventory_id)
                .ok_or(Error::<T, I>::UnknownInventory)?;

            NftsPallet::<T, I>::mint_into(
                inventory_id,
                id,
                &inventory_owner,
                &ItemConfig {
                    settings: ItemSettings::all_enabled(),
                },
                true,
            )?;
            NftsPallet::<T, I>::set_typed_attribute(inventory_id, id, &ItemAttribute::Name, &name)?;

            if let Some(price) = maybe_price {
                NftsPallet::<T, I>::set_typed_attribute(
                    inventory_id,
                    id,
                    &ItemAttribute::Price,
                    &price,
                )?;
            }

            Ok(())
        }

        fn mark_not_for_resale(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            not_for_resale: bool,
        ) -> DispatchResult {
            if not_for_resale {
                NftsPallet::<T, I>::set_typed_attribute(
                    inventory_id,
                    id,
                    &ItemAttribute::NotForResale,
                    &(),
                )
            } else {
                NftsPallet::<T, I>::clear_typed_attribute(
                    inventory_id,
                    id,
                    &ItemAttribute::NotForResale,
                )
            }
        }

        fn mark_can_transfer(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            can_transfer: bool,
        ) -> DispatchResult {
            if can_transfer {
                NftsPallet::<T, I>::enable_transfer(inventory_id, id)
            } else {
                NftsPallet::<T, I>::disable_transfer(inventory_id, id)
            }
        }

        fn set_price(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            price: Self::Price,
        ) -> DispatchResult {
            NftsPallet::<T, I>::set_typed_attribute(inventory_id, id, &ItemAttribute::Price, &price)
        }

        fn set_attribute<K: Encode, V: Encode>(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            key: &K,
            value: V,
        ) -> DispatchResult {
            NftsPallet::<T, I>::set_typed_attribute(inventory_id, id, key, &value)
        }

        fn clear_attribute<K: Encode>(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            key: &K,
        ) -> DispatchResult {
            NftsPallet::<T, I>::clear_typed_attribute(inventory_id, id, key)
        }
    }
}
