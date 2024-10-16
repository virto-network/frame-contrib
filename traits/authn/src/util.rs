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

mod dummy {
    use frame_support::{parameter_types, sp_runtime::str_array as s};

    use crate::{
        AuthorityId, Challenger, DeviceChallengeResponse, DeviceId, HashedUserId,
        UserChallengeResponse,
    };

    use super::{Auth, Dev};

    type DummyAttestation = bool;
    type DummyCredential = bool;
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

    #[allow(dead_code)]
    pub type DummyDev = Dev<DeviceId, DummyAttestation, DummyChallenger, DummyCredential>;
    #[allow(dead_code)]
    pub type Dummy = Auth<DummyDev, DummyAttestation>;

    impl DeviceChallengeResponse<DummyCx> for DummyAttestation {
        fn device_id(&self) -> &DeviceId {
            &DUMMY_DEV
        }

        fn is_valid(&self) -> bool {
            *self
        }
        fn used_challenge(&self) -> (DummyCx, crate::Challenge) {
            (0, [0; 32])
        }
        fn authority(&self) -> crate::AuthorityId {
            DummyAuthority::get()
        }
    }

    impl UserChallengeResponse<DummyCx> for DummyCredential {
        fn user_id(&self) -> crate::HashedUserId {
            DUMMY_USER
        }

        fn is_valid(&self) -> bool {
            *self
        }

        fn used_challenge(&self) -> (DummyCx, crate::Challenge) {
            (0, [0; 32])
        }

        fn authority(&self) -> crate::AuthorityId {
            DummyAuthority::get()
        }
    }
}
