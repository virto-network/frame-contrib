use super::*;
use alloc::vec::Vec;
use core::marker::PhantomData;

use codec::{Decode, DecodeWithMemTracking, Encode};
use frame::deps::{
    frame_support::{
        dispatch::DispatchInfo,
        traits::{
            fungibles::{Inspect, Mutate},
            tokens::Preservation,
            IsSubType,
        },
    },
    frame_system,
};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{
        DispatchInfoOf, DispatchOriginOf, Implication, PostDispatchInfoOf, TransactionExtension,
        ValidateResult, Zero,
    },
    transaction_validity::{InvalidTransaction, TransactionSource, ValidTransaction},
    Saturating,
};

use crate::types::{AssetIdOf, BalanceOf};

type Inner<T> = pallet_assets::Pallet<T>;

/// Extracts asset transfer info from a runtime call using `IsSubType`.
fn extract_asset_transfer<T>(call: &T::RuntimeCall) -> Option<(AssetIdOf<T>, BalanceOf<T>)>
where
    T: Config,
    T::RuntimeCall: IsSubType<pallet_assets::Call<T>>,
{
    match call.is_sub_type()? {
        pallet_assets::Call::transfer { id, amount, .. }
        | pallet_assets::Call::transfer_keep_alive { id, amount, .. }
        | pallet_assets::Call::transfer_approved { id, amount, .. } => {
            Some((id.clone().into(), *amount))
        }
        _ => None,
    }
}

/// Transaction extension that charges community and protocol fees
/// on direct pallet-assets calls.
///
/// Fees are charged in `prepare` (before the call executes).
/// If the call fails, fees are refunded in `post_dispatch_details`.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct ChargeFees<T: Config>(#[codec(skip)] PhantomData<T>);

impl<T: Config> Default for ChargeFees<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Config> core::fmt::Debug for ChargeFees<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "ChargeFees")
    }
}

/// State stored between prepare and post_dispatch for fee refunds.
pub type ChargeFeePre<T> = Vec<(
    <T as frame_system::Config>::AccountId, // payer
    AssetIdOf<T>,                           // asset
    BalanceOf<T>,                           // fee amount
    <T as frame_system::Config>::AccountId, // beneficiary
)>;

impl<T> TransactionExtension<T::RuntimeCall> for ChargeFees<T>
where
    T: Config + Send + Sync,
    T::RuntimeCall:
        Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo> + IsSubType<pallet_assets::Call<T>>,
{
    const IDENTIFIER: &'static str = "ChargeFees";
    type Implicit = ();
    type Val = Option<(AssetIdOf<T>, BalanceOf<T>, T::AccountId)>;
    type Pre = ChargeFeePre<T>;

    fn weight(&self, _: &T::RuntimeCall) -> Weight {
        Weight::from_parts(15_000_000, 0)
    }

    fn validate(
        &self,
        origin: DispatchOriginOf<T::RuntimeCall>,
        call: &T::RuntimeCall,
        _info: &DispatchInfoOf<T::RuntimeCall>,
        _len: usize,
        _self_implicit: Self::Implicit,
        _inherited_implication: &impl Implication,
        _source: TransactionSource,
    ) -> ValidateResult<Self::Val, T::RuntimeCall> {
        // Only inspect signed transactions
        let who = match origin.clone().into() {
            Ok(frame_system::RawOrigin::Signed(ref who)) => who.clone(),
            _ => return Ok((ValidTransaction::default(), None, origin)),
        };

        // Check if the call is an asset operation
        let Some((asset, amount)) = extract_asset_transfer::<T>(call) else {
            return Ok((ValidTransaction::default(), None, origin));
        };

        // Calculate fees and verify sender can afford transfer + fees
        let fees = Pallet::<T>::calculate_fees(asset.clone(), &who, amount);
        let total_fees = fees
            .iter()
            .map(|(_, a)| *a)
            .fold(BalanceOf::<T>::zero(), |a, b| a.saturating_add(b));

        if !total_fees.is_zero() {
            let balance = <Inner<T> as Inspect<T::AccountId>>::balance(asset.clone(), &who);
            if balance < amount.saturating_add(total_fees) {
                return Err(InvalidTransaction::Payment.into());
            }
        }

        Ok((
            ValidTransaction::default(),
            Some((asset, amount, who)),
            origin,
        ))
    }

    fn prepare(
        self,
        val: Self::Val,
        _origin: &DispatchOriginOf<T::RuntimeCall>,
        _call: &T::RuntimeCall,
        _info: &DispatchInfoOf<T::RuntimeCall>,
        _len: usize,
    ) -> Result<Self::Pre, sp_runtime::transaction_validity::TransactionValidityError> {
        let Some((asset, amount, who)) = val else {
            return Ok(Vec::new());
        };

        let fees = Pallet::<T>::calculate_fees(asset.clone(), &who, amount);
        let mut total_fees = BalanceOf::<T>::zero();
        let mut charged = Vec::new();

        for (beneficiary, fee_amount) in fees {
            <Inner<T> as Mutate<T::AccountId>>::transfer(
                asset.clone(),
                &who,
                &beneficiary,
                fee_amount,
                Preservation::Preserve,
            )
            .map_err(|_| InvalidTransaction::Payment)?;
            total_fees = total_fees.saturating_add(fee_amount);
            charged.push((who.clone(), asset.clone(), fee_amount, beneficiary));
        }

        if !total_fees.is_zero() {
            Pallet::<T>::deposit_event(Event::FeesCharged {
                who,
                asset,
                total_fees,
            });
        }

        Ok(charged)
    }

    fn post_dispatch_details(
        pre: Self::Pre,
        _info: &DispatchInfoOf<T::RuntimeCall>,
        _post_info: &PostDispatchInfoOf<T::RuntimeCall>,
        _len: usize,
        result: &DispatchResult,
    ) -> Result<Weight, sp_runtime::transaction_validity::TransactionValidityError> {
        // Refund fees if the call failed
        if result.is_err() {
            for (payer, asset, amount, beneficiary) in pre {
                // Best-effort refund — use Expendable since the beneficiary account
                // may need to be fully drained to return the fee
                let _ = <Inner<T> as Mutate<T::AccountId>>::transfer(
                    asset,
                    &beneficiary,
                    &payer,
                    amount,
                    Preservation::Expendable,
                );
            }
        }
        Ok(Weight::zero())
    }
}
