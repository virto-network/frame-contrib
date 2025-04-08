use super::*;
use alloc::vec::Vec;

mod inventory {
    use super::*;
    use nonfungibles_v2::{Create, InspectEnumerable, Mutate};
    use pallet_nfts::{CollectionConfig, CollectionSettings};

    impl<T: Config<I>, I: 'static> InspectInventory<T::MerchantId> for Pallet<T, I> {
        type Id = T::InventoryId;

        fn all() -> impl Iterator<Item = (T::MerchantId, Self::Id)> {
            T::Nonfungibles::collections().map(|c| c.into())
        }

        fn owned(merchant_id: &T::MerchantId) -> impl Iterator<Item = (T::MerchantId, Self::Id)> {
            T::Nonfungibles::collections()
                .map(|c| c.into())
                .filter(move |(merchant, _)| merchant == merchant_id)
        }

        fn is_active(merchant_id: &T::MerchantId, id: &Self::Id) -> bool {
            T::Nonfungibles::typed_system_attribute::<InventoryAttribute, bool>(
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
            T::Nonfungibles::create_collection_with_id(
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
            T::Nonfungibles::set_typed_collection_attribute(
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
    use nonfungibles_v2::{InspectEnumerable, Mutate, Transfer};
    use pallet_nfts::{ItemConfig, ItemSettings};

    impl<T: Config<I>, I: 'static> InspectItem<AccountIdOf<T>> for Pallet<T, I> {
        type InventoryId = InventoryIdOf<T, I>;
        type Id = ItemType<T::ItemSKU>;
        type Asset = AssetIdOf<T, I>;
        type Balance = AssetBalanceOf<T, I>;

        fn items(
            inventory_id: &Self::InventoryId,
        ) -> impl Iterator<Item = IdItemOf<Self, AccountIdOf<T>>> {
            T::Nonfungibles::items(inventory_id).map(|item| {
                (
                    (*inventory_id, item),
                    Self::item(inventory_id, &item)
                        .expect("item exists, given it's being iterated; qed"),
                )
            })
        }

        fn owned(owner: &AccountIdOf<T>) -> impl Iterator<Item = IdItemOf<Self, AccountIdOf<T>>> {
            T::Nonfungibles::owned(owner).map(|(inventory_id, item)| {
                (
                    (inventory_id, item),
                    Self::item(&inventory_id, &item)
                        .expect("item exists, given it's being iterated; qed"),
                )
            })
        }

        fn item(inventory_id: &Self::InventoryId, id: &Self::Id) -> Option<ItemOf<T, I>> {
            let owner = T::Nonfungibles::owner(inventory_id, id)?;
            let (name, price): ItemInfo<Vec<u8>, ItemPriceOf<T, I>> =
                T::Nonfungibles::typed_system_attribute(
                    inventory_id,
                    Some(id),
                    &ItemAttribute::Info,
                )?;

            Some(Item { name, price, owner })
        }

        fn attribute<K: Encode, V: Decode>(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            key: &K,
        ) -> Option<V> {
            T::Nonfungibles::typed_system_attribute(inventory_id, Some(id), key)
        }

        fn transferable(inventory_id: &Self::InventoryId, id: &Self::Id) -> bool {
            T::Nonfungibles::can_transfer(inventory_id, id)
        }

        fn can_resell(inventory_id: &Self::InventoryId, id: &Self::Id) -> bool {
            T::Nonfungibles::typed_system_attribute::<ItemAttribute, ()>(
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
            maybe_price: Option<ItemPriceOf<T, I>>,
        ) -> DispatchResult {
            let inventory_owner = T::Nonfungibles::collection_owner(inventory_id)
                .ok_or(Error::<T, I>::UnknownInventory)?;

            T::Nonfungibles::mint_into(
                inventory_id,
                id,
                &inventory_owner,
                &ItemConfig {
                    settings: ItemSettings::all_enabled(),
                },
                true,
            )?;
            T::Nonfungibles::set_typed_attribute(
                inventory_id,
                id,
                &ItemAttribute::Info,
                &(name, maybe_price),
            )?;
            Ok(())
        }

        fn enable_resell(inventory_id: &Self::InventoryId, id: &Self::Id) -> DispatchResult {
            let not_for_resale = T::Nonfungibles::typed_system_attribute::<ItemAttribute, ()>(
                inventory_id,
                Some(id),
                &ItemAttribute::NotForResale,
            );

            if not_for_resale.is_some() {
                T::Nonfungibles::clear_typed_attribute(
                    inventory_id,
                    id,
                    &ItemAttribute::NotForResale,
                )?;
            }

            Ok(())
        }

        fn disable_resell(inventory_id: &Self::InventoryId, id: &Self::Id) -> DispatchResult {
            T::Nonfungibles::set_typed_attribute(
                inventory_id,
                id,
                &ItemAttribute::NotForResale,
                &(),
            )
        }

        fn mark_can_transfer(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            can_transfer: bool,
        ) -> DispatchResult {
            if can_transfer {
                T::Nonfungibles::enable_transfer(inventory_id, id)
            } else {
                T::Nonfungibles::disable_transfer(inventory_id, id)
            }
        }

        fn transfer(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            beneficiary: &AccountIdOf<T>,
        ) -> DispatchResult {
            if !T::Nonfungibles::can_transfer(inventory_id, id) {
                T::Nonfungibles::enable_transfer(inventory_id, id)?;
                T::Nonfungibles::transfer(inventory_id, id, beneficiary)?;
                return T::Nonfungibles::disable_transfer(inventory_id, id);
            }

            T::Nonfungibles::transfer(inventory_id, id, beneficiary)
        }

        fn set_price(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            price: ItemPriceOf<T, I>,
        ) -> DispatchResult {
            let (name, _): ItemInfo<Vec<u8>, ItemPriceOf<T, I>> =
                T::Nonfungibles::typed_system_attribute(
                    inventory_id,
                    Some(id),
                    &ItemAttribute::Info,
                )
                .ok_or(Error::<T, I>::UnknownItem)?;

            T::Nonfungibles::set_typed_attribute(
                inventory_id,
                id,
                &ItemAttribute::Info,
                &(name, Some(price)),
            )
        }

        fn set_attribute<K: Encode, V: Encode>(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            key: &K,
            value: V,
        ) -> DispatchResult {
            T::Nonfungibles::set_typed_attribute(inventory_id, id, key, &value)
        }

        fn clear_attribute<K: Encode>(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            key: &K,
        ) -> DispatchResult {
            T::Nonfungibles::clear_typed_attribute(inventory_id, id, key)
        }
    }
}
