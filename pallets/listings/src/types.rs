use super::*;
pub use fc_traits_listings::item::subscriptions::{
    AmendmentPolicy, Cancellation, Eligibility, EndReason, ItemAmendment, PendingConditions,
    ReplacementDropReason, Subscription, SubscriptionConditions, SubscriptionPolicy,
    SubscriptionState,
};
use frame_support::traits::{fungibles::Inspect, Incrementable};
pub use item::ItemPrice;

use serde::{Deserialize, Serialize};

/// The AssetId type bound to the pallet instance.
pub(crate) type AssetIdOf<T, I> = <<T as Config<I>>::Assets as Inspect<AccountIdOf<T>>>::AssetId;

/// The asset Balance type bound to the pallet instance.
pub(crate) type AssetBalanceOf<T, I> =
    <<T as Config<I>>::Assets as Inspect<AccountIdOf<T>>>::Balance;

/// The `MerchantId` configuration parameter.
pub(crate) type MerchantIdOf<T, I = ()> = <T as Config<I>>::MerchantId;

/// The `InventoryId` configuration parameter.
pub(crate) type InventoryIdOf<T, I = ()> = <T as Config<I>>::InventoryId;

/// The composite `InventoryId` bound to the pallet instance.
pub type InventoryIdFor<T, I = ()> =
    InventoryId<<T as Config<I>>::MerchantId, <T as Config<I>>::InventoryId>;

/// The overarching `AccountId` type.
pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

/// The [`ItemPrice`] type bound to the pallet instance.
pub type ItemPriceOf<T, I = ()> = ItemPrice<AssetIdOf<T, I>, AssetBalanceOf<T, I>>;

/// The ID of every item inside the inventory.
pub type ItemIdOf<T, I = ()> = <T as Config<I>>::ItemSKU;

/// A `BoundedVec` limited by the overarching `KeyLimit`.
pub(crate) type ItemKeyOf<T, I = ()> = BoundedVec<u8, <T as Config<I>>::NonfungiblesKeyLimit>;

/// A `BoundedVec` limited by the overarching `ValueLimit`.
pub(crate) type ItemValueOf<T, I = ()> = BoundedVec<u8, <T as Config<I>>::NonfungiblesValueLimit>;

/// The `(MerchantId, InventoryId)` tuple the listings traits use to name an inventory.
pub type InventoryIdTuple<T, I = ()> =
    (<T as Config<I>>::MerchantId, <T as Config<I>>::InventoryId);

/// A tick of the chain clock bound to the pallet instance.
pub type MomentOf<T, I = ()> =
    <<T as Config<I>>::BlockNumberProvider as sp_runtime::traits::BlockNumberProvider>::BlockNumber;

/// The [`SubscriptionConditions`] type bound to the pallet instance.
pub type SubscriptionConditionsOf<T, I = ()> =
    SubscriptionConditions<ItemPriceOf<T, I>, MomentOf<T, I>>;

/// The [`Subscription`] type bound to the pallet instance.
pub type SubscriptionOf<T, I = ()> =
    Subscription<ItemPriceOf<T, I>, MomentOf<T, I>, ItemIdOf<T, I>>;

/// The [`ItemAmendment`] type bound to the pallet instance.
pub type ItemAmendmentOf<T, I = ()> = ItemAmendment<ItemPriceOf<T, I>, MomentOf<T, I>>;

/// The [`PendingConditions`] type bound to the pallet instance.
pub type PendingConditionsOf<T, I = ()> = PendingConditions<ItemPriceOf<T, I>, MomentOf<T, I>>;

/// The key of a subscription: inventory, item and subscriber.
pub type SubscriptionKeyOf<T, I = ()> = (InventoryIdFor<T, I>, ItemIdOf<T, I>, AccountIdOf<T>);

/// The [`ItemSubscription`] type bound to the pallet instance.
pub type ItemSubscriptionOf<T, I = ()> =
    ItemSubscription<SubscriptionConditionsOf<T, I>, AccountIdOf<T>>;

/// The [`SubscriptionRecord`] type bound to the pallet instance.
pub type SubscriptionRecordOf<T, I = ()> = SubscriptionRecord<SubscriptionOf<T, I>, MomentOf<T, I>>;

/// An item's subscription conditions, for new subscriptions, who may subscribe to it, and its
/// subscription policy.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub struct ItemSubscription<Conditions, AccountId> {
    /// The conditions new subscriptions take.
    pub conditions: Conditions,
    /// Who may subscribe.
    pub eligibility: Eligibility<AccountId>,
    /// What the merchant may do to the subscriptions: copied into each one when it starts.
    pub policy: SubscriptionPolicy,
    /// Whether the conditions were withdrawn: no new subscription, and no replacement into the
    /// item, until conditions are set again.
    pub withdrawn: bool,
}

/// How many subscriptions to an item are live and chargeable (not *Defaulted*), and how many of
/// those live when its last item amendment was enacted still have to apply it, be skipped by it,
/// or end.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
    Default,
)]
pub struct ItemCounts {
    /// The live subscriptions that are not *Defaulted*.
    pub live: u32,
    /// The subscriptions behind the item's last amendment.
    pub behind: u32,
}

