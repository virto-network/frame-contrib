//! Recurring subscriptions to listings items.
//!
//! A merchant gives an item [`SubscriptionConditions`]. A subscriber subscribes, paying billing
//! period 0 up front, and then each billing period at its start. Due charges are collected
//! automatically or by anyone, a missed charge suspends the subscription, grace restores it, and a
//! lapse ends it, or keeps it [`SubscriptionState::Defaulted`] until its commitment end when it
//! lapsed within a minimum commitment. Subscriptions can be cancelled, replaced by a subscription
//! to another item at a due tick, and, as the item's [`SubscriptionPolicy`] allows, terminated by
//! the merchant and amended (one by one, or for every subscription to an item) after a notice of
//! at least one billing period.
//!
//! Every transition is reported to an [`OnSubscriptionChanged`] hook in the same block.

use super::*;
use frame_support::{
    pallet_prelude::DispatchError,
    sp_runtime::traits::{AtLeast32BitUnsigned, One},
    weights::Weight,
};
use impl_trait_for_tuples::impl_for_tuples;

/// The conditions on which an item may be subscribed to.
///
/// They are copied into each subscription when it starts, and change for an existing subscription
/// only through an amendment.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub struct SubscriptionConditions<Price, Moment> {
    /// The price of one billing period, charged at its start. It may be zero.
    pub price: Price,
    /// The billing period, in ticks of the chain clock.
    pub period: Moment,
    /// How many billing periods the subscription runs. `None` is open-ended.
    pub term: Option<u32>,
    /// How many billing periods must be paid for, even if the subscription is cancelled earlier.
    pub min_commitment: Option<u32>,
    /// The ticks after a missed charge during which it can still be paid.
    pub grace: Moment,
}

impl<Price, Moment: AtLeast32BitUnsigned + Copy> SubscriptionConditions<Price, Moment> {
    /// Whether the conditions are structurally valid: a non-zero period, a grace shorter than the
    /// period, a positive term limit and minimum commitment (when set), and a minimum commitment no
    /// longer than the term limit.
    ///
    /// Implementations may require more (for example, of the price).
    pub fn is_well_formed(&self) -> bool {
        !self.period.is_zero()
            && self.grace < self.period
            && self.term != Some(0)
            && self.min_commitment != Some(0)
            && match (self.min_commitment, self.term) {
                (Some(commitment), Some(term)) => commitment <= term,
                _ => true,
            }
    }

    /// The commitment end of a subscription anchored at `anchor`: `anchor + c · period`, where `c`
    /// is the minimum commitment, or `anchor` when there is none. Saturating.
    pub fn commitment_end(&self, anchor: Moment) -> Moment {
        let periods = Moment::from(self.min_commitment.unwrap_or(0));
        anchor.saturating_add(self.period.saturating_mul(periods))
    }
}

/// Whether, and with how much notice, a merchant may amend the subscriptions to an item.
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
pub enum AmendmentPolicy {
    /// No amendment, of one subscription or of the item's.
    #[default]
    Disabled,
    /// An amendment takes effect at the first due tick at least `periods` billing periods after
    /// its enactment. `periods` is at least 1.
    WithNotice {
        /// The notice, in billing periods.
        periods: u32,
    },
}

/// What a merchant may do to the subscriptions to an item, beyond what every subscription allows.
/// The default allows nothing: no amendment, and no termination.
///
/// It is kept beside the item's conditions. Each subscription keeps the policy its item had when
/// it started (in [`Subscription::policy`]), so a later change of the item's policy affects new
/// subscriptions only.
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
pub struct SubscriptionPolicy {
    /// Whether, and with how much notice, the subscriptions may be amended.
    pub amendments: AmendmentPolicy,
    /// Whether the merchant may terminate a subscription at once.
    pub terminable_by_merchant: bool,
}

impl SubscriptionPolicy {
    /// Whether the policy is valid: a notice of at least one billing period.
    pub fn is_well_formed(&self) -> bool {
        !matches!(self.amendments, AmendmentPolicy::WithNotice { periods: 0 })
    }

    /// The notice of an amendment, in billing periods, or `None` when amendments are disabled.
    pub fn notice_periods(&self) -> Option<u32> {
        match self.amendments {
            AmendmentPolicy::Disabled => None,
            AmendmentPolicy::WithNotice { periods } => Some(periods),
        }
    }
}

/// The state of a live subscription. An ended subscription has no record.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub enum SubscriptionState<Moment> {
    /// Paid up to its `paid_through`.
    Active,
    /// A charge is due and unpaid, and its grace has not elapsed. `since` is the tick of the
    /// failed attempt.
    Suspended { since: Moment },
    /// It lapsed within its minimum commitment. Kept, with no charge attempted, until its
    /// commitment end (`until`).
    Defaulted { until: Moment },
}

