use super::*;
use fc_traits_authn::{util::AuthorityFromPalletId, *};
use frame_support::traits::Randomness;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_core::Get;

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
    use super::{Authenticator as TAuthenticator, *};
    use codec::DecodeWithMemTracking;

    pub struct Authenticator;

    #[derive(
        TypeInfo, DebugNoBound, EqNoBound, PartialEq, Clone, Encode, Decode, DecodeWithMemTracking,
    )]
    pub struct DeviceAttestation {
        pub(crate) device_id: DeviceId,
        pub(crate) challenge: Challenge,
    }

    #[derive(TypeInfo, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen)]
    pub struct Device {
        pub(crate) device_id: DeviceId,
    }

    #[derive(
        TypeInfo,
        DebugNoBound,
        EqNoBound,
        PartialEq,
        Clone,
        Encode,
        Decode,
        DecodeWithMemTracking,
        Default,
    )]
    pub struct Credential {
        pub(crate) user_id: HashedUserId,
        pub(crate) challenge: Challenge,
    }

    impl TAuthenticator for Authenticator {
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

    impl Challenger for Authenticator {
        type Context = ();

        fn generate(_: &Self::Context, _: &impl ExtrinsicContext) -> Challenge {
            let (hash, _) = RandomnessFromBlockNumber::random_seed();
            hash.0
        }
    }

    impl UserAuthenticator for Device {
        type Authority = PassAuthorityId;
        type Challenger = Authenticator;
        type Credential = Credential;

        // Note: This authenticator should pass intentionally, to pass on simpler tests
        fn verify_credential(&mut self, _: &Self::Credential) -> Option<()> {
            Some(())
        }

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

#[derive(Clone)]
pub struct LastThreeBlocksChallenger;
impl Challenger for LastThreeBlocksChallenger {
    type Context = BlockNumberFor<Test>;

    fn generate(context: &Self::Context, xtc: &impl ExtrinsicContext) -> Challenge {
        let (hash, _) = RandomnessFromBlockNumber::random(&(context, xtc.as_ref()).encode());
        hash.0
    }

    fn check_challenge(
        cx: &Self::Context,
        xtc: &impl ExtrinsicContext,
        challenge: &[u8],
    ) -> Option<()> {
        let block_number = System::block_number();
        let range = block_number.saturating_sub(3)..=block_number;
        (range.contains(cx) && challenge.eq(&Self::generate(cx, xtc))).then_some(())
    }
}

pub mod authenticator_b {
    use super::*;
    use codec::DecodeWithMemTracking;
    use core::marker::PhantomData;
    use frame_support::Parameter;

    type CxOf<Ch> = <Ch as Challenger>::Context;

    pub struct AuthenticatorB<C>(PhantomData<C>);

    #[derive(TypeInfo, Debug, Eq, PartialEq, Clone, Encode, Decode, DecodeWithMemTracking)]
    pub struct DeviceAttestation<Cx> {
        pub(crate) device_id: DeviceId,
        pub(crate) context: Cx,
        pub(crate) challenge: Challenge,
    }

    #[derive(TypeInfo, Encode, Decode, DecodeWithMemTracking, MaxEncodedLen)]
    #[scale_info(skip_type_params(C))]
    pub struct Device<C> {
        pub(crate) device_id: DeviceId,
        pub(crate) nonce: u32,
        _data: PhantomData<C>,
    }

    #[derive(
        TypeInfo, Debug, Eq, PartialEq, Clone, Encode, Decode, DecodeWithMemTracking, Default,
    )]
    pub struct Credential<Cx> {
        pub(crate) user_id: HashedUserId,
        pub(crate) context: Cx,
        pub(crate) challenge: Challenge,
        pub(crate) nonce: u32,
        pub(crate) signature: Option<[u8; 32]>,
    }

    impl<Cx> Credential<Cx> {
        pub fn new(user_id: HashedUserId, context: Cx, nonce: u32, challenge: Challenge) -> Self {
            Self {
                user_id,
                context,
                challenge,
                nonce,
                signature: None,
            }
        }

        pub fn sign(self, signer: &DeviceId) -> Self {
            Self {
                signature: Some(Self::signature(signer, &self)),
                ..self
            }
        }

        // A dummy "signature", to test the signing capabilities
        pub fn signature(signer: &DeviceId, credential: &Self) -> [u8; 32] {
            blake2_256(
                &(
                    signer,
                    credential.user_id,
                    credential.nonce,
                    credential.challenge,
                )
                    .encode(),
            )
        }
    }

    impl<C: Challenger + 'static> Authenticator for AuthenticatorB<C>
    where
        CxOf<C>: Parameter + Send + Sync + Copy + Default + 'static,
    {
        type Authority = PassAuthorityId;
        type Challenger = C;
        type DeviceAttestation = DeviceAttestation<CxOf<C>>;
        type Device = Device<C>;

        fn unpack_device(attestation: Self::DeviceAttestation) -> Self::Device {
            Device {
                device_id: attestation.device_id,
                nonce: 0,
                _data: PhantomData,
            }
        }
    }

    impl<C> UserAuthenticator for Device<C>
    where
        C: Challenger + 'static,
        CxOf<C>: Parameter + Send + Sync + Copy + Default + 'static,
    {
        type Authority = PassAuthorityId;
        type Challenger = C;
        type Credential = Credential<CxOf<C>>;

        fn verify_credential(&mut self, credential: &Self::Credential) -> Option<()> {
            credential.signature.and_then(|signature| {
                (Credential::signature(self.device_id(), &credential).eq(&signature)
                    && credential.nonce == self.nonce)
                    .then(|| {
                        self.nonce = self.nonce + 1;
                    })
            })
        }

        fn device_id(&self) -> &DeviceId {
            &self.device_id
        }
    }

    impl<Cx> DeviceChallengeResponse<Cx> for DeviceAttestation<Cx>
    where
        Cx: Parameter + Copy + 'static,
    {
        fn is_valid(&self) -> bool {
            true
        }

        fn used_challenge(&self) -> (Cx, Challenge) {
            (self.context, self.challenge)
        }

        fn authority(&self) -> AuthorityId {
            PassAuthorityId::get()
        }

        fn device_id(&self) -> &DeviceId {
            &self.device_id
        }
    }

    impl<Cx> UserChallengeResponse<Cx> for Credential<Cx>
    where
        Cx: Parameter + Copy + Default + 'static,
    {
        fn is_valid(&self) -> bool {
            self.signature.is_some()
        }

        fn used_challenge(&self) -> (Cx, Challenge) {
            (self.context, self.challenge)
        }

        fn authority(&self) -> AuthorityId {
            PassAuthorityId::get()
        }

        fn user_id(&self) -> HashedUserId {
            self.user_id
        }
    }
}