/// A stored subscription, with the pallet's bookkeeping for it.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub struct SubscriptionRecord<Subscription, Moment> {
    /// The subscription.
    pub subscription: Subscription,
    /// The bucket of the due queue that holds the subscription's one entry, in the bucket or in
    /// its overflow. Every live subscription has one.
    pub queued_at: Option<Moment>,
    /// While suspended: the total length of the migration pauses that ended at or before the
    /// unpaid charge's due tick. Pauses beyond it extend the grace end.
    pub paused_before: Moment,
}

#[cfg(feature = "runtime-benchmarks")]
pub(crate) type NativeBalanceOf<T, I = ()> = <
<T as Config<I>>::Balances as frame_support::traits::fungible::Inspect<AccountIdOf<T>>
>::Balance;

/// A set of attributes associated to an inventory.
#[derive(Encode)]
pub enum InventoryAttribute {
    /// Indicates if the inventory is archived.
    Archived,
}

/// A set of attributes associated to an item.
#[derive(Encode)]
pub enum ItemAttribute {
    /// The item creator
    #[codec(index = 10)]
    Creator,
    /// The item basic info (name and price).
    #[codec(index = 11)]
    Info,
    /// Whether an item cannot be resold.
    #[codec(index = 12)]
    NotForResale,
}

/// The item's basic information
pub type ItemInfo<Name, Price> = (Name, Option<Price>);

/// The internal representation of a listings inventory ID.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    Default,
    Copy,
    Clone,
    PartialEq,
    Eq,
    Debug,
    MaxEncodedLen,
    TypeInfo,
)]
pub struct InventoryId<MerchantId, Id>(pub MerchantId, pub Id);

impl<MerchantId, Id> From<InventoryId<MerchantId, Id>> for (MerchantId, Id) {
    fn from(InventoryId(merchant_id, inventory_id): InventoryId<MerchantId, Id>) -> Self {
        (merchant_id, inventory_id)
    }
}

impl<MerchantId, Id> From<(MerchantId, Id)> for InventoryId<MerchantId, Id> {
    fn from((merchant_id, inventory_id): (MerchantId, Id)) -> Self {
        Self(merchant_id, inventory_id)
    }
}

impl<MerchantId: Copy, Id: Copy> From<&InventoryId<MerchantId, Id>> for (MerchantId, Id) {
    fn from(value: &InventoryId<MerchantId, Id>) -> Self {
        (*value).into()
    }
}

impl<MerchantId: Copy, Id: Copy> From<&(MerchantId, Id)> for InventoryId<MerchantId, Id> {
    fn from(value: &(MerchantId, Id)) -> Self {
        (*value).into()
    }
}

impl<MerchantId: Copy + Incrementable, Id: Copy + Incrementable> Incrementable
    for InventoryId<MerchantId, Id>
{
    fn increment(&self) -> Option<Self> {
        // Increment shouldn't happen for inventory, but
        // we'll implement it anyway.
        self.1.increment().map(|id| Self(self.0, id))
    }

    fn initial_value() -> Option<Self> {
        Some(Self(MerchantId::initial_value()?, Id::initial_value()?))
    }
}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<InventoryId> {
    fn inventory_id() -> InventoryId;
}

/// Prices and funds subscriptions in the benchmarks.
#[cfg(feature = "runtime-benchmarks")]
pub trait SubscriptionsBenchmarkHelper<AccountId, AssetId, Balance> {
    /// An asset subscriptions can be priced in, created if needed, and a non-zero amount of it
    /// that is a valid price (at least the asset's minimum balance).
    fn price() -> (AssetId, Balance);

    /// Gives `who` enough of `asset` to pay `amount` many times over, fees included.
    fn fund(who: &AccountId, asset: &AssetId, amount: Balance);
}

pub mod test_utils {
    use super::*;
    use core::ops::Deref;

    #[derive(
        Clone,
        Copy,
        PartialEq,
        Eq,
        Encode,
        Decode,
        DecodeWithMemTracking,
        MaxEncodedLen,
        TypeInfo,
        Debug,
        Default,
    )]
    #[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
    pub struct SignedMerchantId(pub [u8; 32]);

    impl From<[u8; 32]> for SignedMerchantId {
        fn from(value: [u8; 32]) -> Self {
            Self(value)
        }
    }

    impl From<Vec<u8>> for SignedMerchantId {
        fn from(value: Vec<u8>) -> Self {
            Self(
                value
                    .try_into()
                    .expect("test `SignedMerchantId` won't exceed 32 bytes"),
            )
        }
    }

    impl Incrementable for SignedMerchantId {
        fn increment(&self) -> Option<Self> {
            let mut inner = self.0;
            let mut i = 0;
            loop {
                if inner[i] == 255 {
                    inner[i] = 0;
                    i += 1;
                    if i == 32 {
                        break;
                    }
                } else {
                    inner[i] += 1;
                    break;
                }
            }

            Some(Self(inner))
        }

        fn initial_value() -> Option<Self> {
            Some([0u8; 32].into())
        }
    }

    impl PartialEq<[u8]> for SignedMerchantId {
        fn eq(&self, other: &[u8]) -> bool {
            self.0 == *other
        }
    }

    impl Deref for SignedMerchantId {
        type Target = [u8; 32];

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
}
