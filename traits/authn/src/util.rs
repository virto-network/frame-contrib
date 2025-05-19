use core::marker::PhantomData;

use codec::{Decode, DecodeWithMemTracking, Encode, FullCodec, MaxEncodedLen};
use frame_support::{sp_runtime::traits::TrailingZeroInput, traits::Get, PalletId};
use scale_info::TypeInfo;

use crate::{
    Authenticator, AuthorityId, Challenger, CxOf, DeviceChallengeResponse, DeviceId,
    UserAuthenticator, UserChallengeResponse,
};

type ChallengerOf<Dev> = <Dev as UserAuthenticator>::Challenger;

#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, Debug)]
#[scale_info(skip_type_params(Id))]
pub struct AuthorityFromPalletId<Id>(PhantomData<Id>);

impl<Id: Get<PalletId>> Get<AuthorityId> for AuthorityFromPalletId<Id> {
    fn get() -> AuthorityId {
        Decode::decode(&mut TrailingZeroInput::new(&Id::get().0))
            .expect("Size of PalletId is less than 32 bytes; qed")
    }
}

#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, Debug)]
#[scale_info(skip_type_params(Dev, Att))]
/// Convenient auto-implementor of the Authenticator trait
pub struct Auth<Dev, Att>(PhantomData<(Dev, Att)>);

impl<Dev, Att> Authenticator for Auth<Dev, Att>
where
    Att: DeviceChallengeResponse<CxOf<ChallengerOf<Dev>>>,
    Dev: UserAuthenticator + From<Att>,
{
    type Authority = <Dev as UserAuthenticator>::Authority;
    type Challenger = ChallengerOf<Dev>;
    type DeviceAttestation = Att;
    type Device = Dev;

    fn unpack_device(attestation: Self::DeviceAttestation) -> Self::Device {
        attestation.into()
    }
}

pub trait VerifyCredential<Cred> {
    fn verify(&self, credential: &Cred) -> Option<()>;
}

/// Convenient auto-implemtator of the UserAuthenticator trait
#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, Debug)]
#[scale_info(skip_type_params(A, Ch, Cred))]
pub struct Dev<T, A, Ch, Cred>(T, PhantomData<(A, Ch, Cred)>);

impl<T, A, Ch, Cred> Dev<T, A, Ch, Cred> {
    pub fn new(t: T) -> Self {
        Self(t, PhantomData)
    }
}

impl<T, A, Ch, Cred> UserAuthenticator for Dev<T, A, Ch, Cred>
where
    T: VerifyCredential<Cred> + AsRef<DeviceId> + FullCodec + MaxEncodedLen + TypeInfo + 'static,
    A: Get<AuthorityId> + 'static,
    Ch: Challenger + 'static,
    Cred: UserChallengeResponse<Ch::Context> + 'static + Send + Sync,
{
    type Authority = A;
    type Challenger = Ch;
    type Credential = Cred;

    fn verify_credential(&self, credential: &Self::Credential) -> Option<()> {
        self.0.verify(credential)
    }

    fn device_id(&self) -> &DeviceId {
        self.0.as_ref()
    }
}

impl<T: MaxEncodedLen, A, Ch, Cr> MaxEncodedLen for Dev<T, A, Ch, Cr> {
    fn max_encoded_len() -> usize {
        T::max_encoded_len()
    }
}

pub mod dummy {
    use super::*;
    use frame_support::{
        parameter_types, sp_runtime::str_array as s, CloneNoBound, DebugNoBound, EqNoBound,
        PartialEqNoBound,
    };
    use scale_info::TypeInfo;

    use crate::{
        AuthorityId, Challenger, DeviceChallengeResponse, DeviceId, ExtrinsicContext, HashedUserId,
        UserChallengeResponse,
    };

    use super::{Auth, Dev, VerifyCredential};

    #[derive(
        Default,
        CloneNoBound,
        DebugNoBound,
        Encode,
        Decode,
        DecodeWithMemTracking,
        MaxEncodedLen,
        TypeInfo,
        PartialEqNoBound,
        EqNoBound,
    )]
    #[scale_info(skip_type_params(A))]
    pub struct DummyAttestation<A>(bool, Option<DeviceId>, PhantomData<A>);

    impl<A> DummyAttestation<A> {
        pub fn new(value: bool, device_id: DeviceId) -> Self {
            Self(value, Some(device_id), PhantomData)
        }
    }

    #[derive(
        CloneNoBound,
        DebugNoBound,
        Encode,
        Decode,
        DecodeWithMemTracking,
        MaxEncodedLen,
        TypeInfo,
        PartialEqNoBound,
        EqNoBound,
    )]
    #[scale_info(skip_type_params(A))]
    pub struct DummyCredential<A>(bool, HashedUserId, PhantomData<A>);

    impl<A> DummyCredential<A> {
        pub fn new(value: bool, user_id: HashedUserId) -> Self {
            Self(value, user_id, PhantomData)
        }
    }

    type DummyChallenger = u8;
    type DummyCx = <DummyChallenger as Challenger>::Context;

    parameter_types! {
        const DummyAuthority: AuthorityId = s("dummy_authority");
    }
    pub const DUMMY_DEV: DeviceId = s("dummy_device");
    pub const DUMMY_USER: HashedUserId = s("dummy_user_hash");

    impl Challenger for DummyChallenger {
        type Context = Self;
        fn generate(cx: &Self::Context, _xtc: &impl ExtrinsicContext) -> crate::Challenge {
            [*cx; 32]
        }
    }

    pub type DummyDev<AuthorityId> = Dev<
        DummyAttestation<AuthorityId>,
        AuthorityId,
        DummyChallenger,
        DummyCredential<AuthorityId>,
    >;
    pub type Dummy<AuthorityId> = Auth<DummyDev<AuthorityId>, DummyAttestation<AuthorityId>>;

    impl<A> From<DummyAttestation<A>> for DummyDev<A> {
        fn from(value: DummyAttestation<A>) -> Self {
            DummyDev::new(value)
        }
    }

    impl<A> AsRef<DeviceId> for DummyAttestation<A> {
        fn as_ref(&self) -> &DeviceId {
            self.1.as_ref().unwrap_or(&DUMMY_DEV)
        }
    }

    impl<A> VerifyCredential<DummyCredential<A>> for DummyAttestation<A> {
        fn verify(&self, _: &DummyCredential<A>) -> Option<()> {
            self.0.then_some(())
        }
    }

    impl<A> DeviceChallengeResponse<DummyCx> for DummyAttestation<A>
    where
        A: Get<AuthorityId> + 'static,
    {
        fn is_valid(&self) -> bool {
            self.0
        }

        fn used_challenge(&self) -> (DummyCx, crate::Challenge) {
            (0, [0; 32])
        }
        fn authority(&self) -> AuthorityId {
            A::get()
        }
        fn device_id(&self) -> &DeviceId {
            self.1.as_ref().unwrap_or(&DUMMY_DEV)
        }
    }

    impl<A> UserChallengeResponse<DummyCx> for DummyCredential<A>
    where
        A: Get<AuthorityId> + 'static,
    {
        fn is_valid(&self) -> bool {
            self.0
        }

        fn used_challenge(&self) -> (DummyCx, crate::Challenge) {
            (0, [0; 32])
        }

        fn authority(&self) -> AuthorityId {
            A::get()
        }

        fn user_id(&self) -> HashedUserId {
            self.1
        }
    }
}
