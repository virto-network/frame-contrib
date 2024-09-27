use crate::Config;
use fc_traits_authn::HashedUserId;
use sp_runtime::traits::StaticLookup;

// pub type HashedUserId<T> = <T as frame_system::Config>::Hash;
pub type ContextOf<T, I> =
    <<<T as Config<I>>::Authenticator as fc_traits_authn::Authenticator>::Challenger as fc_traits_authn::Challenger>::Context;
pub type DeviceOf<T, I> =
    <<T as Config<I>>::Authenticator as fc_traits_authn::Authenticator>::Device;
pub type CredentialOf<T, I> = <DeviceOf<T, I> as fc_traits_authn::UserAuthenticator>::Credential;
pub type DeviceAttestationOf<T, I> =
    <<T as Config<I>>::Authenticator as fc_traits_authn::Authenticator>::DeviceAttestation;
pub type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<T, I = ()>
where
    T: Config<I>,
    I: 'static,
{
    fn device_attestation(device_id: fc_traits_authn::DeviceId) -> DeviceAttestationOf<T, I>;
    fn credential(user_id: HashedUserId) -> CredentialOf<T, I>;
}
