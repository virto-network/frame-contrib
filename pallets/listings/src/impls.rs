use super::*;
use alloc::vec::Vec;

type InventoryIdTuple<T, I> = (<T as Config<I>>::MerchantId, <T as Config<I>>::InventoryId);

mod inventory {
    use super::*;
    use nonfungibles_v2::{Create, InspectEnumerable, Mutate};

    impl<T: Config<I>, I: 'static> InspectInventory for Pallet<T, I> {
        type MerchantId = T::MerchantId;
        type InventoryId = T::InventoryId;

        fn is_active(id: &InventoryIdTuple<T, I>) -> bool {
            T::Nonfungibles::typed_system_attribute::<InventoryAttribute, bool>(
                &(*id).into(),
                None,
                &InventoryAttribute::Archived,
            )
            .is_none()
        }

        fn exists(id: &InventoryIdTuple<T, I>) -> bool {
            T::Nonfungibles::collection_owner(&(*id).into()).is_some()
        }

        fn inventory_attribute<K: Encode, V: Decode>(
            id: &InventoryIdTuple<T, I>,
            key: &K,
        ) -> Option<V> {
            T::Nonfungibles::typed_system_attribute(&(*id).into(), None, &key)
        }
    }

    impl<T: Config<I>, I: 'static> InventoryInspectEnumerable for Pallet<T, I> {
        fn all() -> impl Iterator<Item = impl Into<InventoryIdTuple<T, I>>> {
            T::Nonfungibles::collections()
        }

        fn owned(
            merchant_id: &T::MerchantId,
        ) -> impl Iterator<Item = impl Into<InventoryIdTuple<T, I>>> {
            T::Nonfungibles::collections()
                .map(|c| c.into())
                .filter(move |(merchant, _)| merchant == merchant_id)
        }
    }

    impl<T: Config<I>, I: 'static> InventoryLifecycle<T::AccountId> for Pallet<T, I> {
        fn create(id: InventoryIdTuple<T, I>, owner: &T::AccountId) -> DispatchResult {
            T::Nonfungibles::create_collection_with_id(id.into(), owner, owner, &Default::default())
        }

        fn archive(id: &InventoryIdTuple<T, I>) -> DispatchResult {
            T::Nonfungibles::set_typed_collection_attribute(
                &(*id).into(),
                &InventoryAttribute::Archived,
                &true,
            )
        }
    }

    impl<T: Config<I>, I: 'static> MutateInventory for Pallet<T, I> {
        fn set_items_limit(_id: &InventoryIdTuple<T, I>, _limit: Option<usize>) -> DispatchResult {
            // TODO: Either find out a way to mutate the items limit on Nonfungibles, or limit it via this pallet.
            Ok(())
        }

        fn set_inventory_metadata<M: Encode>(
            id: &InventoryIdTuple<T, I>,
            metadata: M,
        ) -> DispatchResult {
            T::Nonfungibles::set_collection_metadata(None, &id.into(), metadata.encode().as_ref())
        }

        fn clear_inventory_metadata(id: &InventoryIdTuple<T, I>) -> DispatchResult {
            T::Nonfungibles::clear_collection_metadata(None, &id.into())
        }

        fn set_inventory_attribute<K: Encode, V: Encode>(
            id: &InventoryIdTuple<T, I>,
            key: &K,
            value: &V,
        ) -> DispatchResult {
            T::Nonfungibles::set_typed_collection_attribute(&(*id).into(), key, value)
        }

        fn clear_inventory_attribute<K: Encode>(
            id: &InventoryIdTuple<T, I>,
            key: &K,
        ) -> DispatchResult {
            T::Nonfungibles::clear_typed_collection_attribute(&(*id).into(), key).or(Ok(()))
        }
    }
}

mod item {
    use super::*;
    use fc_traits_listings::item::{Item, ItemOf};
    use nonfungibles_v2::{InspectEnumerable, Mutate, Transfer};

    impl<T: Config<I>, I: 'static> InspectItem<AccountIdOf<T>> for Pallet<T, I> {
        type MerchantId = T::MerchantId;
        type InventoryId = T::InventoryId;
        type ItemId = T::ItemSKU;
        type Asset = AssetIdOf<T, I>;
        type Balance = AssetBalanceOf<T, I>;

