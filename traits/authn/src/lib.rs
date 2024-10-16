#![cfg_attr(not(feature = "std"), no_std)]

use codec::{FullCodec, MaxEncodedLen};
use frame_support::{traits::Get, Parameter};
use scale_info::TypeInfo;

pub mod util;

pub use fc_traits_authn_proc::composite_authenticator;

pub mod composite_prelude {
    pub use crate::{
        Authenticator, AuthorityId, Challenge, Challenger, DeviceChallengeResponse, DeviceId,
        HashedUserId, UserAuthenticator, UserChallengeResponse,
    };
    pub use frame_support::traits::Get;
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

// A reasonabily sized secure challenge
const CHALLENGE_SIZE: usize = 32;
pub type Challenge = [u8; CHALLENGE_SIZE];
type CxOf<C> = <C as Challenger>::Context;

pub type DeviceId = [u8; 32];
pub type AuthorityId = [u8; 32];
pub type HashedUserId = [u8; 32];

/// Given some context it deterministically generates a "challenge" used by authenticators
pub trait Challenger {
    type Context;

    fn generate(cx: &Self::Context) -> Challenge;

    /// Ensure that given the context produces the same challenge
    fn check_challenge(cx: &Self::Context, challenge: &[u8]) -> Option<()> {
        Self::generate(cx).eq(challenge).then_some(())
    }
}

/// Authenticator is used to verify authentication devices that in turn are used to verify users
pub trait Authenticator {
    type Authority: Get<AuthorityId>;
    type Challenger: Challenger;
    type DeviceAttestation: DeviceChallengeResponse<CxOf<Self::Challenger>>;
    type Device: UserAuthenticator<Challenger = Self::Challenger>;

    fn verify_device(attestation: Self::DeviceAttestation) -> Option<Self::Device> {
        attestation
            .authority()
            .eq(&Self::Authority::get())
            .then_some(())?;
        let (cx, challenge) = attestation.used_challenge();
        Self::Challenger::check_challenge(&cx, &challenge)?;
        attestation.is_valid().then_some(())?;
        Some(Self::unpack_device(attestation))
    }

    /// Extract device information from the attestation payload
    fn unpack_device(attestation: Self::DeviceAttestation) -> Self::Device;
}

/// A device capable of verifying a user provided credential
pub trait UserAuthenticator: FullCodec + MaxEncodedLen + TypeInfo {
    type Authority: Get<AuthorityId>;
    type Challenger: Challenger;
    type Credential: UserChallengeResponse<CxOf<Self::Challenger>>;

    fn verify_user(&self, credential: &Self::Credential) -> Option<()> {
        credential
            .authority()
            .eq(&Self::Authority::get())
            .then_some(())?;
        let (cx, challenge) = credential.used_challenge();
        Self::Challenger::check_challenge(&cx, &challenge)?;
        credential.is_valid().then_some(())?;
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
