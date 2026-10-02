//! Recurring subscriptions: the lifecycle step, the due queue, migration pauses, and the
//! implementation of [`fc_traits_listings::item::subscriptions`].
//!
//! Every live subscription has exactly one entry in the due queue, in the bucket of its *next
//! action tick*: `paid_through − RenewalLead` for a plain renewal; `paid_through` when that due
//! tick needs more than a charge (an amendment's effective boundary, a replacement, a cancellation
//! or the term limit's end); the grace end of a *Suspended* subscription; the commitment end of a
//! *Defaulted* one. A bucket `k` holds the ticks in `((k − 1)·S, k·S]` and is processed once
//! `k·S ≤ now`, so no entry is processed before its tick.
//!
//! A full bucket overflows into the next ones. When those are full too, the entry goes to its
//! bucket's overflow ([`DueOverflow`], one record per entry), which the walk processes with the
//! bucket. So nothing is refused for a full queue, and a live subscription is never left
//! unqueued. A call that would move an entry to a later tick keeps it where it is: processing it
//! early only requeues it.
//!
//! [`ItemSubscriptionCounts`] keeps, per item, how many subscriptions are live and chargeable, and
//! how many are behind the item's last amendment: set from the first at the amendment's enactment,
//! and decremented, once per subscription, when it applies the amendment, is skipped by it (its
//! policy or its billing period does not admit it), defaults, or ends. O(1) per step.

use super::*;
use fc_traits_listings::item::subscriptions::effective_boundary;
use fc_traits_payments::DirectPayment;
use frame_support::{
    migrations::MigrationStatusHandler,
    storage::{with_storage_layer, with_transaction},
};
use sp_runtime::{
    traits::{CheckedAdd, One, Saturating},
    ArithmeticError, TransactionOutcome,
};

/// How many consecutive buckets an entry may overflow into when its bucket is full.
pub const MAX_QUEUE_PROBES: u32 = 3;

type KeyOf<T, I> = SubscriptionKeyOf<T, I>;

/// An amendment a subscription has not applied yet: its conditions, its effective boundary for
/// the subscription, and, for an item amendment, its sequence number.
type UnappliedAmendment<T, I> = (SubscriptionConditionsOf<T, I>, MomentOf<T, I>, Option<u32>);

/// What a step left of the subscription.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Outcome {
    /// The record changed, or was requeued: store it.
    Updated,
    /// Nothing changed.
    Unchanged,
    /// The subscription ended and its record was removed.
    Ended,
}

/// Who drives a step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// The due queue: do whatever is due, requeue otherwise.
    Queue,
    /// `charge_due`: attempt the charge that is due; refuse otherwise.
    ChargeDue,
    /// `settle`: only the transitions that need no charge.
    Settle,
}

/// The circumstances of a step.
#[derive(Clone, Copy)]
struct Ctx<Moment> {
    /// The current tick.
    now: Moment,
    /// The lowest bucket an entry may be queued in.
    floor: Moment,
}

/// Why a replacement could not take effect, convertible from the errors met on the way.
struct DropReason(ReplacementDropReason);

