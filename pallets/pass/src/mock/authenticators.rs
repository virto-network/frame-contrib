use super::*;
use fc_traits_authn::{util::AuthorityFromPalletId, *};
use frame_support::traits::Randomness;
use sp_core::Get;

pub use authenticator_a::AuthenticatorA;
pub use authenticator_b::AuthenticatorB;

pub struct RandomnessFromBlockNumber;
impl Randomness<H256, u64> for RandomnessFromBlockNumber {
    fn random(subject: &[u8]) -> (H256, u64) {
        let block_number = System::block_number();
        let block_number_as_bytes = block_number.to_le_bytes();
        (
            H256(blake2_256(
                &vec![block_number_as_bytes.to_vec(), subject.to_vec()].concat()[..],
            )),
            block_number,
        )
    }
}

pub(self) type PassAuthorityId = AuthorityFromPalletId<PassPalletId>;

pub mod authenticator_a {
    use super::*;

    pub struct AuthenticatorA;

    #[derive(TypeInfo, DebugNoBound, EqNoBound, PartialEq, Clone, Encode, Decode)]
    pub struct DeviceAttestation {
        pub(crate) device_id: DeviceId,
        pub(crate) challenge: Challenge,
    }

    #[derive(TypeInfo, Encode, Decode, MaxEncodedLen)]
    pub struct Device {
        pub(crate) device_id: DeviceId,
    }

    #[derive(
        TypeInfo, DebugNoBound, EqNoBound, PartialEq, Clone, Encode, Decode, MaxEncodedLen,
    )]
    pub struct Credential {
        pub(crate) user_id: HashedUserId,
        pub(crate) challenge: Challenge,
    }

    impl Authenticator for AuthenticatorA {
        type Authority = PassAuthorityId;
        type Challenger = Self;
        type DeviceAttestation = DeviceAttestation;
        type Device = Device;

        fn unpack_device(attestation: Self::DeviceAttestation) -> Self::Device {
            Device {
                device_id: attestation.device_id,
            }
        }
    }

    impl Challenger for AuthenticatorA {
        type Context = ();

        fn generate(_: &Self::Context) -> Challenge {
            let (hash, _) = RandomnessFromBlockNumber::random_seed();
            hash.0
        }
    }

    impl UserAuthenticator for Device {
        type Authority = PassAuthorityId;
        type Challenger = AuthenticatorA;
        type Credential = Credential;

        fn device_id(&self) -> &DeviceId {
            &self.device_id
        }
    }

    impl DeviceChallengeResponse<()> for DeviceAttestation {
        fn is_valid(&self) -> bool {
            true
        }

        fn used_challenge(&self) -> ((), Challenge) {
            ((), self.challenge)
        }

        fn authority(&self) -> AuthorityId {
            PassAuthorityId::get()
        }

        fn device_id(&self) -> &DeviceId {
            &self.device_id
        }
    }

    impl UserChallengeResponse<()> for Credential {
        fn is_valid(&self) -> bool {
            true
        }

        fn used_challenge(&self) -> ((), Challenge) {
            ((), self.challenge)
        }

        fn authority(&self) -> AuthorityId {
            PassAuthorityId::get()
        }

        fn user_id(&self) -> HashedUserId {
            self.user_id
        }
    }
}

pub mod authenticator_b {
    use super::*;

    pub struct AuthenticatorB;

    #[derive(TypeInfo, DebugNoBound, EqNoBound, PartialEq, Clone, Encode, Decode)]
    pub struct DeviceAttestation {
        pub(crate) device_id: DeviceId,
        pub(crate) challenge: Challenge,
    }

    #[derive(TypeInfo, Encode, Decode, MaxEncodedLen)]
    pub struct Device {
        pub(crate) device_id: DeviceId,
    }

    #[derive(
        TypeInfo, DebugNoBound, EqNoBound, PartialEq, Clone, Encode, Decode, MaxEncodedLen,
    )]
    pub struct Credential {
        pub(crate) device_id: DeviceId,
        pub(crate) user_id: HashedUserId,
        pub(crate) challenge: Challenge,
    }

    impl Authenticator for AuthenticatorB {
        type Authority = PassAuthorityId;
        type Challenger = Self;
        type DeviceAttestation = DeviceAttestation;
        type Device = Device;

        fn unpack_device(attestation: Self::DeviceAttestation) -> Self::Device {
            Device {
                device_id: attestation.device_id,
            }
        }
    }

    impl Challenger for AuthenticatorB {
        type Context = DeviceId;

        fn generate(context: &Self::Context) -> Challenge {
            let (hash, _) = RandomnessFromBlockNumber::random(context);
            hash.0
        }
    }

    impl UserAuthenticator for Device {
        type Authority = PassAuthorityId;
        type Challenger = AuthenticatorB;
        type Credential = Credential;

        fn device_id(&self) -> &DeviceId {
            &self.device_id
        }
    }

    impl DeviceChallengeResponse<DeviceId> for DeviceAttestation {
        fn is_valid(&self) -> bool {
            true
        }

        fn used_challenge(&self) -> (DeviceId, Challenge) {
            (self.device_id, self.challenge)
        }

        fn authority(&self) -> AuthorityId {
            PassAuthorityId::get()
        }

        fn device_id(&self) -> &DeviceId {
            &self.device_id
        }
    }

    impl UserChallengeResponse<DeviceId> for Credential {
        fn is_valid(&self) -> bool {
            true
        }

        fn used_challenge(&self) -> (DeviceId, Challenge) {
            (self.device_id, self.challenge)
        }

        fn authority(&self) -> AuthorityId {
            PassAuthorityId::get()
        }

        fn user_id(&self) -> HashedUserId {
            self.user_id
        }
    }
}
