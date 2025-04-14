use super::*;
use crate::Pallet;
use frame_benchmarking::v2::*;
use frame_contrib_traits::listings::InventoryLifecycle;
use frame_support::traits::fungible::{Inspect, Mutate as FungibleMutate};
use frame_support::traits::fungibles::{Create, Mutate};

fn assert_has_event<T: Config<I>, I: 'static>(generic_event: <T as Config<I>>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_has_event(generic_event.into());
}

fn prepare_named_account<T: Config<I>, I: 'static>(name: &'static str) -> AccountIdOf<T> {
    let who = account(name, 0, 0);
    prepare_account::<T, I>(&who);
    who
}

fn prepare_account<T: Config<I>, I: 'static>(who: &AccountIdOf<T>) {
    <T::BenchmarkHelper as BenchmarkHelper<T, I>>::Balances::set_balance(
        &who,
        <T::BenchmarkHelper as BenchmarkHelper<T, I>>::Balances::minimum_balance(),
    );
}

fn setup<T: Config<I>, I: 'static>(
    carts: u32,
    amount_items: u32,
    caller: T::RuntimeOrigin,
) -> Result<
    (
        PaymentAssetIdOf<T, I>,
        T::OrderId,
        Vec<(InventoryIdOf<T, I>, ItemIdOf<T, I>, Option<T::AccountId>)>,
    ),
    DispatchError,
>
where
    PaymentAssetIdOf<T, I>: Default,
    InventoryIdOf<T, I>: From<(
        MerchantIdOf<T::BenchmarkHelper, T, I>,
        InternalInventoryIdOf<T::BenchmarkHelper, T, I>,
    )>,
{
    let asset_id = prepare_asset::<T, I>()?;
    let items = prepare_items::<T, I>(&asset_id, (0..amount_items).map(|_| 10u32.into()))?
        .into_iter()
        .map(|(inventory_id, id)| (inventory_id, id, None))
        .collect::<Vec<_>>();

    for _ in 1..carts {
        Pallet::<T, I>::create_cart(caller.clone(), None)?;
    }

    let order_id = NextOrderId::<T, I>::get();
    Pallet::<T, I>::create_cart(caller.clone(), None)?;

    Ok((asset_id, order_id, items))
}

fn prepare_asset<T: Config<I>, I: 'static>() -> Result<PaymentAssetIdOf<T, I>, DispatchError>
where
    PaymentAssetIdOf<T, I>: Default,
{
    let admin = prepare_named_account::<T, I>("asset_admin");
    let asset_id: PaymentAssetIdOf<T, I> = Default::default();
    <T::BenchmarkHelper as BenchmarkHelper<T, I>>::Assets::create(
        asset_id.clone(),
        admin,
        false,
        1u32.into(),
    )?;

    Ok(asset_id)
}

fn prepare_asset_account<T: Config<I>, I: 'static>(
    id: PaymentAssetIdOf<T, I>,
    who: &AccountIdOf<T>,
    amount: PaymentBalanceOf<T, I>,
) -> DispatchResult {
    <T::BenchmarkHelper as BenchmarkHelper<T, I>>::Assets::mint_into(id, who, amount)?;
    Ok(())
}

fn prepare_items<T: Config<I>, I: 'static>(
    asset_id: &PaymentAssetIdOf<T, I>,
    prices: impl Iterator<Item = PaymentBalanceOf<T, I>>,
) -> Result<Vec<(InventoryIdOf<T, I>, ItemIdOf<T, I>)>, DispatchError>
where
    InventoryIdOf<T, I>: From<(
        MerchantIdOf<T::BenchmarkHelper, T, I>,
        InternalInventoryIdOf<T::BenchmarkHelper, T, I>,
    )>,
    PaymentAssetIdOf<T, I>: Default,
{
    let owner = &prepare_named_account::<T, I>("merchant_owner");
    let (merchant_id, id) = T::BenchmarkHelper::inventory_id();

    T::Listings::create(&merchant_id, &id, owner)?;

    let inventory_id = (merchant_id, id).into();

    prices
        .into_iter()
        .enumerate()
        .map(|(i, amount)| {
            let item_id = T::BenchmarkHelper::item_id(i);
            T::Listings::publish(
                &inventory_id,
                &item_id,
                b"".to_vec(),
                Some(ItemPrice {
                    asset: asset_id.clone(),
                    amount,
                }),
            )?;

            Ok((inventory_id.clone(), item_id))
        })
        .collect::<Result<Vec<_>, _>>()
}

#[instance_benchmarks(
where
    PaymentAssetIdOf<T, I>: Default,
	InventoryIdOf<T, I>: From<(MerchantIdOf<T::BenchmarkHelper, T, I>, InternalInventoryIdOf<T::BenchmarkHelper, T, I>)>,
)]
mod benchmarks {
    use super::*;
    use sp_runtime::Saturating;