impl From<DispatchError> for DropReason {
    fn from(_: DispatchError) -> Self {
        DropReason(ReplacementDropReason::ChargeFailed)
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    // Clock and queue.

    /// The current tick of the chain clock.
    pub(crate) fn now() -> MomentOf<T, I> {
        T::BlockNumberProvider::current_block_number()
    }

    fn bucket_size() -> MomentOf<T, I> {
        T::DueBucketSize::get().max(One::one())
    }

    /// The bucket an action at `tick` is queued in: `⌈tick / DueBucketSize⌉`.
    pub(crate) fn bucket_of(tick: MomentOf<T, I>) -> MomentOf<T, I> {
        let size = Self::bucket_size();
        let quotient = tick / size;
        if (tick % size).is_zero() {
            quotient
        } else {
            quotient.saturating_add(One::one())
        }
    }

    /// The last bucket that may be processed at `now`.
    pub(crate) fn current_bucket(now: MomentOf<T, I>) -> MomentOf<T, I> {
        now / Self::bucket_size()
    }

    /// The lowest bucket an entry may be queued in from a call: the cursor, set to the current
    /// bucket the first time.
    pub(crate) fn queue_floor(now: MomentOf<T, I>) -> MomentOf<T, I> {
        DueCursor::<T, I>::get().unwrap_or_else(|| {
            let cursor = Self::current_bucket(now);
            DueCursor::<T, I>::put(cursor);
            cursor
        })
    }

    /// Queues `key` at the bucket of `tick`, or the floor if later, overflowing into the next
    /// buckets when full. When those are full too, the entry goes to the overflow of its bucket.
    /// Never refused. Returns the bucket that holds the entry.
    fn enqueue(key: &KeyOf<T, I>, tick: MomentOf<T, I>, floor: MomentOf<T, I>) -> MomentOf<T, I> {
        let target = Self::bucket_of(tick).max(floor);
        let mut bucket = target;
        for _ in 0..MAX_QUEUE_PROBES {
            if DueQueue::<T, I>::mutate(bucket, |queue| queue.try_push(key.clone()).is_ok()) {
                return bucket;
            }
            bucket = bucket.saturating_add(One::one());
        }
        DueOverflow::<T, I>::insert(target, key, ());
        target
    }

    /// Removes `key`'s entry from `bucket`, or from its overflow.
    fn dequeue(key: &KeyOf<T, I>, bucket: MomentOf<T, I>) {
        let mut found = false;
        DueQueue::<T, I>::mutate_exists(bucket, |maybe_queue| {
            if let Some(queue) = maybe_queue {
                let len = queue.len();
                queue.retain(|queued| queued != key);
                found = queue.len() < len;
                if queue.is_empty() {
                    *maybe_queue = None;
                }
            }
        });
        if !found {
            DueOverflow::<T, I>::remove(bucket, key);
        }
    }

    /// Moves the subscription's entry to the bucket of `tick`. An entry no later than that is
    /// kept: when it is processed, the step finds nothing due yet and requeues it.
    fn requeue(
        key: &KeyOf<T, I>,
        record: &mut SubscriptionRecordOf<T, I>,
        tick: MomentOf<T, I>,
        ctx: Ctx<MomentOf<T, I>>,
    ) {
        let target = Self::bucket_of(tick).max(ctx.floor);
        if record.queued_at.is_some_and(|bucket| bucket <= target) {
            return;
        }
        if let Some(bucket) = record.queued_at.take() {
            Self::dequeue(key, bucket);
        }
        record.queued_at = Some(Self::enqueue(key, tick, ctx.floor));
    }

    /// Whether a record of `key` blocks a new subscription at `now`. A *Defaulted* record past its
    /// commitment end never does: it is ended (*lapsed*) first.
    fn blocks_new(key: &KeyOf<T, I>, now: MomentOf<T, I>) -> bool {
        let Some(mut existing) = Self::record(key) else {
            return false;
        };
        match existing.subscription.state {
            SubscriptionState::Defaulted { until } if now >= until => {
                Self::end(key, &mut existing, EndReason::Lapsed);
                false
            }
            _ => true,
        }
    }

    // Records.

    fn record(key: &KeyOf<T, I>) -> Option<SubscriptionRecordOf<T, I>> {
        Subscriptions::<T, I>::get((key.0, key.1, key.2.clone()))
    }

    fn store(key: &KeyOf<T, I>, record: &SubscriptionRecordOf<T, I>) {
        Subscriptions::<T, I>::insert((key.0, key.1, key.2.clone()), record)
    }

    // The item's counts.

    /// The sequence number of the item's last amendment, 0 when it has none.
    fn item_seq(key: &KeyOf<T, I>) -> u32 {
        ItemAmendments::<T, I>::get((key.0, key.1)).map_or(0, |amendment| amendment.seq)
    }

    fn update_counts(
        item: (InventoryIdFor<T, I>, ItemIdOf<T, I>),
        f: impl FnOnce(&mut ItemCounts),
    ) {
        ItemSubscriptionCounts::<T, I>::mutate_exists(item, |maybe_counts| {
            let mut counts = maybe_counts.take().unwrap_or_default();
            f(&mut counts);
            *maybe_counts = (counts != ItemCounts::default()).then_some(counts);
        });
    }

    /// A subscription to the item started: one more live.
    fn count_started(key: &KeyOf<T, I>) {
        Self::update_counts((key.0, key.1), |counts| {
            counts.live = counts.live.saturating_add(1)
        });
    }

    /// A live subscription that is not *Defaulted* ends or defaults: one less live, and one less
    /// behind if it had not applied the item's last amendment.
    fn count_left(key: &KeyOf<T, I>, sub: &SubscriptionOf<T, I>) {
        let behind = sub.amendment_seq < Self::item_seq(key);
        Self::update_counts((key.0, key.1), |counts| {
            counts.live = counts.live.saturating_sub(1);
            if behind {
                counts.behind = counts.behind.saturating_sub(1);
            }
        });
    }

    /// A subscription's sequence number moved from `before` to `after`: if it reached the item's
    /// last amendment, applied or skipped, one less behind.
    fn count_caught_up(key: &KeyOf<T, I>, before: u32, after: u32) {
        let seq = Self::item_seq(key);
        if before < seq && after >= seq {
            Self::update_counts((key.0, key.1), |counts| {
                counts.behind = counts.behind.saturating_sub(1)
            });
        }
    }

    fn key(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
    ) -> KeyOf<T, I> {
        ((*inventory_id).into(), *id, who.clone())
    }

    fn inventory_of(key: &KeyOf<T, I>) -> InventoryIdTuple<T, I> {
        key.0.into()
    }

    // Conditions and amendments.

    /// Checks conditions: well formed, a period longer than the lead, and a price of zero or of
    /// at least an existing asset's minimum balance.
    pub(crate) fn validate_conditions(
        conditions: &SubscriptionConditionsOf<T, I>,
    ) -> DispatchResult {
        ensure!(
            conditions.is_well_formed() && conditions.period > T::RenewalLead::get(),
            Error::<T, I>::InvalidConditions
        );
        let ItemPrice { asset, amount } = &conditions.price;
        if !amount.is_zero() {
            use frame_support::traits::fungibles::Inspect;
            ensure!(
                T::Assets::asset_exists(asset.clone())
                    && *amount >= T::Assets::minimum_balance(asset.clone()),
                Error::<T, I>::InvalidConditions
            );
        }
        Ok(())
    }

    /// The amendment of the subscription's item that covers it and that it has not applied, with
    /// its effective boundary for the subscription, after the notice of the subscription's policy.
    /// An item amendment does not cover a subscription whose policy allows no amendment, one with
    /// a different billing period, or one with its own amendment pending.
    fn item_amendment_for(
        key: &KeyOf<T, I>,
        sub: &SubscriptionOf<T, I>,
    ) -> Option<(ItemAmendmentOf<T, I>, MomentOf<T, I>)> {
        let notice = sub.policy.notice_periods()?;
        let amendment = ItemAmendments::<T, I>::get((key.0, key.1))?;
        if amendment.seq <= sub.amendment_seq
            || sub.pending_conditions.is_some()
            || amendment.conditions.period != sub.conditions.period
        {
            return None;
        }
        let boundary = effective_boundary(
            sub.anchor,
            sub.conditions.period,
            amendment.enacted_at,
            notice,
        )?;
        Some((amendment, boundary))
    }

    /// The amendment the subscription has not applied yet, its own or its item's: the conditions,
    /// the effective boundary, and the item amendment's sequence number.
    fn amendment_of(
        key: &KeyOf<T, I>,
        sub: &SubscriptionOf<T, I>,
    ) -> Option<UnappliedAmendment<T, I>> {
        if let Some(pending) = &sub.pending_conditions {
            return Some((pending.conditions.clone(), pending.effective_at, None));
        }
        Self::item_amendment_for(key, sub)
            .map(|(amendment, boundary)| (amendment.conditions, boundary, Some(amendment.seq)))
    }

    /// The subscription as it stands at `now`, with an amendment whose effective boundary has
    /// passed in force even if no step has applied it yet. Bookkeeping only: nothing is stored.
    fn in_force_at(
        key: &KeyOf<T, I>,
        sub: &SubscriptionOf<T, I>,
        now: MomentOf<T, I>,
    ) -> SubscriptionOf<T, I> {
        let mut current = sub.clone();
        Self::absorb_amendment(key, &mut current, sub.paid_through.min(now));
        current
    }

    /// Whether an amendment is pending for the subscription at `now`: enacted, and its effective
    /// boundary still ahead. An item amendment enacted after the boundary of the subscription's
    /// own amendment is pending once that one is in force, applied or not.
    fn amendment_pending(
        key: &KeyOf<T, I>,
        sub: &SubscriptionOf<T, I>,
        now: MomentOf<T, I>,
    ) -> bool {
        Self::amendment_of(key, &Self::in_force_at(key, sub, now))
            .is_some_and(|(_, boundary, _)| now < boundary)
    }

    /// Takes an amendment effective at or before `due` into the subscription's conditions, and
    /// returns its effective boundary.
    fn absorb_amendment(
        key: &KeyOf<T, I>,
        sub: &mut SubscriptionOf<T, I>,
        due: MomentOf<T, I>,
    ) -> Option<MomentOf<T, I>> {
        let (conditions, boundary, seq) = Self::amendment_of(key, sub)?;
        if boundary > due {
            return None;
        }
        sub.conditions = conditions;
        sub.pending_conditions = None;
        let seq = seq.unwrap_or_else(|| {
            // A subscription's own amendment also settles the item amendment enacted before its
            // boundary, which did not cover it. One enacted from the boundary on covers it: it
            // stays pending, with its own boundary.
            ItemAmendments::<T, I>::get((key.0, key.1)).map_or(0, |a| {
                if a.enacted_at < boundary {
                    a.seq
                } else {
                    a.seq.saturating_sub(1)
                }
            })
        });
        sub.amendment_seq = seq.max(sub.amendment_seq);
        Some(boundary)
    }

    /// Brings an amendment effective at or before `due` into force.
    fn apply_amendment(key: &KeyOf<T, I>, sub: &mut SubscriptionOf<T, I>, due: MomentOf<T, I>) {
        let before = sub.amendment_seq;
        let Some(boundary) = Self::absorb_amendment(key, sub, due) else {
            return;
        };
        Self::count_caught_up(key, before, sub.amendment_seq);

        T::OnSubscriptionChanged::on_amendment_in_force(
            &Self::inventory_of(key),
            &key.1,
            &key.2,
            boundary,
        );
        Self::deposit_event(Event::<T, I>::AmendmentInForce {
            inventory_id: key.0,
            id: key.1,
            who: key.2.clone(),
            effective_at: boundary,
        });
    }

    /// Brings into force, for a call at `now`, an amendment whose effective boundary has passed
    /// before any step applied it, so a call sees what is pending now.
    fn apply_in_force(key: &KeyOf<T, I>, sub: &mut SubscriptionOf<T, I>, now: MomentOf<T, I>) {
        Self::apply_amendment(key, sub, sub.paid_through.min(now));
    }

    /// The subscription as it will stand at its due tick, with any amendment effective there.
    fn at_due(key: &KeyOf<T, I>, sub: &SubscriptionOf<T, I>) -> SubscriptionOf<T, I> {
        let mut at_due = sub.clone();
        if let Some((conditions, boundary, _)) = Self::amendment_of(key, sub) {
            if boundary <= sub.paid_through {
                at_due.conditions = conditions;
            }
        }
        at_due
    }

    // The schedule of an *Active* subscription.

    /// Whether the due tick needs more than a renewal charge, so the lead does not apply.
    fn needs_boundary(key: &KeyOf<T, I>, sub: &SubscriptionOf<T, I>) -> bool {
        let due = sub.paid_through;
        if matches!(sub.cancel_requested, Some(Cancellation::FreeExit)) {
            return true;
        }
        if Self::amendment_of(key, sub).is_some_and(|(_, boundary, _)| boundary <= due) {
            return true;
        }
        let commitment_end = sub.commitment_end();
        (matches!(sub.cancel_requested, Some(Cancellation::Ordinary)) && due >= commitment_end)
            || sub.replacement_due()
            || sub.term_reached()
    }

    /// Whether a charge is attempted at the due tick: of the renewal, or of a replacement.
    fn charge_expected(key: &KeyOf<T, I>, sub: &SubscriptionOf<T, I>) -> bool {
        if matches!(sub.cancel_requested, Some(Cancellation::FreeExit)) {
            return false;
        }
        let sub = Self::at_due(key, sub);
        let due = sub.paid_through;
        let commitment_end = sub.commitment_end();
        if matches!(sub.cancel_requested, Some(Cancellation::Ordinary)) && due >= commitment_end {
            return false;
        }
        !sub.term_reached() || (sub.replacement.is_some() && due >= commitment_end)
    }

    /// The tick of the subscription's next action.
    fn action_tick(
        key: &KeyOf<T, I>,
        record: &SubscriptionRecordOf<T, I>,
        now: MomentOf<T, I>,
    ) -> MomentOf<T, I> {
        let sub = &record.subscription;
        match sub.state {
            SubscriptionState::Active => {
                if Self::needs_boundary(key, sub) {
                    sub.paid_through
                } else {
                    sub.paid_through.saturating_sub(T::RenewalLead::get())
                }
            }
            SubscriptionState::Suspended { .. } => Self::grace_end_of(record, now),
            SubscriptionState::Defaulted { until } => until,
        }
    }

    // Migration pauses.

    /// The total length of the completed pauses that ended at or before `due`. Exact while at
    /// most one pause ended after `due`.
    fn paused_before(due: MomentOf<T, I>) -> MomentOf<T, I> {
        let total = PausedTicks::<T, I>::get();
        match LastPause::<T, I>::get() {
            Some((start, end)) if end > due => total.saturating_sub(end.saturating_sub(start)),
            _ => total,
        }
    }

    /// The grace end of the subscription's next (or unpaid) charge: `due + grace`, extended by the
    /// length of every migration pause that ends after the due tick and begins before the grace
    /// end extended so far, an ongoing one included.
    pub(crate) fn grace_end_of(
        record: &SubscriptionRecordOf<T, I>,
        now: MomentOf<T, I>,
    ) -> MomentOf<T, I> {
        let sub = &record.subscription;
        let due = sub.paid_through;
        let unextended = due.saturating_add(sub.conditions.grace);
        let before = match sub.state {
            SubscriptionState::Suspended { .. } => record.paused_before,
            _ => Self::paused_before(due),
        };

        let mut extension = PausedTicks::<T, I>::get().saturating_sub(before);
        if let Some((start, end)) = LastPause::<T, I>::get() {
            let length = end.saturating_sub(start);
            // The last pause is counted unless it began only after the grace end without it.
            if end > due
                && length <= extension
                && start >= unextended.saturating_add(extension.saturating_sub(length))
            {
                extension = extension.saturating_sub(length);
            }
        }

        let mut grace_end = unextended.saturating_add(extension);
        if let Some(start) = PauseStartedAt::<T, I>::get() {
            if start < grace_end {
                grace_end = grace_end.saturating_add(now.saturating_sub(start));
            }
        }
        grace_end
    }

    // Money.

    /// Charges `price` for billing period `period` of `key`: a direct payment from the subscriber
    /// to the inventory owner, or nothing for a zero price. A failure moves nothing: a direct
    /// payment is atomic on its own.
    fn charge(key: &KeyOf<T, I>, price: &ItemPriceOf<T, I>, period: u32) -> DispatchResult {
        if price.amount.is_zero() {
            return Ok(());
        }
        let owner =
            T::Nonfungibles::collection_owner(&key.0).ok_or(Error::<T, I>::UnknownInventory)?;
        T::Payments::pay(
            &key.2,
            price.asset.clone(),
            price.amount,
            &owner,
            Some((key.0, key.1, period)),
        )
        .map(|_| ())
    }

    /// Charges the subscription's next billing period and advances its `paid_through`.
    fn charge_next(key: &KeyOf<T, I>, record: &mut SubscriptionRecordOf<T, I>) -> DispatchResult {
        let sub = &mut record.subscription;
        let period = sub.periods_charged;
        let paid_through = sub
            .paid_through
            .checked_add(&sub.conditions.period)
            .ok_or(ArithmeticError::Overflow)?;
        Self::charge(key, &sub.conditions.price, period)?;

        sub.paid_through = paid_through;
        sub.periods_charged = period.saturating_add(1);
        // An item amendment that does not cover the subscription is settled for it: skipped.
        if let Some(amendment) = ItemAmendments::<T, I>::get((key.0, key.1)) {
            if amendment.seq > sub.amendment_seq && Self::item_amendment_for(key, sub).is_none() {
                Self::count_caught_up(key, sub.amendment_seq, amendment.seq);
                sub.amendment_seq = amendment.seq;
            }
        }

        T::OnSubscriptionChanged::on_charged(
            &Self::inventory_of(key),
            &key.1,
            &key.2,
            period,
            paid_through,
        );
        Self::deposit_event(Event::<T, I>::SubscriptionCharged {
            inventory_id: key.0,
            id: key.1,
            who: key.2.clone(),
            period,
            amount: sub.conditions.price.amount,
            paid_through,
        });
        Ok(())
    }

    // Transitions.

    /// Ends the subscription: removes its record and its entry, and notifies.
    fn end(key: &KeyOf<T, I>, record: &mut SubscriptionRecordOf<T, I>, reason: EndReason) {
        if let Some(bucket) = record.queued_at.take() {
            Self::dequeue(key, bucket);
        }
        // A *Defaulted* one left the counts when it defaulted.
        if !matches!(
            record.subscription.state,
            SubscriptionState::Defaulted { .. }
        ) {
            Self::count_left(key, &record.subscription);
        }
        Subscriptions::<T, I>::remove((key.0, key.1, key.2.clone()));
        T::OnSubscriptionChanged::on_ended(
            &Self::inventory_of(key),
            &key.1,
            &key.2,
            reason.clone(),
        );
        Self::deposit_event(Event::<T, I>::SubscriptionEnded {
            inventory_id: key.0,
            id: key.1,
            who: key.2.clone(),
            reason,
        });
    }

    /// Drops a scheduled replacement, if any, and notifies.
    fn drop_replacement(
        key: &KeyOf<T, I>,
        sub: &mut SubscriptionOf<T, I>,
        reason: ReplacementDropReason,
    ) {
        if let Some(new_id) = sub.replacement.take() {
            Self::notify_dropped(key, new_id, reason);
        }
    }

    fn notify_dropped(key: &KeyOf<T, I>, new_id: ItemIdOf<T, I>, reason: ReplacementDropReason) {
        T::OnSubscriptionChanged::on_replacement_dropped(
            &Self::inventory_of(key),
            &key.1,
            &key.2,
            &new_id,
            reason.clone(),
        );
        Self::deposit_event(Event::<T, I>::ReplacementDropped {
            inventory_id: key.0,
            id: key.1,
            who: key.2.clone(),
            new_id,
            reason,
        });
    }

    /// *Active* → *Suspended*, after a charge attempted at or after its due tick failed.
    fn suspend(key: &KeyOf<T, I>, record: &mut SubscriptionRecordOf<T, I>, now: MomentOf<T, I>) {
        record.paused_before = Self::paused_before(record.subscription.paid_through);
        record.subscription.state = SubscriptionState::Suspended { since: now };
        let grace_end = Self::grace_end_of(record, now);
        T::OnSubscriptionChanged::on_suspended(&Self::inventory_of(key), &key.1, &key.2, grace_end);
        Self::deposit_event(Event::<T, I>::SubscriptionSuspended {
            inventory_id: key.0,
            id: key.1,
            who: key.2.clone(),
            grace_end,
        });
    }

    /// *Suspended* past its grace end → *Defaulted* within the commitment, *Ended* (*lapsed*)
    /// otherwise.
    fn lapse(
        key: &KeyOf<T, I>,
        record: &mut SubscriptionRecordOf<T, I>,
        ctx: Ctx<MomentOf<T, I>>,
    ) -> Result<Outcome, DispatchError> {
        let due = record.subscription.paid_through;
        let commitment_end = record.subscription.commitment_end();
        if due >= commitment_end {
            Self::end(key, record, EndReason::Lapsed);
            return Ok(Outcome::Ended);
        }

        Self::drop_replacement(
            key,
            &mut record.subscription,
            ReplacementDropReason::SubscriptionDefaulted,
        );
        // It is charged no more: it leaves the counts, and no item amendment waits for it.
        Self::count_left(key, &record.subscription);
        record.subscription.state = SubscriptionState::Defaulted {
            until: commitment_end,
        };
        T::OnSubscriptionChanged::on_defaulted(
            &Self::inventory_of(key),
            &key.1,
            &key.2,
            commitment_end,
        );
        Self::deposit_event(Event::<T, I>::SubscriptionDefaulted {
            inventory_id: key.0,
            id: key.1,
            who: key.2.clone(),
            until: commitment_end,
        });

        if ctx.now >= commitment_end {
            Self::end(key, record, EndReason::Lapsed);
            return Ok(Outcome::Ended);
        }
        Self::requeue(key, record, commitment_end, ctx);
        Ok(Outcome::Updated)
    }

    /// Tries the replacement by `new_id` at the due tick `due`. On success the old subscription
    /// is gone, and the new one is stored and queued.
    fn replace(
        key: &KeyOf<T, I>,
        record: &mut SubscriptionRecordOf<T, I>,
        new_id: ItemIdOf<T, I>,
        due: MomentOf<T, I>,
        ctx: Ctx<MomentOf<T, I>>,
    ) -> Result<(), DropReason> {
        let inventory = Self::inventory_of(key);
        T::OnSubscriptionChanged::allow_replacement(&inventory, &key.1, &key.2, &new_id)
            .map_err(|e| DropReason(ReplacementDropReason::Refused(e)))?;

        let not_subscribable = DropReason(ReplacementDropReason::NotSubscribable);
        let new_key: KeyOf<T, I> = (key.0, new_id, key.2.clone());
        let mut item = ItemConditions::<T, I>::get((key.0, new_id)).ok_or(not_subscribable)?;
        if item.withdrawn || !Self::is_active(&inventory) || Self::blocks_new(&new_key, ctx.now) {
            return Err(DropReason(ReplacementDropReason::NotSubscribable));
        }
        if !item.eligibility.admits(&key.2) {
            return Err(DropReason(ReplacementDropReason::NotEligible));
        }

        let conditions = item.conditions.clone();
        let paid_through = due
            .checked_add(&conditions.period)
            .ok_or(DropReason(ReplacementDropReason::ChargeFailed))?;
        Self::charge(&new_key, &conditions.price, 0)?;

        // The old subscription ends.
        if let Some(bucket) = record.queued_at.take() {
            Self::dequeue(key, bucket);
        }
        Subscriptions::<T, I>::remove((key.0, key.1, key.2.clone()));
        Self::count_left(key, &record.subscription);

        // The new one starts, anchored at the old one's due tick, with the new item's policy.
        let policy = item.policy;
        if let Eligibility::Once(only) = &item.eligibility {
            item.eligibility = Eligibility::Taken(only.clone());
            ItemConditions::<T, I>::insert((key.0, new_id), item);
        }
        let mut new_record = SubscriptionRecord {
            subscription: Subscription {
                conditions,
                anchor: due,
                paid_through,
                periods_charged: 1,
                state: SubscriptionState::Active,
                cancel_requested: None,
                replacement: None,
                pending_conditions: None,
                amendment_seq: Self::item_seq(&new_key),
                replacement_at_term_end: false,
                policy,
            },
            queued_at: None,
            paused_before: Zero::zero(),
        };
        let tick = Self::action_tick(&new_key, &new_record, ctx.now);
        Self::requeue(&new_key, &mut new_record, tick, ctx);
        Self::store(&new_key, &new_record);
        Self::count_started(&new_key);

        T::OnSubscriptionChanged::on_replaced(&inventory, &key.1, &key.2, &new_id, due);
        Self::deposit_event(Event::<T, I>::SubscriptionReplaced {
            inventory_id: key.0,
            id: key.1,
            who: key.2.clone(),
            new_id,
            new_anchor: due,
        });
        Ok(())
    }

    /// The step of an *Active* subscription at or after its due tick (§5.2).
    fn at_due_tick(
        key: &KeyOf<T, I>,
        record: &mut SubscriptionRecordOf<T, I>,
        ctx: Ctx<MomentOf<T, I>>,
    ) -> Result<Outcome, DispatchError> {
        let due = record.subscription.paid_through;

        // With the commitment waived, it ends at `paid_through`, and no amendment applies.
        if matches!(
            record.subscription.cancel_requested,
            Some(Cancellation::FreeExit)
        ) {
            Self::end(key, record, EndReason::Cancelled);
            return Ok(Outcome::Ended);
        }

        Self::apply_amendment(key, &mut record.subscription, due);
        let commitment_end = record.subscription.commitment_end();

        if matches!(
            record.subscription.cancel_requested,
            Some(Cancellation::Ordinary)
        ) && due >= commitment_end
        {
            Self::end(key, record, EndReason::Cancelled);
            return Ok(Outcome::Ended);
        }

        if record.subscription.replacement_due() {
            if let Some(new_id) = record.subscription.replacement.take() {
                let replaced =
                    with_transaction(|| match Self::replace(key, record, new_id, due, ctx) {
                        Ok(()) => TransactionOutcome::Commit(Ok(())),
                        Err(reason) => TransactionOutcome::Rollback(Err(reason)),
                    });
                match replaced {
                    Ok(()) => return Ok(Outcome::Ended),
                    Err(DropReason(reason)) => Self::notify_dropped(key, new_id, reason),
                }
            }
        }

        if record.subscription.term_reached() {
            Self::end(key, record, EndReason::Completed);
            return Ok(Outcome::Ended);
        }

        if Self::charge_next(key, record).is_ok() {
            let tick = Self::action_tick(key, record, ctx.now);
            Self::requeue(key, record, tick, ctx);
            return Ok(Outcome::Updated);
        }

        Self::suspend(key, record, ctx.now);
        let grace_end = Self::grace_end_of(record, ctx.now);
        if ctx.now >= grace_end {
            return Self::lapse(key, record, ctx);
        }
        Self::requeue(key, record, grace_end, ctx);
        Ok(Outcome::Updated)
    }

    /// One step of a subscription at `ctx.now`: whatever is due, as `mode` allows.
    fn step(
        key: &KeyOf<T, I>,
        record: &mut SubscriptionRecordOf<T, I>,
        ctx: Ctx<MomentOf<T, I>>,
        mode: Mode,
    ) -> Result<Outcome, DispatchError> {
        let now = ctx.now;
        match record.subscription.state {
            SubscriptionState::Active => {
                let action_at = Self::action_tick(key, record, now);
                if now < action_at || mode == Mode::Settle {
                    return match mode {
                        Mode::ChargeDue => Err(Error::<T, I>::NothingDue.into()),
                        Mode::Settle => Ok(Outcome::Unchanged),
                        Mode::Queue => {
                            Self::requeue(key, record, action_at, ctx);
                            Ok(Outcome::Updated)
                        }
                    };
                }

                // A replacement scheduled before an item amendment was enacted never takes effect.
                if record.subscription.replacement.is_some()
                    && Self::item_amendment_for(key, &record.subscription).is_some()
                {
                    Self::drop_replacement(
                        key,
                        &mut record.subscription,
                        ReplacementDropReason::AmendmentEnacted,
                    );
                }

                let due = record.subscription.paid_through;
                if now < due {
                    // Within the lead: a plain renewal.
                    return match Self::charge_next(key, record) {
                        Ok(()) => {
                            let tick = Self::action_tick(key, record, now);
                            Self::requeue(key, record, tick, ctx);
                            Ok(Outcome::Updated)
                        }
                        Err(_) if mode == Mode::ChargeDue => {
                            Err(Error::<T, I>::ChargeFailed.into())
                        }
                        Err(_) => {
                            Self::requeue(key, record, due, ctx);
                            Ok(Outcome::Updated)
                        }
                    };
                }

                if mode == Mode::ChargeDue && !Self::charge_expected(key, &record.subscription) {
                    return Err(Error::<T, I>::NothingDue.into());
                }
                Self::at_due_tick(key, record, ctx)
            }
            SubscriptionState::Suspended { .. } => {
                let grace_end = Self::grace_end_of(record, now);
                if now >= grace_end {
                    return match mode {
                        Mode::ChargeDue => Err(Error::<T, I>::GraceElapsed.into()),
                        Mode::Queue | Mode::Settle => Self::lapse(key, record, ctx),
                    };
                }
                match mode {
                    Mode::Settle => Ok(Outcome::Unchanged),
                    Mode::Queue => {
                        Self::requeue(key, record, grace_end, ctx);
                        Ok(Outcome::Updated)
                    }
                    Mode::ChargeDue => {
                        Self::charge_next(key, record).map_err(|_| Error::<T, I>::ChargeFailed)?;
                        record.subscription.state = SubscriptionState::Active;
                        T::OnSubscriptionChanged::on_restored(
                            &Self::inventory_of(key),
                            &key.1,
                            &key.2,
                        );
                        Self::deposit_event(Event::<T, I>::SubscriptionRestored {
                            inventory_id: key.0,
                            id: key.1,
                            who: key.2.clone(),
                        });
                        let tick = Self::action_tick(key, record, now);
                        Self::requeue(key, record, tick, ctx);
                        Ok(Outcome::Updated)
                    }
                }
            }
            SubscriptionState::Defaulted { until } => {
                if mode == Mode::ChargeDue {
                    return Err(Error::<T, I>::GraceElapsed.into());
                }
                if now >= until {
                    Self::end(key, record, EndReason::Lapsed);
                    return Ok(Outcome::Ended);
                }
                match mode {
                    Mode::Queue => {
                        Self::requeue(key, record, until, ctx);
                        Ok(Outcome::Updated)
                    }
                    _ => Ok(Outcome::Unchanged),
                }
            }
        }
    }

    /// The circumstances of a call at `now`.
    fn call_ctx(now: MomentOf<T, I>) -> Ctx<MomentOf<T, I>> {
        Ctx {
            now,
            floor: Self::queue_floor(now),
        }
    }

    /// Runs a step for a call.
    fn call_step(key: &KeyOf<T, I>, mode: Mode) -> DispatchResult {
        let mut record = Self::record(key).ok_or(Error::<T, I>::NoSubscription)?;
        let ctx = Self::call_ctx(Self::now());
        if Self::step(key, &mut record, ctx, mode)? == Outcome::Updated {
            Self::store(key, &record);
        }
        Ok(())
    }

    // The due queue.

    /// `weight`, plus the worst case of the hooks one subscription can trigger
    /// ([`OnSubscriptionChanged::max_hook_weight`]): the weight of a call or a queue entry that
    /// can notify. This pallet's benchmarks cannot see the hooks' work, so it is added here.
    pub(crate) fn with_hooks(weight: Weight) -> Weight {
        weight.saturating_add(T::OnSubscriptionChanged::max_hook_weight())
    }

    /// The weight of processing one queued subscription, whatever it does, from its bucket or
    /// from the bucket's overflow, its hooks included.
    pub(crate) fn due_item_weight() -> Weight {
        Self::with_hooks(
            T::WeightInfo::process_due(1)
                .max(T::WeightInfo::process_due_replacement())
                .max(T::WeightInfo::process_due_overflow()),
        )
    }

    /// The weight `on_poll` and `on_idle` take before walking any bucket: the reads of the
    /// clock, `CatchUpFrom`, `DueCursor` and `PauseStartedAt`, and one write of a cursor. It is a
    /// constant rather than a benchmark, since each is one item of a known size; the walk itself
    /// is weighed with the benchmarks (`skip_empty_bucket` per bucket read, and an entry's
    /// worst case per entry).
    fn hook_base_weight() -> Weight {
        T::DbWeight::get().reads_writes(4, 1)
    }

    /// Processes the queued entry of `key` in `bucket`, taken out of the queue.
    fn process_entry(key: &KeyOf<T, I>, bucket: MomentOf<T, I>, now: MomentOf<T, I>) {
        let Some(mut record) = Self::record(key) else {
            return;
        };
        if record.queued_at != Some(bucket) {
            // A stale entry: the subscription is queued elsewhere.
            return;
        }
        record.queued_at = None;

        let ctx = Ctx {
            now,
            floor: bucket.saturating_add(One::one()),
        };
        let mut stepped = record.clone();
        let result = with_storage_layer(|| {
            if Self::step(key, &mut stepped, ctx, Mode::Queue)? == Outcome::Updated {
                Self::store(key, &stepped);
            }
            Ok::<_, DispatchError>(())
        });
        if result.is_err() {
            // Nothing changed: try again from the next bucket.
            record.queued_at = Some(Self::enqueue(key, now, ctx.floor));
            Self::store(key, &record);
        }
    }

    /// Walks the buckets `from..=to`, each with its overflow, processing at most `budget` entries
    /// within `meter`. Returns the first bucket not fully processed (`to + 1` when every one
    /// was).
    pub(crate) fn walk(
        from: MomentOf<T, I>,
        to: MomentOf<T, I>,
        now: MomentOf<T, I>,
        meter: &mut WeightMeter,
        budget: &mut u32,
    ) -> MomentOf<T, I> {
        let mut bucket = from;
        while bucket <= to {
            if meter
                .try_consume(T::WeightInfo::skip_empty_bucket())
                .is_err()
            {
                return bucket;
            }
            let entries = DueQueue::<T, I>::take(bucket);
            for (index, key) in entries.iter().enumerate() {
                if *budget == 0 || meter.try_consume(Self::due_item_weight()).is_err() {
                    // Keep the rest for later. Nothing was queued here meanwhile: every requeue
                    // goes to a later bucket.
                    let rest = entries.iter().skip(index).cloned().collect::<Vec<_>>();
                    DueQueue::<T, I>::insert(bucket, BoundedVec::truncate_from(rest));
                    return bucket;
                }
                *budget = budget.saturating_sub(1);
                Self::process_entry(key, bucket, now);
            }
            // Then its overflow, one entry at a time. Reading whether there is one more is part of
            // the bucket's weight, or of the entry's.
            while let Some(key) = DueOverflow::<T, I>::iter_key_prefix(bucket).next() {
                if *budget == 0 || meter.try_consume(Self::due_item_weight()).is_err() {
                    return bucket;
                }
                *budget = budget.saturating_sub(1);
                DueOverflow::<T, I>::remove(bucket, &key);
                Self::process_entry(&key, bucket, now);
            }
            // The last tick the clock can reach: nothing comes after it.
            let Some(next) = bucket.checked_add(&One::one()) else {
                return bucket;
            };
            bucket = next;
        }
        bucket
    }

    /// `on_idle`: the regular walk of the due queue, oldest bucket first, unless a catch-up after
    /// a migration pause is open.
    pub(crate) fn process_due_queue(meter: &mut WeightMeter) {
        if meter.try_consume(Self::hook_base_weight()).is_err() {
            return;
        }
        let now = Self::now();
        let current = Self::current_bucket(now);

        if let Some(catch_up) = CatchUpFrom::<T, I>::get() {
            if catch_up > current {
                CatchUpFrom::<T, I>::kill();
            }
            return;
        }

        let Some(cursor) = DueCursor::<T, I>::get() else {
            return;
        };
        if cursor > current {
            return;
        }
        let mut budget = T::MaxChargesPerBlock::get();
        let next = Self::walk(cursor, current, now, meter, &mut budget);
        if next != cursor {
            DueCursor::<T, I>::put(next);
        }
    }

    /// `on_poll`: after a migration pause, the buckets from the pause's start up to now, before
    /// any other due charge and before the block's transactions.
    pub(crate) fn process_catch_up(meter: &mut WeightMeter) {
        if meter.try_consume(Self::hook_base_weight()).is_err() {
            return;
        }
        let Some(catch_up) = CatchUpFrom::<T, I>::get() else {
            return;
        };
        let now = Self::now();
        let current = Self::current_bucket(now);
        let cursor = DueCursor::<T, I>::get();
        // Buckets before the regular cursor are empty.
        let from = cursor.map_or(catch_up, |cursor| catch_up.max(cursor));
        if from > current {
            CatchUpFrom::<T, I>::put(from);
            return;
        }

        let mut budget = T::MaxChargesPerBlock::get();
        let next = Self::walk(from, current, now, meter, &mut budget);
        CatchUpFrom::<T, I>::put(next);
        // The regular walk follows where the catch-up emptied its buckets.
        if cursor.is_some_and(|cursor| cursor >= catch_up) {
            DueCursor::<T, I>::put(next);
        }
    }

    /// Subscribes `who` to an item: the trait's `subscribe`, and call 9's. A zero price is
    /// accepted only when `zero_price_allowed` (a consumer's subscription, not a direct one).
    pub(crate) fn do_subscribe(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
        zero_price_allowed: bool,
    ) -> DispatchResult {
        with_storage_layer(|| {
            let key = Self::key(inventory_id, id, who);
            Self::ensure_active_inventory(&key.0)?;
            let mut item = ItemConditions::<T, I>::get((key.0, key.1))
                .filter(|item| !item.withdrawn)
                .ok_or(Error::<T, I>::NotSubscribable)?;
            ensure!(
                zero_price_allowed || !item.conditions.price.amount.is_zero(),
                Error::<T, I>::ZeroPriceDirectSubscription
            );
            ensure!(item.eligibility.admits(who), Error::<T, I>::NotEligible);

            let now = Self::now();
            ensure!(
                !Self::blocks_new(&key, now),
                Error::<T, I>::AlreadySubscribed
            );

            let conditions = item.conditions.clone();
            let policy = item.policy;
            let paid_through = now
                .checked_add(&conditions.period)
                .ok_or(ArithmeticError::Overflow)?;
            // Billing period 0, before anything exists.
            Self::charge(&key, &conditions.price, 0).map_err(|_| Error::<T, I>::ChargeFailed)?;

            if let Eligibility::Once(only) = &item.eligibility {
                item.eligibility = Eligibility::Taken(only.clone());
                ItemConditions::<T, I>::insert((key.0, key.1), item);
            }
            let mut record = SubscriptionRecord {
                subscription: Subscription {
                    conditions,
                    anchor: now,
                    paid_through,
                    periods_charged: 1,
                    state: SubscriptionState::Active,
                    cancel_requested: None,
                    replacement: None,
                    pending_conditions: None,
                    amendment_seq: Self::item_seq(&key),
                    replacement_at_term_end: false,
                    policy,
                },
                queued_at: None,
                paused_before: Zero::zero(),
            };
            // A new entry, never refused: a full queue overflows.
            let tick = Self::action_tick(&key, &record, now);
            Self::requeue(&key, &mut record, tick, Self::call_ctx(now));
            Self::store(&key, &record);
            Self::count_started(&key);

            T::OnSubscriptionChanged::on_started(inventory_id, id, who);
            Self::deposit_event(Event::<T, I>::SubscriptionStarted {
                inventory_id: key.0,
                id: key.1,
                who: key.2.clone(),
                anchor: now,
                paid_through,
            });
            Ok(())
        })
    }

    /// Schedules the replacement of a subscription by one to `new_id`, also waiting for the term
    /// limit's end when `at_term_end`: the trait's `schedule_replacement` and
    /// `schedule_replacement_at_term_end`, and call 13's. A target priced at zero is accepted
    /// only when `zero_price_allowed` (a consumer's replacement, not a direct one), as in
    /// [`do_subscribe`](Self::do_subscribe).
    ///
    /// The replacement takes the target item's conditions and policy as they stand at the
    /// replacement tick, not as they stood when it was scheduled (`AC-B8.1`).
    pub(crate) fn schedule(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
        new_id: &ItemIdOf<T, I>,
        at_term_end: bool,
        zero_price_allowed: bool,
    ) -> DispatchResult {
        with_storage_layer(|| {
            let key = Self::key(inventory_id, id, who);
            let mut record = Self::record(&key).ok_or(Error::<T, I>::NoSubscription)?;
            ensure!(
                matches!(record.subscription.state, SubscriptionState::Active),
                Error::<T, I>::NoSubscription
            );
            let now = Self::now();
            Self::apply_in_force(&key, &mut record.subscription, now);
            let sub = &record.subscription;
            ensure!(sub.replacement.is_none(), Error::<T, I>::ReplacementPending);
            ensure!(
                Self::amendment_of(&key, sub).is_none(),
                Error::<T, I>::ChangePending
            );
            ensure!(sub.cancel_requested.is_none(), Error::<T, I>::CancelPending);

            let item = ItemConditions::<T, I>::get((key.0, *new_id))
                .filter(|item| !item.withdrawn)
                .ok_or(Error::<T, I>::NotSubscribable)?;
            ensure!(
                zero_price_allowed || !item.conditions.price.amount.is_zero(),
                Error::<T, I>::ZeroPriceDirectSubscription
            );
            ensure!(item.eligibility.admits(who), Error::<T, I>::NotEligible);
            ensure!(
                !Self::blocks_new(&(key.0, *new_id, who.clone()), now),
                Error::<T, I>::AlreadySubscribed
            );

            record.subscription.replacement = Some(*new_id);
            record.subscription.replacement_at_term_end = at_term_end;
            let tick = Self::action_tick(&key, &record, now);
            Self::requeue(&key, &mut record, tick, Self::call_ctx(now));
            Self::store(&key, &record);

            Self::deposit_event(Event::<T, I>::ReplacementScheduled {
                inventory_id: key.0,
                id: key.1,
                who: key.2.clone(),
                new_id: *new_id,
            });
            Ok(())
        })
    }
}

impl<T: Config<I>, I: 'static> MigrationStatusHandler for Pallet<T, I> {
    /// A migration pause began: no transaction or background processing runs until it ends.
    fn started() {
        if !PauseStartedAt::<T, I>::exists() {
            PauseStartedAt::<T, I>::put(Self::now());
        }
    }

