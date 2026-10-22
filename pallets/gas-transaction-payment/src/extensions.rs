use super::*;

use codec::{Decode, DecodeWithMemTracking, Encode};
use core::{fmt, marker::PhantomData};
use frame::deps::{
    frame_support::dispatch::DispatchInfo,
    sp_runtime::traits::{DispatchOriginOf, Implication, PostDispatchInfoOf},
};

use scale_info::{StaticTypeInfo, TypeInfo};

/// The custom [`InvalidTransaction`] code [`ChargeTransactionPayment`] rejects a transaction with
/// when preparation disagrees with validation: validation chose the gas tank, and in preparation
/// [`GasBurner::prepare_gas`] no longer covers the transaction.
///
/// The transaction is then invalid, as if it had failed validation. It is never prepared on the fee
/// path, which validation did not check, and nothing panics.
pub const PATH_MISMATCH: u8 = 1;

/// A transaction extension that pays for a signed transaction with gas from the signer's
/// [`Config::GasTank`] when it covers the transaction, and otherwise defers to the fee extension `S`.
///
/// It is invisible in metadata: its identifier, implicit data, type information and encoding are
/// `S`'s. See the [crate documentation](crate) for how to integrate it.
#[derive(Decode, DecodeWithMemTracking, Encode, Clone, Eq, PartialEq)]
pub struct ChargeTransactionPayment<T, S>(pub S, PhantomData<T>);

// Make this extension "invisible" from the outside (i.e. metadata type information)
impl<T: Config, S: TransactionExtension<T::RuntimeCall> + StaticTypeInfo> TypeInfo
    for ChargeTransactionPayment<T, S>
{
    type Identity = S;
    fn type_info() -> scale_info::Type {
        S::type_info()
    }
}

impl<T, S: Encode> fmt::Debug for ChargeTransactionPayment<T, S> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ChargeTxBurningGas<{:?}>", self.0.encode())
    }
    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

impl<T, S> ChargeTransactionPayment<T, S> {
    /// Wraps the fee extension `s`.
    pub fn new(s: S) -> Self {
        Self(s, PhantomData)
    }
}

impl<T, S> ChargeTransactionPayment<T, S>
where
    T: Config,
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
    /// The weight `CheckWeight` books for the transaction before dispatch: its declared call and
    /// extension weights, plus the base extrinsic weight of its class, plus its length as proof size.
    pub(crate) fn estimate(info: &DispatchInfo, len: usize) -> Weight {
        frame_system::calculate_consumed_extrinsic_weight::<T::RuntimeCall>(
            &T::BlockWeights::get(),
            info,
            len,
        )
    }

    /// The same parts after dispatch: the call and extension weights left after refunds, plus the
    /// same base and length, capped at the [estimate](Self::estimate).
    pub(crate) fn actual(info: &DispatchInfo, post_info: &PostDispatchInfo, len: usize) -> Weight {
        post_info
            .calc_actual_weight(info)
            .saturating_add(T::BlockWeights::get().get(info.class).base_extrinsic)
            .saturating_add_proof_size(len as u64)
            .min(Self::estimate(info, len))
    }
}

/// What [`ChargeTransactionPayment`] carries from preparation to post-dispatch.
#[derive(PartialEq)]
pub enum Pre<AccountId, P> {
    /// The gas tank pays: the signer, and what [`GasBurner::prepare_gas`] returned.
    Burner(AccountId, Weight),
    /// The fee extension pays, with its own value.
    Inner(P),
}

impl<AccountId, P> fmt::Debug for Pre<AccountId, P>
where
    AccountId: fmt::Debug,
    P: fmt::Debug,
{
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Pre::Burner(who, gas) => write!(f, "Pre::Burner({who:?}, {gas:?})"),
            Pre::Inner(inner) => write!(f, "Pre::Inner({inner:?})"),
        }
    }
    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

impl<T, S> TransactionExtension<T::RuntimeCall> for ChargeTransactionPayment<T, S>
where
    T: Config + Send + Sync,
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    S: TransactionExtension<T::RuntimeCall> + StaticTypeInfo,
{
    const IDENTIFIER: &'static str = S::IDENTIFIER;
    type Implicit = S::Implicit;
    /// `None` when validation chose the gas tank, `Some` with the fee extension's value otherwise.
    type Val = Option<S::Val>;
    type Pre = Pre<T::AccountId, S::Pre>;

    fn weight(&self, call: &T::RuntimeCall) -> Weight {
        T::WeightInfo::charge_transaction_payment().saturating_add(self.0.weight(call))
    }

    fn validate(
        &self,
        origin: DispatchOriginOf<T::RuntimeCall>,
        call: &T::RuntimeCall,
        info: &DispatchInfoOf<T::RuntimeCall>,
        len: usize,
        self_implicit: Self::Implicit,
        inherited_implication: &impl Implication,
        source: TransactionSource,
    ) -> ValidateResult<Self::Val, T::RuntimeCall> {
        if let frame_system::RawOrigin::Signed(ref who) = origin
            .clone()
            .into()
            .map_err(|_| InvalidTransaction::BadSigner)?
        {
            let estimate = Self::estimate(info, len);
            if T::GasTank::check_available_gas(who, &estimate).is_some() {
                return Ok((ValidTransaction::default(), None, origin));
            }
        }

        self.0
            .validate(
                origin,
                call,
                info,
                len,
                self_implicit,
                inherited_implication,
                source,
            )
            .map(|(valid, val, origin)| (valid, Some(val), origin))
    }

    fn prepare(
        self,
        val: Self::Val,
        origin: &DispatchOriginOf<T::RuntimeCall>,
        call: &T::RuntimeCall,
        info: &DispatchInfoOf<T::RuntimeCall>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        // Validation chose the fee path: prepare it with its value, and never ask the tank again.
        if let Some(val) = val {
            return self.0.prepare(val, origin, call, info, len).map(Pre::Inner);
        }

        // Validation chose the tank, so it must still cover the transaction.
        let Ok(frame_system::RawOrigin::Signed(who)) = origin.clone().into() else {
            return Err(InvalidTransaction::Custom(PATH_MISMATCH).into());
        };
        let remaining = T::GasTank::prepare_gas(&who, &Self::estimate(info, len))
            .ok_or(InvalidTransaction::Custom(PATH_MISMATCH))?;

        Ok(Pre::Burner(who, remaining))
    }

    fn post_dispatch_details(
        pre: Self::Pre,
        info: &DispatchInfoOf<T::RuntimeCall>,
        post_info: &PostDispatchInfoOf<T::RuntimeCall>,
        len: usize,
        result: &DispatchResult,
    ) -> Result<Weight, TransactionValidityError> {
        match pre {
            Pre::Inner(pre) => S::post_dispatch_details(pre, info, post_info, len, result),
            Pre::Burner(who, expected_remaining) => {
                if post_info.pays_fee(info) == Pays::Yes {
                    let used = Self::actual(info, post_info, len);
                    let remaining = T::GasTank::burn_gas(&who, &expected_remaining, &used);
                    Pallet::<T>::deposit_event(Event::GasBurned { who, remaining });
                } else {
                    // No fee: nothing is charged, and the tank drops what preparation noted.
                    T::GasTank::cancel_gas(&who, &expected_remaining);
                }

                // Only this extension's own unspent weight, and none is measured. The call's weight
                // stays booked for the block.
                Ok(Weight::zero())
            }
        }
    }
}
