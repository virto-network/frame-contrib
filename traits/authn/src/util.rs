use core::marker::PhantomData;

use codec::{Decode, Encode, FullCodec, MaxEncodedLen};
use frame_support::{sp_runtime::traits::TrailingZeroInput, traits::Get, PalletId};
use scale_info::TypeInfo;

use crate::{
    Authenticator, AuthorityId, Challenger, CxOf, DeviceChallengeResponse, DeviceId,
    UserAuthenticator, UserChallengeResponse,
};

type ChallengerOf<Dev> = <Dev as UserAuthenticator>::Challenger;

pub struct AuthorityFromPalletId<Id>(PhantomData<Id>);

impl<Id: Get<PalletId>> Get<AuthorityId> for AuthorityFromPalletId<Id> {
    fn get() -> AuthorityId {
        Decode::decode(&mut TrailingZeroInput::new(&Id::get().0))
            .expect("Size of PalletId is less than 32 bytes; qed")
    }
}

/// Convenient auto-implemtator of the Authenticator trait
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
#[derive(Encode, Decode, TypeInfo)]
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
    Cred: UserChallengeResponse<Ch::Context> + 'static,
{
    type Authority = A;
    type Challenger = Ch;
    type Credential = Cred;

    fn device_id(&self) -> &DeviceId {
        self.0.as_ref()
    }

    fn verify_credential(&self, credential: &Self::Credential) -> Option<()> {
        self.0.verify(credential)
    }
}

impl<T: MaxEncodedLen, A, Ch, Cr> MaxEncodedLen for Dev<T, A, Ch, Cr> {
    fn max_encoded_len() -> usize {
        T::max_encoded_len()
    }
}

// TODO implement here
mod pass_key {
    use codec::{Decode, Encode};
    use scale_info::TypeInfo;

    use super::{Auth, Dev};
    use crate::{DeviceChallengeResponse, DeviceId, UserChallengeResponse};

    #[allow(dead_code)]
    pub type PassKey<A> = Dev<(), A, (), PassKeyAssertion>;
    #[allow(dead_code)]
    pub type PassKeyManager<A> = Auth<PassKey<A>, PassKeyAttestation>;

    #[derive(Clone, Debug, Decode, Encode, TypeInfo, PartialEq, Eq)]
    pub struct PassKeyAttestation;

    impl<Cx> DeviceChallengeResponse<Cx> for PassKeyAttestation {
        fn is_valid(&self) -> bool {
            todo!()
        }

        fn used_challenge(&self) -> (Cx, crate::Challenge) {
            todo!()
        }

        fn authority(&self) -> crate::AuthorityId {
            todo!()
        }

        fn device_id(&self) -> &DeviceId {
            todo!()
        }
    }

    #[derive(Clone, Debug, Encode, Decode, TypeInfo, PartialEq, Eq)]
    pub struct PassKeyAssertion;

    impl<Cx> UserChallengeResponse<Cx> for PassKeyAssertion {
        fn is_valid(&self) -> bool {
            todo!()
        }

        fn used_challenge(&self) -> (Cx, crate::Challenge) {
            todo!()
        }

        fn authority(&self) -> crate::AuthorityId {
            todo!()
        }

        fn user_id(&self) -> crate::HashedUserId {
            todo!()
        }
    }
}

pub mod dummy {
    use core::marker::PhantomData;

    use codec::{Decode, Encode, MaxEncodedLen};
    use frame_support::{
        parameter_types, sp_runtime::str_array as s, traits::Get, DebugNoBound, EqNoBound,
        PartialEqNoBound,
    };
    use scale_info::TypeInfo;

    use crate::{
        AuthorityId, Challenger, DeviceChallengeResponse, DeviceId, HashedUserId,
        UserChallengeResponse,
    };

    use super::{Auth, Dev, VerifyCredential};

    #[derive(
        PartialEqNoBound, EqNoBound, DebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo,
    )]
    #[scale_info(skip_type_params(A))]
    pub struct DummyAttestation<A>(bool, PhantomData<A>);

    impl<A> DummyAttestation<A> {
        pub fn new(value: bool) -> Self {
            Self(value, PhantomData)
        }
    }

    impl<A> Clone for DummyAttestation<A> {
        fn clone(&self) -> Self {
            Self(self.0.clone(), PhantomData)
        }
    }

    #[derive(
        PartialEqNoBound, EqNoBound, DebugNoBound, Encode, Decode, MaxEncodedLen, TypeInfo,
    )]
    #[scale_info(skip_type_params(A))]
    pub struct DummyCredential<A>(bool, PhantomData<A>);

    impl<A> Clone for DummyCredential<A> {
        fn clone(&self) -> Self {
            Self(self.0.clone(), PhantomData)
        }
    }

    impl<A> DummyCredential<A> {
        pub fn new(value: bool) -> Self {
            Self(value, PhantomData)
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
        fn generate(cx: &Self::Context) -> crate::Challenge {
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
            &DUMMY_DEV
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
        fn device_id(&self) -> &DeviceId {
            &DUMMY_DEV
        }

        fn is_valid(&self) -> bool {
            self.0
        }
        fn used_challenge(&self) -> (DummyCx, crate::Challenge) {
            (0, [0; 32])
        }
        fn authority(&self) -> crate::AuthorityId {
            A::get()
        }
    }

    impl<A> UserChallengeResponse<DummyCx> for DummyCredential<A>
    where
        A: Get<AuthorityId> + 'static,
    {
        fn user_id(&self) -> crate::HashedUserId {
            DUMMY_USER
        }

        fn is_valid(&self) -> bool {
            self.0
        }

        fn used_challenge(&self) -> (DummyCx, crate::Challenge) {
            (0, [0; 32])
        }

        fn authority(&self) -> AuthorityId {
            A::get()
        }
    }
}