/// Why a subscription ended.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub enum EndReason {
    /// Its term limit's last period was paid and ended.
    Completed,
    /// The subscriber cancelled it.
    Cancelled,
    /// A charge stayed unpaid past grace, or a *Defaulted* subscription reached its commitment end.
    Lapsed,
    /// A scheduled replacement took effect: a subscription to another item started in its place.
    Replaced,
    /// The merchant terminated it.
    Terminated,
}

/// A cancellation the subscriber requested, which takes effect at a later due tick.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub enum Cancellation {
    /// Billing continues, period by period, until the commitment end. The subscription ends at the
    /// first due tick at or after both `paid_through` and the commitment end.
    Ordinary,
    /// Requested while an amendment was pending: the minimum commitment is waived, and the
    /// subscription ends at its `paid_through`.
    FreeExit,
}

/// An amendment of one subscription, pending until its effective boundary.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub struct PendingConditions<Price, Moment> {
    /// The conditions that apply from `effective_at`.
    pub conditions: SubscriptionConditions<Price, Moment>,
    /// The due tick from which the conditions apply. The charge for the billing period that
    /// starts there is never attempted before it.
    pub effective_at: Moment,
}

/// A subscription to an item.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub struct Subscription<Price, Moment, ItemId> {
    /// The conditions in force, copied from the item at subscription and changed only by an
    /// amendment.
    pub conditions: SubscriptionConditions<Price, Moment>,
    /// The tick at which it started. Every billing period is counted from it.
    pub anchor: Moment,
    /// The tick up to which its billing is settled. It is the due tick of the next charge.
    pub paid_through: Moment,
    /// How many billing periods have been charged.
    pub periods_charged: u32,
    /// Its state.
    pub state: SubscriptionState<Moment>,
    /// A cancellation requested by the subscriber, if any.
    pub cancel_requested: Option<Cancellation>,
    /// An item that replaces this one at the first due tick at or after the commitment end.
    pub replacement: Option<ItemId>,
    /// An amendment of this subscription, pending until its effective boundary.
    pub pending_conditions: Option<PendingConditions<Price, Moment>>,
    /// The sequence number of the last amendment of the item ([`ItemAmendment`]) this
    /// subscription has applied, or was subscribed after.
    pub amendment_seq: u32,
    /// Whether the scheduled replacement also waits for the end of the term limit (see
    /// [`Mutate::schedule_replacement_at_term_end`]). Meaningful only while `replacement` is set.
    pub replacement_at_term_end: bool,
    /// The policy of its item when it started (or when it replaced another subscription), kept
    /// for its whole life: a later change of the item's policy never applies to it.
    pub policy: SubscriptionPolicy,
}

impl<Price, Moment: AtLeast32BitUnsigned + Copy, ItemId> Subscription<Price, Moment, ItemId> {
    /// The commitment end: `anchor + c · period`, or `anchor` when there is no commitment.
    pub fn commitment_end(&self) -> Moment {
        self.conditions.commitment_end(self.anchor)
    }

    /// Whether the term limit's last period has been charged, so no further charge is due.
    pub fn term_reached(&self) -> bool {
        self.conditions
            .term
            .is_some_and(|term| self.periods_charged >= term)
    }

    /// Whether the scheduled replacement, if any, takes effect at the due tick `paid_through`:
    /// it is at or after the commitment end and, for a replacement that waits for it, the term
    /// limit's last period is paid.
    pub fn replacement_due(&self) -> bool {
        self.replacement.is_some()
            && self.paid_through >= self.commitment_end()
            && (!self.replacement_at_term_end || self.term_reached())
    }

    /// How many billing periods of the term limit are left to charge. `None` when open-ended.
    pub fn term_remaining(&self) -> Option<u32> {
        self.conditions
            .term
            .map(|term| term.saturating_sub(self.periods_charged))
    }

    /// Whether the pool, or whatever the subscription pays for, is paid at `now`: *Active* and
    /// `now < paid_through`. Computed from the clock, never from whether any bookkeeping has run.
    pub fn is_paid(&self, now: Moment) -> bool {
        matches!(self.state, SubscriptionState::Active) && now < self.paid_through
    }
}

/// Who may subscribe to an item.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub enum Eligibility<AccountId> {
    /// Any subscriber, any number of independent subscriptions.
    Anyone,
    /// Exactly one subscriber, once.
    Once(AccountId),
    /// A [`Eligibility::Once`] item whose one subscription has started: no further subscription.
    Taken(AccountId),
}

impl<AccountId: PartialEq> Eligibility<AccountId> {
    /// Whether `who` may start a subscription to the item now.
    pub fn admits(&self, who: &AccountId) -> bool {
        match self {
            Eligibility::Anyone => true,
            Eligibility::Once(only) => only == who,
            Eligibility::Taken(_) => false,
        }
    }
}

/// An amendment of an item's conditions for every live subscription to it, applied lazily: each
/// subscription takes it at its own effective boundary.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub struct ItemAmendment<Price, Moment> {
    /// The amendment's sequence number for the item, starting at 1.
    pub seq: u32,
    /// The amended conditions.
    pub conditions: SubscriptionConditions<Price, Moment>,
    /// The tick at which it was enacted.
    pub enacted_at: Moment,
}

