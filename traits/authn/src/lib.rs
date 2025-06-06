#![cfg_attr(not(feature = "std"), no_std)]

use codec::{FullCodec, MaxEncodedLen};
use frame_support::{traits::Get, Parameter};
use scale_info::TypeInfo;

pub mod util;

pub use fc_traits_authn_proc::composite_authenticator;

const LOG_TARGET: &str = "authn";

pub mod prelude {
    pub use crate::{
        Authenticator, AuthorityId, Challenge, Challenger, DeviceChallengeResponse, DeviceId,
        ExtrinsicContext, HashedUserId, UserAuthenticator, UserChallengeResponse,
    };
    pub use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
    pub use frame_support::{
        pallet_prelude::TypeInfo, traits::Get, DebugNoBound, EqNoBound, Parameter,
    };
}

#[macro_export]
macro_rules! composite_authenticators {
    // Match a single composite authenticator with the format:
    // pub CompositePassA<AuthorityA> { AuthA, AuthB };
    ($(
        pub $name:path {
            $($auth:path),* $(,)?
        };
    )*) => {
        $(
            $crate::composite_authenticator!(
                pub $name {
                    $($auth),*
                }
            );
        )*
    }
}

// A reasonably sized secure challenge
const CHALLENGE_SIZE: usize = 32;
pub type Challenge = [u8; CHALLENGE_SIZE];
type CxOf<C> = <C as Challenger>::Context;

pub type DeviceId = [u8; 32];
pub type AuthorityId = [u8; 32];
pub const HASHED_USER_ID_LEN: usize = 32;
pub type HashedUserId = [u8; HASHED_USER_ID_LEN];

/// An extrinsic context, provided by the Authenticator verification call. The type cannot be
/// known by the challenger implementation, so the content would be handled as a slice of bytes.
pub trait ExtrinsicContext: AsRef<[u8]> + core::fmt::Debug {}
impl<T> ExtrinsicContext for T where T: AsRef<[u8]> + core::fmt::Debug {}

/// Given some context it deterministically generates a "challenge" used by authenticators
pub trait Challenger {
    type Context: Parameter;

    fn generate(cx: &Self::Context, xtc: &impl ExtrinsicContext) -> Challenge;

    /// Ensure that given the context produces the same challenge
    fn check_challenge(
        cx: &Self::Context,
        xtc: &impl ExtrinsicContext,
        challenge: &[u8],
    ) -> Option<()> {
        Self::generate(cx, xtc).eq(challenge).then_some(())
    }
}

/// Authenticator is used to verify authentication devices that in turn are used to verify users
pub trait Authenticator {
    type Authority: Get<AuthorityId>;
    type Challenger: Challenger;
    type DeviceAttestation: DeviceChallengeResponse<CxOf<Self::Challenger>>;
    type Device: UserAuthenticator<Challenger = Self::Challenger>;

    fn verify_device(
        attestation: Self::DeviceAttestation,
        xtc: &impl ExtrinsicContext,
    ) -> Option<Self::Device> {
        log::trace!(target: LOG_TARGET, "Verifying device with attestation: {:?}", attestation);

        log::trace!(target: LOG_TARGET, "Assert authority {:?}", attestation.authority());
        attestation
            .authority()
            .eq(&Self::Authority::get())
            .then_some(())?;
        log::trace!(target: LOG_TARGET, "Authority verified");

        let (cx, challenge) = &attestation.used_challenge();
        log::trace!(target: LOG_TARGET, "Check challenge {:?} (with cx={cx:?}, xtc={xtc:?})", challenge);
        Self::Challenger::check_challenge(cx, xtc, challenge)?;
        log::trace!(target: LOG_TARGET, "Challenge checked");

        log::trace!(target: LOG_TARGET, "Validate attestation");
        attestation.is_valid().then_some(())?;

        log::trace!(target: LOG_TARGET, "Retrieve device");
        Some(Self::unpack_device(attestation))
    }

    /// Extract device information from the attestation payload
    fn unpack_device(attestation: Self::DeviceAttestation) -> Self::Device;
}

/// A device capable of verifying a user provided credential
pub trait UserAuthenticator: FullCodec + MaxEncodedLen + TypeInfo {
    type Authority: Get<AuthorityId>;
    type Challenger: Challenger;
    type Credential: UserChallengeResponse<CxOf<Self::Challenger>> + Send + Sync;

    fn verify_user(
        &self,
        credential: &Self::Credential,
        xtc: &impl ExtrinsicContext,
    ) -> Option<()> {
        log::trace!(target: LOG_TARGET, "Verifying user for credential: {:?}", credential);

        log::trace!(target: LOG_TARGET, "Assert authority {:?}", credential.authority());
        credential
            .authority()
            .eq(&Self::Authority::get())
            .then_some(())?;
        log::trace!(target: LOG_TARGET, "Authority verified");

        let (cx, challenge) = &credential.used_challenge();
        log::trace!(target: LOG_TARGET, "Check challenge {:?}", challenge);
        Self::Challenger::check_challenge(cx, xtc, challenge)?;
        log::trace!(target: LOG_TARGET, "Challenge checked");

        log::trace!(target: LOG_TARGET, "Credential verified");
        credential.is_valid().then_some(())?;

        log::trace!(target: LOG_TARGET, "Verify credential");
        self.verify_credential(credential)
    }

    fn verify_credential(&self, credential: &Self::Credential) -> Option<()>;

    fn device_id(&self) -> &DeviceId;
}

/// A response to a challenge for creating a new authentication device
pub trait DeviceChallengeResponse<Cx>: Parameter {
    fn is_valid(&self) -> bool;
    fn used_challenge(&self) -> (Cx, Challenge);
    fn authority(&self) -> AuthorityId;
    fn device_id(&self) -> &DeviceId;
}

/// A response to a challenge for identifying a user
pub trait UserChallengeResponse<Cx>: Parameter {
    fn is_valid(&self) -> bool;
    fn used_challenge(&self) -> (Cx, Challenge);
    fn authority(&self) -> AuthorityId;
    fn user_id(&self) -> HashedUserId;
}
