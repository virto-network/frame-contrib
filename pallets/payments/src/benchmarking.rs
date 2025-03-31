use super::*;
#[allow(unused)]
use crate::{types::*, Pallet as Payments};
use alloc::vec;
use frame_benchmarking::{account, v2::*};
use frame_support::traits::fungibles::Create;
use frame_support::{
    assert_ok,
    traits::{fungibles::Mutate, Get},
    BoundedVec,
};
use frame_system::RawOrigin;
use sp_runtime::Percent;

macro_rules! assert_has_event {
	($patt:pat $(if $guard:expr)?) => {
		assert!(frame_system::Pallet::<T>::events().iter().any(|record| {
			let e = <T as Config>::RuntimeEvent::from(record.event.clone());
			matches!(e.try_into(), Ok($patt) $(if $guard)?)
		}));
	};
}

fn create_accounts<T: Config>() -> (
    T::AccountId,
    T::AccountId,
    AccountIdLookupOf<T>,
    AccountIdLookupOf<T>,
) {
    let sender: T::AccountId = account("Alice", 0, 10);
    let beneficiary: T::AccountId = account("Bob", 0, 11);
    let sender_lookup = T::Lookup::unlookup(sender.clone());
    let beneficiary_lookup = T::Lookup::unlookup(beneficiary.clone());

    (sender, beneficiary, sender_lookup, beneficiary_lookup)
}

fn create_asset<T: Config>(
    id: AssetIdOf<T>,
    admin: AccountIdOf<T>,
    is_sufficient: bool,
    min_balance: BalanceOf<T>,
) {
    T::Assets::create(id, admin, is_sufficient, min_balance).unwrap();
}

fn create_and_mint_asset<T: Config>(
    sender: &T::AccountId,
    beneficiary: &T::AccountId,
    asset: &AssetIdOf<T>,
) -> Result<(), BenchmarkError> {
    create_asset::<T>(
        asset.clone(),
        sender.clone(),
        true,
        <BalanceOf<T>>::from(1u32),
    );
    T::Assets::mint_into(asset.clone(), &sender, <BalanceOf<T>>::from(10000000u32))?;
    T::Assets::mint_into(
        asset.clone(),
        &beneficiary,
        <BalanceOf<T>>::from(10000000u32),
    )?;

    Ok(())
}

fn create_payment<T: Config>(
    amount: &BalanceOf<T>,
    asset: &AssetIdOf<T>,
    remark: Option<BoundedDataOf<T>>,
) -> Result<
    (
        T::PaymentId,
        T::AccountId,
        T::AccountId,
        AccountIdLookupOf<T>,
        AccountIdLookupOf<T>,
    ),
    BenchmarkError,
> {
    let (sender, beneficiary, sender_lookup, beneficiary_lookup) = create_accounts::<T>();
    create_and_mint_asset::<T>(&sender, &beneficiary, &asset)?;

    let (payment_id, payment_detail) = Payments::<T>::do_create_payment(
        &sender,
        beneficiary.clone(),
        asset.clone(),
        amount.clone(),
        PaymentState::Created,
        T::IncentivePercentage::get(),
        remark.as_ref().map(|x| x.as_slice()),
    )?;

    // reserve funds for payment
    Payments::<T>::reserve_payment_amount(&sender, &payment_detail)?;

    log::info!("reserve_payment_amount executed");

    // TODO: check storage items

    Ok((
        payment_id,
        sender,
        beneficiary,
        sender_lookup,
        beneficiary_lookup,
    ))
}

#[benchmarks(
where
    AssetIdOf<T>: Default,
)]
mod benchmarks {
    use super::*;
    #[benchmark]
    fn pay(q: Linear<1, { T::MaxRemarkLength::get() }>) -> Result<(), BenchmarkError> {
        let (sender, beneficiary, _, beneficiary_lookup) = create_accounts::<T>();

        let asset_id: AssetIdOf<T> = <AssetIdOf<T>>::default();
        create_and_mint_asset::<T>(&sender, &beneficiary, &asset_id)?;
        let payment_amount = <BalanceOf<T>>::from(100000_u32);

        let order_remark: Option<BoundedDataOf<T>> = if q == 0 {
            None
        } else {
            Some(BoundedVec::try_from(vec![1 as u8; q as usize]).unwrap())
        };

        #[extrinsic_call]
        _(
            RawOrigin::Signed(sender.clone()),
            beneficiary_lookup,
            asset_id.clone(),
            payment_amount,
            order_remark.clone(),
        );

        assert_has_event!(
            Event::PaymentCreated { asset, amount, remark, .. }
            if asset == asset_id && amount == payment_amount &&
                remark == order_remark.as_ref().map(|r| BoundedVec::truncate_from(r.encode()))
        );
        Ok(())
    }

