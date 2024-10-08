use codec::{Decode, Encode};
use core::marker::PhantomData;
use fc_traits_gas_tank::GasBurner;
use frame_support::{
    dispatch::{DispatchInfo, Pays, PostDispatchInfo},
    pallet_prelude::{TransactionValidityError, ValidTransaction},
    weights::Weight,
};
use scale_info::{StaticTypeInfo, TypeInfo};
use sp_runtime::traits::{DispatchInfoOf, Dispatchable, SignedExtension};

use crate::{Config, Event, Pallet};

#[derive(Encode, Decode, Clone, Eq, PartialEq)]
pub struct ChargeTransactionPayment<T, S: SignedExtension>(pub S, PhantomData<T>);

// Make this extension "invisible" from the outside (ie metadata type information)
impl<T, S: SignedExtension + StaticTypeInfo> TypeInfo for ChargeTransactionPayment<T, S> {
    type Identity = S;
    fn type_info() -> scale_info::Type {
        S::type_info()
    }
}

impl<T, S: SignedExtension + Encode> core::fmt::Debug for ChargeTransactionPayment<T, S> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "ChargeTxBurningGas<{:?}>", self.0.encode())
    }
    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut core::fmt::Formatter) -> core::fmt::Result {
        Ok(())
    }
}

impl<T, S: SignedExtension> ChargeTransactionPayment<T, S> {
    pub fn new(s: S) -> Self {
        Self(s, PhantomData)
    }
}

#[derive(PartialEq)]
pub enum Pre<S: SignedExtension + Encode> {
    Burner(Weight),
    Inner(S::Pre),
}

impl<S: SignedExtension + Encode> core::fmt::Debug for Pre<S> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Pre::Burner(gas) => write!(f, "Pre::Burner({:?})", gas),
            Pre::Inner(_) => write!(f, "Pre::Inner(<inner>)"),
        }
    }
    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut core::fmt::Formatter) -> core::fmt::Result {
        Ok(())
    }
}

impl<T, S> SignedExtension for ChargeTransactionPayment<T, S>
where
    T: Config + Send + Sync,
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    S: SignedExtension<AccountId = T::AccountId, Call = T::RuntimeCall>,
    S::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
    const IDENTIFIER: &'static str = S::IDENTIFIER;
    type AccountId = S::AccountId;
    type Call = S::Call;
    type AdditionalSigned = S::AdditionalSigned;
    type Pre = (Self::AccountId, Pre<S>);

    fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
        self.0.additional_signed()
    }

    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        let pre = if let Some(leftover) = T::GasBurner::check_available_gas(who, &info.weight) {
            Pre::Burner(leftover)
        } else {
            Pre::Inner(self.0.pre_dispatch(who, call, info, len)?)
        };

        Ok((who.clone(), pre))
    }

    fn validate(
        &self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> frame_support::pallet_prelude::TransactionValidity {
        if T::GasBurner::check_available_gas(who, &info.weight).is_some() {
            Ok(ValidTransaction::default())
        } else {
            self.0.validate(who, call, info, len)
        }
    }

    fn post_dispatch(
        pre: Option<Self::Pre>,
        info: &DispatchInfoOf<Self::Call>,
        post_info: &sp_runtime::traits::PostDispatchInfoOf<Self::Call>,
        len: usize,
        result: &sp_runtime::DispatchResult,
    ) -> Result<(), frame_support::pallet_prelude::TransactionValidityError> {
        if let Some((who, pre)) = pre {
            match pre {
                Pre::Inner(inner_pre) => {
                    S::post_dispatch(Some(inner_pre), info, post_info, len, result)?
                }
                Pre::Burner(expected_remaining) => {
                    let used_gas = post_info.actual_weight.unwrap_or(info.weight);
                    let should_burn_gas = post_info.pays_fee == Pays::Yes;

                    if should_burn_gas {
                        let remaining =
                            T::GasBurner::burn_gas(&who, &expected_remaining, &used_gas);
                        Pallet::<T>::deposit_event(Event::GasBurned { who, remaining });
                    }
                }
            }
        }
        Ok(())
    }
}