        fn item(
            inventory_id: &InventoryIdTuple<T, I>,
            id: &Self::ItemId,
        ) -> Option<ItemOf<Self, AccountIdOf<T>>> {
            let owner = T::Nonfungibles::owner(&inventory_id.into(), id)?;
            let (name, price): ItemInfo<Vec<u8>, ItemPriceOf<T, I>> =
                T::Nonfungibles::typed_system_attribute(
                    &inventory_id.into(),
                    Some(id),
                    &ItemAttribute::Info,
                )?;

            Some(Item { name, owner, price })
        }

        fn creator(
            inventory_id: &InventoryIdTuple<T, I>,
            id: &Self::ItemId,
        ) -> Option<AccountIdOf<T>> {
            T::Nonfungibles::typed_system_attribute(
                &inventory_id.into(),
                Some(id),
                &ItemAttribute::Creator,
            )
        }

        fn attribute<K: Encode, V: Decode>(
            inventory_id: &InventoryIdTuple<T, I>,
            id: &Self::ItemId,
            key: &K,
        ) -> Option<V> {
            T::Nonfungibles::typed_system_attribute(&inventory_id.into(), Some(id), key)
        }

        fn transferable(inventory_id: &InventoryIdTuple<T, I>, id: &Self::ItemId) -> bool {
            T::Nonfungibles::can_transfer(&inventory_id.into(), id)
        }

