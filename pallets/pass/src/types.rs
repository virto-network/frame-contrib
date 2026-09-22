use super::*;
use crate::filter::{DeviceFilter, SpendMatcher};

use alloc::borrow::ToOwned;
use codec::EncodeLike;
use frame_support::traits::MapSuccess;
use frame_system::EnsureSigned;
use sp_runtime::{
    morph_types,
    traits::{Hash, TrailingZeroInput},
    Saturating,
};

// pub type HashedUserId<T> = <T as frame_system::Config>::Hash;
pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
pub(crate) type BlockNumberFor<T, I> =
    <<T as Config<I>>::BlockNumberProvider as BlockNumberProvider>::BlockNumber;
pub type ContextOf<T, I = ()> =
    <<<T as Config<I>>::Authenticator as Authenticator>::Challenger as Challenger>::Context;
pub type DeviceOf<T, I = ()> = <<T as Config<I>>::Authenticator as Authenticator>::Device;
pub type CredentialOf<T, I = ()> = <DeviceOf<T, I> as UserAuthenticator>::Credential;
pub type DeviceAttestationOf<T, I = ()> =
    <<T as Config<I>>::Authenticator as Authenticator>::DeviceAttestation;
pub type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;
pub type BalanceOf<T, I = ()> =
    <<T as Config<I>>::Balances as Inspect<<T as frame_system::Config>::AccountId>>::Balance;
pub type DeviceFilterOf<T, I = ()> =
    DeviceFilter<
        <<T as Config<I>>::SpendMatcher as SpendMatcher<
            <T as frame_system::Config>::RuntimeCall,
        >>::AssetId,
        <<T as Config<I>>::SpendMatcher as SpendMatcher<
            <T as frame_system::Config>::RuntimeCall,
        >>::Balance,
        <T as Config<I>>::MaxFilteredCalls,
        <T as Config<I>>::MaxFilteredAssets,
    >;
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

/// A [`Consideration`] where the first `N` items of a footprint are free: only the items beyond
/// `N` are charged, through `C`.
///
/// It is stored as `Option<C>` whatever `N` is, so changing `N` (or moving from
/// [`FirstItemIsFree`]) needs no storage migration. Nesting considerations instead (e.g.
/// `FirstItemIsFree<FirstItemIsFree<C>>`) adds one `Option` per layer and changes the encoding.
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(N))]
#[codec(mel_bound(C: MaxEncodedLen))]
pub struct FirstItemsAreFree<N, C>(
    pub(crate) Option<C>,
    // `fn() -> N` keeps the marker `Send + Sync` whatever `N` is.
    #[codec(skip)] PhantomData<fn() -> N>,
);

/// A [`Consideration`] where only the first item is free.
pub type FirstItemIsFree<C> = FirstItemsAreFree<ConstU32<1>, C>;

impl<N, C> FirstItemsAreFree<N, C> {
    fn wrap(inner: Option<C>) -> Self {
        Self(inner, PhantomData)
    }
}

// Implemented by hand so they don't require anything of `N`, which is only a marker.
impl<N, C: Clone> Clone for FirstItemsAreFree<N, C> {
    fn clone(&self) -> Self {
        Self::wrap(self.0.clone())
    }
}

impl<N, C: core::fmt::Debug> core::fmt::Debug for FirstItemsAreFree<N, C> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("FirstItemsAreFree").field(&self.0).finish()
    }
}

impl<N, C: PartialEq> PartialEq for FirstItemsAreFree<N, C> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<N, C: Eq> Eq for FirstItemsAreFree<N, C> {}

