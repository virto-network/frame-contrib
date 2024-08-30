use codec::{Decode, Encode};
use core::marker::PhantomData;
use frame_support::{
    dispatch::{DispatchInfo, Pays, PostDispatchInfo},
    pallet_prelude::{TransactionValidityError, ValidTransaction},
};
use scale_info::{StaticTypeInfo, TypeInfo};
use sp_runtime::traits::{DispatchInfoOf, Dispatchable, SignedExtension};

use crate::{traits::GasBurner, AccountIdOf, Config, Event, Pallet};

#[derive(Encode, Decode, Clone, Eq, PartialEq)]
pub struct ChargeTxBurningGas<T, S>(pub S, PhantomData<T>);

// Make this extension "invisible" from the outside (ie metadata type information)
impl<T, S: StaticTypeInfo> TypeInfo for ChargeTxBurningGas<T, S> {
    type Identity = S;
    fn type_info() -> scale_info::Type {
        S::type_info()
    }
}

impl<T, S: Encode> core::fmt::Debug for ChargeTxBurningGas<T, S> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "ChargeTxBurningGas<{:?}>", self.0.encode())
    }
    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut core::fmt::Formatter) -> core::fmt::Result {
        Ok(())
    }
}

impl<T, S> ChargeTxBurningGas<T, S> {
    pub fn new(s: S) -> Self {
        Self(s, PhantomData)
    }
}

impl<T, S> SignedExtension for ChargeTxBurningGas<T, S>
where
    T: Config + Send + Sync,
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    S: SignedExtension<AccountId = T::AccountId>,
    S::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
    const IDENTIFIER: &'static str = S::IDENTIFIER;
    type AccountId = AccountIdOf<T>;
    type Call = S::Call;
    type AdditionalSigned = S::AdditionalSigned;
    type Pre = (AccountIdOf<T>, Option<S::Pre>);

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
        let pre = if let Some(_) = T::GasHandler::check_available_gas(who, &Some(info.weight)) {
            None
        } else {
            Some(self.0.pre_dispatch(who, call, info, len)?)
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
        if let Some(_) = T::GasHandler::check_available_gas(who, &Some(info.weight)) {
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
        if let Some((who, maybe_inner_pre)) = pre {
            if let Some(pre) = maybe_inner_pre {
                S::post_dispatch(Some(pre), &info.clone().into(), post_info, len, result)?;
            } else {
                let actual_weight = post_info.actual_weight.unwrap_or(info.weight);
                let should_burn_gas = post_info.pays_fee == Pays::Yes;

                if should_burn_gas {
                    let remaining = T::GasHandler::burn_gas(&who, &actual_weight);
                    Pallet::<T>::deposit_event(Event::GasBurned { who, remaining });
                }
            }
        }
        Ok(())
    }
}