impl<Price, Moment: AtLeast32BitUnsigned + Copy> ItemAmendment<Price, Moment> {
    /// Every subscription with this amendment's billing period and a notice of `notice_periods`
    /// billing periods has its effective boundary before this tick:
    /// `enacted_at + (notice_periods + 1) · period`. Saturating.
    pub fn last_boundary(&self, notice_periods: u32) -> Moment {
        let periods = Moment::from(notice_periods.saturating_add(1));
        self.enacted_at
            .saturating_add(self.conditions.period.saturating_mul(periods))
    }
}

/// The effective boundary of an amendment enacted at `enacted_at`, with a notice of
/// `notice_periods` billing periods, for a subscription anchored at `anchor` with billing period
/// `period`: the first due tick `b = anchor + k · period` with
/// `b − enacted_at ≥ notice_periods · period`, that is
/// `anchor + ⌈(enacted_at + notice_periods · period − anchor) / period⌉ · period`. A notice of one
/// period is `REQ-CT-14`'s rule.
///
/// `None` if `period` is zero or the result overflows.
pub fn effective_boundary<Moment: AtLeast32BitUnsigned + Copy>(
    anchor: Moment,
    period: Moment,
    enacted_at: Moment,
    notice_periods: u32,
) -> Option<Moment> {
    if period.is_zero() {
        return None;
    }
    let notice = period.checked_mul(&Moment::from(notice_periods))?;
    let distance = enacted_at.checked_add(&notice)?.saturating_sub(anchor);
    let periods = distance
        .checked_add(&period.saturating_sub(One::one()))?
        .checked_div(&period)?;
    anchor.checked_add(&periods.checked_mul(&period)?)
}

/// Why a scheduled replacement did not take effect.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub enum ReplacementDropReason {
    /// A dependant refused it (see [`OnSubscriptionChanged::allow_replacement`]).
    Refused(DispatchError),
    /// The new item has no subscription conditions, or cannot be subscribed to.
    NotSubscribable,
    /// The subscriber is not eligible for the new item.
    NotEligible,
    /// The new subscription's first charge failed.
    ChargeFailed,
    /// An amendment covering the subscription was enacted.
    AmendmentEnacted,
    /// The subscriber cancelled the subscription.
    SubscriptionCancelled,
    /// The subscription lapsed within its commitment.
    SubscriptionDefaulted,
    /// The subscriber cancelled the replacement.
    ReplacementCancelled,
    /// The merchant terminated the subscription.
    SubscriptionTerminated,
}

/// The refusals of a subscriptions system that a consumer tells apart, to answer with its own
/// errors ([`Inspect::subscription_error`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubscriptionError {
    /// The item has no subscription conditions, they were withdrawn, or it cannot be subscribed
    /// to.
    NotSubscribable,
    /// The subscription conditions, or the subscription policy, are not valid.
    InvalidConditions,
    /// The subscriber is not eligible for the item.
    NotEligible,
    /// The subscriber already has a live subscription to the item.
    AlreadySubscribed,
    /// There is no subscription in a state the operation accepts.
    NoSubscription,
    /// No charge is due, within lead, or within grace.
    NothingDue,
    /// The subscription's grace has elapsed.
    GraceElapsed,
    /// The charge could not be made.
    ChargeFailed,
    /// A replacement is already scheduled.
    ReplacementPending,
    /// An amendment is pending, or a previous one is still to be applied.
    ChangePending,
    /// There is no scheduled replacement to cancel.
    NoPendingReplacement,
    /// A cancellation is pending.
    CancelPending,
    /// The subscription's (or the item's) policy allows no amendment.
    AmendmentsDisabled,
    /// The subscription's policy does not let the merchant terminate it.
    NotTerminable,
    /// A direct subscription to an item priced at zero.
    ZeroPriceDirectSubscription,
}

/// A termination of a subscription, and its dispute, if any. Disputes are not supported yet.
#[derive(Encode, Decode)]
pub struct SubscriptionTermination<AccountId, Price, Moment, ItemId, Reason> {
    pub when: Moment,
    pub reason: Reason,
    pub subscription: Subscription<Price, Moment, ItemId>,
    pub maybe_dispute: Option<TerminationDispute<AccountId, Reason>>,
}

/// A submitted dispute over a termination.
#[derive(Encode, Decode)]
pub struct TerminationDispute<AccountId, Reason> {
    pub reason: Reason,
    pub state: DisputeState<AccountId, Reason>,
}

