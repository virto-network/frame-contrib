use crate::{Config, CredentialOf, Pallet, WeightInfo};
use codec::{Decode, DecodeWithMemTracking, Encode};
use fc_traits_authn::DeviceId;
use frame_support::{
    dispatch::RawOrigin,
    pallet_prelude::{TransactionValidityError, Weight},
    CloneNoBound, DebugNoBound, DefaultNoBound, EqNoBound, PartialEqNoBound,
};
use frame_system::{ensure_signed, pallet_prelude::RuntimeCallFor};
use scale_info::TypeInfo;
use sp_core::blake2_256;
use sp_runtime::{
    traits::{DispatchInfoOf, DispatchOriginOf, Implication, TransactionExtension, ValidateResult},
    transaction_validity::{InvalidTransaction, TransactionSource, ValidTransaction},
};

/// Handles the authentication of a Pass account. If the authentication is successful, a signed
/// origin associated to the device would be set.
///
/// Throws an [`UnknownOrigin`][InvalidTransaction::UnknownOrigin] error otherwise.
#[derive(
    DefaultNoBound,
    Encode,
    Decode,
    DecodeWithMemTracking,
    CloneNoBound,
    EqNoBound,
    PartialEqNoBound,
    DebugNoBound,
    TypeInfo,
)]
#[scale_info(skip_type_params(T, I))]
pub struct PassAuthenticate<T: Config<I>, I: 'static = ()>(Option<AuthenticateParams<T, I>>);

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    CloneNoBound,
    EqNoBound,
    PartialEqNoBound,
    DebugNoBound,
    TypeInfo,
)]
#[scale_info(skip_type_params(T, I))]
pub struct AuthenticateParams<T: Config<I>, I: 'static = ()> {
    device_id: DeviceId,
    credential: CredentialOf<T, I>,
}

impl<T, I> PassAuthenticate<T, I>
where
    T: Config<I>,
    I: 'static,
{
    pub fn from(device_id: DeviceId, credential: CredentialOf<T, I>) -> Self {
        Self(Some(AuthenticateParams {
            device_id,
            credential,
        }))
    }
}

impl<T, I> TransactionExtension<RuntimeCallFor<T>> for PassAuthenticate<T, I>
where
    T: Config<I>,
    I: 'static,
{
    const IDENTIFIER: &'static str = "PassAuthenticate";
    type Implicit = ();
    type Val = ();
    type Pre = ();

    fn weight(&self, _call: &RuntimeCallFor<T>) -> Weight {
        T::WeightInfo::authenticate()
    }

    fn validate(
        &self,
        origin: DispatchOriginOf<RuntimeCallFor<T>>,
        _call: &RuntimeCallFor<T>,
        _info: &DispatchInfoOf<RuntimeCallFor<T>>,
        _len: usize,
        _self_implicit: Self::Implicit,
        inherited_implication: &impl Implication,
        _source: TransactionSource,
    ) -> ValidateResult<Self::Val, RuntimeCallFor<T>> {
        let origin = if let Some(params) = &self.0 {
            Pallet::<T, I>::authenticate(
                &params.device_id,
                &params.credential,
                &inherited_implication.using_encoded(blake2_256),
            )
            .map(|address| RawOrigin::Signed(address).into())
            .map_err(|e| {
                log::error!(target: "pallet_pass", "Authentication failed: {:?}", e);
                InvalidTransaction::BadSigner.into()
            })
        } else {
            // If we're not attempting to authenticate, let's check if the origin is signed, and is
            // maybe an existing session key. Given that, we'll pass the actual `pass_account_id`.
            //
            // Otherwise, just pass the previous origin to the rest of the extensions pipeline.

            Ok::<_, TransactionValidityError>(if let Ok(who) = ensure_signed(origin.clone()) {
                Pallet::<T, I>::pass_account_from_session_key(&who)
                    .or(Some(who))
                    .map(|who| RawOrigin::Signed(who).into())
                    .unwrap()
            } else {
                origin
            })
        }?;

        Ok((ValidTransaction::default(), (), origin))
    }

    fn prepare(
        self,
        _val: Self::Val,
        _origin: &DispatchOriginOf<RuntimeCallFor<T>>,
        _call: &RuntimeCallFor<T>,
        _info: &DispatchInfoOf<RuntimeCallFor<T>>,
        _len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        Ok(())
    }
}
