use super::*;

use crate::Pallet;
use alloc::vec;
use frame_benchmarking::v2::*;
use frame_support::traits::{fungible::Unbalanced, tokens::Precision};
use sp_runtime::traits::Bounded;

fn assert_has_event<T: Config<I>, I: 'static>(generic_event: <T as Config<I>>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_has_event(generic_event.into())
}

type InventoryInfoOf<T, I> = (OriginFor<T>, InventoryIdFor<T, I>, AccountIdOf<T>);
type ItemSetupFor<T, I> = (OriginFor<T>, InventoryIdFor<T, I>, ItemIdOf<T, I>);

fn inventory_info<T: Config<I>, I: 'static>() -> Result<InventoryInfoOf<T, I>, DispatchError> {
    let inventory_id = T::BenchmarkHelper::inventory_id();
    let origin = T::CreateInventoryOrigin::try_successful_origin(&inventory_id)
        .map_err(|_| DispatchError::BadOrigin)?;

    let owner = T::CreateInventoryOrigin::ensure_origin(origin.clone(), &inventory_id)
        .map_err(|_| DispatchError::BadOrigin)?;

    let max = NativeBalanceOf::<T, I>::max_value();
    T::Balances::increase_balance(&owner, max, Precision::Exact)?;

    Ok((origin, inventory_id, owner))
}

fn setup_inventory<T: Config<I>, I: 'static>(
) -> Result<(OriginFor<T>, InventoryIdFor<T, I>), DispatchError> {
    let (origin, inventory_id, _) = inventory_info::<T, I>()?;
    Pallet::<T, I>::create_inventory(origin.clone(), inventory_id)?;
    Ok((origin, inventory_id))
}

fn setup_item<T: Config<I>, I: 'static>() -> Result<ItemSetupFor<T, I>, DispatchError>
where
    ItemIdOf<T, I>: Default,
{
    let (origin, inventory_id) = setup_inventory::<T, I>()?;
    let item_id = Default::default();

    Pallet::<T, I>::publish_item(
        origin.clone(),
        inventory_id,
        item_id,
        BoundedVec::truncate_from(b"".to_vec()),
        None,
    )?;

    Ok((origin, inventory_id, item_id))
}

#[instance_benchmarks(
where
    AssetIdOf<T, I>: Default,
    ItemIdOf<T, I>: Default,
)]
mod benchmarks {
    use super::*;

    #[benchmark]
    pub fn create_inventory() -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id, owner) = inventory_info::<T, I>()?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id);

        // Verification code
        let InventoryId(merchant, id) = inventory_id;
        assert_has_event::<T, I>(
            Event::<T, I>::InventoryCreated {
                merchant,
                id,
                owner,
            }
            .into(),
        );

        Ok(())
    }

    #[benchmark]
    pub fn archive_inventory() -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id) = setup_inventory::<T, I>()?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id);

        // Verification code
        let InventoryId(merchant, id) = inventory_id;
        assert_has_event::<T, I>(Event::<T, I>::InventoryArchived { merchant, id }.into());

        Ok(())
    }

    #[benchmark]
    pub fn publish_item(
        q: Linear<
            1,
            {
                T::NonfungiblesValueLimit::get()
                    - <Option<ItemPriceOf<T, I>> as MaxEncodedLen>::max_encoded_len() as u32
                    - <codec::Compact<u32> as MaxEncodedLen>::max_encoded_len() as u32
            },
        >,
    ) -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id) = setup_inventory::<T, I>()?;
        let id = Default::default();
        let name = BoundedVec::truncate_from(vec![0u8; q as usize]);
        let price = ItemPrice {
            asset: Default::default(),
            amount: 1u32.into(),
        };

        #[extrinsic_call]
        _(
            origin as T::RuntimeOrigin,
            inventory_id,
            id,
            name,
            Some(price.clone()),
        );

        // Verification code
        assert_has_event::<T, I>(Event::<T, I>::ItemPublished { inventory_id, id }.into());
        assert_has_event::<T, I>(
            Event::<T, I>::ItemPriceSet {
                inventory_id,
                id,
                price,
            }
            .into(),
        );

        Ok(())
    }

    #[benchmark]
    pub fn mark_item_can_transfer() -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id, id) = setup_item::<T, I>()?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id, id, false);

        // Verification code
        assert!(!Pallet::<T, I>::transferable(&inventory_id.into(), &id));

        Ok(())
    }

    #[benchmark]
    pub fn mark_item_not_for_resale() -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id, id) = setup_item::<T, I>()?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id, id, true);

        // Verification code
        assert!(!Pallet::<T, I>::can_resell(&inventory_id.into(), &id));

        Ok(())
    }

    #[benchmark]
    pub fn set_item_price() -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id, id) = setup_item::<T, I>()?;
        let price = ItemPrice {
            asset: Default::default(),
            amount: 10u32.into(),
        };

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, inventory_id, id, price.clone());

        // Verification code
        assert_has_event::<T, I>(
            Event::<T, I>::ItemPriceSet {
                inventory_id,
                id,
                price,
            }
            .into(),
        );

        Ok(())
    }

    #[benchmark]
    pub fn set_item_attribute(
        p: Linear<
            1,
            {
                T::NonfungiblesKeyLimit::get()
                    - <codec::Compact<u32> as MaxEncodedLen>::max_encoded_len() as u32
            },
        >,
        q: Linear<
            1,
            {
                T::NonfungiblesValueLimit::get()
                    - <codec::Compact<u32> as MaxEncodedLen>::max_encoded_len() as u32
            },
        >,
    ) -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id, id) = setup_item::<T, I>()?;

        let key = BoundedVec::truncate_from(vec![0u8; p as usize]);
        let value = BoundedVec::truncate_from(vec![0u8; q as usize]);

        #[extrinsic_call]
        _(
            origin as T::RuntimeOrigin,
            inventory_id,
            id,
            key.clone(),
            Some(value.clone()),
        );

        // Verification code
        assert_eq!(
            Pallet::<T, I>::attribute(&inventory_id.into(), &id, &key),
            Some(value)
        );

        Ok(())
    }

    #[benchmark]
    pub fn clear_item_attribute(
        p: Linear<
            1,
            {
                T::NonfungiblesKeyLimit::get()
                    - <codec::Compact<u32> as MaxEncodedLen>::max_encoded_len() as u32
            },
        >,
        q: Linear<
            1,
            {
                T::NonfungiblesValueLimit::get()
                    - <codec::Compact<u32> as MaxEncodedLen>::max_encoded_len() as u32
            },
        >,
    ) -> Result<(), BenchmarkError> {
        // Setup code
        let (origin, inventory_id, id) = setup_item::<T, I>()?;

        let key = BoundedVec::truncate_from(vec![0u8; p as usize]);
        let value = BoundedVec::truncate_from(vec![0u8; q as usize]);

        Pallet::<T, I>::set_item_attribute(
            origin.clone(),
            inventory_id,
            id,
            key.clone(),
            Some(value),
        )?;

        #[extrinsic_call]
        set_item_attribute(
            origin as T::RuntimeOrigin,
            inventory_id,
            id,
            key.clone(),
            None,
        );

        // Verification code
        assert_eq!(
            Pallet::<T, I>::attribute(&inventory_id.into(), &id, &key),
            None::<Vec<u8>>
        );

        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), mock::Test);
}
