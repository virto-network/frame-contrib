use crate::Config;
use sp_runtime::traits::StaticLookup;

// pub type HashedUserId<T> = <T as frame_system::Config>::Hash;
pub type DeviceOf<T, I> =
    <<T as Config<I>>::Authenticator as fc_traits_authn::Authenticator>::Device;
pub type CredentialOf<T, I> = <DeviceOf<T, I> as fc_traits_authn::UserAuthenticator>::Credential;
pub type DeviceAttestationOf<T, I> =
    <<T as Config<I>>::Authenticator as fc_traits_authn::Authenticator>::DeviceAttestation;
pub type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;