        fn can_resell(inventory_id: &InventoryIdTuple<T, I>, id: &Self::ItemId) -> bool {
            T::Nonfungibles::typed_system_attribute::<ItemAttribute, ()>(
                &inventory_id.into(),
                Some(id),
                &ItemAttribute::NotForResale,
            )
            .is_none()
        }
    }

    impl<T: Config<I>, I: 'static> ItemInspectEnumerable<AccountIdOf<T>> for Pallet<T, I> {
        fn items(
            inventory_id: &InventoryIdTuple<T, I>,
        ) -> impl Iterator<Item = (Self::ItemId, ItemOf<Self, AccountIdOf<T>>)> {
            T::Nonfungibles::items(&inventory_id.into()).map(move |item_id| {
                (
                    item_id,
                    Self::item(inventory_id, &item_id)
                        .expect("item exists, given it's being iterated; qed"),
                )
            })
        }

        fn owned(
            owner: &AccountIdOf<T>,
        ) -> impl Iterator<
            Item = (
                impl Into<InventoryIdTuple<T, I>>,
                Self::ItemId,
                ItemOf<Self, AccountIdOf<T>>,
            ),
        > {
            T::Nonfungibles::owned(owner).map(|(inventory_id, item)| {
                let inventory_id: InventoryIdTuple<T, I> = inventory_id.into();

                (
                    inventory_id,
                    item,
                    Self::item(&inventory_id, &item)
                        .expect("item exists, given it's being iterated; qed"),
                )
            })
        }
    }

    impl<T: Config<I>, I: 'static> MutateItem<AccountIdOf<T>> for Pallet<T, I> {
        fn publish(
            inventory_id: &InventoryIdTuple<T, I>,
            id: &Self::ItemId,
            name: Vec<u8>,
            maybe_price: Option<ItemPriceOf<T, I>>,
        ) -> DispatchResult {
            let inventory_id: &InventoryIdFor<T, I> = &inventory_id.into();
            let inventory_owner = T::Nonfungibles::collection_owner(inventory_id)
                .ok_or(Error::<T, I>::UnknownInventory)?;

            T::Nonfungibles::mint_into(
                inventory_id,
                id,
                &inventory_owner.clone(),
                &Default::default(),
                true,
            )?;
            T::Nonfungibles::set_typed_attribute(
                inventory_id,
                id,
                &ItemAttribute::Creator,
                &inventory_owner,
            )?;
            T::Nonfungibles::set_typed_attribute(
                inventory_id,
                id,
                &ItemAttribute::Info,
                &(name, maybe_price),
            )?;
            Ok(())
        }

        fn enable_resell(
            inventory_id: &InventoryIdTuple<T, I>,
            id: &Self::ItemId,
        ) -> DispatchResult {
            let not_for_resale = T::Nonfungibles::typed_system_attribute::<ItemAttribute, ()>(
                &inventory_id.into(),
                Some(id),
                &ItemAttribute::NotForResale,
            );

            if not_for_resale.is_some() {
                T::Nonfungibles::clear_typed_attribute(
                    &inventory_id.into(),
                    id,
                    &ItemAttribute::NotForResale,
                )?;
            }

            Ok(())
        }

        fn disable_resell(
            inventory_id: &InventoryIdTuple<T, I>,
            id: &Self::ItemId,
        ) -> DispatchResult {
            T::Nonfungibles::set_typed_attribute(
                &inventory_id.into(),
                id,
                &ItemAttribute::NotForResale,
                &(),
            )
        }

        fn enable_transfer(
            inventory_id: &InventoryIdTuple<T, I>,
            id: &Self::ItemId,
        ) -> DispatchResult {
            let inventory_id: &InventoryIdFor<T, I> = &inventory_id.into();
            if !T::Nonfungibles::can_transfer(inventory_id, id) {
                return T::Nonfungibles::enable_transfer(inventory_id, id);
            }

            Ok(())
        }

        fn disable_transfer(
            inventory_id: &InventoryIdTuple<T, I>,
            id: &Self::ItemId,
        ) -> DispatchResult {
            T::Nonfungibles::disable_transfer(&inventory_id.into(), id)
        }

        fn transfer(
            inventory_id: &InventoryIdTuple<T, I>,
            id: &Self::ItemId,
            beneficiary: &AccountIdOf<T>,
        ) -> DispatchResult {
            let inventory_id: &InventoryIdFor<T, I> = &inventory_id.into();
            if !T::Nonfungibles::can_transfer(inventory_id, id) {
                T::Nonfungibles::enable_transfer(inventory_id, id)?;
                T::Nonfungibles::transfer(inventory_id, id, beneficiary)?;
                return T::Nonfungibles::disable_transfer(inventory_id, id);
            }

            T::Nonfungibles::transfer(inventory_id, id, beneficiary)
        }

        fn creator_transfer(
            inventory_id: &fc_traits_listings::item::InventoryIdOf<Self, AccountIdOf<T>>,
            id: &Self::ItemId,
            beneficiary: &AccountIdOf<T>,
        ) -> DispatchResult {
            Self::transfer(inventory_id, id, beneficiary)?;
            Self::set_attribute(inventory_id, id, &ItemAttribute::Creator, beneficiary)
        }

        fn set_price(
            inventory_id: &InventoryIdTuple<T, I>,
            id: &Self::ItemId,
            price: ItemPriceOf<T, I>,
        ) -> DispatchResult {
            let inventory_id: &InventoryIdFor<T, I> = &inventory_id.into();
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

        fn clear_price(
            inventory_id: &fc_traits_listings::item::InventoryIdOf<Self, AccountIdOf<T>>,
            id: &Self::ItemId,
        ) -> DispatchResult {
            let inventory_id: &InventoryIdFor<T, I> = &inventory_id.into();
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
                &(name, None::<ItemPriceOf<T, I>>),
            )
        }

        fn set_metadata<M: Encode>(
            inventory_id: &fc_traits_listings::item::InventoryIdOf<Self, AccountIdOf<T>>,
            id: &Self::ItemId,
            metadata: M,
        ) -> DispatchResult {
            T::Nonfungibles::set_item_metadata(
                None,
                &inventory_id.into(),
                id,
                metadata.encode().as_ref(),
            )
        }

        fn clear_metadata(
            inventory_id: &fc_traits_listings::item::InventoryIdOf<Self, AccountIdOf<T>>,
            id: &Self::ItemId,
        ) -> DispatchResult {
            T::Nonfungibles::clear_item_metadata(None, &inventory_id.into(), id)
        }

        fn set_attribute<K: Encode, V: Encode>(
            inventory_id: &InventoryIdTuple<T, I>,
            id: &Self::ItemId,
            key: &K,
            value: V,
        ) -> DispatchResult {
            T::Nonfungibles::set_typed_attribute(&inventory_id.into(), id, key, &value)
        }

        fn clear_attribute<K: Encode>(
            inventory_id: &InventoryIdTuple<T, I>,
            id: &Self::ItemId,
            key: &K,
        ) -> DispatchResult {
            T::Nonfungibles::clear_typed_attribute(&inventory_id.into(), id, key)
        }
    }
}