/// The state of a dispute.
#[derive(Default, Encode, Decode)]
pub enum DisputeState<AccountId, Reason> {
    #[default]
    /// A dispute process for the termination has been submitted.
    Submitted,
    /// The dispute has been assigned to a judge. A judge can be any actor in the system in
    /// charge of reviewing a dispute and issuing a resolution (accepting or rejecting it).
    Assigned(AccountId),
    /// The dispute is being reviewed. This process can take any amount of time (or even be
    /// skipped at all if the judge resolves immediately upon being assigned with the dispute).
    InReview(AccountId),
    /// The dispute is resolved in favour to the submitter by a judge. A reason must be given.
    Accepted(AccountId, Reason),
    /// The dispute is resolved in favour to the terminator by a judge. A reason must be given.
    Rejected(AccountId, Reason),
}

impl<AccountId: Clone, Reason> TerminationDispute<AccountId, Reason> {
    /// Initializes a new dispute.
    pub fn new(reason: Reason) -> Self {
        Self {
            reason,
            state: Default::default(),
        }
    }

    /// Assigns a dispute to a judge.
    pub fn assign(&mut self, judge: AccountId) -> DispatchResult {
        match self.state {
            DisputeState::Submitted => {
                self.state = DisputeState::Assigned(judge);
                Ok(())
            }
            _ => Err(DispatchError::Other("Invalid state")),
        }
    }

    /// Bumps the dispute towards an [DisputeState::InReview] state.
    pub fn start_review(&mut self) -> DispatchResult {
        match &self.state {
            DisputeState::Assigned(judge) => {
                self.state = DisputeState::InReview(judge.clone());
                Ok(())
            }
            _ => Err(DispatchError::Other("Invalid state")),
        }
    }

    /// Resolves the dispute in favour of the submitter.
    pub fn approve(&mut self, reason: Reason) -> DispatchResult {
        match &self.state {
            DisputeState::Assigned(judge) | DisputeState::InReview(judge) => {
                self.state = DisputeState::Accepted(judge.clone(), reason);
                Ok(())
            }
            _ => Err(DispatchError::Other("Invalid state")),
        }
    }

    /// Resolves the dispute in favour of the terminator.
    pub fn reject(&mut self, reason: Reason) -> DispatchResult {
        match &self.state {
            DisputeState::Assigned(judge) | DisputeState::InReview(judge) => {
                self.state = DisputeState::Rejected(judge.clone(), reason);
                Ok(())
            }
            _ => Err(DispatchError::Other("Invalid state")),
        }
    }
}

/// The error the dispute methods return: disputes over a termination are not supported.
pub const DISPUTES_UNSUPPORTED: DispatchError =
    DispatchError::Other("subscription disputes are not supported");

pub use {Inspect as InspectSubscription, Mutate as MutateSubscription};

/// The price of an item of `T`.
pub type ItemPriceOf<T, AccountId> =
    ItemPrice<<T as InspectItem<AccountId>>::Asset, <T as InspectItem<AccountId>>::Balance>;
/// The clock type of `T`.
pub type MomentOf<T, AccountId> = <T as Inspect<AccountId>>::Moment;
/// [`SubscriptionConditions`] of `T`.
pub type SubscriptionConditionsOf<T, AccountId> =
    SubscriptionConditions<ItemPriceOf<T, AccountId>, MomentOf<T, AccountId>>;
/// [`Subscription`] of `T`.
pub type SubscriptionOf<T, AccountId> = Subscription<
    ItemPriceOf<T, AccountId>,
    MomentOf<T, AccountId>,
    <T as InspectItem<AccountId>>::ItemId,
>;
/// [`PendingConditions`] of `T`.
pub type PendingConditionsOf<T, AccountId> =
    PendingConditions<ItemPriceOf<T, AccountId>, MomentOf<T, AccountId>>;
/// [`ItemAmendment`] of `T`.
pub type ItemAmendmentOf<T, AccountId> =
    ItemAmendment<ItemPriceOf<T, AccountId>, MomentOf<T, AccountId>>;
/// [`SubscriptionTermination`] of `T`.
pub type SubscriptionTerminationOf<T, AccountId, Reason> = SubscriptionTermination<
    AccountId,
    ItemPriceOf<T, AccountId>,
    MomentOf<T, AccountId>,
    <T as InspectItem<AccountId>>::ItemId,
    Reason,
>;

/// Methods to read subscription items and subscriptions. A subscription is keyed by
/// `(inventory, item, subscriber)`.
pub trait Inspect<AccountId>: InspectItem<AccountId> {
    /// A tick of the chain clock every period, anchor and due tick is counted in.
    type Moment: AtLeast32BitUnsigned + Parameter + MaxEncodedLen + Copy;

