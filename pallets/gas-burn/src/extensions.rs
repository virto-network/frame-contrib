use codec::{Decode, Encode};
use core::marker::PhantomData;
use frame_support::{
    dispatch::{DispatchInfo, Pays, PostDispatchInfo},
    pallet_prelude::{TransactionValidityError, ValidTransaction},
    weights::Weight,
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
    type Pre = (AccountIdOf<T>, Pre<S>);

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
        if let Some(_) = T::GasBurner::check_available_gas(who, &info.weight) {
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
                Pre::Inner(inner_pre) => S::post_dispatch(
                    Some(inner_pre),
                    &info.clone().into(),
                    post_info,
                    len,
                    result,
                )?,
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
