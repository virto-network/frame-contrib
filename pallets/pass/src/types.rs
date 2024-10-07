use crate::Config;
use frame_support::traits::{fungible::Inspect, MapSuccess};
use frame_system::{pallet_prelude::OriginFor, EnsureSigned};
use sp_core::TypedGet;
use sp_runtime::{morph_types, traits::StaticLookup};

// pub type HashedUserId<T> = <T as frame_system::Config>::Hash;
type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
pub type ContextOf<T, I> =
<<<T as Config<I>>::Authenticator as fc_traits_authn::Authenticator>::Challenger as fc_traits_authn::Challenger>::Context;
pub type DeviceOf<T, I> =
    <<T as Config<I>>::Authenticator as fc_traits_authn::Authenticator>::Device;
pub type CredentialOf<T, I> = <DeviceOf<T, I> as fc_traits_authn::UserAuthenticator>::Credential;
pub type DeviceAttestationOf<T, I> =
    <<T as Config<I>>::Authenticator as fc_traits_authn::Authenticator>::DeviceAttestation;
pub type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;
pub type BalanceOf<T, I> =
    <<T as Config<I>>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;
pub type DepositInformation<T, I = ()> = (
    <T as frame_system::Config>::AccountId,
    BalanceOf<T, I>,
    <T as frame_system::Config>::AccountId,
);

morph_types! {
    pub type PaymentForCreate<
        AccountId,
        GetAmount: TypedGet,
        GetReceiver: TypedGet<Type = AccountId>
    >: Morph = |sender: AccountId| -> Option<(AccountId, GetAmount::Type, GetReceiver::Type)> {
        Some((sender, GetAmount::get(), GetReceiver::get()))
    };
}

pub type EnsureSignedPays<T, Amount, Beneficiary> =
    MapSuccess<EnsureSigned<AccountIdOf<T>>, PaymentForCreate<AccountIdOf<T>, Amount, Beneficiary>>;

#[cfg(feature = "runtime-benchmarks")]
use fc_traits_authn::HashedUserId;
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<T, I = ()>
where
    T: Config<I>,
    I: 'static,
{
    fn register_origin() -> OriginFor<T>;
    fn device_attestation(device_id: fc_traits_authn::DeviceId) -> DeviceAttestationOf<T, I>;
    fn credential(user_id: HashedUserId) -> CredentialOf<T, I>;
}