    /// The [`SubscriptionConditions`] new subscriptions to an item take, if it has any.
    fn subscription_conditions(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> Option<SubscriptionConditionsOf<Self, AccountId>>;

    /// Who may subscribe to an item, if it has subscription conditions.
    fn eligibility(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> Option<Eligibility<AccountId>>;

    /// The [`SubscriptionPolicy`] new subscriptions to an item take, if it has subscription
    /// conditions.
    fn policy(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> Option<SubscriptionPolicy>;

    /// Whether the item's subscription conditions were withdrawn: no new subscription, and no
    /// replacement into it, until conditions are set again. They stay readable through
    /// [`Inspect::subscription_conditions`]; existing subscriptions keep their own.
    fn conditions_withdrawn(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> bool;

    /// The last amendment of an item's conditions for every subscription to it, if any.
    fn item_amendment(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> Option<ItemAmendmentOf<Self, AccountId>>;

    /// The live subscription of `who` to an item, if any, as stored.
    fn subscription(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
    ) -> Option<SubscriptionOf<Self, AccountId>>;

    /// Whether the subscription is paid at `now`: *Active* and `now < paid_through`. One read.
    fn is_paid(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
        now: Self::Moment,
    ) -> bool {
        Self::subscription(inventory_id, id, who).is_some_and(|s| s.is_paid(now))
    }

    /// The subscription's commitment end (its anchor, when it has no commitment).
    fn commitment_end(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
    ) -> Option<Self::Moment> {
        Self::subscription(inventory_id, id, who).map(|s| s.commitment_end())
    }

    /// The grace end of the subscription's next (or unpaid) charge, extended by every migration
    /// pause that counts against it. `None` if there is no subscription, it is *Defaulted*, or no
    /// further charge is due.
    fn grace_end(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
    ) -> Option<Self::Moment>;

    /// The due tick of the subscription's next (or unpaid) charge. `None` if there is no
    /// subscription, it is *Defaulted*, or no further charge is due (its term limit is reached, or
    /// a cancellation takes effect at `paid_through`).
    fn next_due(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
    ) -> Option<Self::Moment>;

    /// The amendment pending for the subscription at `now`, of the subscription itself or of its
    /// item, with its effective boundary. While it is pending, cancelling waives the commitment.
    fn pending_amendment(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
        now: Self::Moment,
    ) -> Option<PendingConditionsOf<Self, AccountId>>;

    /// Whether the subscription's record blocks a new subscription at `now`: it exists, unless it
    /// is *Defaulted* and its commitment end has passed.
    fn blocks_new_subscription(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
        now: Self::Moment,
    ) -> bool {
        Self::subscription(inventory_id, id, who).is_some_and(|s| match s.state {
            SubscriptionState::Defaulted { until } => now < until,
            _ => true,
        })
    }

    /// Which of the [`SubscriptionError`]s `error` is, if it is one of this system's refusals,
    /// so a consumer can answer with its own error of the same meaning. Defaults to `None`.
    fn subscription_error(_error: &DispatchError) -> Option<SubscriptionError> {
        None
    }

    /// If a subscription termination has been disputed, retrieves the [TerminationDispute]
    /// information of such subscription. Disputes are not supported: `None`.
    fn dispute<Reason: Encode>(
        _inventory_id: &InventoryIdOf<Self, AccountId>,
        _id: &Self::ItemId,
        _who: &AccountId,
    ) -> Option<SubscriptionTerminationOf<Self, AccountId, Reason>> {
        None
    }
}

/// Methods to change subscription items and subscriptions.
///
/// Each method is transactional: an error changes nothing.
pub trait Mutate<AccountId>: Inspect<AccountId> {
    /// Sets the [`SubscriptionConditions`] new subscriptions to an existing item take. Existing
    /// subscriptions are not touched. An item given conditions for the first time admits anyone;
    /// its eligibility is kept otherwise.
    fn set_conditions(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        conditions: SubscriptionConditionsOf<Self, AccountId>,
    ) -> DispatchResult;

    /// Makes an item with conditions subscribable by exactly `who`, once.
    fn set_exclusive(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
    ) -> DispatchResult;

    /// Sets the [`SubscriptionPolicy`] of an item with conditions, for new subscriptions. Existing
    /// subscriptions keep the policy they started with.
    fn set_policy(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        policy: SubscriptionPolicy,
    ) -> DispatchResult;

    /// Withdraws an item's subscription conditions: new subscriptions, and replacements into the
    /// item, are refused until conditions are set again. Existing subscriptions keep their
    /// conditions and renew, and an item amendment already enacted stays in force for them.
    fn withdraw_conditions(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult;

    /// Subscribes `who` to an item: charges billing period 0 first, then creates the subscription,
    /// *Active*, anchored now, with the item's conditions and policy. A failed charge creates
    /// nothing. A price of zero is accepted: the consumer that subscribes decides whether to offer
    /// one (`REQ-SB-11`).
    fn subscribe(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
    ) -> DispatchResult;

    /// Attempts the subscription's charge that is due, within its lead, or unpaid within grace.
    ///
    /// `Ok` when a transition happened (charged, restored, suspended, replaced). An error when
    /// nothing is due, grace has elapsed, or the attempt failed and nothing changed.
    fn charge_due(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
    ) -> DispatchResult;

    /// Applies the transitions due at the current tick that need no charge: a lapse or default
    /// past grace, and the end of a *Defaulted* subscription at its commitment end. `Ok` when
    /// there is nothing to do.
    fn settle(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
    ) -> DispatchResult;

    /// Cancels the subscription: at the later of `paid_through` and the commitment end, or, with
    /// an amendment pending, at `paid_through` with the commitment waived. A *Suspended*
    /// subscription outside its commitment, or with an amendment pending, ends at once. A pending
    /// replacement is dropped.
    fn cancel(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
    ) -> DispatchResult;

    /// Terminates the subscription at once, whatever its state, if its policy lets the merchant
    /// terminate it. Nothing is refunded. A pending replacement is dropped.
    fn terminate(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
    ) -> DispatchResult;

    /// Schedules a subscription to `new_id` (in the same inventory) to replace this one at the
    /// first due tick at or after both `paid_through` and the commitment end, if its first charge
    /// succeeds then.
    fn schedule_replacement(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
        new_id: &Self::ItemId,
    ) -> DispatchResult;

    /// Schedules a replacement like [`Mutate::schedule_replacement`] that also waits for the
    /// end of the subscription's term limit: it takes effect at the first due tick at or after
    /// both the commitment end and the term limit's end, if its first charge succeeds then. For an
    /// open-ended subscription it is [`Mutate::schedule_replacement`].
    ///
    /// This is a trial's conversion, and any switch scheduled during a trial (`REQ-CT-9`,
    /// `REQ-CT-13`).
    fn schedule_replacement_at_term_end(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
        new_id: &Self::ItemId,
    ) -> DispatchResult;

    /// Drops a scheduled replacement before it takes effect.
    fn cancel_replacement(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
    ) -> DispatchResult;

    /// Amends one subscription's conditions, without its subscriber's acceptance, if its policy
    /// allows amendments. They apply from its effective boundary, which is returned: the first due
    /// tick at least the policy's notice (in billing periods) from now. The billing period cannot
    /// change.
    fn amend(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        who: &AccountId,
        conditions: SubscriptionConditionsOf<Self, AccountId>,
    ) -> Result<Self::Moment, DispatchError>;

    /// Amends an item's conditions: at once for new subscriptions, and for every live
    /// subscription to it whose policy allows amendments at that subscription's own effective
    /// boundary (with its own notice), lazily. Returns the recorded amendment. The billing period
    /// cannot change.
    ///
    /// Refused if the item's policy allows no amendment, and while any subscription live when the
    /// previous item amendment was enacted has not yet applied it, been skipped by it, or ended.
    fn amend_item(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        conditions: SubscriptionConditionsOf<Self, AccountId>,
    ) -> Result<ItemAmendmentOf<Self, AccountId>, DispatchError>;

    /// Disputes a termination. Not supported.
    fn dispute_termination<Reason: Encode>(
        _inventory_id: &InventoryIdOf<Self, AccountId>,
        _id: &Self::ItemId,
        _who: &AccountId,
        _dispute_reason: Reason,
    ) -> DispatchResult {
        Err(DISPUTES_UNSUPPORTED)
    }

    /// Resolves a dispute over a termination. Not supported.
    fn resolve_termination_dispute<Reason: Encode>(
        _inventory_id: &InventoryIdOf<Self, AccountId>,
        _id: &Self::ItemId,
        _who: &AccountId,
        _dispute_reason: Reason,
    ) -> DispatchResult {
        Err(DISPUTES_UNSUPPORTED)
    }
}

/// Hooks called on every transition of a subscription, in the same block. Every method defaults
/// to doing nothing. Implemented for tuples: each member is called in order, and
/// [`allow_replacement`](OnSubscriptionChanged::allow_replacement) allows only if every member
/// does.
///
/// Each method receives the subscription's key: `(inventory, item, who)`.
///
/// - A subscription starts with `on_started` (billing period 0 is charged), or with
///   `on_replaced` on the subscription it replaces.
/// - Each later charge calls `on_charged`; a restoring one calls `on_restored` after it.
/// - An ended subscription calls `on_ended`, except one that was replaced, which calls
///   `on_replaced` (with the new item and anchor) instead: the new subscription is keyed
///   `(inventory, new_item, who)`.
///
/// The subscriptions system runs these hooks inside its own calls and its automatic processing,
/// so it weighs them with [`max_hook_weight`](OnSubscriptionChanged::max_hook_weight), once per
/// subscription it acts on.
pub trait OnSubscriptionChanged<InventoryId, ItemId, AccountId, Moment> {
    /// The worst-case weight of all the hooks one subscription can trigger in one call or one
    /// processed entry of the due queue, taken together: for example an amendment coming into
    /// force, then a replacement allowed and taking effect. The subscriptions system adds it to
    /// the weight of every call and every processed entry that can notify, so an implementation
    /// that reads or writes storage must cover its heaviest combination here. Defaults to zero,
    /// for hooks that do no storage work.
    fn max_hook_weight() -> Weight {
        Weight::zero()
    }

    /// A subscription started, and billing period 0 was charged.
    fn on_started(_inventory: &InventoryId, _item: &ItemId, _who: &AccountId) {}

    /// Billing period `period` (counted from 0) was charged; the subscription is now paid through
    /// `paid_through`.
    fn on_charged(
        _inventory: &InventoryId,
        _item: &ItemId,
        _who: &AccountId,
        _period: u32,
        _paid_through: Moment,
    ) {
    }

    /// A charge failed at or after its due tick; the subscription is suspended until it is paid
    /// or `grace_end` passes.
    fn on_suspended(
        _inventory: &InventoryId,
        _item: &ItemId,
        _who: &AccountId,
        _grace_end: Moment,
    ) {
    }

    /// A suspended subscription was restored by paying its unpaid charge (reported first with
    /// `on_charged`).
    fn on_restored(_inventory: &InventoryId, _item: &ItemId, _who: &AccountId) {}

    /// The subscription lapsed within its commitment: it is kept, with no charge attempted, until
    /// `until`.
    fn on_defaulted(_inventory: &InventoryId, _item: &ItemId, _who: &AccountId, _until: Moment) {}

    /// An amendment of this subscription was enacted. It applies from `effective_at`.
    fn on_amendment_enacted(
        _inventory: &InventoryId,
        _item: &ItemId,
        _who: &AccountId,
        _effective_at: Moment,
    ) {
    }

    /// An amendment of the item's conditions, its `seq`-th, was enacted at `enacted_at`. Each
    /// subscription it covers takes it at its own effective boundary, reported with
    /// `on_amendment_in_force`.
    fn on_item_amendment_enacted(
        _inventory: &InventoryId,
        _item: &ItemId,
        _seq: u32,
        _enacted_at: Moment,
    ) {
    }

    /// An amendment (of this subscription, or of its item) came into force at `effective_at`.
    fn on_amendment_in_force(
        _inventory: &InventoryId,
        _item: &ItemId,
        _who: &AccountId,
        _effective_at: Moment,
    ) {
    }

    /// The subscription ended, and its record was removed.
    fn on_ended(_inventory: &InventoryId, _item: &ItemId, _who: &AccountId, _reason: EndReason) {}

    /// The subscription was replaced: it ended, and a subscription of `who` to `new_item`, anchored
    /// at `new_anchor`, started with its billing period 0 charged.
    fn on_replaced(
        _inventory: &InventoryId,
        _item: &ItemId,
        _who: &AccountId,
        _new_item: &ItemId,
        _new_anchor: Moment,
    ) {
    }

    /// Asked when a scheduled replacement is about to take effect. An error refuses it, and is
    /// reported as the reason it was dropped.
    fn allow_replacement(
        _inventory: &InventoryId,
        _item: &ItemId,
        _who: &AccountId,
        _new_item: &ItemId,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    /// A scheduled replacement by `new_item` was dropped, for `reason`. The subscription goes on.
    fn on_replacement_dropped(
        _inventory: &InventoryId,
        _item: &ItemId,
        _who: &AccountId,
        _new_item: &ItemId,
        _reason: ReplacementDropReason,
    ) {
    }
}

#[impl_for_tuples(16)]
impl<InventoryId, ItemId, AccountId, Moment: Copy>
    OnSubscriptionChanged<InventoryId, ItemId, AccountId, Moment> for Tuple
{
    /// The sum of every member's: each member runs for the same event.
    fn max_hook_weight() -> Weight {
        let mut weight = Weight::zero();
        for_tuples!( #( weight = weight.saturating_add(Tuple::max_hook_weight()); )* );
        weight
    }

    fn on_started(inventory: &InventoryId, item: &ItemId, who: &AccountId) {
        for_tuples!( #( Tuple::on_started(inventory, item, who); )* );
    }

    fn on_charged(
        inventory: &InventoryId,
        item: &ItemId,
        who: &AccountId,
        period: u32,
        paid_through: Moment,
    ) {
        for_tuples!( #( Tuple::on_charged(inventory, item, who, period, paid_through); )* );
    }

    fn on_suspended(inventory: &InventoryId, item: &ItemId, who: &AccountId, grace_end: Moment) {
        for_tuples!( #( Tuple::on_suspended(inventory, item, who, grace_end); )* );
    }

    fn on_restored(inventory: &InventoryId, item: &ItemId, who: &AccountId) {
        for_tuples!( #( Tuple::on_restored(inventory, item, who); )* );
    }

    fn on_defaulted(inventory: &InventoryId, item: &ItemId, who: &AccountId, until: Moment) {
        for_tuples!( #( Tuple::on_defaulted(inventory, item, who, until); )* );
    }

    fn on_amendment_enacted(
        inventory: &InventoryId,
        item: &ItemId,
        who: &AccountId,
        effective_at: Moment,
    ) {
        for_tuples!( #( Tuple::on_amendment_enacted(inventory, item, who, effective_at); )* );
    }

    fn on_item_amendment_enacted(
        inventory: &InventoryId,
        item: &ItemId,
        seq: u32,
        enacted_at: Moment,
    ) {
        for_tuples!( #( Tuple::on_item_amendment_enacted(inventory, item, seq, enacted_at); )* );
    }

    fn on_amendment_in_force(
        inventory: &InventoryId,
        item: &ItemId,
        who: &AccountId,
        effective_at: Moment,
    ) {
        for_tuples!( #( Tuple::on_amendment_in_force(inventory, item, who, effective_at); )* );
    }

    fn on_ended(inventory: &InventoryId, item: &ItemId, who: &AccountId, reason: EndReason) {
        for_tuples!( #( Tuple::on_ended(inventory, item, who, reason.clone()); )* );
    }

    fn on_replaced(
        inventory: &InventoryId,
        item: &ItemId,
        who: &AccountId,
        new_item: &ItemId,
        new_anchor: Moment,
    ) {
        for_tuples!( #( Tuple::on_replaced(inventory, item, who, new_item, new_anchor); )* );
    }

    fn allow_replacement(
        inventory: &InventoryId,
        item: &ItemId,
        who: &AccountId,
        new_item: &ItemId,
    ) -> Result<(), DispatchError> {
        for_tuples!( #( Tuple::allow_replacement(inventory, item, who, new_item)?; )* );
        Ok(())
    }

    fn on_replacement_dropped(
        inventory: &InventoryId,
        item: &ItemId,
        who: &AccountId,
        new_item: &ItemId,
        reason: ReplacementDropReason,
    ) {
        for_tuples!(
            #( Tuple::on_replacement_dropped(inventory, item, who, new_item, reason.clone()); )*
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Light;
    impl OnSubscriptionChanged<u32, u32, u64, u64> for Light {
        fn max_hook_weight() -> Weight {
            Weight::from_parts(10, 1)
        }
    }

    struct Heavy;
    impl OnSubscriptionChanged<u32, u32, u64, u64> for Heavy {
        fn max_hook_weight() -> Weight {
            Weight::from_parts(300, 20)
        }
    }

    struct Silent;
    impl OnSubscriptionChanged<u32, u32, u64, u64> for Silent {}

    #[test]
    fn hooks_with_no_storage_work_weigh_nothing() {
        assert_eq!(
            <Silent as OnSubscriptionChanged<u32, u32, u64, u64>>::max_hook_weight(),
            Weight::zero()
        );
        assert_eq!(
            <() as OnSubscriptionChanged<u32, u32, u64, u64>>::max_hook_weight(),
            Weight::zero()
        );
    }

    // 0009-A17
    #[test]
    fn default_policy_allows_no_amendment_and_no_termination() {
        let policy = SubscriptionPolicy::default();
        assert_eq!(policy.amendments, AmendmentPolicy::Disabled);
        assert!(!policy.terminable_by_merchant);
        assert_eq!(policy.notice_periods(), None);
        assert!(policy.is_well_formed());
    }

    // 0009-A17
    #[test]
    fn a_notice_is_at_least_one_billing_period() {
        let with_notice = |periods| SubscriptionPolicy {
            amendments: AmendmentPolicy::WithNotice { periods },
            terminable_by_merchant: false,
        };
        assert!(!with_notice(0).is_well_formed());
        assert!(with_notice(1).is_well_formed());
        assert_eq!(with_notice(3).notice_periods(), Some(3));
    }

    // REQ-CT-14, 0009-A17
    #[test]
    fn effective_boundary_is_the_first_due_tick_a_notice_away() {
        // Anchored at 5, period 30, enacted at 6.
        assert_eq!(effective_boundary(5u64, 30, 6, 1), Some(65));
        assert_eq!(effective_boundary(5u64, 30, 6, 2), Some(95));
        // Enacted on a due tick: exactly the notice later.
        assert_eq!(effective_boundary(5u64, 30, 35, 1), Some(65));
        assert_eq!(effective_boundary(5u64, 30, 35, 3), Some(125));
        assert_eq!(effective_boundary(5u64, 0, 6, 1), None);
        assert_eq!(effective_boundary(5u64, 30, u64::MAX - 10, 1), None);

        let amendment = ItemAmendment {
            seq: 1,
            conditions: SubscriptionConditions {
                price: 0u32,
                period: 30u64,
                term: None,
                min_commitment: None,
                grace: 0,
            },
            enacted_at: 6,
        };
        assert_eq!(amendment.last_boundary(1), 66);
        assert_eq!(amendment.last_boundary(2), 96);
    }

    #[test]
    fn tuple_of_hooks_weighs_the_sum_of_its_members() {
        type Hooks = (Light, Heavy, Silent);
        assert_eq!(
            <Hooks as OnSubscriptionChanged<u32, u32, u64, u64>>::max_hook_weight(),
            Weight::from_parts(310, 21)
        );
    }
}
