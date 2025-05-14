use crate::Config;
use codec::{Decode, EncodeLike};
use core::marker::PhantomData;
use fc_traits_authn::{
    composite_prelude::{Encode, MaxEncodedLen, TypeInfo},
    HashedUserId, HASHED_USER_ID_LEN,
};
use frame_support::{
    dispatch::DispatchResult,
    traits::{fungible::Inspect, Consideration, Footprint, MapSuccess},
};
use frame_system::EnsureSigned;
use sp_core::TypedGet;
use sp_runtime::{
    morph_types,
    traits::StaticLookup,
    traits::{Hash, TrailingZeroInput},
    DispatchError, Saturating,
};

// pub type HashedUserId<T> = <T as frame_system::Config>::Hash;
pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
pub type ContextOf<T, I = ()> =
<<<T as Config<I>>::Authenticator as fc_traits_authn::Authenticator>::Challenger as fc_traits_authn::Challenger>::Context;
pub type DeviceOf<T, I = ()> =
    <<T as Config<I>>::Authenticator as fc_traits_authn::Authenticator>::Device;
pub type CredentialOf<T, I = ()> =
    <DeviceOf<T, I> as fc_traits_authn::UserAuthenticator>::Credential;
pub type DeviceAttestationOf<T, I = ()> =
    <<T as Config<I>>::Authenticator as fc_traits_authn::Authenticator>::DeviceAttestation;
pub type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;
pub type BalanceOf<T, I = ()> =
    <<T as Config<I>>::Balances as Inspect<<T as frame_system::Config>::AccountId>>::Balance;
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

pub trait AddressGenerator<T: Config<I>, I: 'static> {
    /// Generates an account address for a [HashedUserId]. Returns `Some(address)`
    /// if the process is successful, or
    fn generate_address(id: HashedUserId) -> T::AccountId;
}

impl<T: Config<I>, I: 'static> AddressGenerator<T, I> for () {
    fn generate_address(id: HashedUserId) -> T::AccountId {
        // we know the length of HashedUserId
        let mut input = [0u8; 2 * HASHED_USER_ID_LEN];
        input[HASHED_USER_ID_LEN..].copy_from_slice(&id);

        T::AccountId::decode(&mut TrailingZeroInput::new(
            T::Hashing::hash(&input).as_ref(),
        ))
        .expect("using trailing zero input, the decode is guaranteed; qed")
    }
}

#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, Debug, Eq, PartialEq)]
pub struct FirstItemIsFree<C>(pub(crate) Option<C>);

impl<AccountId, C> Consideration<AccountId, Footprint> for FirstItemIsFree<C>
where
    C: Consideration<AccountId, Footprint>,
{
    fn new(who: &AccountId, new: Footprint) -> Result<Self, DispatchError> {
        if new.count.le(&1) {
            Ok(Self(None))
        } else {
            C::new(
                who,
                Footprint {
                    count: new.count.saturating_sub(1),
                    size: new.size,
                },
            )
            .map(Some)
            .map(Self)
        }
    }

    fn update(self, who: &AccountId, new: Footprint) -> Result<Self, DispatchError> {
        if new.count.ge(&1) {
            if let Some(c) = self.0 {
                c.update(
                    who,
                    Footprint {
                        count: new.count.saturating_sub(1),
                        size: new
                            .size
                            .saturating_div(new.count.max(1))
                            .saturating_mul(new.count.saturating_sub(1)),
                    },
                )
                .map(Some)
                .map(Self)
            } else {
                Self::new(who, new)
            }
        } else {
            self.drop(who).map(|_| Self(None))
        }
    }

    fn drop(self, who: &AccountId) -> Result<(), DispatchError> {
        if let Some(c) = self.0 {
            c.drop(who)
        } else {
            Ok(())
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn ensure_successful(who: &AccountId, new: Footprint) {
        C::ensure_successful(who, new)
    }
}

pub struct ConsiderationHandler<A, S, C, T>(PhantomData<(A, S, C, T)>);

impl<Account, Storage, Consideration, BlobType>
    ConsiderationHandler<Account, Storage, Consideration, BlobType>
where
    Account: EncodeLike + MaxEncodedLen,
    Consideration: frame_support::traits::Consideration<Account, Footprint>,
    Storage: frame_support::StorageMap<
        Account,
        (Consideration, u64),
        Query = Option<(Consideration, u64)>,
    >,
    BlobType: MaxEncodedLen,
{
    /// Makes a mutation on the consideration storage for an address
    fn mutate_consideration(address: &Account, f: impl FnOnce(&mut u64)) -> DispatchResult {
        Storage::try_mutate(address, |maybe_consideration| {
            let (consideration, mut count) = match maybe_consideration {
                Some(c) => c.to_owned(),
                _ => (Consideration::new(address, Footprint::default())?, 0),
            };

            f(&mut count);

            *maybe_consideration = Some((
                consideration.update(
                    address,
                    Footprint::from_parts(count as usize, BlobType::max_encoded_len()),
                )?,
                count,
            ));

            Ok(())
        })
    }

    /// Increments the consideration count for an address
    pub fn increment(address: &Account) -> DispatchResult {
        Self::mutate_consideration(address, u64::saturating_inc)
    }

    /// Decrements the consideration count for an address
    pub fn decrement(address: &Account) -> DispatchResult {
        Self::mutate_consideration(address, u64::saturating_dec)
    }
}

#[cfg(feature = "runtime-benchmarks")]
pub use benchmarks::BenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks {
    use super::*;
    use fc_traits_authn::{ExtrinsicContext, HashedUserId};

    #[cfg(feature = "runtime-benchmarks")]
    pub trait BenchmarkHelper<T, I = ()>
    where
        T: Config<I>,
        I: 'static,
    {
        fn device_attestation(
            device_id: fc_traits_authn::DeviceId,
            xtc: &impl ExtrinsicContext,
        ) -> DeviceAttestationOf<T, I>;
        fn credential(user_id: HashedUserId, xtc: &impl ExtrinsicContext) -> CredentialOf<T, I>;
    }
}
