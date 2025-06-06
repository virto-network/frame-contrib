use super::{Payment as PaymentDetails, *};
use fc_traits_payments::{Inspect, Mutate, Payment};
use frame_support::pallet_prelude::Get;
use frame_support::BoundedVec;

impl<T: Config> Inspect<AccountIdOf<T>> for Pallet<T> {
    type Id = T::PaymentId;
    type AssetId = AssetIdOf<T>;
    type Balance = BalanceOf<T>;

    fn details(id: &Self::Id) -> Option<Payment<AccountIdOf<T>, Self::AssetId, Self::Balance>> {
        let (creator, beneficiary) = PaymentParties::<T>::get(id).ok()?;
        let PaymentDetail { asset, amount, .. } =
            PaymentDetails::<T>::get(creator.clone(), id).ok()?;

        Some(Payment::new(creator, beneficiary, asset, amount))
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn sender_costs(
        asset: &Self::AssetId,
        sender: &AccountIdOf<T>,
        beneficiary: &AccountIdOf<T>,
        amount: &Self::Balance,
    ) -> Self::Balance {
        T::FeeHandler::apply_fees(asset, sender, beneficiary, amount, None)
            .sender_pays
            .iter()
            .fold(
                T::IncentivePercentage::get().mul_floor(*amount),
                |amount, (_, fee, _)| amount.saturating_add(*fee),
            )
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn beneficiary_costs(
        asset: &Self::AssetId,
        sender: &AccountIdOf<T>,
        beneficiary: &AccountIdOf<T>,
        amount: &Self::Balance,
    ) -> Self::Balance {
        T::FeeHandler::apply_fees(asset, sender, beneficiary, amount, None)
            .beneficiary_pays
            .iter()
            .fold(Zero::zero(), |amount, (_, fee, _)| {
                amount.saturating_add(*fee)
            })
    }
}

impl<T: Config> Mutate<AccountIdOf<T>> for Pallet<T> {
    fn create<Details: Encode>(
        sender: &AccountIdOf<T>,
        asset: Self::AssetId,
        amount: Self::Balance,
        beneficiary: &AccountIdOf<T>,
        details: Option<Details>,
    ) -> Result<Self::Id, DispatchError> {
        let remark = details.map(|d| d.encode());

        let (payment_id, payment_detail) = Self::do_create_payment(
            sender,
            beneficiary.clone(),
            asset.clone(),
            amount,
            PaymentState::Created,
            T::IncentivePercentage::get(),
            remark.as_deref(),
        )?;

        // reserve funds for payment
        Self::reserve_payment_amount(sender, &payment_detail)?;

        let (_, total_beneficiary_fee_amount_mandatory, total_beneficiary_fee_amount_optional) =
            payment_detail.fees.summary_for(Role::Beneficiary, false)?;

        let fees = total_beneficiary_fee_amount_mandatory
            .checked_add(&total_beneficiary_fee_amount_optional)
            .ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;

        let beneficiary_amount = payment_detail
            .amount
            .checked_sub(&fees)
            .ok_or(DispatchError::Arithmetic(ArithmeticError::Underflow))?;

        // notify external systems about payment success
        T::OnPaymentStatusChanged::on_payment_charge_success(&payment_id, fees, beneficiary_amount);

        // emit `PaymentCreated` event
        Self::deposit_event(Event::PaymentCreated {
            payment_id,
            asset,
            amount,
            remark: remark.map(BoundedVec::truncate_from),
        });

        Ok(payment_id)
    }
}
