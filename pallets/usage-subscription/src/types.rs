//! The types of usage subscriptions: offers, terms, contracts and their pending changes.

use super::*;
use fc_traits_listings::item::{
    subscriptions::{ItemPriceOf, SubscriptionConditions},
    InspectItem,
};
use sp_runtime::traits::{AtLeast32BitUnsigned, One};

/// The overarching account type.
pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
/// A group, as the memberships system names it.
pub type GroupOf<T> =
    <<T as Config>::Memberships as fc_traits_memberships::Inspect<AccountIdOf<T>>>::Group;
/// A membership of a group, as the memberships system names it.
pub type MembershipOf<T> =
    <<T as Config>::Memberships as fc_traits_memberships::Inspect<AccountIdOf<T>>>::Membership;
/// The merchant of the collective's inventory.
pub type MerchantIdOf<T> =
    <<T as Config>::Subscriptions as InspectItem<AccountIdOf<T>>>::MerchantId;
/// The collective's inventory, within its merchant.
pub type InventoryIdOf<T> =
    <<T as Config>::Subscriptions as InspectItem<AccountIdOf<T>>>::InventoryId;
/// An offer: the listings item that carries its subscription conditions.
pub type OfferIdOf<T> = <<T as Config>::Subscriptions as InspectItem<AccountIdOf<T>>>::ItemId;
/// A tick of the chain clock.
pub type MomentOf<T> = <<T as Config>::Subscriptions as subs::Inspect<AccountIdOf<T>>>::Moment;
/// The price of a billing period: an asset and an amount.
pub type PriceOf<T> = ItemPriceOf<<T as Config>::Subscriptions, AccountIdOf<T>>;
/// The subscription conditions listings keeps for an offer and for each contract.
pub type ConditionsOf<T> = SubscriptionConditions<PriceOf<T>, MomentOf<T>>;
/// [`Terms`] of the pallet.
pub type TermsOf<T> = Terms<PriceOf<T>, MomentOf<T>>;
/// [`OfferKind`] of the pallet.
pub type OfferKindOf<T> = OfferKind<GroupOf<T>>;
/// [`Offer`] of the pallet.
pub type OfferOf<T> = Offer<GroupOf<T>, MomentOf<T>>;
/// [`Contract`] of the pallet.
pub type ContractOf<T> = Contract<OfferIdOf<T>, GroupOf<T>, MomentOf<T>>;
/// [`PendingChange`] of the pallet.
pub type PendingChangeOf<T> = PendingChange<OfferIdOf<T>, MomentOf<T>>;

/// What kind of offer it is (SPEC §5.1).
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
)]
pub enum OfferKind<Group> {
    /// Open-ended, with no minimum commitment, for any usable group.
    Standard,
    /// A few billing periods, free or discounted, that a group may start once, ever.
    Trial,
    /// Agreed with exactly one group by a collective referendum, optionally with a minimum
    /// commitment and a term limit. Once accepted, it reads as withdrawn.
    Custom(Group),
}

/// Whether an offer can be subscribed to.
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
)]
pub enum OfferStatus {
    /// New contracts may be made from it.
    Open,
    /// No new contract may be made from it: it was withdrawn, or it is a custom offer that was
    /// accepted. Contracts made from it are not touched.
    Withdrawn,
}

/// The terms of an offer, or of an amendment: what a contract copies when it starts.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub struct Terms<Price, Moment> {
    /// The weight the pool allows per usage period, in both components. Both must be non-zero.
    pub allowance: Weight,
    /// The usage period, in ticks: the pool's usage resets at each of its boundaries.
    pub usage_period: Moment,
    /// The price of one billing period, charged at its start.
    pub price: Price,
    /// The billing period, in ticks.
    pub billing_period: Moment,
    /// How many billing periods a contract runs. `None` is open-ended.
    pub term: Option<u32>,
    /// How many billing periods a contract must be paid for, even if cancelled earlier.
    pub min_commitment: Option<u32>,
    /// The ticks after a missed charge during which the group can still pay.
    pub grace: Moment,
}

impl<Price: Clone, Moment: Copy> Terms<Price, Moment> {
    /// The money and time half of the terms, which listings keeps.
    pub fn conditions(&self) -> SubscriptionConditions<Price, Moment> {
        SubscriptionConditions {
            price: self.price.clone(),
            period: self.billing_period,
            term: self.term,
            min_commitment: self.min_commitment,
            grace: self.grace,
        }
    }
}

/// An offer the collective made available. Its money and time terms are the subscription
/// conditions of its listings item; this record holds its kind and its weight terms.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub struct Offer<Group, Moment> {
    /// Its kind, and for a custom offer, its group.
    pub kind: OfferKind<Group>,
    /// The allowance a contract made from it now copies.
    pub allowance: Weight,
    /// The usage period a contract made from it copies.
    pub usage_period: Moment,
    /// Whether new contracts may be made from it.
    pub status: OfferStatus,
}

