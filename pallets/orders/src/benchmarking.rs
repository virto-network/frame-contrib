use super::*;

use crate::Pallet;
use frame_benchmarking::v2::*;
use frame_contrib_traits::listings::InventoryLifecycle;
use frame_support::traits::fungible::{Inspect, Mutate as FungibleMutate};
use frame_support::traits::fungibles::{Create, Mutate};
use frame_support::traits::DefensiveSaturating;

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
        who,
        <T::BenchmarkHelper as BenchmarkHelper<T, I>>::Balances::minimum_balance(),
    );
}

type SetupFor<T, I> = (
    PaymentAssetIdOf<T, I>,
    <T as Config<I>>::OrderId,
    Vec<CartItemParameterOf<T, I>>,
);

fn setup<T: Config<I>, I: 'static>(
    carts: u32,
    amount_items: u32,
    caller: T::RuntimeOrigin,
) -> Result<SetupFor<T, I>, DispatchError>
where
    PaymentAssetIdOf<T, I>: Default,
{
    let asset_id = prepare_asset::<T, I>()?;
    let items = prepare_items::<T, I>(&asset_id, (0..amount_items).map(|_| 10u32.into()))?
        .into_iter()
        .map(|full_id| (full_id, None))
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

fn prepare_owner<T: Config<I>, I: 'static>(
    owner: &AccountIdOf<T>,
    amount: usize,
) -> DispatchResult {
    let amount = <T::BenchmarkHelper as BenchmarkHelper<T, I>>::InventoryDeposit::get()
        .defensive_saturating_add(
            <T::BenchmarkHelper as BenchmarkHelper<T, I>>::ItemDeposit::get()
                .defensive_saturating_mul((amount as u32).into()),
        );
    <T::BenchmarkHelper as BenchmarkHelper<T, I>>::Balances::mint_into(owner, amount.into())?;
    Ok(())
}

fn prepare_items<T: Config<I>, I: 'static>(
    asset_id: &PaymentAssetIdOf<T, I>,
    prices: impl Iterator<Item = PaymentBalanceOf<T, I>>,
) -> Result<Vec<ItemFullIdOf<T, I>>, DispatchError>
where
    PaymentAssetIdOf<T, I>: Default,
{
    let prices = prices.into_iter().enumerate().collect::<Vec<_>>();

    let owner = &prepare_named_account::<T, I>("merchant_owner");
    prepare_owner::<T, I>(owner, prices.len())?;
    let inventory_id = T::BenchmarkHelper::inventory_id();

    T::Listings::create(inventory_id, owner)?;

    prices
        .into_iter()
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

            Ok((inventory_id, item_id))
        })
        .collect::<Result<Vec<_>, _>>()
}

#[instance_benchmarks(
where
    PaymentAssetIdOf<T, I>: Default,
    crate::types::BalanceOf<T, I>: From<u32>,
)]
mod benchmarks {
    use super::*;
    use frame_support::traits::fungibles::Inspect;
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
            .map(|full_id| (full_id, None))
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
        let seller_account = account("merchant_owner", 0, 0);

        let (asset_id, order_id, items) = setup::<T, I>(max_carts, max_items, caller.clone())?;

        let beneficiary = prepare_named_account::<T, I>("beneficiary");

        let items = items
            .into_iter()
            .map(|(full_id, _)| (full_id, Some(beneficiary.clone())));

        let amount = items.clone().fold::<PaymentBalanceOf<T, I>, _>(
            <T::BenchmarkHelper as BenchmarkHelper<T, I>>::Assets::minimum_balance(
                asset_id.clone(),
            ),
            |price, ((ref inventory_id, ref id), _)| {
                let item = T::Listings::item(inventory_id, id).expect("item already created; qed");
                let ItemPrice { amount, .. } =
                    item.price.expect("prices given for every item; qed");
                price.saturating_add(amount)
            },
        );

        let sender_costs = items.clone().fold::<PaymentBalanceOf<T, I>, _>(
            Zero::zero(),
            |costs, ((ref inventory_id, ref id), _)| {
                let item = T::Listings::item(inventory_id, id).expect("item already created; qed");
                let ItemPrice { amount, .. } =
                    item.price.expect("prices given for every item; qed");
                let payment_costs =
                    T::Payments::sender_costs(&asset_id, &caller_account, &seller_account, &amount);
                costs.saturating_add(payment_costs)
            },
        );

        prepare_account::<T, I>(&caller_account);
        prepare_asset_account::<T, I>(
            asset_id.clone(),
            &caller_account,
            amount.saturating_add(sender_costs),
        )?;

        Pallet::<T, I>::set_cart_items(caller.clone(), order_id.clone(), items.collect())?;
        Pallet::<T, I>::checkout(caller.clone(), order_id.clone())?;

        #[extrinsic_call]
        _(caller as T::RuntimeOrigin, order_id.clone());

        // Verification code
        assert_has_event::<T, I>(Event::<T, I>::OrderInProgress { order_id }.into());
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), mock::Test);
}
