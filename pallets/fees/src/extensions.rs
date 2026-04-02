use super::*;
use core::marker::PhantomData;

use codec::{Decode, DecodeWithMemTracking, Encode};
use frame::deps::{
    frame_support::{
        dispatch::DispatchInfo,
        traits::{fungibles, tokens::Preservation},
    },
    frame_system,
};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{
        DispatchInfoOf, DispatchOriginOf, Implication, PostDispatchInfoOf, TransactionExtension,
        ValidateResult,
    },
    transaction_validity::{InvalidTransaction, TransactionSource, ValidTransaction},
};

use crate::types::{AssetIdOf, BalanceOf};

/// Transaction extension that charges community and protocol fees
/// on direct pallet-assets calls detected by `T::CallInspector`.
///
/// Fees are charged in `prepare` (before the call executes) so
/// the sender's balance is reduced before the transfer runs.
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

impl<T> TransactionExtension<T::RuntimeCall> for ChargeFees<T>
where
    T: Config + Send + Sync,
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
    const IDENTIFIER: &'static str = "ChargeFees";
    type Implicit = ();
    type Val = Option<(AssetIdOf<T>, BalanceOf<T>, T::AccountId)>;
    type Pre = ();

    fn weight(&self, _: &T::RuntimeCall) -> Weight {
        // Reads: ProtocolFees (1), CommunityFees (1), asset balance (1), community detection
        // Writes: per-fee transfer (up to MaxProtocolFees + MaxCommunityFees)
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
        let Some((asset, amount)) = T::CallInspector::extract_asset_transfer(call) else {
            return Ok((ValidTransaction::default(), None, origin));
        };

        // Calculate fees and verify sender can afford transfer + fees
        let fees = Pallet::<T>::calculate_fees(asset.clone(), &who, amount);
        let total_fees = fees
            .iter()
            .map(|(_, a)| *a)
            .fold(BalanceOf::<T>::zero(), |a, b| a.saturating_add(b));

        if !total_fees.is_zero() {
            let balance =
                <T::Assets as fungibles::Inspect<T::AccountId>>::balance(asset.clone(), &who);
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
            return Ok(());
        };

        let fees = Pallet::<T>::calculate_fees(asset.clone(), &who, amount);
        let mut total_fees = BalanceOf::<T>::zero();

        for (beneficiary, fee_amount) in fees {
            <T::Assets as fungibles::Mutate<T::AccountId>>::transfer(
                asset.clone(),
                &who,
                &beneficiary,
                fee_amount,
                Preservation::Preserve,
            )
            .map_err(|_| InvalidTransaction::Payment)?;
            total_fees = total_fees.saturating_add(fee_amount);
        }

        if !total_fees.is_zero() {
            Pallet::<T>::deposit_event(Event::FeesCharged { who, total_fees });
        }

        Ok(())
    }

    fn post_dispatch_details(
        _pre: Self::Pre,
        _info: &DispatchInfoOf<T::RuntimeCall>,
        _post_info: &PostDispatchInfoOf<T::RuntimeCall>,
        _len: usize,
        _result: &DispatchResult,
    ) -> Result<Weight, sp_runtime::transaction_validity::TransactionValidityError> {
        Ok(Weight::zero())
    }
}
