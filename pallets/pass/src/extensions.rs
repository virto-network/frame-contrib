use crate::{
    AuthenticatedDevice, CallMatcher, Config, CredentialOf, DeviceFilters, Pallet, SpendMatcher,
    WeightInfo,
};
use codec::{Decode, DecodeWithMemTracking, Encode};
use fc_traits_authn::DeviceId;
use frame_support::{
    dispatch::RawOrigin,
    pallet_prelude::{DispatchResult, TransactionValidityError, Weight},
    traits::Get,
    CloneNoBound, DebugNoBound, DefaultNoBound, EqNoBound, PartialEqNoBound,
};
use frame_system::{ensure_signed, pallet_prelude::RuntimeCallFor};
use scale_info::TypeInfo;
use sp_io::hashing::blake2_256;
use sp_runtime::{
    traits::{
        DispatchInfoOf, DispatchOriginOf, Implication, PostDispatchInfoOf, TransactionExtension,
        ValidateResult,
    },
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
    /// The authenticated `(account, device_id)`, if any, and the part of
    /// [`weight`][Self::weight] that `validate` did not spend.
    type Val = (Option<(T::AccountId, DeviceId)>, Weight);
    type Pre = (Option<(T::AccountId, DeviceId)>, Weight);

    /// Charges only for the branch this extension will actually take:
    ///
    /// - with a credential, the pallet's own authentication overhead;
    /// - without one, the (much cheaper) session key lookup.
    ///
    /// Whatever `validate` ends up not spending is refunded in
    /// [`post_dispatch_details`][Self::post_dispatch_details].
    fn weight(&self, _call: &RuntimeCallFor<T>) -> Weight {
        match &self.0 {
            Some(_) => T::WeightInfo::authenticate(),
            None => T::WeightInfo::authenticate_none(),
        }
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
            if !filter.allows(
                T::CallMatcher::call_indices(call),
                T::SpendMatcher::spending_amount(call),
            ) {
                log::error!(target: "pallet_pass", "Device filter rejected call");
                return Err(InvalidTransaction::Call.into());
            }

            Ok::<_, TransactionValidityError>((
                (Some((address.clone(), params.device_id)), Weight::zero()),
                RawOrigin::Signed(address).into(),
            ))
        } else {
            // Check if the origin is signed by a session key.
            // Otherwise, pass the origin through unchanged.
            if let Ok(who) = ensure_signed(origin.clone()) {
                if let Some((account, filter)) = Pallet::<T, I>::pass_account_from_session_key(&who)
                {
                    if !filter.allows(
                        T::CallMatcher::call_indices(call),
                        T::SpendMatcher::spending_amount(call),
                    ) {
                        return Err(InvalidTransaction::Call.into());
                    }
                    Ok(((None, Weight::zero()), RawOrigin::Signed(account).into()))
                } else {
                    Ok(((None, Weight::zero()), RawOrigin::Signed(who).into()))
                }
            } else {
                // Not signed: the session key lookup never happened.
                Ok(((None, Self::unsigned_refund()), origin))
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
        // Defense-in-depth: clear any stale authentication context from a
        // previous transaction whose `post_dispatch_details` may have failed
        // to run (e.g. due to a node panic mid-dispatch).
        AuthenticatedDevice::<T, I>::kill();

        // Store the authenticated (account, device_id) so extrinsics can
        // read it for no-escalation checks.
        if let (Some(ref auth), _) = val {
            AuthenticatedDevice::<T, I>::put(auth);
        }
        Ok(val)
    }

    fn post_dispatch_details(
        (device, unspent): Self::Pre,
        _info: &DispatchInfoOf<RuntimeCallFor<T>>,
        _post_info: &PostDispatchInfoOf<RuntimeCallFor<T>>,
        _len: usize,
        _result: &DispatchResult,
    ) -> Result<Weight, TransactionValidityError> {
        // Clear transient storage regardless of success/failure.
        if device.is_some() {
            AuthenticatedDevice::<T, I>::kill();
        }
        Ok(unspent)
    }
}

impl<T, I> PassAuthenticate<T, I>
where
    T: Config<I>,
    I: 'static,
{
    /// The part of [`WeightInfo::authenticate_none`] that an unsigned origin
    /// does not spend: the `SessionKeys` read (and its proof), which only
    /// happens for signed origins.
    fn unsigned_refund() -> Weight {
        let charged = T::WeightInfo::authenticate_none();
        Weight::from_parts(T::DbWeight::get().reads(1).ref_time(), charged.proof_size())
            .min(charged)
    }
}