    #[benchmark]
    fn release() -> Result<(), BenchmarkError> {
        let amount = <BalanceOf<T>>::from(100000_u32);
        let asset = <AssetIdOf<T>>::default();
        let (payment_id, sender, _beneficiary, _, beneficiary_lookup) =
            create_payment::<T>(&amount, &asset, None)?;

        log::info!("beneficiary_lookup: {:?}", beneficiary_lookup);

        #[extrinsic_call]
        _(RawOrigin::Signed(sender), payment_id);

        assert_has_event!(Event::PaymentReleased { .. });
        Ok(())
    }

    #[benchmark]
    fn cancel() -> Result<(), BenchmarkError> {
        let amount = <BalanceOf<T>>::from(100000_u32);
        let asset = <AssetIdOf<T>>::default();
        let (payment_id, _sender, beneficiary, _sender_lookup, _beneficiary_lookup) =
            create_payment::<T>(&amount, &asset, None)?;

        #[extrinsic_call]
        _(RawOrigin::Signed(beneficiary.clone()), payment_id);

        assert_has_event!(Event::PaymentCancelled { .. });
        Ok(())
    }

    #[benchmark]
    fn request_refund() -> Result<(), BenchmarkError> {
        let amount = <BalanceOf<T>>::from(100000_u32);
        let asset = <AssetIdOf<T>>::default();
        let (payment_id, sender, _beneficiary, _sender_lookup, _beneficiary_lookup) =
            create_payment::<T>(&amount, &asset, None)?;

        #[extrinsic_call]
        _(RawOrigin::Signed(sender.clone()), payment_id);

        let current_block = frame_system::Pallet::<T>::block_number();
        assert_has_event!(
            Event::PaymentCreatorRequestedRefund { expiry, .. }
            if expiry == (current_block + T::CancelBufferBlockLength::get())
        );
        Ok(())
    }

    #[benchmark]
    fn dispute_refund() -> Result<(), BenchmarkError> {
        let amount = <BalanceOf<T>>::from(100000_u32);
        let asset = <AssetIdOf<T>>::default();
        let (payment_id, sender, beneficiary, _sender_lookup, _beneficiary_lookup) =
            create_payment::<T>(&amount, &asset, None)?;

        assert_ok!(Payments::<T>::request_refund(
            RawOrigin::Signed(sender.clone()).into(),
            payment_id
        ));

        #[extrinsic_call]
        _(RawOrigin::Signed(beneficiary.clone()), payment_id);

        assert_has_event!(Event::PaymentRefundDisputed { .. });
        Ok(())
    }

    #[benchmark]
    fn resolve_dispute() -> Result<(), BenchmarkError> {
        let amount = <BalanceOf<T>>::from(100000_u32);
        let asset = <AssetIdOf<T>>::default();
        let (payment_id, sender, beneficiary, _sender_lookup, _beneficiary_lookup) =
            create_payment::<T>(&amount, &asset, None)?;

        assert_ok!(Payments::<T>::request_refund(
            RawOrigin::Signed(sender.clone()).into(),
            payment_id
        ));
        assert_ok!(Payments::<T>::dispute_refund(
            RawOrigin::Signed(beneficiary.clone()).into(),
            payment_id
        ));

        let dispute_result = DisputeResult {
            percent_beneficiary: Percent::from_percent(90),
            in_favor_of: Role::Sender,
        };

        #[extrinsic_call]
        _(RawOrigin::Root, payment_id, dispute_result);

        assert_has_event!(Event::PaymentDisputeResolved { .. });
        Ok(())
    }

    #[benchmark]
    fn request_payment() -> Result<(), BenchmarkError> {
        let (sender, beneficiary, sender_lookup, _beneficiary_lookup) = create_accounts::<T>();
        let asset: AssetIdOf<T> = <AssetIdOf<T>>::default();
        create_and_mint_asset::<T>(&sender, &beneficiary, &asset)?;
        let amount = <BalanceOf<T>>::from(100000_u32);

        #[extrinsic_call]
        _(
            RawOrigin::Signed(beneficiary.clone()),
            sender_lookup,
            asset,
            amount,
        );

        assert_has_event!(Event::PaymentRequestCreated { .. });
        Ok(())
    }

    #[benchmark]
    fn accept_and_pay() -> Result<(), BenchmarkError> {
        let (sender, beneficiary, _sender_lookup, _beneficiary_lookup) = create_accounts::<T>();
        let asset: AssetIdOf<T> = <AssetIdOf<T>>::default();
        create_and_mint_asset::<T>(&sender, &beneficiary, &asset)?;
        let amount = <BalanceOf<T>>::from(100000_u32);

        let (payment_id, _) = Payments::<T>::do_create_payment(
            &sender,
            beneficiary,
            asset.clone(),
            amount.clone(),
            PaymentState::PaymentRequested,
            T::IncentivePercentage::get(),
            None,
        )?;

        #[extrinsic_call]
        _(RawOrigin::Signed(sender.clone()), payment_id);

        assert_has_event!(Event::PaymentRequestCompleted { .. });
        Ok(())
    }

    impl_benchmark_test_suite!(Payments, crate::mock::new_test_ext(), crate::mock::Test);
}