    /// A migration pause ended: its ticks never count against grace, and the charges it held up
    /// are processed first.
    ///
    /// `pallet-migrations` does not call it when a migration fails or is force-unstuck. Until it
    /// is called, the pause stays open and every grace end keeps growing, so a runtime whose
    /// failed-migration handler does not freeze the chain must call it from that handler.
    fn completed() {
        let Some(start) = PauseStartedAt::<T, I>::take() else {
            return;
        };
        let end = Self::now().max(start);
        PausedTicks::<T, I>::mutate(|total| *total = total.saturating_add(end - start));
        LastPause::<T, I>::put((start, end));
        let from = Self::bucket_of(start);
        CatchUpFrom::<T, I>::mutate(|catch_up| {
            *catch_up = Some(catch_up.map_or(from, |catch_up| catch_up.min(from)))
        });
    }
}

impl<T: Config<I>, I: 'static> subs::Inspect<AccountIdOf<T>> for Pallet<T, I> {
    type Moment = MomentOf<T, I>;

    fn subscription_conditions(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
    ) -> Option<SubscriptionConditionsOf<T, I>> {
        ItemConditions::<T, I>::get((InventoryIdFor::<T, I>::from(*inventory_id), *id))
            .map(|item| item.conditions)
    }

    fn eligibility(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
    ) -> Option<Eligibility<AccountIdOf<T>>> {
        ItemConditions::<T, I>::get((InventoryIdFor::<T, I>::from(*inventory_id), *id))
            .map(|item| item.eligibility)
    }

    fn policy(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
    ) -> Option<SubscriptionPolicy> {
        ItemConditions::<T, I>::get((InventoryIdFor::<T, I>::from(*inventory_id), *id))
            .map(|item| item.policy)
    }

    fn conditions_withdrawn(inventory_id: &InventoryIdTuple<T, I>, id: &ItemIdOf<T, I>) -> bool {
        ItemConditions::<T, I>::get((InventoryIdFor::<T, I>::from(*inventory_id), *id))
            .is_some_and(|item| item.withdrawn)
    }

    /// Matches this pallet's own errors, as this instance raises them.
    fn subscription_error(error: &DispatchError) -> Option<subs::SubscriptionError> {
        use subs::SubscriptionError as E;
        [
            (Error::<T, I>::NotSubscribable, E::NotSubscribable),
            (Error::<T, I>::InvalidConditions, E::InvalidConditions),
            (Error::<T, I>::NotEligible, E::NotEligible),
            (Error::<T, I>::AlreadySubscribed, E::AlreadySubscribed),
            (Error::<T, I>::NoSubscription, E::NoSubscription),
            (Error::<T, I>::NothingDue, E::NothingDue),
            (Error::<T, I>::GraceElapsed, E::GraceElapsed),
            (Error::<T, I>::ChargeFailed, E::ChargeFailed),
            (Error::<T, I>::ReplacementPending, E::ReplacementPending),
            (Error::<T, I>::ChangePending, E::ChangePending),
            (Error::<T, I>::NoPendingReplacement, E::NoPendingReplacement),
            (Error::<T, I>::CancelPending, E::CancelPending),
            (Error::<T, I>::AmendmentsDisabled, E::AmendmentsDisabled),
            (Error::<T, I>::NotTerminable, E::NotTerminable),
            (
                Error::<T, I>::ZeroPriceDirectSubscription,
                E::ZeroPriceDirectSubscription,
            ),
        ]
        .into_iter()
        .find_map(|(own, kind)| (*error == DispatchError::from(own)).then_some(kind))
    }

    fn item_amendment(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
    ) -> Option<ItemAmendmentOf<T, I>> {
        ItemAmendments::<T, I>::get((InventoryIdFor::<T, I>::from(*inventory_id), *id))
    }

    fn subscription(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
    ) -> Option<SubscriptionOf<T, I>> {
        Self::record(&Self::key(inventory_id, id, who)).map(|record| record.subscription)
    }

    fn grace_end(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
    ) -> Option<MomentOf<T, I>> {
        let key = Self::key(inventory_id, id, who);
        let record = Self::record(&key)?;
        match record.subscription.state {
            SubscriptionState::Active if !Self::charge_expected(&key, &record.subscription) => None,
            SubscriptionState::Defaulted { .. } => None,
            _ => Some(Self::grace_end_of(&record, Self::now())),
        }
    }

    fn next_due(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
    ) -> Option<MomentOf<T, I>> {
        let key = Self::key(inventory_id, id, who);
        let sub = Self::record(&key)?.subscription;
        match sub.state {
            SubscriptionState::Active if !Self::charge_expected(&key, &sub) => None,
            SubscriptionState::Defaulted { .. } => None,
            _ => Some(sub.paid_through),
        }
    }

    fn pending_amendment(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
        now: MomentOf<T, I>,
    ) -> Option<PendingConditionsOf<T, I>> {
        let key = Self::key(inventory_id, id, who);
        let sub = Self::record(&key)?.subscription;
        if matches!(sub.state, SubscriptionState::Defaulted { .. }) {
            return None;
        }
        Self::amendment_of(&key, &Self::in_force_at(&key, &sub, now))
            .filter(|(_, effective_at, _)| now < *effective_at)
            .map(|(conditions, effective_at, _)| PendingConditions {
                conditions,
                effective_at,
            })
    }
}

