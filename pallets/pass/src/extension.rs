use core::marker::PhantomData;

use codec::{Decode, Encode};
use frame_support::pallet_prelude::TransactionValidityError;
use scale_info::{StaticTypeInfo, TypeInfo};
use sp_runtime::traits::{DispatchInfoOf, SignedExtension};

use crate::{Config, Pallet};

#[derive(Encode, Decode)]
pub struct ChargeTransactionToPassAccount<S, T, I = ()>(S, PhantomData<(T, I)>);

impl<S: SignedExtension, T, I> ChargeTransactionToPassAccount<S, T, I> {
    pub fn new(s: S) -> Self {
        Self(s, PhantomData)
    }
}

impl<S: Clone, T, I> Clone for ChargeTransactionToPassAccount<S, T, I> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<S: Eq, T, I> Eq for ChargeTransactionToPassAccount<S, T, I> {}
impl<S: PartialEq, T, I> PartialEq for ChargeTransactionToPassAccount<S, T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<S: SignedExtension + StaticTypeInfo, T, I> TypeInfo
    for ChargeTransactionToPassAccount<S, T, I>
{
    type Identity = S;
    fn type_info() -> scale_info::Type {
        S::type_info()
    }
}

impl<S: SignedExtension + Encode, T, I> core::fmt::Debug
    for ChargeTransactionToPassAccount<S, T, I>
{
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "ChargeTransactionToPassAccount<{:?}>", self.0.encode())
    }
    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut core::fmt::Formatter) -> core::fmt::Result {
        Ok(())
    }
}

impl<S, T, I> SignedExtension for ChargeTransactionToPassAccount<S, T, I>
where
    T: Config<I> + Send + Sync,
    I: 'static + Send + Sync,
    S: SignedExtension<AccountId = T::AccountId, Call = <T as frame_system::Config>::RuntimeCall>,
{
    const IDENTIFIER: &'static str = S::IDENTIFIER;
    type AccountId = S::AccountId;
    type Call = S::Call;
    type AdditionalSigned = S::AdditionalSigned;
    type Pre = S::Pre;

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
        let who = Pallet::<T, I>::signer_from_session_key(&who).unwrap_or(who.clone());
        self.0.pre_dispatch(&who, call, info, len)
    }

    fn validate(
        &self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> frame_support::pallet_prelude::TransactionValidity {
        let who = Pallet::<T, I>::signer_from_session_key(&who).unwrap_or(who.clone());
        self.0.validate(&who, call, info, len)
    }

    fn post_dispatch(
        pre: Option<Self::Pre>,
        info: &DispatchInfoOf<Self::Call>,
        post_info: &sp_runtime::traits::PostDispatchInfoOf<Self::Call>,
        len: usize,
        result: &sp_runtime::DispatchResult,
    ) -> Result<(), frame_support::pallet_prelude::TransactionValidityError> {
        S::post_dispatch(pre, info, post_info, len, result)
    }
}
