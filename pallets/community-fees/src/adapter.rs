use core::marker::PhantomData;
use frame::deps::frame_support::traits::{
    fungibles::{self, Dust},
    tokens::{DepositConsequence, Fortitude, Preservation, Provenance, WithdrawConsequence},
};
use sp_runtime::{traits::Zero, DispatchError, Saturating};

use crate::{
    types::{AssetIdOf, BalanceOf},
    Config, Event, Pallet,
};

/// A fungibles adapter that charges community and protocol fees on transfers.
///
/// Wraps the inner `T::Assets` implementation. All read operations are delegated;
/// `transfer` is intercepted to charge configured fees on top.
pub struct WithFees<T>(PhantomData<T>);

// ---------------------------------------------------------------------------
// fungibles::Inspect — pure delegation
// ---------------------------------------------------------------------------
impl<T: Config> fungibles::Inspect<T::AccountId> for WithFees<T> {
    type AssetId = AssetIdOf<T>;
    type Balance = BalanceOf<T>;

    fn total_issuance(asset: AssetIdOf<T>) -> BalanceOf<T> {
        T::Assets::total_issuance(asset)
    }

    fn active_issuance(asset: AssetIdOf<T>) -> BalanceOf<T> {
        T::Assets::active_issuance(asset)
    }

    fn minimum_balance(asset: AssetIdOf<T>) -> BalanceOf<T> {
        T::Assets::minimum_balance(asset)
    }

    fn total_balance(asset: AssetIdOf<T>, who: &T::AccountId) -> BalanceOf<T> {
        T::Assets::total_balance(asset, who)
    }

    fn balance(asset: AssetIdOf<T>, who: &T::AccountId) -> BalanceOf<T> {
        T::Assets::balance(asset, who)
    }

    fn reducible_balance(
        asset: AssetIdOf<T>,
        who: &T::AccountId,
        preservation: Preservation,
        force: Fortitude,
    ) -> BalanceOf<T> {
        T::Assets::reducible_balance(asset, who, preservation, force)
    }

    fn can_deposit(
        asset: AssetIdOf<T>,
        who: &T::AccountId,
        amount: BalanceOf<T>,
        provenance: Provenance,
    ) -> DepositConsequence {
        T::Assets::can_deposit(asset, who, amount, provenance)
    }

    fn can_withdraw(
        asset: AssetIdOf<T>,
        who: &T::AccountId,
        amount: BalanceOf<T>,
    ) -> WithdrawConsequence<BalanceOf<T>> {
        T::Assets::can_withdraw(asset, who, amount)
    }

    fn asset_exists(asset: AssetIdOf<T>) -> bool {
        T::Assets::asset_exists(asset)
    }
}

// ---------------------------------------------------------------------------
// fungibles::Unbalanced — pure delegation
// ---------------------------------------------------------------------------
impl<T: Config> fungibles::Unbalanced<T::AccountId> for WithFees<T> {
    fn handle_dust(dust: Dust<T::AccountId, Self>) {
        T::Assets::handle_dust(Dust(dust.0, dust.1));
    }

    fn write_balance(
        asset: AssetIdOf<T>,
        who: &T::AccountId,
        amount: BalanceOf<T>,
    ) -> Result<Option<BalanceOf<T>>, DispatchError> {
        T::Assets::write_balance(asset, who, amount)
    }

    fn set_total_issuance(asset: AssetIdOf<T>, amount: BalanceOf<T>) {
        T::Assets::set_total_issuance(asset, amount)
    }
}

// ---------------------------------------------------------------------------
// fungibles::Mutate — intercept `transfer` to charge fees on top
// ---------------------------------------------------------------------------
impl<T: Config> fungibles::Mutate<T::AccountId> for WithFees<T>
where
    T::AccountId: Eq,
{
    fn transfer(
        asset: AssetIdOf<T>,
        source: &T::AccountId,
        dest: &T::AccountId,
        amount: BalanceOf<T>,
        preservation: Preservation,
    ) -> Result<BalanceOf<T>, DispatchError> {
        let fees = Pallet::<T>::calculate_fees(source, amount);

        // Charge fees on top — each fee goes from source to the fee beneficiary.
        // Uses the inner implementation directly to avoid recursion.
        for (beneficiary, fee_amount) in &fees {
            T::Assets::transfer(
                asset.clone(),
                source,
                beneficiary,
                *fee_amount,
                Preservation::Preserve,
            )?;
        }

        let total_fees: BalanceOf<T> = fees
            .iter()
            .map(|(_, a)| *a)
            .fold(BalanceOf::<T>::zero(), |acc, x| acc.saturating_add(x));

        if !total_fees.is_zero() {
            Pallet::<T>::deposit_event(Event::FeesCharged {
                who: source.clone(),
                total_fees,
            });
        }

        // Execute the original transfer
        T::Assets::transfer(asset, source, dest, amount, preservation)
    }
}
