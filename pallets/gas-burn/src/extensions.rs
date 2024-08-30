use codec::{Decode, Encode};
use core::marker::PhantomData;
use frame_support::{
    dispatch::{DispatchInfo, Pays, PostDispatchInfo},
    pallet_prelude::InvalidTransaction,
    weights::Weight,
    DebugNoBound,
};
use frame_system::pallet_prelude::RuntimeCallFor;
use scale_info::{StaticTypeInfo, TypeInfo};
use sp_runtime::traits::{DispatchInfoOf, Dispatchable, SignedExtension};

use crate::{traits::GasBurner, AccountIdOf, Config, Event, Pallet};

#[derive(Encode, Decode, Clone, Eq, PartialEq, DebugNoBound)]
pub struct ChargeTxWithPrepaidGas<T, S>(PhantomData<(T, S)>);

// Make this extension "invisible" from the outside (ie metadata type information)
impl<T, S: StaticTypeInfo> TypeInfo for ChargeTxWithPrepaidGas<T, S> {
    type Identity = S;
    fn type_info() -> scale_info::Type {
        S::type_info()
    }
}

impl<T, S> SignedExtension for ChargeTxWithPrepaidGas<T, S>
where
    T: Config + Send + Sync,
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    S: SignedExtension<AccountId = T::AccountId>,
{
    const IDENTIFIER: &'static str = "CheckGasHandler";
    type AccountId = AccountIdOf<T>;
    type Call = RuntimeCallFor<T>;
    type AdditionalSigned = ();
    type Pre = AccountIdOf<T>;

    fn additional_signed(
        &self,
    ) -> Result<Self::AdditionalSigned, frame_support::pallet_prelude::TransactionValidityError>
    {
        Ok(())
    }

    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        _call: &RuntimeCallFor<T>,
        info: &DispatchInfoOf<RuntimeCallFor<T>>,
        _len: usize,
    ) -> Result<Self::Pre, frame_support::pallet_prelude::TransactionValidityError> {
        T::GasHandler::check_available_gas(who, &Some(info.weight))
            .map(|_| who.clone())
            .ok_or(InvalidTransaction::Payment.into())
    }

    fn post_dispatch(
        pre: Option<Self::Pre>,
        info: &DispatchInfoOf<Self::Call>,
        post_info: &sp_runtime::traits::PostDispatchInfoOf<Self::Call>,
        _len: usize,
        _result: &sp_runtime::DispatchResult,
    ) -> Result<(), frame_support::pallet_prelude::TransactionValidityError> {
        let who = pre.expect("pre given on pre_dispatch; qed");

        let actual_weight = post_info.actual_weight.unwrap_or(info.weight);
        let should_burn_gas = post_info.pays_fee == Pays::Yes;

        if should_burn_gas {
            let remaining = T::GasHandler::burn_gas(&who, &actual_weight);
            Pallet::<T>::deposit_event(Event::GasBurned { who, remaining });
        }

        Ok(())
    }
}
