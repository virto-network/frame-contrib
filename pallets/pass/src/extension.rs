use core::marker::PhantomData;

use crate::{Config, Pallet};
use codec::{Decode, Encode};
use frame_support::pallet_prelude::TransactionValidityError;
use scale_info::{StaticTypeInfo, TypeInfo};
use sp_runtime::traits::{DispatchInfoOf, SignedExtension};

/// Changes the origin account to inner extensions if the signer is a session key, so the validations
/// and handling of these extensions (like charging to an account) happens on behalf of the `AccountId`
/// of the account this session key is being associated to.
///
/// In the future, this extension would be deprecated in favour of a couple of an extension that issues
/// authorized origins from `pallet-pass`.
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

    fn validate(
        &self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> frame_support::pallet_prelude::TransactionValidity {
        let who = Pallet::<T, I>::signer_from_session_key(who).unwrap_or(who.clone());
        self.0.validate(&who, call, info, len)
    }

    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        let who = Pallet::<T, I>::signer_from_session_key(who).unwrap_or(who.clone());
        self.0.pre_dispatch(&who, call, info, len)
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

/// Skips some checks (like [`frame_system::CheckNonce`]) if the caller is a session key.
///
/// This extension should be deprecated in the future in favour of a couple of `TransactionExtension`s
/// that:
///
/// 1. Issues a new custom origin from `pallet-pass` to wrap the account and skip checks. Then,
/// 2. After those checks, replaces this custom origin with [`frame_system::RawOrigin::Signed`],
///    where the signed `account_id` is the derived `AccountId` associated to the account handled by
///    `pallet-pass`.
#[derive(Encode, Decode)]
pub struct SkipCheckIfPassAccount<S, T, I = ()>(S, PhantomData<(T, I)>);

impl<S: SignedExtension, T, I> SkipCheckIfPassAccount<S, T, I> {
    pub fn new(s: S) -> Self {
        Self(s, PhantomData)
    }
}

impl<S: Clone, T, I> Clone for SkipCheckIfPassAccount<S, T, I> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<S: Eq, T, I> Eq for SkipCheckIfPassAccount<S, T, I> {}
impl<S: PartialEq, T, I> PartialEq for SkipCheckIfPassAccount<S, T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<S: SignedExtension + StaticTypeInfo, T, I> TypeInfo for SkipCheckIfPassAccount<S, T, I> {
    type Identity = S;
    fn type_info() -> scale_info::Type {
        S::type_info()
    }
}

impl<S: SignedExtension + Encode, T, I> core::fmt::Debug for SkipCheckIfPassAccount<S, T, I> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "SkipCheckIfPassAccount<{:?}>", self.0.encode())
    }
    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut core::fmt::Formatter) -> core::fmt::Result {
        Ok(())
    }
}

impl<S, T, I> SignedExtension for SkipCheckIfPassAccount<S, T, I>
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

    fn validate(
        &self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> frame_support::pallet_prelude::TransactionValidity {
        if Pallet::<T, I>::signer_from_session_key(who).is_some() {
            return Ok(Default::default());
        }
        self.0.validate(&who, call, info, len)
    }

    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        if Pallet::<T, I>::signer_from_session_key(who).is_some() {
            return Ok((None, Default::default()));
        }
        self.0.pre_dispatch(&who, call, info, len)
    }

    fn post_dispatch(
        pre: Option<Self::Pre>,
        info: &DispatchInfoOf<Self::Call>,
        post_info: &sp_runtime::traits::PostDispatchInfoOf<Self::Call>,
        len: usize,
        result: &sp_runtime::DispatchResult,
    ) -> Result<(), frame_support::pallet_prelude::TransactionValidityError> {
        if let Some(pre) = pre {
            S::post_dispatch(pre, info, post_info, len, result)
        } else {
            Ok(())
        }
    }
}
