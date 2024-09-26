use codec::{FullCodec, MaxEncodedLen};
use frame_support::Parameter;
use scale_info::TypeInfo;

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
    const AUTHORITY: AuthorityId;
    type Challenger: Challenger;
    type DeviceAttestation: DeviceChallengeResponse<CxOf<Self::Challenger>>;
    type Device: UserAuthenticator<Challenger = Self::Challenger>;

    fn verify_device(&self, attestation: &Self::DeviceAttestation) -> Option<Self::Device> {
        attestation.authority().eq(&Self::AUTHORITY).then_some(())?;
        let (cx, challenge) = attestation.used_challenge();
        Self::Challenger::check_challenge(&cx, &challenge)?;
        attestation.is_valid().then_some(())?;
        Some(self.unpack_device(attestation))
    }

    /// Extract device information from the verification payload
    fn unpack_device(&self, verification: &Self::DeviceAttestation) -> Self::Device;
}

/// A device capable of verifying a user provided credential
pub trait UserAuthenticator: FullCodec + MaxEncodedLen + TypeInfo {
    const AUTHORITY: AuthorityId;
    type Challenger: Challenger;
    type Credential: UserChallengeResponse<CxOf<Self::Challenger>>;

    fn verify_user(&self, credential: &Self::Credential) -> Option<()> {
        credential.authority().eq(&Self::AUTHORITY).then_some(())?;
        let (cx, challenge) = credential.used_challenge();
        Self::Challenger::check_challenge(&cx, &challenge)?;
        credential.is_valid().then_some(())
    }

    fn device_id(&self) -> DeviceId;
}

pub trait ChallengeResponse<Cx>: Parameter {
    fn is_valid(&self) -> bool;
    fn used_challenge(&self) -> (Cx, Challenge);
    fn authority(&self) -> AuthorityId;
}

/// A response to a challenge for creating a new authentication device
pub trait DeviceChallengeResponse<Cx>: ChallengeResponse<Cx> {
    fn device_id(&self) -> DeviceId;
}

/// A response to a challenge for identifying a user
pub trait UserChallengeResponse<Cx>: ChallengeResponse<Cx> {
    fn user_id(&self) -> HashedUserId;
}
