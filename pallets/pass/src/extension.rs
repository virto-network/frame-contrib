use core::{fmt, marker::PhantomData};

use crate::{Config, Pallet};
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::pallet_prelude::{TransactionValidityError, Weight};
use frame_system::ensure_signed;
use frame_system::pallet_prelude::RuntimeCallFor;
use scale_info::{StaticTypeInfo, TypeInfo};
use sp_runtime::traits::{
    DispatchInfoOf, DispatchOriginOf, Implication, TransactionExtension, ValidateResult,
};
use sp_runtime::transaction_validity::TransactionSource;

/// Changes the origin account to inner extensions if the signer is a session key, so the validations
/// and handling of these extensions (like charging to an account) happens on behalf of the `AccountId`
/// of the account this session key is being associated to.
///
/// In the future, this extension would be deprecated in favour of a couple of an extension that issues
/// authorized origins from `pallet-pass`.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq)]
pub struct ChargeTransactionToPassAccount<S, T, I = ()>(pub S, PhantomData<T>, PhantomData<I>);

impl<S, T, I> ChargeTransactionToPassAccount<S, T, I> {
    pub fn new(s: S) -> Self {
        Self(s, PhantomData, PhantomData)
    }
}

impl<S: StaticTypeInfo, T, I> TypeInfo for ChargeTransactionToPassAccount<S, T, I> {
    type Identity = S;
    fn type_info() -> scale_info::Type {
        S::type_info()
    }
}

impl<S: fmt::Debug, T, I> fmt::Debug for ChargeTransactionToPassAccount<S, T, I> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ChargeTransactionToPassAccount<{:?}>", self.0)
    }
    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

impl<S, T, I: 'static> ChargeTransactionToPassAccount<S, T, I>
where
    T: Config<I>,
{
    fn convert_origin(
        origin: &DispatchOriginOf<RuntimeCallFor<T>>,
    ) -> DispatchOriginOf<RuntimeCallFor<T>> {
        ensure_signed(origin.clone())
            .and_then(|ref who| {
                let pass_account_id =
                    Pallet::<T, I>::signer_from_session_key(who).unwrap_or(who.clone());
                Ok(frame_system::RawOrigin::Signed(pass_account_id).into())
            })
            .unwrap_or(origin.clone())
    }
}

impl<S, T, I: 'static> TransactionExtension<RuntimeCallFor<T>>
for ChargeTransactionToPassAccount<S, T, I>
where
    T: Config<I> + Send + Sync,
    I: Clone + Eq + Send + Sync,
    S: TransactionExtension<RuntimeCallFor<T>> + StaticTypeInfo,
{
    const IDENTIFIER: &'static str = S::IDENTIFIER;
    type Implicit = S::Implicit;
    type Val = S::Val;
    type Pre = S::Pre;

    fn weight(&self, call: &RuntimeCallFor<T>) -> Weight {
        self.0.weight(call)
    }

    fn validate(
        &self,
        origin: DispatchOriginOf<RuntimeCallFor<T>>,
        call: &RuntimeCallFor<T>,
        info: &DispatchInfoOf<RuntimeCallFor<T>>,
        len: usize,
        self_implicit: Self::Implicit,
        inherited_implication: &impl Implication,
        source: TransactionSource,
    ) -> ValidateResult<Self::Val, RuntimeCallFor<T>> {
        self.0.validate(
            Self::convert_origin(&origin),
            call,
            info,
            len,
            self_implicit,
            inherited_implication,
            source,
        )
    }

    fn prepare(
        self,
        val: Self::Val,
        origin: &DispatchOriginOf<RuntimeCallFor<T>>,
        call: &RuntimeCallFor<T>,
        info: &DispatchInfoOf<RuntimeCallFor<T>>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        self.0
            .prepare(val, &Self::convert_origin(origin), call, info, len)
    }
}