impl<AccountId, N, C> Consideration<AccountId, Footprint> for FirstItemsAreFree<N, C>
where
    N: Get<u32> + 'static,
    C: Consideration<AccountId, Footprint>,
{
    fn new(who: &AccountId, new: Footprint) -> Result<Self, DispatchError> {
        let free = N::get() as u64;
        if new.count.le(&free) {
            Ok(Self::wrap(None))
        } else {
            C::new(
                who,
                Footprint {
                    count: new.count.saturating_sub(free),
                    size: new.size,
                },
            )
            .map(Some)
            .map(Self::wrap)
        }
    }

    fn update(self, who: &AccountId, new: Footprint) -> Result<Self, DispatchError> {
        let free = N::get() as u64;
        if new.count.ge(&1) {
            if let Some(c) = self.0 {
                c.update(
                    who,
                    Footprint {
                        count: new.count.saturating_sub(free),
                        size: new
                            .size
                            .saturating_div(new.count.max(1))
                            .saturating_mul(new.count.saturating_sub(free)),
                    },
                )
                .map(Some)
                .map(Self::wrap)
            } else {
                Self::new(who, new)
            }
        } else {
            self.drop(who).map(|_| Self::wrap(None))
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
        (Consideration, u32),
        Query = Option<(Consideration, u32)>,
    >,
    BlobType: MaxEncodedLen,
{
    /// Makes a mutation on the consideration storage for an address
    fn mutate_consideration(address: &Account, f: impl FnOnce(&mut u32)) -> DispatchResult {
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
        Self::mutate_consideration(address, u32::saturating_inc)
    }

    /// Decrements the consideration count for an address
    pub fn decrement(address: &Account) -> DispatchResult {
        Self::mutate_consideration(address, u32::saturating_dec)
    }
}

#[cfg(test)]
mod first_items_are_free {
    use super::*;

    /// Records the footprint it was last charged for.
    #[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, Debug, Eq, PartialEq)]
    struct Charged(Footprint);

    impl Consideration<u64, Footprint> for Charged {
        fn new(_: &u64, new: Footprint) -> Result<Self, DispatchError> {
            Ok(Self(new))
        }
        fn update(self, _: &u64, new: Footprint) -> Result<Self, DispatchError> {
            Ok(Self(new))
        }
        fn drop(self, _: &u64) -> Result<(), DispatchError> {
            Ok(())
        }
        #[cfg(feature = "runtime-benchmarks")]
        fn ensure_successful(_: &u64, _: Footprint) {}
    }

    type TwoFree = FirstItemsAreFree<ConstU32<2>, Charged>;

    fn items(count: u64) -> Footprint {
        Footprint {
            count,
            size: count * 10,
        }
    }

    #[test]
    fn encodes_as_option_of_the_inner_consideration_for_any_n() {
        let inner = Some(Charged(items(3)));
        let expected = inner.encode();
        assert_eq!(
            FirstItemIsFree::<Charged>::wrap(inner.clone()).encode(),
            expected
        );
        assert_eq!(TwoFree::wrap(inner.clone()).encode(), expected);

        // So values written by `FirstItemIsFree` decode as `FirstItemsAreFree<N, _>`.
        assert_eq!(
            TwoFree::decode(&mut &expected[..]).unwrap(),
            TwoFree::wrap(inner)
        );
        assert_eq!(
            TwoFree::max_encoded_len(),
            FirstItemIsFree::<Charged>::max_encoded_len()
        );
    }

    #[test]
    fn nesting_is_not_encoding_compatible() {
        // What `FirstItemIsFree<FirstItemIsFree<C>>` stores, for contrast.
        let nested = FirstItemIsFree::<FirstItemIsFree<Charged>>::wrap(Some(
            FirstItemIsFree::wrap(Some(Charged(items(1)))),
        ));
        assert_ne!(nested.encode(), Some(Charged(items(1))).encode());
    }

    #[test]
    fn only_items_beyond_n_are_charged() {
        assert_eq!(TwoFree::new(&0, items(1)).unwrap(), TwoFree::wrap(None));
        assert_eq!(TwoFree::new(&0, items(2)).unwrap(), TwoFree::wrap(None));
        assert_eq!(
            TwoFree::new(&0, items(3)).unwrap(),
            TwoFree::wrap(Some(Charged(Footprint { count: 1, size: 30 })))
        );
    }

    #[test]
    fn first_item_is_free_behaves_as_before() {
        type OneFree = FirstItemIsFree<Charged>;
        assert_eq!(OneFree::new(&0, items(1)).unwrap(), OneFree::wrap(None));
        assert_eq!(
            OneFree::new(&0, items(2)).unwrap(),
            OneFree::wrap(Some(Charged(Footprint { count: 1, size: 20 })))
        );
        // `update` scales the size down to the charged items.
        assert_eq!(
            OneFree::wrap(Some(Charged(items(1))))
                .update(&0, items(4))
                .unwrap(),
            OneFree::wrap(Some(Charged(Footprint { count: 3, size: 30 })))
        );
    }

    #[test]
    fn update_charges_once_beyond_n_and_releases_at_zero() {
        // Growing from free into charged.
        assert_eq!(
            TwoFree::wrap(None).update(&0, items(3)).unwrap(),
            TwoFree::wrap(Some(Charged(Footprint { count: 1, size: 30 })))
        );
        // An existing ticket is resized to the charged items only.
        assert_eq!(
            TwoFree::wrap(Some(Charged(items(1))))
                .update(&0, items(5))
                .unwrap(),
            TwoFree::wrap(Some(Charged(Footprint { count: 3, size: 30 })))
        );
        // Dropping to zero items releases it.
        assert_eq!(
            TwoFree::wrap(Some(Charged(items(1))))
                .update(&0, items(0))
                .unwrap(),
            TwoFree::wrap(None)
        );
    }
}