impl<T: Config<I>, I: 'static> subs::Mutate<AccountIdOf<T>> for Pallet<T, I> {
    fn set_conditions(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        conditions: SubscriptionConditionsOf<T, I>,
    ) -> DispatchResult {
        let inventory: InventoryIdFor<T, I> = (*inventory_id).into();
        Self::ensure_active_inventory(&inventory)?;
        ensure!(
            Self::item(inventory_id, id).is_some(),
            Error::<T, I>::UnknownItem
        );
        Self::validate_conditions(&conditions)?;

        // Setting conditions again on a withdrawn item publishes them anew.
        ItemConditions::<T, I>::mutate((inventory, *id), |maybe_item| match maybe_item {
            Some(item) => {
                item.conditions = conditions.clone();
                item.withdrawn = false;
            }
            None => {
                *maybe_item = Some(ItemSubscription {
                    conditions: conditions.clone(),
                    eligibility: Eligibility::Anyone,
                    policy: SubscriptionPolicy::default(),
                    withdrawn: false,
                })
            }
        });

        Self::deposit_event(Event::<T, I>::SubscriptionConditionsSet {
            inventory_id: inventory,
            id: *id,
            conditions,
        });
        Ok(())
    }

    fn set_exclusive(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
    ) -> DispatchResult {
        let inventory: InventoryIdFor<T, I> = (*inventory_id).into();
        ItemConditions::<T, I>::try_mutate((inventory, *id), |maybe_item| {
            let item = maybe_item.as_mut().ok_or(Error::<T, I>::NotSubscribable)?;
            ensure!(
                !matches!(item.eligibility, Eligibility::Taken(_)),
                Error::<T, I>::NotEligible
            );
            item.eligibility = Eligibility::Once(who.clone());
            Ok::<_, DispatchError>(())
        })?;

        Self::deposit_event(Event::<T, I>::SubscriptionExclusiveSet {
            inventory_id: inventory,
            id: *id,
            who: who.clone(),
        });
        Ok(())
    }

    fn set_policy(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        policy: SubscriptionPolicy,
    ) -> DispatchResult {
        let inventory: InventoryIdFor<T, I> = (*inventory_id).into();
        ensure!(policy.is_well_formed(), Error::<T, I>::InvalidConditions);
        ItemConditions::<T, I>::try_mutate((inventory, *id), |maybe_item| {
            let item = maybe_item.as_mut().ok_or(Error::<T, I>::NotSubscribable)?;
            item.policy = policy;
            Ok::<_, DispatchError>(())
        })?;

        Self::deposit_event(Event::<T, I>::SubscriptionPolicySet {
            inventory_id: inventory,
            id: *id,
            policy,
        });
        Ok(())
    }

    fn withdraw_conditions(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
    ) -> DispatchResult {
        let inventory: InventoryIdFor<T, I> = (*inventory_id).into();
        ItemConditions::<T, I>::try_mutate((inventory, *id), |maybe_item| {
            let item = maybe_item
                .as_mut()
                .filter(|item| !item.withdrawn)
                .ok_or(Error::<T, I>::NotSubscribable)?;
            item.withdrawn = true;
            Ok::<_, DispatchError>(())
        })?;

        Self::deposit_event(Event::<T, I>::SubscriptionConditionsWithdrawn {
            inventory_id: inventory,
            id: *id,
        });
        Ok(())
    }

    fn subscribe(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
    ) -> DispatchResult {
        Self::do_subscribe(inventory_id, id, who, true)
    }

    fn charge_due(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
    ) -> DispatchResult {
        with_storage_layer(|| Self::call_step(&Self::key(inventory_id, id, who), Mode::ChargeDue))
    }

    fn settle(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
    ) -> DispatchResult {
        with_storage_layer(|| Self::call_step(&Self::key(inventory_id, id, who), Mode::Settle))
    }

    fn cancel(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
    ) -> DispatchResult {
        with_storage_layer(|| {
            let key = Self::key(inventory_id, id, who);
            let mut record = Self::record(&key).ok_or(Error::<T, I>::NoSubscription)?;
            let now = Self::now();
            let sub = &record.subscription;
            let amendment_pending = Self::amendment_pending(&key, sub, now);
            let due = sub.paid_through;
            let commitment_end = sub.commitment_end();

            let cancellation = match sub.state {
                SubscriptionState::Defaulted { .. } => {
                    return Err(Error::<T, I>::NoSubscription.into())
                }
                SubscriptionState::Suspended { .. } => {
                    Self::drop_replacement(
                        &key,
                        &mut record.subscription,
                        ReplacementDropReason::SubscriptionCancelled,
                    );
                    if amendment_pending || due >= commitment_end {
                        Self::end(&key, &mut record, EndReason::Cancelled);
                        return Ok(());
                    }
                    Cancellation::Ordinary
                }
                SubscriptionState::Active => {
                    let free_exit = amendment_pending
                        || matches!(sub.cancel_requested, Some(Cancellation::FreeExit));
                    Self::drop_replacement(
                        &key,
                        &mut record.subscription,
                        ReplacementDropReason::SubscriptionCancelled,
                    );
                    if now >= due && (free_exit || due >= commitment_end) {
                        Self::end(&key, &mut record, EndReason::Cancelled);
                        return Ok(());
                    }
                    if free_exit {
                        Cancellation::FreeExit
                    } else {
                        Cancellation::Ordinary
                    }
                }
            };

            record.subscription.cancel_requested = Some(cancellation.clone());
            if matches!(record.subscription.state, SubscriptionState::Active) {
                // Its next action is never earlier than its entry, which is kept.
                let tick = Self::action_tick(&key, &record, now);
                Self::requeue(&key, &mut record, tick, Self::call_ctx(now));
            }
            Self::store(&key, &record);

            Self::deposit_event(Event::<T, I>::CancellationRequested {
                inventory_id: key.0,
                id: key.1,
                who: key.2.clone(),
                cancellation,
            });
            Ok(())
        })
    }

    fn terminate(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
    ) -> DispatchResult {
        let key = Self::key(inventory_id, id, who);
        let mut record = Self::record(&key).ok_or(Error::<T, I>::NoSubscription)?;
        ensure!(
            record.subscription.policy.terminable_by_merchant,
            Error::<T, I>::NotTerminable
        );
        Self::drop_replacement(
            &key,
            &mut record.subscription,
            ReplacementDropReason::SubscriptionTerminated,
        );
        Self::end(&key, &mut record, EndReason::Terminated);
        Ok(())
    }

    /// A consumer's replacement: a target priced at zero is accepted, as in `subscribe`. The
    /// replacement takes the target item's conditions and policy as they stand at the
    /// replacement tick, not as they stood when it was scheduled (`AC-B8.1`).
    fn schedule_replacement(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
        new_id: &ItemIdOf<T, I>,
    ) -> DispatchResult {
        Self::schedule(inventory_id, id, who, new_id, false, true)
    }

    /// As [`schedule_replacement`](subs::Mutate::schedule_replacement), also waiting for the
    /// term limit's end.
    fn schedule_replacement_at_term_end(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
        new_id: &ItemIdOf<T, I>,
    ) -> DispatchResult {
        Self::schedule(inventory_id, id, who, new_id, true, true)
    }

    fn cancel_replacement(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
    ) -> DispatchResult {
        with_storage_layer(|| {
            let key = Self::key(inventory_id, id, who);
            let mut record = Self::record(&key).ok_or(Error::<T, I>::NoSubscription)?;
            let new_id = record
                .subscription
                .replacement
                .take()
                .ok_or(Error::<T, I>::NoPendingReplacement)?;

            if matches!(record.subscription.state, SubscriptionState::Active) {
                let now = Self::now();
                let tick = Self::action_tick(&key, &record, now);
                Self::requeue(&key, &mut record, tick, Self::call_ctx(now));
            }
            Self::store(&key, &record);
            Self::notify_dropped(&key, new_id, ReplacementDropReason::ReplacementCancelled);
            Ok(())
        })
    }

    fn amend(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        who: &AccountIdOf<T>,
        conditions: SubscriptionConditionsOf<T, I>,
    ) -> Result<MomentOf<T, I>, DispatchError> {
        with_storage_layer(|| {
            let key = Self::key(inventory_id, id, who);
            let mut record = Self::record(&key).ok_or(Error::<T, I>::NoSubscription)?;
            ensure!(
                !matches!(
                    record.subscription.state,
                    SubscriptionState::Defaulted { .. }
                ),
                Error::<T, I>::NoSubscription
            );
            let notice = record
                .subscription
                .policy
                .notice_periods()
                .ok_or(Error::<T, I>::AmendmentsDisabled)?;
            let now = Self::now();
            Self::apply_in_force(&key, &mut record.subscription, now);
            let sub = &record.subscription;
            Self::validate_conditions(&conditions)?;
            ensure!(
                conditions.period == sub.conditions.period,
                Error::<T, I>::InvalidConditions
            );
            ensure!(
                Self::amendment_of(&key, sub).is_none(),
                Error::<T, I>::ChangePending
            );

            let effective_at = effective_boundary(sub.anchor, sub.conditions.period, now, notice)
                .ok_or(ArithmeticError::Overflow)?;
            // At most one change pending: the amendment cancels a scheduled replacement.
            Self::drop_replacement(
                &key,
                &mut record.subscription,
                ReplacementDropReason::AmendmentEnacted,
            );
            record.subscription.pending_conditions = Some(PendingConditions {
                conditions,
                effective_at,
            });
            if matches!(record.subscription.state, SubscriptionState::Active) {
                let tick = Self::action_tick(&key, &record, now);
                Self::requeue(&key, &mut record, tick, Self::call_ctx(now));
            }
            Self::store(&key, &record);

            T::OnSubscriptionChanged::on_amendment_enacted(inventory_id, id, who, effective_at);
            Self::deposit_event(Event::<T, I>::AmendmentEnacted {
                inventory_id: key.0,
                id: key.1,
                who: key.2.clone(),
                effective_at,
            });
            Ok(effective_at)
        })
    }

    fn amend_item(
        inventory_id: &InventoryIdTuple<T, I>,
        id: &ItemIdOf<T, I>,
        conditions: SubscriptionConditionsOf<T, I>,
    ) -> Result<ItemAmendmentOf<T, I>, DispatchError> {
        let inventory: InventoryIdFor<T, I> = (*inventory_id).into();
        let mut item =
            ItemConditions::<T, I>::get((inventory, *id)).ok_or(Error::<T, I>::NotSubscribable)?;
        let notice = item
            .policy
            .notice_periods()
            .ok_or(Error::<T, I>::AmendmentsDisabled)?;
        Self::validate_conditions(&conditions)?;
        ensure!(
            conditions.period == item.conditions.period,
            Error::<T, I>::InvalidConditions
        );

        // Every subscription live at the previous amendment's enactment has applied it, been
        // skipped by it, or ended; every later one took it at its start.
        let counts = ItemSubscriptionCounts::<T, I>::get((inventory, *id)).unwrap_or_default();
        ensure!(counts.behind == 0, Error::<T, I>::ChangePending);
        let now = Self::now();
        let seq = ItemAmendments::<T, I>::get((inventory, *id))
            .map_or(Some(1), |previous| previous.seq.checked_add(1))
            .ok_or(ArithmeticError::Overflow)?;

        item.conditions = conditions.clone();
        ItemConditions::<T, I>::insert((inventory, *id), item);
        let amendment = ItemAmendment {
            seq,
            conditions,
            enacted_at: now,
        };
        ItemAmendments::<T, I>::insert((inventory, *id), amendment.clone());
        // Every live subscription is behind it now.
        Self::update_counts((inventory, *id), |counts| counts.behind = counts.live);

        T::OnSubscriptionChanged::on_item_amendment_enacted(inventory_id, id, seq, now);
        Self::deposit_event(Event::<T, I>::ItemAmended {
            inventory_id: inventory,
            id: *id,
            seq,
            enacted_at: now,
            last_boundary: amendment.last_boundary(notice),
        });
        Ok(amendment)
    }
}
