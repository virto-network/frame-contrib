use super::*;

use codec::{Decode, DecodeWithMemTracking, Encode};
use core::{fmt, marker::PhantomData};
use frame_support::{
    dispatch::{DispatchInfo, Pays, PostDispatchInfo},
    pallet_prelude::{TransactionValidityError, ValidTransaction},
    weights::Weight,
};
use scale_info::{StaticTypeInfo, TypeInfo};
use sp_runtime::{
    traits::{
        DispatchInfoOf, DispatchOriginOf, Dispatchable, Implication, PostDispatchInfoOf,
        TransactionExtension, ValidateResult,
    },
    transaction_validity::{InvalidTransaction, TransactionSource},
    DispatchResult,
};

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
    pub fn new(s: S) -> Self {
        Self(s, PhantomData)
    }
}

#[derive(PartialEq)]
pub enum Pre<AccountId, P> {
    Burner(AccountId, Weight),
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
    type Val = Option<S::Val>;
    type Pre = Pre<T::AccountId, S::Pre>;

    fn weight(&self, _: &T::RuntimeCall) -> Weight {
        T::WeightInfo::charge_transaction_payment()
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
        match origin
            .clone()
            .into()
            .map_err(|_| InvalidTransaction::BadSigner)?
        {
            frame_system::RawOrigin::Signed(ref who) => {
                let gas = T::GasTank::check_available_gas(who, &info.call_weight);
                if gas.is_some() {
                    Ok((ValidTransaction::default(), None, origin))
                } else {
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
            }
            _ => self
                .0
                .validate(
                    origin,
                    call,
                    info,
                    len,
                    self_implicit,
                    inherited_implication,
                    source,
                )
                .map(|(valid, val, origin)| (valid, Some(val), origin)),
        }
    }

    fn prepare(
        self,
        val: Self::Val,
        origin: &DispatchOriginOf<T::RuntimeCall>,
        call: &T::RuntimeCall,
        info: &DispatchInfoOf<T::RuntimeCall>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        match origin
            .clone()
            .into()
            .map_err(|_| InvalidTransaction::BadSigner)?
        {
            frame_system::RawOrigin::Signed(who) => {
                let pre = if let Some(leftover) =
                    T::GasTank::check_available_gas(&who, &info.call_weight)
                {
                    Pre::Burner(who, leftover)
                } else {
                    self.0
                        .prepare(
                            val.expect("value was given on validate; qed"),
                            origin,
                            call,
                            info,
                            len,
                        )
                        .map(Pre::Inner)?
                };

                Ok(pre)
            }
            _ => self
                .0
                .prepare(
                    val.expect("value was given on validate; qed"),
                    origin,
                    call,
                    info,
                    len,
                )
                .map(Pre::Inner),
        }
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
                let used_gas = post_info.actual_weight.unwrap_or(info.call_weight);
                let should_burn_gas = post_info.pays_fee == Pays::Yes;

                if should_burn_gas {
                    let remaining = T::GasTank::burn_gas(&who, &expected_remaining, &used_gas);
                    Pallet::<T>::deposit_event(Event::GasBurned { who, remaining });
                    Ok(used_gas)
                } else {
                    Ok(Weight::zero())
                }
            }
        }
    }
}