/// A change pending for a contract.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub enum PendingChange<OfferId, Moment> {
    /// A switch to the offer, at the first billing boundary at or after both `paid through` and
    /// the commitment end (`REQ-CT-7`).
    Switch(OfferId),
    /// A trial's conversion into the offer, at the trial's end (`REQ-CT-13`).
    Convert(OfferId),
    /// An amendment of a custom contract, pending until its effective boundary (`REQ-CT-8`).
    Amend {
        /// The amended allowance.
        allowance: Weight,
        /// The effective boundary: the charge at the amended price is never taken before it.
        effective_at: Moment,
        /// The start of the first usage window at or after the effective boundary, from which the
        /// amended allowance applies.
        from_window: Moment,
    },
}

/// An amended allowance in force, that applies from a usage window on.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub struct NextAllowance<Moment> {
    /// The amended allowance.
    pub allowance: Weight,
    /// The start of the first usage window it applies to.
    pub from_window: Moment,
}

/// A group's usage contract: the weight half. The money half (price, billing period, `paid
/// through`, state, commitment, grace) is the listings subscription keyed by the collective's
/// inventory, the offer, and the group's account.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub struct Contract<OfferId, Group, Moment> {
    /// The offer it was made from.
    pub offer: OfferId,
    /// The offer's kind, copied at subscription.
    pub kind: OfferKind<Group>,
    /// The allowance per usage period, copied at subscription and changed only by an amendment.
    pub allowance: Weight,
    /// The usage period. It never changes.
    pub usage_period: Moment,
    /// The tick at which it started. Every usage window is counted from it.
    pub anchor: Moment,
    /// The start of the usage window `used` belongs to. Written only by the payment step's
    /// charge.
    pub window_start: Moment,
    /// The weight charged to the pool in the window starting at `window_start`.
    pub used: Weight,
    /// A switch, conversion or amendment pending, if any.
    pub pending: Option<PendingChange<OfferId, Moment>>,
    /// An amended allowance in force from a later usage window, if any.
    pub next_allowance: Option<NextAllowance<Moment>>,
    /// The last amendment of its offer it has applied, or that was enacted before it started.
    pub amendment_seq: u32,
}

impl<OfferId, Group, Moment: AtLeast32BitUnsigned + Copy> Contract<OfferId, Group, Moment> {
    /// A contract to `offer`, anchored at `anchor`, with a fresh window.
    pub fn start(
        offer: OfferId,
        terms: &Offer<Group, Moment>,
        anchor: Moment,
        amendment_seq: u32,
    ) -> Self
    where
        Group: Clone,
    {
        Self {
            offer,
            kind: terms.kind.clone(),
            allowance: terms.allowance,
            usage_period: terms.usage_period,
            anchor,
            window_start: anchor,
            used: Weight::zero(),
            pending: None,
            next_allowance: None,
            amendment_seq,
        }
    }

    /// The start of the usage window that contains `now`: `anchor + ⌊(now − anchor) / U⌋ · U`.
    ///
    /// `None` if `now` is before the anchor, the usage period is zero, or the arithmetic
    /// overflows: there is no current window then (`INV-5`, `INV-14`).
    pub fn window_at(&self, now: Moment) -> Option<Moment> {
        window_start(self.anchor, self.usage_period, now)
    }

    /// The weight used in the window starting at `window`: the recorded usage only if it was
    /// recorded in that window (`DEC-3`).
    pub fn used_in(&self, window: Moment) -> Weight {
        if self.window_start == window {
            self.used
        } else {
            Weight::zero()
        }
    }
}

/// The start of the usage window of a contract anchored at `anchor`, with usage period `period`,
/// that contains `now`. `None` before the anchor, for a zero period, or on overflow.
pub fn window_start<Moment: AtLeast32BitUnsigned + Copy>(
    anchor: Moment,
    period: Moment,
    now: Moment,
) -> Option<Moment> {
    let elapsed = now.checked_sub(&anchor)?;
    let windows = elapsed.checked_div(&period)?;
    anchor.checked_add(&windows.checked_mul(&period)?)
}

/// The start of the first usage window, of a contract anchored at `anchor` with usage period
/// `period`, that starts at or after `tick`. Saturating.
pub fn first_window_from<Moment: AtLeast32BitUnsigned + Copy>(
    anchor: Moment,
    period: Moment,
    tick: Moment,
) -> Moment {
    if tick <= anchor || period.is_zero() {
        return anchor;
    }
    let distance = tick.saturating_sub(anchor);
    // `period` is non-zero here.
    let windows = distance.saturating_add(period.saturating_sub(One::one())) / period;
    anchor.saturating_add(windows.saturating_mul(period))
}

/// Why a contract ended (SPEC §5.2, *Ended*).
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
)]
pub enum EndReason {
    /// Its term limit's last period was paid and ended, and nothing took over.
    Completed,
    /// The group cancelled it.
    Cancelled,
    /// A charge stayed unpaid past grace outside the commitment, or a *Defaulted* contract
    /// reached its commitment end.
    Lapsed,
    /// A switch took effect: a contract to another offer started in its place.
    Switched,
    /// A trial's conversion took effect: a contract to the named offer started in its place.
    Converted,
    /// The collective terminated it.
    Terminated,
}