    #[benchmark]
    pub fn create_cart(
        p: Linear<1, { T::MaxCartLen::get() }>,
        q: Linear<1, { T::MaxItemLen::get() }>,
    ) -> Result<(), BenchmarkError> {
        // Setup code
        let caller =
            T::CreateOrigin::try_successful_origin().map_err(|_| DispatchError::BadOrigin)?;
        let asset_id = &prepare_asset::<T, I>()?;
        let items = prepare_items::<T, I>(asset_id, (0..q).map(|_| 10u32.into()))?
            .into_iter()
            .map(|(inventory_id, id)| (inventory_id, id, None))
            .collect::<Vec<_>>();

        let (owner, max_carts) =
            T::CreateOrigin::ensure_origin(caller.clone()).map_err(|_| DispatchError::BadOrigin)?;

        for _ in 1..p.max(max_carts) {
            Pallet::<T, I>::create_cart(caller.clone(), None)?;
        }

        let order_id = NextOrderId::<T, I>::get();

        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, Some(items));

        // Verification code
        assert_has_event::<T, I>(Event::<T, I>::CartCreated { owner, order_id }.into());
        Ok(())
    }

    #[benchmark]
    pub fn set_cart_items(q: Linear<1, { T::MaxItemLen::get() }>) -> Result<(), BenchmarkError> {
        // Setup code
        let caller =
            T::CreateOrigin::try_successful_origin().map_err(|_| DispatchError::BadOrigin)?;
        let (_, max_carts) =
            T::CreateOrigin::ensure_origin(caller.clone()).map_err(|_| DispatchError::BadOrigin)?;
        let (_, order_id, items) = setup::<T, I>(max_carts, q, caller.clone())?;

        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, order_id, items);

        // Verification code
        Ok(())
    }

    #[benchmark]
    pub fn checkout() -> Result<(), BenchmarkError> {
        // Setup code
        let caller =
            T::CreateOrigin::try_successful_origin().map_err(|_| DispatchError::BadOrigin)?;
        let (_, max_carts) =
            T::CreateOrigin::ensure_origin(caller.clone()).map_err(|_| DispatchError::BadOrigin)?;
        let (_, max_items) = T::OrderAdminOrigin::ensure_origin(caller.clone())
            .map_err(|_| DispatchError::BadOrigin)?;

        let (_, order_id, items) = setup::<T, I>(max_carts, max_items, caller.clone())?;
        Pallet::<T, I>::set_cart_items(caller.clone(), order_id.clone(), items)?;

        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, order_id.clone());

        // Verification code
        assert_has_event::<T, I>(Event::<T, I>::OrderCheckout { order_id }.into());
        Ok(())
    }

    #[benchmark]
    pub fn cancel() -> Result<(), BenchmarkError> {
        // Setup code
        let caller =
            T::CreateOrigin::try_successful_origin().map_err(|_| DispatchError::BadOrigin)?;
        let (_, max_carts) =
            T::CreateOrigin::ensure_origin(caller.clone()).map_err(|_| DispatchError::BadOrigin)?;
        let (_, max_items) = T::OrderAdminOrigin::ensure_origin(caller.clone())
            .map_err(|_| DispatchError::BadOrigin)?;

        let (_, order_id, items) = setup::<T, I>(max_carts, max_items, caller.clone())?;
        Pallet::<T, I>::set_cart_items(caller.clone(), order_id.clone(), items)?;
        Pallet::<T, I>::checkout(caller.clone(), order_id.clone())?;

        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, order_id.clone());

        // Verification code
        assert_has_event::<T, I>(Event::<T, I>::OrderCancelled { order_id }.into());
        Ok(())
    }

    #[benchmark]
    pub fn pay() -> Result<(), BenchmarkError> {
        // Setup code
        let caller =
            T::CreateOrigin::try_successful_origin().map_err(|_| DispatchError::BadOrigin)?;
        let (caller_account, max_carts) =
            T::CreateOrigin::ensure_origin(caller.clone()).map_err(|_| DispatchError::BadOrigin)?;
        let (_, max_items) = T::OrderAdminOrigin::ensure_origin(caller.clone())
            .map_err(|_| DispatchError::BadOrigin)?;

        let (asset_id, order_id, items) = setup::<T, I>(max_carts, max_items, caller.clone())?;

        let beneficiary = prepare_named_account::<T, I>("beneficiary");

        let items = items
            .into_iter()
            .map(|(inventory_id, id, _)| (inventory_id, id, Some(beneficiary.clone())));

        let price = items.clone().fold(
            Into::<PaymentBalanceOf<T, I>>::into(1u32),
            |price, (ref inventory_id, ref id, _)| {
                let item = T::Listings::item(inventory_id, id).expect("item already created; qed");
                let ItemPrice { amount, .. } =
                    item.price.expect("prices given for every item; qed");
                price.saturating_add(amount)
            },
        );

        prepare_account::<T, I>(&caller_account);
        prepare_asset_account::<T, I>(asset_id, &caller_account, price)?;

        Pallet::<T, I>::set_cart_items(caller.clone(), order_id.clone(), items.collect())?;
        Pallet::<T, I>::checkout(caller.clone(), order_id.clone())?;

        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, order_id.clone());

        // Verification code
        assert_has_event::<T, I>(Event::<T, I>::OrderInProgress { order_id }.into());
        Ok(())
    }

    impl_benchmark_test_suite!(
        Pallet,
        sp_io::TestExternalities::default(),
        crate::mock::Test
    );
}
