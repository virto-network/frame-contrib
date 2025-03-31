use super::{Payment as PaymentDetails, *};
use fc_traits_payments::{Inspect, Mutate, Payment};

impl<T: Config> Inspect<AccountIdOf<T>> for Pallet<T> {
	type Id = T::PaymentId;
	type AssetId = AssetIdOf<T>;
	type Balance = BalanceOf<T>;

	fn details(id: Self::Id) -> Option<Payment<AccountIdOf<T>, Self::AssetId, Self::Balance>> {
		let (creator, _) = PaymentParties::<T>::get(id).ok()?;
		let PaymentDetail {
			asset,
			amount,
			beneficiary,
			..
		} = PaymentDetails::<T>::get(creator, id).ok()?;

		Some(Payment::new(beneficiary, asset, amount))
	}
}

impl<T: Config> Mutate<AccountIdOf<T>> for Pallet<T> {
	fn create(
		creator: &AccountIdOf<T>,
		asset: Self::AssetId,
		amount: Self::Balance,
		beneficiary: &AccountIdOf<T>,
	) -> Result<Self::Id, DispatchError> {
		let (id, _) = Self::do_create_payment(
			creator,
			beneficiary.clone(),
			asset,
			amount,
			PaymentState::Created,
			Percent::zero(),
			Some(&*b""),
		)?;
		Ok(id)
	}
}
