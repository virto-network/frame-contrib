use core::marker::PhantomData;
use frame::deps::frame_support::storage::with_transaction;
use frame::deps::frame_support::traits::{
    fungibles::{Dust, Inspect, Mutate, Unbalanced},
    tokens::{DepositConsequence, Fortitude, Preservation, Provenance, WithdrawConsequence},
};
use sp_runtime::{traits::Zero, DispatchError, Saturating, TransactionOutcome};

use crate::{
    types::{AssetIdOf, BalanceOf},
    Config, Event, Pallet,
};

/// Shorthand for accessing pallet-assets through the fungibles traits.
type Inner<T> = pallet_assets::Pallet<T>;

/// A fungibles adapter that charges community and protocol fees on transfers.
///
/// Wraps `pallet_assets::Pallet<T>`. All read operations are delegated;
/// `transfer` is intercepted to charge configured fees on top.
pub struct WithFees<T>(PhantomData<T>);

// ---------------------------------------------------------------------------
// fungibles::Inspect — pure delegation
// ---------------------------------------------------------------------------
impl<T: Config> Inspect<T::AccountId> for WithFees<T> {
    type AssetId = AssetIdOf<T>;
    type Balance = BalanceOf<T>;

    fn total_issuance(asset: AssetIdOf<T>) -> BalanceOf<T> {
        <Inner<T> as Inspect<T::AccountId>>::total_issuance(asset)
    }

    fn active_issuance(asset: AssetIdOf<T>) -> BalanceOf<T> {
        <Inner<T> as Inspect<T::AccountId>>::active_issuance(asset)
    }

    fn minimum_balance(asset: AssetIdOf<T>) -> BalanceOf<T> {
        <Inner<T> as Inspect<T::AccountId>>::minimum_balance(asset)
    }

    fn total_balance(asset: AssetIdOf<T>, who: &T::AccountId) -> BalanceOf<T> {
        <Inner<T> as Inspect<T::AccountId>>::total_balance(asset, who)
    }

    fn balance(asset: AssetIdOf<T>, who: &T::AccountId) -> BalanceOf<T> {
        <Inner<T> as Inspect<T::AccountId>>::balance(asset, who)
    }

    fn reducible_balance(
        asset: AssetIdOf<T>,
        who: &T::AccountId,
        preservation: Preservation,
        force: Fortitude,
    ) -> BalanceOf<T> {
        <Inner<T> as Inspect<T::AccountId>>::reducible_balance(asset, who, preservation, force)
    }

    fn can_deposit(
        asset: AssetIdOf<T>,
        who: &T::AccountId,
        amount: BalanceOf<T>,
        provenance: Provenance,
    ) -> DepositConsequence {
        <Inner<T> as Inspect<T::AccountId>>::can_deposit(asset, who, amount, provenance)
    }

    fn can_withdraw(
        asset: AssetIdOf<T>,
        who: &T::AccountId,
        amount: BalanceOf<T>,
    ) -> WithdrawConsequence<BalanceOf<T>> {
        <Inner<T> as Inspect<T::AccountId>>::can_withdraw(asset, who, amount)
    }

    fn asset_exists(asset: AssetIdOf<T>) -> bool {
        <Inner<T> as Inspect<T::AccountId>>::asset_exists(asset)
    }
}

// ---------------------------------------------------------------------------
// fungibles::Unbalanced — pure delegation
// ---------------------------------------------------------------------------
impl<T: Config> Unbalanced<T::AccountId> for WithFees<T> {
    fn handle_dust(dust: Dust<T::AccountId, Self>) {
        <Inner<T> as Unbalanced<T::AccountId>>::handle_dust(Dust(dust.0, dust.1));
    }

    fn write_balance(
        asset: AssetIdOf<T>,
        who: &T::AccountId,
        amount: BalanceOf<T>,
    ) -> Result<Option<BalanceOf<T>>, DispatchError> {
        <Inner<T> as Unbalanced<T::AccountId>>::write_balance(asset, who, amount)
    }

    fn set_total_issuance(asset: AssetIdOf<T>, amount: BalanceOf<T>) {
        <Inner<T> as Unbalanced<T::AccountId>>::set_total_issuance(asset, amount)
    }
}

// ---------------------------------------------------------------------------
// fungibles::Mutate — intercept `transfer` to charge fees on top
// ---------------------------------------------------------------------------
impl<T: Config> Mutate<T::AccountId> for WithFees<T>
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
        with_transaction(|| {
            let fees = Pallet::<T>::calculate_fees(asset.clone(), source, amount);

            // Charge fees on top — each fee goes from source to the fee beneficiary.
            for (beneficiary, fee_amount) in &fees {
                if let Err(e) = <Inner<T> as Mutate<T::AccountId>>::transfer(
                    asset.clone(),
                    source,
                    beneficiary,
                    *fee_amount,
                    Preservation::Preserve,
                ) {
                    return TransactionOutcome::Rollback(Err(e));
                }
            }

            let total_fees: BalanceOf<T> = fees
                .iter()
                .map(|(_, a)| *a)
                .fold(BalanceOf::<T>::zero(), |acc, x| acc.saturating_add(x));

            // Execute the original transfer
            let result = <Inner<T> as Mutate<T::AccountId>>::transfer(
                asset.clone(),
                source,
                dest,
                amount,
                preservation,
            );

            match result {
                Ok(actual) => {
                    if !total_fees.is_zero() {
                        Pallet::<T>::deposit_event(Event::FeesCharged {
                            who: source.clone(),
                            asset,
                            total_fees,
                        });
                    }
                    TransactionOutcome::Commit(Ok(actual))
                }
                Err(e) => TransactionOutcome::Rollback(Err(e)),
            }
        })
    }
}
