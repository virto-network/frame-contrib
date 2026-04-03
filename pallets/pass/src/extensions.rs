use crate::{
    AuthenticatedDevice, Config, CredentialOf, DeviceFilters, Pallet, SpendMatcher, WeightInfo,
};
use codec::{Decode, DecodeWithMemTracking, Encode};
use fc_traits_authn::DeviceId;
use frame_support::{
    dispatch::RawOrigin,
    pallet_prelude::{DispatchResult, TransactionValidityError, Weight},
    CloneNoBound, DebugNoBound, DefaultNoBound, EqNoBound, PartialEqNoBound,
};
use frame_system::{ensure_signed, pallet_prelude::RuntimeCallFor};
use scale_info::TypeInfo;
use sp_core::blake2_256;
use sp_runtime::{
    traits::{
        DispatchInfoOf, DispatchOriginOf, Implication, PostDispatchInfoOf, TransactionExtension,
        ValidateResult,
    },
    transaction_validity::{InvalidTransaction, TransactionSource, ValidTransaction},
};

/// Extract (pallet_index, call_index) from a SCALE-encoded RuntimeCall.
fn call_indices<C: Encode>(call: &C) -> (u8, u8) {
    call.using_encoded(|bytes| {
        if bytes.len() >= 2 {
            (bytes[0], bytes[1])
        } else {
            (0, 0)
        }
    })
}

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
    /// The authenticated device_id, if any.
    type Val = Option<DeviceId>;
    type Pre = Option<DeviceId>;

    fn weight(&self, _call: &RuntimeCallFor<T>) -> Weight {
        T::WeightInfo::authenticate()
    }

    fn validate(
        &self,
        origin: DispatchOriginOf<RuntimeCallFor<T>>,
        call: &RuntimeCallFor<T>,
        _info: &DispatchInfoOf<RuntimeCallFor<T>>,
        _len: usize,
        _self_implicit: Self::Implicit,
        inherited_implication: &impl Implication,
        _source: TransactionSource,
    ) -> ValidateResult<Self::Val, RuntimeCallFor<T>> {
        let (device_id, origin) = if let Some(params) = &self.0 {
            let address = Pallet::<T, I>::authenticate(
                &params.device_id,
                &params.credential,
                &inherited_implication.using_encoded(blake2_256),
            )
            .map_err(|e| {
                log::error!(target: "pallet_pass", "Authentication failed: {:?}", e);
                TransactionValidityError::from(InvalidTransaction::BadSigner)
            })?;

            // Check the device's call filter (missing filter = denied)
            let filter = DeviceFilters::<T, I>::get(&address, &params.device_id)
                .ok_or(TransactionValidityError::from(InvalidTransaction::Call))?;
            if !filter.allows(call_indices(call), T::SpendMatcher::spending_amount(call)) {
                log::error!(target: "pallet_pass", "Device filter rejected call");
                return Err(InvalidTransaction::Call.into());
            }

            Ok::<_, TransactionValidityError>((
                Some(params.device_id),
                RawOrigin::Signed(address).into(),
            ))
        } else {
            // Check if the origin is signed by a session key.
            // Otherwise, pass the origin through unchanged.
            if let Ok(who) = ensure_signed(origin.clone()) {
                if let Some((account, filter)) = Pallet::<T, I>::pass_account_from_session_key(&who)
                {
                    if !filter.allows(call_indices(call), T::SpendMatcher::spending_amount(call)) {
                        return Err(InvalidTransaction::Call.into());
                    }
                    Ok((None, RawOrigin::Signed(account).into()))
                } else {
                    Ok((None, RawOrigin::Signed(who).into()))
                }
            } else {
                Ok((None, origin))
            }
        }?;

        Ok((ValidTransaction::default(), device_id, origin))
    }

    fn prepare(
        self,
        val: Self::Val,
        _origin: &DispatchOriginOf<RuntimeCallFor<T>>,
        _call: &RuntimeCallFor<T>,
        _info: &DispatchInfoOf<RuntimeCallFor<T>>,
        _len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        // Store the authenticated device_id so extrinsics can read it
        // for no-escalation checks.
        if let Some(device_id) = val {
            AuthenticatedDevice::<T, I>::put(device_id);
        }
        Ok(val)
    }

    fn post_dispatch_details(
        pre: Self::Pre,
        _info: &DispatchInfoOf<RuntimeCallFor<T>>,
        _post_info: &PostDispatchInfoOf<RuntimeCallFor<T>>,
        _len: usize,
        _result: &DispatchResult,
    ) -> Result<Weight, TransactionValidityError> {
        // Clear transient storage regardless of success/failure
        if pre.is_some() {
            AuthenticatedDevice::<T, I>::kill();
        }
        Ok(Weight::zero())
    }
}
