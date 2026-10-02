//! Tests for listings subscriptions (F-02), grouped by subject: SPEC §5.2's lifecycle in its
//! generic form, the conditions, subscribing, charges, grace and default, cancellation,
//! replacements, amendments, reads and notifications, the due queue, and migration pauses.
//! Tests are named after what they verify.

use crate::{
    mock::*, AmendmentPolicy, Cancellation, CatchUpFrom, DueCursor, DueQueue, Eligibility,
    EndReason, Error, Event, InventoryId, InventoryIdFor, ItemPrice, ItemSubscriptionCounts,
    PendingConditions, ReplacementDropReason, SubscriptionConditions, SubscriptionConditionsOf,
    SubscriptionOf, SubscriptionPolicy, SubscriptionState, Subscriptions,
};
use fc_traits_listings::item::subscriptions::{
    effective_boundary, InspectSubscription, MutateSubscription, SubscriptionError,
};
use frame_support::{
    assert_noop, assert_ok,
    migrations::MigrationStatusHandler,
    traits::{fungibles::Mutate as _, Get, Hooks},
    weights::{Weight, WeightMeter},
    BoundedVec,
};
use sp_runtime::{traits::Zero, DispatchError};

const INV: InventoryIdFor<Test> = InventoryId(crate::test_utils::SignedMerchantId([0u8; 32]), 1);
const ITEM: u32 = 1;
const OTHER: u32 = 2;
const PRICE: Balance = 10;
const OTHER_PRICE: Balance = 20;
/// The billing period: 30 days.
const B: u64 = 30 * DAYS;
const L: u64 = DAYS;
const G: u64 = 3 * DAYS;

type ListingsError = Error<Test>;

fn inv() -> (crate::test_utils::SignedMerchantId, u32) {
    INV.into()
}

fn key(who: &AccountId, item: u32) -> HookKey {
    (inv(), item, who.clone())
}

fn conditions(
    price: Balance,
    term: Option<u32>,
    min_commitment: Option<u32>,
) -> SubscriptionConditionsOf<Test> {
    SubscriptionConditions {
        price: ItemPrice {
            asset: SUBSCRIPTION_ASSET,
            amount: price,
        },
        period: B,
        term,
        min_commitment,
        grace: G,
    }
}

/// A policy that lets the merchant amend, with `periods` billing periods of notice, and
/// terminate.
fn with_notice(periods: u32) -> SubscriptionPolicy {
    SubscriptionPolicy {
        amendments: AmendmentPolicy::WithNotice { periods },
        terminable_by_merchant: true,
    }
}

/// Publishes an item with `conditions` and the policy most tests need: amendments with one
/// billing period of notice, and termination by the merchant.
fn publish(id: u32, conditions: SubscriptionConditionsOf<Test>) {
    publish_with_policy(id, conditions, with_notice(1));
}

fn publish_with_policy(
    id: u32,
    conditions: SubscriptionConditionsOf<Test>,
    policy: SubscriptionPolicy,
) {
    assert_ok!(Listings::publish_item(
        RuntimeOrigin::signed(ROOT),
        INV,
        id,
        BoundedVec::truncate_from(b"subscription".to_vec()),
        None
    ));
    assert_ok!(Listings::set_subscription_conditions(
        RuntimeOrigin::signed(ROOT),
        INV,
        id,
        conditions,
        None
    ));
    if policy != SubscriptionPolicy::default() {
        assert_ok!(Listings::set_subscription_policy(
            RuntimeOrigin::signed(ROOT),
            INV,
            id,
            policy
        ));
    }
}

/// Subscribes through the trait, as a consumer does: a zero price is accepted.
fn subscribe_as_consumer(who: &AccountId, id: u32) {
    assert_ok!(<Listings as MutateSubscription<AccountId>>::subscribe(
        &inv(),
        &id,
        who
    ));
}

fn amend_item(id: u32, conditions: SubscriptionConditionsOf<Test>) -> sp_runtime::DispatchResult {
    Listings::amend_item_subscriptions(RuntimeOrigin::signed(ROOT), INV, id, conditions)
}

fn behind(id: u32) -> u32 {
    ItemSubscriptionCounts::<Test>::get((INV, id)).map_or(0, |counts| counts.behind)
}

fn subscribe(who: &AccountId, id: u32) {
    assert_ok!(Listings::subscribe(
        RuntimeOrigin::signed(who.clone()),
        INV,
        id
    ));
}

fn sub(who: &AccountId, id: u32) -> Option<SubscriptionOf<Test>> {
    <Listings as InspectSubscription<AccountId>>::subscription(&inv(), &id, who)
}

fn state(who: &AccountId, id: u32) -> Option<SubscriptionState<u64>> {
    sub(who, id).map(|s| s.state)
}

fn balance(who: &AccountId) -> Balance {
    Assets::balance(SUBSCRIPTION_ASSET, who)
}

fn set_funds(who: &AccountId, amount: Balance) {
    Assets::set_balance(SUBSCRIPTION_ASSET, who, amount);
}

fn charge_due(who: &AccountId, id: u32) -> sp_runtime::DispatchResult {
    Listings::charge_due(RuntimeOrigin::signed(CHARLIE), INV, id, who.clone())
}

fn is_paid(who: &AccountId, id: u32, now: u64) -> bool {
    <Listings as InspectSubscription<AccountId>>::is_paid(&inv(), &id, who, now)
}

fn idle() -> Weight {
    <Listings as Hooks<u64>>::on_idle(1, Weight::MAX)
}

fn poll() {
    let mut meter = WeightMeter::new();
    <Listings as Hooks<u64>>::on_poll(1, &mut meter);
}

/// Moves the clock to `tick`, one bucket at a time, running `on_poll` and `on_idle` at each
/// step: the due queue running on time.
fn run_to(tick: u64) {
    let mut now = Clock::now();
    while now < tick {
        now = ((now / HOURS + 1) * HOURS).min(tick);
        Clock::set(now);
        poll();
        idle();
    }
}

fn hooks() -> Vec<Hook> {
    RecordHooks::take()
}

fn ended(who: &AccountId, id: u32, reason: EndReason) -> Hook {
    Hook::Ended(key(who, id), reason)
}

/// One test per row of SPEC §5.2's transition table, in generic form.
mod lifecycle {
    use super::*;

    // §5.2: — → Active (subscribe, and the first charge succeeds)
    #[test]
    fn subscription_starts_active_when_the_first_charge_succeeds() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            Clock::set(5);
            let owner_before = balance(&ROOT);
            hooks();

            subscribe(&ALICE, ITEM);

            let s = sub(&ALICE, ITEM).expect("subscribed");
            assert_eq!(s.state, SubscriptionState::Active);
            assert_eq!(s.anchor, 5);
            assert_eq!(s.paid_through, 5 + B);
            assert_eq!(s.periods_charged, 1);
            assert_eq!(balance(&ROOT), owner_before + PRICE);
            assert_eq!(balance(&ALICE), 1_000 - PRICE);
            assert_eq!(hooks(), vec![Hook::Started(key(&ALICE, ITEM))]);
            System::assert_has_event(
                Event::SubscriptionStarted {
                    inventory_id: INV,
                    id: ITEM,
                    who: ALICE,
                    anchor: 5,
                    paid_through: 5 + B,
                }
                .into(),
            );
        })
    }

    // §5.2: Active → Active (a charge attempted at t ≥ d − L succeeds)
    #[test]
    fn active_subscription_stays_active_when_charged_within_the_lead() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            hooks();

            run_to(B - L - 1);
            assert_eq!(sub(&ALICE, ITEM).unwrap().paid_through, B);
            run_to(B - L);
            let s = sub(&ALICE, ITEM).unwrap();
            assert_eq!(s.paid_through, 2 * B);
            assert_eq!(s.periods_charged, 2);
            assert_eq!(s.anchor, 0);
            assert_eq!(hooks(), vec![Hook::Charged(key(&ALICE, ITEM), 1, 2 * B)]);
        })
    }

    // §5.2: Active → Active (a charge attempted at t ≥ d succeeds, where d = b)
    #[test]
    fn active_subscription_stays_active_when_charged_at_the_effective_boundary() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            Clock::set(10);
            let b = <Listings as MutateSubscription<AccountId>>::amend(
                &inv(),
                &ITEM,
                &ALICE,
                conditions(15, None, None),
            )
            .unwrap();
            assert_eq!(b, 2 * B);

            run_to(2 * B - 1);
            assert_eq!(sub(&ALICE, ITEM).unwrap().paid_through, 2 * B);
            run_to(2 * B);
            let s = sub(&ALICE, ITEM).unwrap();
            assert_eq!(s.paid_through, 3 * B);
            assert_eq!(s.conditions.price.amount, 15);
            assert_eq!(s.pending_conditions, None);
        })
    }

    // §5.2: Active → Active (an amendment that covers it is enacted)
    #[test]
    fn active_subscription_stays_active_when_an_amendment_is_enacted() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            hooks();
            Clock::set(10);

            assert_ok!(Listings::amend_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE,
                conditions(15, None, None)
            ));

            let s = sub(&ALICE, ITEM).unwrap();
            assert_eq!(s.state, SubscriptionState::Active);
            assert_eq!(s.conditions.price.amount, PRICE);
            assert_eq!(
                s.pending_conditions,
                Some(PendingConditions {
                    conditions: conditions(15, None, None),
                    effective_at: 2 * B
                })
            );
            assert_eq!(
                hooks(),
                vec![Hook::AmendmentEnacted(key(&ALICE, ITEM), 2 * B)]
            );
        })
    }

    // §5.2: Suspended → Suspended (an amendment that covers it is enacted)
    #[test]
    fn suspended_subscription_stays_suspended_when_an_amendment_is_enacted() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            assert!(matches!(
                state(&ALICE, ITEM),
                Some(SubscriptionState::Suspended { .. })
            ));

            assert_ok!(<Listings as MutateSubscription<AccountId>>::amend(
                &inv(),
                &ITEM,
                &ALICE,
                conditions(15, None, None),
            ));
            let s = sub(&ALICE, ITEM).unwrap();
            assert!(matches!(s.state, SubscriptionState::Suspended { .. }));
            assert_eq!(s.pending_conditions.unwrap().effective_at, 2 * B);
        })
    }

    // §5.2: Active → Suspended (a charge attempted at t ≥ d fails)
    #[test]
    fn active_subscription_is_suspended_when_the_charge_at_due_fails() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            hooks();

            // A failed attempt within the lead changes nothing.
            run_to(B - 1);
            assert_eq!(state(&ALICE, ITEM), Some(SubscriptionState::Active));
            assert!(hooks().is_empty());

            run_to(B);
            assert_eq!(
                state(&ALICE, ITEM),
                Some(SubscriptionState::Suspended { since: B })
            );
            assert_eq!(hooks(), vec![Hook::Suspended(key(&ALICE, ITEM), B + G)]);
            assert!(!is_paid(&ALICE, ITEM, B));
        })
    }

    // §5.2: Suspended → Active (a charge attempted at t < d + G succeeds)
    #[test]
    fn suspended_subscription_is_restored_when_charged_within_grace() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B + DAYS);
            hooks();

            set_funds(&ALICE, 100);
            assert_ok!(charge_due(&ALICE, ITEM));

            let s = sub(&ALICE, ITEM).unwrap();
            assert_eq!(s.state, SubscriptionState::Active);
            assert_eq!(s.anchor, 0);
            assert_eq!(s.paid_through, 2 * B);
            assert!(is_paid(&ALICE, ITEM, B + DAYS));
            assert_eq!(
                hooks(),
                vec![
                    Hook::Charged(key(&ALICE, ITEM), 1, 2 * B),
                    Hook::Restored(key(&ALICE, ITEM))
                ]
            );
        })
    }

    // §5.2: Suspended → Defaulted (t ≥ d + G, the charge is still unpaid, and d < E)
    #[test]
    fn suspended_subscription_defaults_when_grace_ends_within_the_commitment() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            hooks();

            run_to(B + G);
            assert_eq!(
                state(&ALICE, ITEM),
                Some(SubscriptionState::Defaulted { until: 3 * B })
            );
            assert_eq!(hooks(), vec![Hook::Defaulted(key(&ALICE, ITEM), 3 * B)]);
            // No arrears are collected, and no charge is attempted.
            set_funds(&ALICE, 100);
            assert_noop!(charge_due(&ALICE, ITEM), ListingsError::GraceElapsed);
        })
    }

    // §5.2: Suspended → Ended (lapsed: t ≥ d + G, the charge is still unpaid, and d ≥ E)
    #[test]
    fn suspended_subscription_lapses_when_grace_ends_outside_the_commitment() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            hooks();

            run_to(B + G);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(hooks(), vec![ended(&ALICE, ITEM, EndReason::Lapsed)]);
        })
    }

    // §5.2: Defaulted → Ended (lapsed: t ≥ E)
    #[test]
    fn defaulted_subscription_lapses_at_the_commitment_end() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B + G);
            hooks();

            run_to(3 * B - 1);
            assert!(sub(&ALICE, ITEM).is_some());
            run_to(3 * B);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(hooks(), vec![ended(&ALICE, ITEM, EndReason::Lapsed)]);
        })
    }

    // §5.2: Active → Ended (completed: t ≥ paid through, and the term limit's last period is paid)
    #[test]
    fn active_subscription_completes_once_its_term_limit_is_paid() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, Some(2), None));
            subscribe(&ALICE, ITEM);
            let owner_before = balance(&ROOT);
            hooks();

            run_to(2 * B - 1);
            let s = sub(&ALICE, ITEM).unwrap();
            assert_eq!(s.periods_charged, 2);
            assert_eq!(s.term_remaining(), Some(0));
            run_to(2 * B);
            assert_eq!(sub(&ALICE, ITEM), None);
            // REQ-BL-6: one renewal, then nothing.
            assert_eq!(balance(&ROOT), owner_before + PRICE);
            assert_eq!(
                hooks(),
                vec![
                    Hook::Charged(key(&ALICE, ITEM), 1, 2 * B),
                    ended(&ALICE, ITEM, EndReason::Completed)
                ]
            );
        })
    }

    // §5.2: Active (trial) → Ended (converted at the trial's end; the new first charge succeeds)
    #[test]
    fn trial_is_replaced_by_its_conversion_at_the_term_end() {
        new_test_ext().execute_with(|| {
            // A free trial of one period that converts into OTHER, offered by a consumer.
            publish(ITEM, conditions(0, Some(1), None));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe_as_consumer(&ALICE, ITEM);
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));
            hooks();

            run_to(B);
            assert_eq!(sub(&ALICE, ITEM), None);
            let s = sub(&ALICE, OTHER).expect("converted");
            assert_eq!(s.anchor, B);
            assert_eq!(s.paid_through, 2 * B);
            assert_eq!(balance(&ALICE), 1_000 - OTHER_PRICE);
            assert_eq!(hooks(), vec![Hook::Replaced(key(&ALICE, ITEM), OTHER, B)]);
        })
    }

    // §5.2: Active → Ended (switched at paid through ≥ E; the new first charge succeeds)
    #[test]
    fn active_subscription_is_replaced_when_the_new_first_charge_succeeds() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe(&ALICE, ITEM);
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));
            hooks();

            // No lead for the replacement's tick: nothing happens before it.
            run_to(B - 1);
            assert_eq!(balance(&ALICE), 1_000 - PRICE);
            run_to(B);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(sub(&ALICE, OTHER).unwrap().anchor, B);
            assert_eq!(balance(&ALICE), 1_000 - PRICE - OTHER_PRICE);
            assert_eq!(hooks(), vec![Hook::Replaced(key(&ALICE, ITEM), OTHER, B)]);
            System::assert_has_event(
                Event::SubscriptionReplaced {
                    inventory_id: INV,
                    id: ITEM,
                    who: ALICE,
                    new_id: OTHER,
                    new_anchor: B,
                }
                .into(),
            );
        })
    }

    // §5.2: Active → Ended (cancelled: t ≥ paid through, and paid through ≥ E)
    #[test]
    fn cancelled_subscription_ends_at_paid_through_past_the_commitment() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            Clock::set(10);
            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            hooks();

            run_to(B - 1);
            // REQ-CT-5: usable until `paid_through`, and no charge.
            assert!(is_paid(&ALICE, ITEM, B - 1));
            assert_eq!(balance(&ALICE), 1_000 - PRICE);
            run_to(B);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(hooks(), vec![ended(&ALICE, ITEM, EndReason::Cancelled)]);
        })
    }

    // §5.2: Active → Ended (cancelled while an amendment was pending; the commitment is waived)
    #[test]
    fn cancelling_while_an_amendment_is_pending_waives_the_commitment() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            Clock::set(10);
            assert_ok!(<Listings as MutateSubscription<AccountId>>::amend(
                &inv(),
                &ITEM,
                &ALICE,
                conditions(15, None, Some(3)),
            ));
            Clock::set(20);
            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            assert_eq!(
                sub(&ALICE, ITEM).unwrap().cancel_requested,
                Some(Cancellation::FreeExit)
            );
            hooks();

            run_to(B);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(balance(&ALICE), 1_000 - PRICE);
            assert_eq!(hooks(), vec![ended(&ALICE, ITEM, EndReason::Cancelled)]);
        })
    }

    // §5.2: Suspended → Ended (cancelled, and d ≥ E)
    #[test]
    fn cancelling_a_suspended_subscription_outside_the_commitment_ends_it() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            hooks();

            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(hooks(), vec![ended(&ALICE, ITEM, EndReason::Cancelled)]);
        })
    }

    // §5.2: Suspended → Ended (cancelled while an amendment is pending)
    #[test]
    fn cancelling_a_suspended_subscription_with_an_amendment_pending_ends_it() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            assert_ok!(<Listings as MutateSubscription<AccountId>>::amend(
                &inv(),
                &ITEM,
                &ALICE,
                conditions(15, None, Some(3)),
            ));
            hooks();

            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(hooks(), vec![ended(&ALICE, ITEM, EndReason::Cancelled)]);
        })
    }

    // §5.2: Active → Ended (terminated)
    #[test]
    fn terminating_an_active_subscription_ends_it() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            hooks();
            assert_ok!(Listings::terminate_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE
            ));
            assert_eq!(sub(&ALICE, ITEM), None);
            // No refund.
            assert_eq!(balance(&ALICE), 1_000 - PRICE);
            assert_eq!(hooks(), vec![ended(&ALICE, ITEM, EndReason::Terminated)]);
            // It can subscribe again at once.
            subscribe(&ALICE, ITEM);
        })
    }

    // §5.2: Suspended → Ended (terminated)
    #[test]
    fn terminating_a_suspended_subscription_ends_it() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            hooks();
            assert_ok!(Listings::terminate_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE
            ));
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(hooks(), vec![ended(&ALICE, ITEM, EndReason::Terminated)]);
        })
    }

    // §5.2: Defaulted → Ended (terminated)
    #[test]
    fn terminating_a_defaulted_subscription_ends_it() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B + G);
            hooks();
            assert_ok!(Listings::terminate_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE
            ));
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(hooks(), vec![ended(&ALICE, ITEM, EndReason::Terminated)]);
            // The block lifts at once.
            set_funds(&ALICE, 100);
            subscribe(&ALICE, ITEM);
        })
    }

    // REQ-CT-3
    #[test]
    fn defaulted_subscription_refuses_every_other_transition() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B + G);
            assert!(matches!(
                state(&ALICE, ITEM),
                Some(SubscriptionState::Defaulted { .. })
            ));
            // A *Defaulted* subscription cannot be cancelled, replaced, amended or charged.
            assert_noop!(
                Listings::cancel_subscription(RuntimeOrigin::signed(ALICE), INV, ITEM),
                ListingsError::NoSubscription
            );
            assert_noop!(
                Listings::schedule_replacement(RuntimeOrigin::signed(ALICE), INV, ITEM, OTHER),
                ListingsError::NoSubscription
            );
            assert_noop!(
                <Listings as MutateSubscription<AccountId>>::amend(
                    &inv(),
                    &ITEM,
                    &ALICE,
                    conditions(15, None, Some(3))
                ),
                ListingsError::NoSubscription
            );
            assert_noop!(charge_due(&ALICE, ITEM), ListingsError::GraceElapsed);
        })
    }
}

/// Publishing, changing and validating an item's subscription conditions.
mod conditions {
    use super::*;

    // AC-E1.1
    #[test]
    fn published_conditions_are_announced_and_readable() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, Some(12), Some(2)));
            System::assert_has_event(
                Event::SubscriptionConditionsSet {
                    inventory_id: INV,
                    id: ITEM,
                    conditions: conditions(PRICE, Some(12), Some(2)),
                }
                .into(),
            );
            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::subscription_conditions(
                    &inv(),
                    &ITEM
                ),
                Some(conditions(PRICE, Some(12), Some(2)))
            );
        })
    }

    // CTR-SUB-1
    #[test]
    fn conditions_change_only_by_the_owner_and_on_a_published_item() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            assert_ok!(Listings::set_subscription_conditions(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                conditions(20, None, None),
                None
            ));
            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::subscription_conditions(
                    &inv(),
                    &ITEM
                )
                .unwrap()
                .price
                .amount,
                20
            );
            assert_noop!(
                Listings::set_subscription_conditions(
                    RuntimeOrigin::signed(ALICE),
                    INV,
                    ITEM,
                    conditions(20, None, None),
                    None
                ),
                DispatchError::BadOrigin
            );
            assert_noop!(
                Listings::set_subscription_conditions(
                    RuntimeOrigin::signed(ROOT),
                    INV,
                    9,
                    conditions(20, None, None),
                    None
                ),
                ListingsError::UnknownItem
            );
            assert_noop!(
                Listings::subscribe(RuntimeOrigin::signed(ALICE), INV, 9),
                ListingsError::NotSubscribable
            );
        })
    }

    // CTR-SUB-1
    #[test]
    fn conditions_are_validated() {
        new_test_ext().execute_with(|| {
            assert_ok!(Listings::publish_item(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                BoundedVec::truncate_from(b"item".to_vec()),
                None
            ));
            let set = |c: SubscriptionConditionsOf<Test>| {
                Listings::set_subscription_conditions(
                    RuntimeOrigin::signed(ROOT),
                    INV,
                    ITEM,
                    c,
                    None,
                )
            };
            let with = |f: &dyn Fn(&mut SubscriptionConditionsOf<Test>)| {
                let mut c = conditions(PRICE, Some(4), Some(2));
                f(&mut c);
                c
            };

            assert_ok!(set(with(&|_| {})));
            // A zero price is valid for any asset, even one that does not exist.
            assert_ok!(set(with(&|c| c.price = ItemPrice {
                asset: 99,
                amount: 0
            })));

            for invalid in [
                with(&|c| {
                    c.price = ItemPrice {
                        asset: 99,
                        amount: 10,
                    }
                }),
                // Asset 1's minimum balance is 10.
                with(&|c| {
                    c.price = ItemPrice {
                        asset: 1,
                        amount: 5,
                    }
                }),
                with(&|c| c.period = 0),
                with(&|c| c.period = L),
                with(&|c| c.grace = c.period),
                with(&|c| c.term = Some(0)),
                with(&|c| c.min_commitment = Some(0)),
                with(&|c| c.min_commitment = Some(5)),
            ] {
                assert_noop!(set(invalid), ListingsError::InvalidConditions);
            }
            assert_ok!(set(with(&|c| c.price = ItemPrice {
                asset: 1,
                amount: 10
            })));
        })
    }

    // REQ-SB-1
    #[test]
    fn changing_conditions_does_not_touch_existing_subscriptions() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            assert_ok!(Listings::set_subscription_conditions(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                conditions(50, None, None),
                None
            ));
            subscribe(&BOB, ITEM);

            assert_eq!(sub(&ALICE, ITEM).unwrap().conditions.price.amount, PRICE);
            assert_eq!(sub(&BOB, ITEM).unwrap().conditions.price.amount, 50);
            run_to(B);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE);
        })
    }

    // CTR-SUB-1, REQ-SB-1, INV-9
    #[test]
    fn withdrawn_conditions_refuse_new_subscriptions_and_replacements_only() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe(&ALICE, ITEM);
            subscribe(&BOB, OTHER);
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(BOB),
                INV,
                OTHER,
                ITEM
            ));

            assert_noop!(
                Listings::withdraw_subscription_conditions(RuntimeOrigin::signed(ALICE), INV, ITEM),
                DispatchError::BadOrigin
            );
            assert_ok!(Listings::withdraw_subscription_conditions(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM
            ));
            System::assert_has_event(
                Event::SubscriptionConditionsWithdrawn {
                    inventory_id: INV,
                    id: ITEM,
                }
                .into(),
            );
            assert!(
                <Listings as InspectSubscription<AccountId>>::conditions_withdrawn(&inv(), &ITEM)
            );
            // The conditions stay readable.
            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::subscription_conditions(
                    &inv(),
                    &ITEM
                ),
                Some(conditions(PRICE, None, None))
            );
            assert_noop!(
                Listings::withdraw_subscription_conditions(RuntimeOrigin::signed(ROOT), INV, ITEM),
                ListingsError::NotSubscribable
            );

            // No new subscription, and no new replacement into it.
            assert_noop!(
                Listings::subscribe(RuntimeOrigin::signed(CHARLIE), INV, ITEM),
                ListingsError::NotSubscribable
            );
            subscribe(&CHARLIE, OTHER);
            assert_noop!(
                Listings::schedule_replacement(RuntimeOrigin::signed(CHARLIE), INV, OTHER, ITEM),
                ListingsError::NotSubscribable
            );

            // The existing subscription keeps its conditions and renews; the scheduled
            // replacement into the withdrawn item is dropped at its tick.
            run_to(B);
            assert_eq!(sub(&ALICE, ITEM).unwrap().periods_charged, 2);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE);
            assert_eq!(sub(&BOB, OTHER).unwrap().periods_charged, 2);
            assert!(sub(&BOB, ITEM).is_none());
            assert!(hooks().contains(&Hook::ReplacementDropped(
                key(&BOB, OTHER),
                ITEM,
                ReplacementDropReason::NotSubscribable
            )));

            // Setting conditions again publishes them anew.
            assert_ok!(Listings::set_subscription_conditions(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                conditions(PRICE, None, None),
                None
            ));
            assert!(
                !<Listings as InspectSubscription<AccountId>>::conditions_withdrawn(&inv(), &ITEM)
            );
            subscribe(&CHARLIE, ITEM);
        })
    }

    // CTR-SUB-1, CTR-SUB-6
    #[test]
    fn withdrawal_keeps_an_enacted_item_amendment_in_force() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            Clock::set(1);
            assert_ok!(amend_item(ITEM, conditions(15, None, None)));
            assert_ok!(Listings::withdraw_subscription_conditions(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM
            ));
            run_to(2 * B);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE - 15);
        })
    }
}

/// The subscription policy: what a merchant may do to the subscriptions to its item, opted into
/// per item and kept by each subscription from its start (0009-A17).
mod policies {
    use super::*;

    // 0009-A17
    #[test]
    fn default_policy_refuses_amendments_and_termination() {
        new_test_ext().execute_with(|| {
            publish_with_policy(ITEM, conditions(PRICE, None, None), Default::default());
            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::policy(&inv(), &ITEM),
                Some(SubscriptionPolicy::default())
            );
            subscribe(&ALICE, ITEM);
            assert_eq!(
                sub(&ALICE, ITEM).unwrap().policy,
                SubscriptionPolicy::default()
            );

            assert_noop!(
                Listings::amend_subscription(
                    RuntimeOrigin::signed(ROOT),
                    INV,
                    ITEM,
                    ALICE,
                    conditions(15, None, None)
                ),
                ListingsError::AmendmentsDisabled
            );
            assert_noop!(
                amend_item(ITEM, conditions(15, None, None)),
                ListingsError::AmendmentsDisabled
            );
            assert_noop!(
                Listings::terminate_subscription(RuntimeOrigin::signed(ROOT), INV, ITEM, ALICE),
                ListingsError::NotTerminable
            );
            // The subscriber still cancels.
            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
        })
    }

    // 0009-A17
    #[test]
    fn the_merchant_opts_in_per_item() {
        new_test_ext().execute_with(|| {
            publish_with_policy(ITEM, conditions(PRICE, None, None), Default::default());
            let set = |origin: AccountId, id: u32, policy| {
                Listings::set_subscription_policy(RuntimeOrigin::signed(origin), INV, id, policy)
            };
            assert_noop!(set(ALICE, ITEM, with_notice(1)), DispatchError::BadOrigin);
            assert_noop!(
                set(ROOT, ITEM, with_notice(0)),
                ListingsError::InvalidConditions
            );
            assert_ok!(Listings::publish_item(
                RuntimeOrigin::signed(ROOT),
                INV,
                OTHER,
                BoundedVec::truncate_from(b"no conditions".to_vec()),
                None
            ));
            assert_noop!(
                set(ROOT, OTHER, with_notice(1)),
                ListingsError::NotSubscribable
            );

            let only_amendments = SubscriptionPolicy {
                amendments: AmendmentPolicy::WithNotice { periods: 1 },
                terminable_by_merchant: false,
            };
            assert_ok!(set(ROOT, ITEM, only_amendments));
            System::assert_has_event(
                Event::SubscriptionPolicySet {
                    inventory_id: INV,
                    id: ITEM,
                    policy: only_amendments,
                }
                .into(),
            );
            subscribe(&ALICE, ITEM);
            Clock::set(1);
            assert_ok!(Listings::amend_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE,
                conditions(15, None, None)
            ));
            assert_noop!(
                Listings::terminate_subscription(RuntimeOrigin::signed(ROOT), INV, ITEM, ALICE),
                ListingsError::NotTerminable
            );
        })
    }

    // 0009-A17, INV-9
    #[test]
    fn a_subscription_keeps_the_policy_its_item_had_when_it_started() {
        new_test_ext().execute_with(|| {
            publish_with_policy(ITEM, conditions(PRICE, None, None), Default::default());
            subscribe(&ALICE, ITEM);
            assert_ok!(Listings::set_subscription_policy(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                with_notice(1)
            ));
            subscribe(&BOB, ITEM);
            assert_eq!(
                sub(&ALICE, ITEM).unwrap().policy,
                SubscriptionPolicy::default()
            );
            assert_eq!(sub(&BOB, ITEM).unwrap().policy, with_notice(1));

            // The merchant cannot opt ALICE in: no amendment, no termination of hers.
            Clock::set(1);
            assert_noop!(
                Listings::amend_subscription(
                    RuntimeOrigin::signed(ROOT),
                    INV,
                    ITEM,
                    ALICE,
                    conditions(15, None, None)
                ),
                ListingsError::AmendmentsDisabled
            );
            assert_noop!(
                Listings::terminate_subscription(RuntimeOrigin::signed(ROOT), INV, ITEM, ALICE),
                ListingsError::NotTerminable
            );

            // An item amendment applies to BOB at his boundary, and skips ALICE, who is no longer
            // waited for once her next charge settles it.
            assert_ok!(amend_item(ITEM, conditions(15, None, None)));
            assert_eq!(behind(ITEM), 2);
            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::pending_amendment(
                    &inv(),
                    &ITEM,
                    &ALICE,
                    1
                ),
                None
            );
            run_to(B);
            assert_eq!(behind(ITEM), 1);
            run_to(2 * B);
            assert_eq!(behind(ITEM), 0);
            assert_eq!(balance(&ALICE), 1_000 - 3 * PRICE);
            assert_eq!(balance(&BOB), 1_000 - 2 * PRICE - 15);

            // Disabling the item's policy later leaves BOB's as it was.
            assert_ok!(Listings::set_subscription_policy(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                Default::default()
            ));
            assert_ok!(Listings::terminate_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                BOB
            ));
        })
    }

    // 0009-A17, REQ-CT-14, REQ-SB-13
    #[test]
    fn a_notice_of_several_periods_moves_the_effective_boundary() {
        new_test_ext().execute_with(|| {
            publish_with_policy(ITEM, conditions(PRICE, None, None), with_notice(2));
            subscribe(&ALICE, ITEM);
            Clock::set(1);
            assert_ok!(Listings::amend_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE,
                conditions(15, None, None)
            ));
            // The first due tick at least two periods after tick 1.
            let pending = sub(&ALICE, ITEM).unwrap().pending_conditions.unwrap();
            assert_eq!(pending.effective_at, 3 * B);

            // Charged at the old price at B and 2B, never early at the new one, then amended.
            run_to(3 * B - 1);
            assert_eq!(balance(&ALICE), 1_000 - 3 * PRICE);
            run_to(3 * B);
            assert_eq!(balance(&ALICE), 1_000 - 3 * PRICE - 15);

            // An item amendment takes each subscription's own notice.
            publish_with_policy(OTHER, conditions(PRICE, None, None), with_notice(3));
            subscribe(&BOB, OTHER);
            Clock::set(3 * B + 1);
            assert_ok!(amend_item(OTHER, conditions(15, None, None)));
            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::pending_amendment(
                    &inv(),
                    &OTHER,
                    &BOB,
                    3 * B + 1
                )
                .map(|pending| pending.effective_at),
                Some(7 * B)
            );
        })
    }

    // CTR-SUB-2, REQ-SB-7
    #[test]
    fn terminating_drops_a_pending_replacement_with_its_reason() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe(&ALICE, ITEM);
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));
            hooks();
            assert_ok!(Listings::terminate_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE
            ));
            assert_eq!(
                hooks(),
                vec![
                    Hook::ReplacementDropped(
                        key(&ALICE, ITEM),
                        OTHER,
                        ReplacementDropReason::SubscriptionTerminated
                    ),
                    ended(&ALICE, ITEM, EndReason::Terminated),
                ]
            );
        })
    }
}

/// Subscribing: the first charge, eligibility, and one live subscription per key.
mod subscribing {
    use super::*;

    // AC-E2.1, REQ-SB-2
    #[test]
    fn period_zero_is_charged_up_front() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            let s = sub(&ALICE, ITEM).unwrap();
            assert_eq!(s.paid_through, s.anchor + B);
            assert_eq!(balance(&ALICE), 1_000 - PRICE);
        })
    }

    // REQ-SB-2
    #[test]
    fn failed_first_charge_creates_nothing() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            set_funds(&ALICE, PRICE - 1);
            let owner_before = balance(&ROOT);
            hooks();

            assert_noop!(
                Listings::subscribe(RuntimeOrigin::signed(ALICE), INV, ITEM),
                ListingsError::ChargeFailed
            );
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(balance(&ALICE), PRICE - 1);
            assert_eq!(balance(&ROOT), owner_before);
            assert!(hooks().is_empty());
        })
    }

    // AC-E2.2, REQ-SB-3
    #[test]
    fn subscriptions_to_one_item_are_independent() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            Clock::set(DAYS);
            subscribe(&BOB, ITEM);
            set_funds(&BOB, 0);

            run_to(DAYS + B);
            assert_eq!(state(&ALICE, ITEM), Some(SubscriptionState::Active));
            assert_eq!(sub(&ALICE, ITEM).unwrap().paid_through, 2 * B);
            assert!(matches!(
                state(&BOB, ITEM),
                Some(SubscriptionState::Suspended { .. })
            ));
        })
    }

    // REQ-SB-3
    #[test]
    fn exclusive_item_admits_one_subscriber_once() {
        new_test_ext().execute_with(|| {
            assert_ok!(Listings::publish_item(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                BoundedVec::truncate_from(b"custom".to_vec()),
                None
            ));
            assert_ok!(Listings::set_subscription_conditions(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                conditions(PRICE, None, None),
                Some(ALICE)
            ));
            assert_ok!(Listings::set_subscription_policy(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                with_notice(1)
            ));
            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::eligibility(&inv(), &ITEM),
                Some(Eligibility::Once(ALICE))
            );
            assert_noop!(
                Listings::subscribe(RuntimeOrigin::signed(BOB), INV, ITEM),
                ListingsError::NotEligible
            );
            subscribe(&ALICE, ITEM);
            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::eligibility(&inv(), &ITEM),
                Some(Eligibility::Taken(ALICE))
            );

            assert_ok!(Listings::terminate_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE
            ));
            assert_noop!(
                Listings::subscribe(RuntimeOrigin::signed(ALICE), INV, ITEM),
                ListingsError::NotEligible
            );
            assert_noop!(
                <Listings as MutateSubscription<AccountId>>::set_exclusive(&inv(), &ITEM, &BOB),
                ListingsError::NotEligible
            );
        })
    }

    // INV-8
    #[test]
    fn one_live_subscription_per_key_in_any_state() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            set_funds(&ALICE, 100);
            assert_noop!(
                Listings::subscribe(RuntimeOrigin::signed(ALICE), INV, ITEM),
                ListingsError::AlreadySubscribed
            );
            set_funds(&ALICE, 0);
            run_to(B + G);
            set_funds(&ALICE, 100);
            assert_noop!(
                Listings::subscribe(RuntimeOrigin::signed(ALICE), INV, ITEM),
                ListingsError::AlreadySubscribed
            );
        })
    }

    // CTR-SUB-2
    #[test]
    fn subscription_calls_refuse_the_wrong_state_or_origin() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            assert_noop!(
                Listings::cancel_subscription(RuntimeOrigin::signed(ALICE), INV, ITEM),
                ListingsError::NoSubscription
            );
            subscribe(&ALICE, ITEM);
            assert_noop!(
                Listings::subscribe(RuntimeOrigin::signed(ALICE), INV, ITEM),
                ListingsError::AlreadySubscribed
            );
            assert_noop!(charge_due(&ALICE, ITEM), ListingsError::NothingDue);
            assert_noop!(
                Listings::terminate_subscription(RuntimeOrigin::signed(ALICE), INV, ITEM, ALICE),
                DispatchError::BadOrigin
            );
            assert_noop!(
                Listings::charge_due(RuntimeOrigin::none(), INV, ITEM, ALICE),
                DispatchError::BadOrigin
            );
        })
    }
}

/// Charging: when a charge is due, what it moves, and how billing periods are counted.
mod charges {
    use super::*;

    // AC-E3.1, REQ-SB-4
    #[test]
    fn due_charges_are_collected_automatically_and_by_anyone() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            subscribe(&BOB, ITEM);

            // Automatically.
            run_to(B - L);
            assert_eq!(sub(&ALICE, ITEM).unwrap().paid_through, 2 * B);
            // By anyone, within the lead, before the queue gets there.
            Clock::set(2 * B - L);
            assert_ok!(charge_due(&BOB, ITEM));
            assert_eq!(sub(&BOB, ITEM).unwrap().paid_through, 3 * B);
        })
    }

    // AC-D3.1, REQ-BL-4
    #[test]
    fn charge_is_due_only_when_due_or_within_the_lead() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            Clock::set(B - L - 1);
            assert_noop!(charge_due(&ALICE, ITEM), ListingsError::NothingDue);
            Clock::set(B - L);
            assert_ok!(charge_due(&ALICE, ITEM));
        })
    }

    // INV-10, AC-D1.1
    #[test]
    fn each_period_is_charged_once_for_exactly_its_price_to_the_owner() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            let owner_before = balance(&ROOT);
            subscribe(&ALICE, ITEM);
            run_to(5 * B);
            // Periods 0 to 5: the sixth renews within its lead before 5B.
            assert_eq!(sub(&ALICE, ITEM).unwrap().periods_charged, 6);
            assert_eq!(balance(&ROOT), owner_before + 6 * PRICE);
            assert_eq!(balance(&ALICE), 1_000 - 6 * PRICE);
        })
    }

    // AC-D1.2, INV-10
    #[test]
    fn second_charge_of_the_same_period_is_refused() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            Clock::set(B - L);
            assert_ok!(charge_due(&ALICE, ITEM));
            assert_noop!(charge_due(&ALICE, ITEM), ListingsError::NothingDue);
            // The queue does not charge it again either.
            run_to(B);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE);
        })
    }

    // REQ-BL-3
    #[test]
    fn charge_is_a_direct_payment_to_the_inventory_owner() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            assert!(System::events().iter().any(|record| matches!(
                &record.event,
                RuntimeEvent::Payments(fc_pallet_payments::Event::PaymentDirect {
                    sender,
                    beneficiary,
                    amount: PRICE,
                    fees: 0,
                    ..
                }) if *sender == ALICE && *beneficiary == ROOT
            )));
            assert_eq!(
                fc_pallet_payments::PaymentParties::<Test>::iter().count(),
                0
            );
        })
    }

    // REQ-BL-5
    #[test]
    fn failed_charge_moves_nothing() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, PRICE - 1);
            let owner_before = balance(&ROOT);
            Clock::set(B - L);
            assert_noop!(charge_due(&ALICE, ITEM), ListingsError::ChargeFailed);
            run_to(B);
            assert_eq!(balance(&ALICE), PRICE - 1);
            assert_eq!(balance(&ROOT), owner_before);
        })
    }

    // REQ-SB-11, REQ-BL-7
    #[test]
    fn zero_price_moves_nothing_and_counts_as_paid() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(0, Some(3), None));
            set_funds(&ALICE, 0);
            let owner_before = balance(&ROOT);

            subscribe_as_consumer(&ALICE, ITEM);
            run_to(2 * B);
            let s = sub(&ALICE, ITEM).unwrap();
            assert_eq!(s.periods_charged, 3);
            assert!(is_paid(&ALICE, ITEM, 2 * B));
            assert_eq!(balance(&ROOT), owner_before);
            assert_eq!(
                fc_pallet_payments::PaymentParties::<Test>::iter().count(),
                0
            );
        })
    }

    // REQ-SB-11, 0009-A19.2
    #[test]
    fn direct_subscription_to_a_zero_price_is_refused() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(0, None, None));
            assert_noop!(
                Listings::subscribe(RuntimeOrigin::signed(ALICE), INV, ITEM),
                ListingsError::ZeroPriceDirectSubscription
            );
            // A consumer may offer it, deciding itself who subscribes.
            subscribe_as_consumer(&ALICE, ITEM);
            assert_eq!(state(&ALICE, ITEM), Some(SubscriptionState::Active));
        })
    }

    // AC-D2.1, REQ-CT-4
    #[test]
    fn subscription_is_unpaid_from_the_due_tick_before_any_bookkeeping() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            assert!(is_paid(&ALICE, ITEM, B - 1));
            // The queue never ran, the state is still *Active*, and the subscription is unpaid.
            Clock::set(B);
            assert_eq!(state(&ALICE, ITEM), Some(SubscriptionState::Active));
            assert!(!is_paid(&ALICE, ITEM, B));
        })
    }

    // REQ-BL-1
    #[test]
    fn billing_period_n_starts_at_anchor_plus_n_periods() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            Clock::set(123);
            subscribe(&ALICE, ITEM);
            for n in 1..4u32 {
                run_to(123 + n as u64 * B - L + HOURS);
                let s = sub(&ALICE, ITEM).unwrap();
                assert_eq!(s.paid_through, 123 + (n as u64 + 1) * B);
                assert_eq!(s.periods_charged, n + 1);
            }
        })
    }

    // INV-14, NFR-6
    #[test]
    fn one_minute_and_two_year_periods_renew() {
        new_test_ext().execute_with(|| {
            RenewalLead::set(&2);
            let mut minute = conditions(1, None, None);
            minute.period = MINUTES;
            minute.grace = 5;
            publish(ITEM, minute);
            let mut two_years = conditions(PRICE, None, Some(1));
            two_years.period = 2 * 365 * DAYS;
            two_years.grace = 30 * DAYS;
            publish(OTHER, two_years);

            subscribe(&ALICE, ITEM);
            subscribe(&BOB, OTHER);
            for n in 1..=3u64 {
                Clock::set(n * MINUTES - 2);
                assert_ok!(charge_due(&ALICE, ITEM));
                assert_eq!(sub(&ALICE, ITEM).unwrap().paid_through, (n + 1) * MINUTES);
            }
            Clock::set(2 * 365 * DAYS - 2);
            assert_ok!(charge_due(&BOB, OTHER));
            assert_eq!(sub(&BOB, OTHER).unwrap().paid_through, 4 * 365 * DAYS);
        })
    }

    // INV-14
    #[test]
    fn period_arithmetic_refuses_or_saturates() {
        assert_eq!(effective_boundary(0u64, 0, 10, 1), None);
        assert_eq!(
            effective_boundary(0u64, u64::MAX / 2, u64::MAX - 1, 1),
            None
        );
        let c = SubscriptionConditions {
            price: (),
            period: u64::MAX / 2,
            term: None,
            min_commitment: Some(u32::MAX),
            grace: 0,
        };
        assert_eq!(c.commitment_end(10), u64::MAX);

        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            Clock::set(u64::MAX - 5);
            assert_noop!(
                Listings::subscribe(RuntimeOrigin::signed(ALICE), INV, ITEM),
                sp_runtime::ArithmeticError::Overflow
            );
        })
    }
}

/// Unpaid subscriptions: suspension, grace, default, lapse, and settling them.
mod grace_and_default {
    use super::*;

    // AC-E3.2, REQ-SB-5, REQ-SB-9
    #[test]
    fn unpaid_subscription_suspends_then_lapses_or_defaults() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(PRICE, None, Some(2)));
            subscribe(&ALICE, ITEM);
            subscribe(&ALICE, OTHER);
            set_funds(&ALICE, 0);

            run_to(B);
            assert!(matches!(
                state(&ALICE, ITEM),
                Some(SubscriptionState::Suspended { .. })
            ));
            assert!(matches!(
                state(&ALICE, OTHER),
                Some(SubscriptionState::Suspended { .. })
            ));
            run_to(B + G);
            assert_eq!(state(&ALICE, ITEM), None);
            assert_eq!(
                state(&ALICE, OTHER),
                Some(SubscriptionState::Defaulted { until: 2 * B })
            );
        })
    }

    // AC-B4.2
    #[test]
    fn paying_after_grace_is_refused() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            set_funds(&ALICE, 100);
            Clock::set(B + G);
            assert_noop!(charge_due(&ALICE, ITEM), ListingsError::GraceElapsed);
            idle();
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_noop!(charge_due(&ALICE, ITEM), ListingsError::NoSubscription);
        })
    }

    // AC-B7.2, REQ-CT-12
    #[test]
    fn defaulted_blocks_until_the_commitment_end_then_never() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B + G);
            set_funds(&ALICE, 100);
            assert!(
                <Listings as InspectSubscription<AccountId>>::blocks_new_subscription(
                    &inv(),
                    &ITEM,
                    &ALICE,
                    3 * B - 1
                )
            );
            assert!(!is_paid(&ALICE, ITEM, B + G));

            // Past the commitment end, before any bookkeeping ran.
            Clock::set(3 * B);
            assert!(
                !<Listings as InspectSubscription<AccountId>>::blocks_new_subscription(
                    &inv(),
                    &ITEM,
                    &ALICE,
                    3 * B
                )
            );
            hooks();
            subscribe(&ALICE, ITEM);
            assert_eq!(
                hooks(),
                vec![
                    ended(&ALICE, ITEM, EndReason::Lapsed),
                    Hook::Started(key(&ALICE, ITEM))
                ]
            );
            assert_eq!(sub(&ALICE, ITEM).unwrap().anchor, 3 * B);
        })
    }

    // REQ-CT-12
    #[test]
    fn settle_ends_a_defaulted_subscription_past_its_commitment_end() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B + G);
            assert_ok!(<Listings as MutateSubscription<AccountId>>::settle(
                &inv(),
                &ITEM,
                &ALICE
            ));
            assert!(sub(&ALICE, ITEM).is_some());
            Clock::set(3 * B);
            hooks();
            assert_ok!(<Listings as MutateSubscription<AccountId>>::settle(
                &inv(),
                &ITEM,
                &ALICE
            ));
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(hooks(), vec![ended(&ALICE, ITEM, EndReason::Lapsed)]);
        })
    }

    // REQ-CT-12
    #[test]
    fn anyone_settles_a_subscription_past_grace_or_commitment_end() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            subscribe(&ALICE, OTHER);
            set_funds(&ALICE, 0);
            run_to(B);
            // Nothing processes the queue past grace.
            Clock::set(B + G);
            hooks();
            assert_ok!(Listings::settle_subscription(
                RuntimeOrigin::signed(CHARLIE),
                INV,
                ITEM,
                ALICE
            ));
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_ok!(Listings::settle_subscription(
                RuntimeOrigin::signed(CHARLIE),
                INV,
                OTHER,
                ALICE
            ));
            assert_eq!(
                state(&ALICE, OTHER),
                Some(SubscriptionState::Defaulted { until: 3 * B })
            );
            Clock::set(3 * B);
            assert_ok!(Listings::settle_subscription(
                RuntimeOrigin::signed(CHARLIE),
                INV,
                OTHER,
                ALICE
            ));
            assert_eq!(sub(&ALICE, OTHER), None);
            assert_eq!(
                hooks(),
                vec![
                    ended(&ALICE, ITEM, EndReason::Lapsed),
                    Hook::Defaulted(key(&ALICE, OTHER), 3 * B),
                    ended(&ALICE, OTHER, EndReason::Lapsed),
                ]
            );
        })
    }
}

/// Cancellation: the commitment, and the free exit while an amendment is pending.
mod cancellation {
    use super::*;

    // AC-E4.1, REQ-SB-6
    #[test]
    fn cancel_takes_effect_at_the_commitment_end_and_terminate_at_once() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            subscribe(&BOB, ITEM);
            Clock::set(10);
            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            assert_ok!(Listings::terminate_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                BOB
            ));
            assert_eq!(sub(&BOB, ITEM), None);

            run_to(3 * B - 1);
            assert_eq!(sub(&ALICE, ITEM).unwrap().paid_through, 3 * B);
            run_to(3 * B);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(balance(&ALICE), 1_000 - 3 * PRICE);
        })
    }

    // AC-B7.1, REQ-CT-10
    #[test]
    fn cancelling_within_the_commitment_keeps_billing_to_its_end() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            Clock::set(10);
            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            hooks();

            run_to(3 * B);
            assert_eq!(
                hooks(),
                vec![
                    Hook::Charged(key(&ALICE, ITEM), 1, 2 * B),
                    Hook::Charged(key(&ALICE, ITEM), 2, 3 * B),
                    ended(&ALICE, ITEM, EndReason::Cancelled),
                ]
            );
        })
    }

    // REQ-SB-13
    #[test]
    fn free_exit_while_an_amendment_is_pending() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(6)));
            subscribe(&ALICE, ITEM);
            run_to(B - L);
            Clock::set(B + 10);
            assert_ok!(<Listings as MutateSubscription<AccountId>>::amend(
                &inv(),
                &ITEM,
                &ALICE,
                conditions(15, None, Some(6)),
            ));
            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));

            run_to(2 * B);
            // Ended at `paid_through` (2B ≤ b = 3B), never *Defaulted*, nothing more charged.
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE);
            subscribe(&ALICE, ITEM);
        })
    }

    // CTR-SUB-6
    #[test]
    fn free_exit_while_an_item_amendment_is_pending() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(5)));
            Clock::set(10 * DAYS);
            subscribe(&BOB, ITEM);
            run_to(20 * DAYS);
            assert_ok!(Listings::amend_item_subscriptions(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                conditions(15, None, Some(5))
            ));
            run_to(45 * DAYS);
            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(BOB),
                INV,
                ITEM
            ));
            assert_eq!(
                sub(&BOB, ITEM).unwrap().cancel_requested,
                Some(Cancellation::FreeExit)
            );
            run_to(70 * DAYS);
            assert_eq!(sub(&BOB, ITEM), None);
            assert_eq!(balance(&BOB), 1_000 - 2 * PRICE);
        })
    }

    // REQ-CT-15
    #[test]
    fn free_exit_for_an_item_amendment_after_own_amendment_boundary() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(10)));
            subscribe(&ALICE, ITEM);
            Clock::set(1);
            assert_ok!(Listings::amend_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE,
                conditions(15, None, Some(10))
            ));
            run_to(2 * B - 1);
            Clock::set(2 * B + 5);
            assert_ok!(Listings::amend_item_subscriptions(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                conditions(30, None, Some(10))
            ));
            hooks();
            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            // Its `paid_through` has passed: it ends at once, commitment waived.
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(hooks(), vec![ended(&ALICE, ITEM, EndReason::Cancelled)]);
        })
    }
}

/// Replacements: scheduling one, taking effect, falling back, and being dropped.
mod replacement {
    use super::*;

    // REQ-SB-8
    #[test]
    fn replacement_preconditions() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe(&ALICE, ITEM);
            assert_noop!(
                Listings::schedule_replacement(RuntimeOrigin::signed(ALICE), INV, ITEM, 9),
                ListingsError::NotSubscribable
            );
            assert_noop!(
                Listings::schedule_replacement(RuntimeOrigin::signed(BOB), INV, ITEM, OTHER),
                ListingsError::NoSubscription
            );
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));
            assert_noop!(
                Listings::schedule_replacement(RuntimeOrigin::signed(ALICE), INV, ITEM, OTHER),
                ListingsError::ReplacementPending
            );
        })
    }

    // 0009-A19.2, REQ-SB-11
    #[test]
    fn direct_replacement_into_a_zero_price_is_refused() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(0, None, None));
            subscribe(&ALICE, ITEM);
            assert_noop!(
                Listings::schedule_replacement(RuntimeOrigin::signed(ALICE), INV, ITEM, OTHER),
                ListingsError::ZeroPriceDirectSubscription
            );
            assert_eq!(sub(&ALICE, ITEM).unwrap().replacement, None);

            // A consumer may schedule it, deciding itself who subscribes.
            assert_ok!(
                <Listings as MutateSubscription<AccountId>>::schedule_replacement(
                    &inv(),
                    &ITEM,
                    &ALICE,
                    &OTHER
                )
            );
            assert_eq!(sub(&ALICE, ITEM).unwrap().replacement, Some(OTHER));
            run_to(B);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(state(&ALICE, OTHER), Some(SubscriptionState::Active));
        })
    }

    // REQ-SB-8
    #[test]
    fn replacement_waits_for_the_commitment_end() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(2)));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe(&ALICE, ITEM);
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));

            run_to(2 * B - 1);
            assert_eq!(sub(&ALICE, ITEM).unwrap().paid_through, 2 * B);
            run_to(2 * B);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(sub(&ALICE, OTHER).unwrap().anchor, 2 * B);
        })
    }

    // REQ-CT-9, REQ-CT-13
    #[test]
    fn replacement_at_term_end_waits_for_the_last_period() {
        new_test_ext().execute_with(|| {
            // A trial of three periods that converts into OTHER at its end.
            publish(ITEM, conditions(PRICE, Some(3), None));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe(&ALICE, ITEM);
            assert_ok!(
                <Listings as MutateSubscription<AccountId>>::schedule_replacement_at_term_end(
                    &inv(),
                    &ITEM,
                    &ALICE,
                    &OTHER
                )
            );
            hooks();

            // The boundaries within the term renew the trial.
            run_to(2 * B);
            assert_eq!(sub(&ALICE, ITEM).map(|s| s.periods_charged), Some(3));
            assert_eq!(sub(&ALICE, OTHER), None);
            assert_eq!(balance(&ALICE), 1_000 - 3 * PRICE);

            // At the term's end, the replacement takes over.
            run_to(3 * B);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(sub(&ALICE, OTHER).map(|s| s.anchor), Some(3 * B));
            assert_eq!(balance(&ALICE), 1_000 - 3 * PRICE - OTHER_PRICE);
            assert!(hooks().contains(&Hook::Replaced(key(&ALICE, ITEM), OTHER, 3 * B)));
        })
    }

    // REQ-SB-8
    #[test]
    fn replacement_falls_back_when_its_first_charge_fails() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe(&ALICE, ITEM);
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));
            set_funds(&ALICE, OTHER_PRICE - 1);
            hooks();

            run_to(B);
            assert_eq!(sub(&ALICE, OTHER), None);
            let s = sub(&ALICE, ITEM).unwrap();
            assert_eq!(s.paid_through, 2 * B);
            assert_eq!(s.replacement, None);
            assert_eq!(
                hooks(),
                vec![
                    Hook::ReplacementDropped(
                        key(&ALICE, ITEM),
                        OTHER,
                        ReplacementDropReason::ChargeFailed
                    ),
                    Hook::Charged(key(&ALICE, ITEM), 1, 2 * B),
                ]
            );
        })
    }

    // REQ-SB-8
    #[test]
    fn replacement_refused_by_a_dependant_completes_at_the_term_limit() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(0, Some(1), None));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe_as_consumer(&ALICE, ITEM);
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));
            RecordHooks::refuse_replacements(true);
            hooks();

            run_to(B);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(sub(&ALICE, OTHER), None);
            assert_eq!(balance(&ALICE), 1_000);
            assert_eq!(
                hooks(),
                vec![
                    Hook::ReplacementDropped(
                        key(&ALICE, ITEM),
                        OTHER,
                        ReplacementDropReason::Refused(REPLACEMENT_REFUSED)
                    ),
                    ended(&ALICE, ITEM, EndReason::Completed),
                ]
            );
        })
    }

    // REQ-SB-12
    #[test]
    fn scheduled_replacement_cancelled_before_it_takes_effect() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe(&ALICE, ITEM);
            assert_noop!(
                Listings::cancel_replacement(RuntimeOrigin::signed(ALICE), INV, ITEM),
                ListingsError::NoPendingReplacement
            );
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));
            hooks();
            assert_ok!(Listings::cancel_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            assert_eq!(
                hooks(),
                vec![Hook::ReplacementDropped(
                    key(&ALICE, ITEM),
                    OTHER,
                    ReplacementDropReason::ReplacementCancelled
                )]
            );

            run_to(B);
            assert_eq!(sub(&ALICE, OTHER), None);
            assert_eq!(sub(&ALICE, ITEM).unwrap().paid_through, 2 * B);
        })
    }

    // CTR-SUB-2
    #[test]
    fn replacement_not_scheduled_while_cancelling() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe(&ALICE, ITEM);
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));
            hooks();
            // Cancelling drops the replacement…
            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            assert_eq!(
                hooks(),
                vec![Hook::ReplacementDropped(
                    key(&ALICE, ITEM),
                    OTHER,
                    ReplacementDropReason::SubscriptionCancelled
                )]
            );
            // …and a cancellation pending refuses one.
            assert_noop!(
                Listings::schedule_replacement(RuntimeOrigin::signed(ALICE), INV, ITEM, OTHER),
                ListingsError::CancelPending
            );
        })
    }

    // CTR-SUB-6
    #[test]
    fn item_amendment_refuses_new_replacements_and_drops_scheduled_ones() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe(&ALICE, ITEM);
            subscribe(&BOB, ITEM);
            Clock::set(5 * DAYS);
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));
            Clock::set(20 * DAYS);
            assert_ok!(Listings::amend_item_subscriptions(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                conditions(15, None, None)
            ));
            // A replacement requested while it is pending is refused.
            assert_noop!(
                Listings::schedule_replacement(RuntimeOrigin::signed(BOB), INV, ITEM, OTHER),
                ListingsError::ChangePending
            );
            hooks();

            // One scheduled before the enactment is dropped, with the reason, and the
            // subscription renews at the old price.
            run_to(30 * DAYS);
            assert_eq!(sub(&ALICE, OTHER), None);
            assert_eq!(sub(&ALICE, ITEM).unwrap().paid_through, 60 * DAYS);
            assert!(hooks().contains(&Hook::ReplacementDropped(
                key(&ALICE, ITEM),
                OTHER,
                ReplacementDropReason::AmendmentEnacted
            )));
        })
    }

    // REQ-CT-12
    #[test]
    fn expired_defaulted_record_does_not_block_scheduling_a_replacement() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(OTHER_PRICE, None, Some(2)));
            subscribe(&ALICE, OTHER);
            set_funds(&ALICE, 0);
            run_to(B + G);
            assert_eq!(
                state(&ALICE, OTHER),
                Some(SubscriptionState::Defaulted { until: 2 * B })
            );
            set_funds(&ALICE, 1_000);
            subscribe(&ALICE, ITEM);
            // Past its commitment end, before the queue removes it.
            Clock::set(2 * B + 1);
            hooks();
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));
            assert_eq!(sub(&ALICE, OTHER), None);
            assert_eq!(hooks(), vec![ended(&ALICE, OTHER, EndReason::Lapsed)]);
        })
    }

    // REQ-CT-12
    #[test]
    fn expired_defaulted_record_does_not_block_a_replacement_taking_effect() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            let mut short = conditions(OTHER_PRICE, None, Some(2));
            short.period = 5 * DAYS;
            publish(OTHER, short);
            subscribe(&ALICE, ITEM);
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));
            subscribe(&ALICE, OTHER);
            set_funds(&ALICE, 0);
            run_to(9 * DAYS);
            assert_eq!(
                state(&ALICE, OTHER),
                Some(SubscriptionState::Defaulted { until: 10 * DAYS })
            );
            // At the replacement's tick, before the queue removes the *Defaulted* record.
            set_funds(&ALICE, 1_000);
            Clock::set(B);
            hooks();
            assert_ok!(charge_due(&ALICE, ITEM));
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(sub(&ALICE, OTHER).unwrap().anchor, B);
            assert_eq!(
                hooks(),
                vec![
                    ended(&ALICE, OTHER, EndReason::Lapsed),
                    Hook::Replaced(key(&ALICE, ITEM), OTHER, B),
                ]
            );
        })
    }
}

/// Amending one subscription: its effective boundary, and at most one change pending.
mod amendments {
    use super::*;

    // REQ-SB-10
    #[test]
    fn amendment_without_acceptance_keeps_the_anchor() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            Clock::set(7);
            subscribe(&ALICE, ITEM);
            Clock::set(10);
            assert_ok!(Listings::amend_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE,
                conditions(15, Some(10), Some(5))
            ));
            // Processed at the first bucket boundary at or after `b = 7 + 2B`.
            run_to(7 + 2 * B + HOURS);
            let s = sub(&ALICE, ITEM).unwrap();
            assert_eq!(s.anchor, 7);
            assert_eq!(s.conditions, conditions(15, Some(10), Some(5)));
        })
    }

    // REQ-SB-10
    #[test]
    fn amendment_cannot_change_the_billing_period() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            let mut other_period = conditions(15, None, None);
            other_period.period = 2 * B;
            assert_noop!(
                Listings::amend_subscription(
                    RuntimeOrigin::signed(ROOT),
                    INV,
                    ITEM,
                    ALICE,
                    other_period.clone()
                ),
                ListingsError::InvalidConditions
            );
            assert_noop!(
                Listings::amend_item_subscriptions(
                    RuntimeOrigin::signed(ROOT),
                    INV,
                    ITEM,
                    other_period
                ),
                ListingsError::InvalidConditions
            );
            assert_noop!(
                Listings::amend_subscription(
                    RuntimeOrigin::signed(ALICE),
                    INV,
                    ITEM,
                    ALICE,
                    conditions(15, None, None)
                ),
                DispatchError::BadOrigin
            );
        })
    }

    // REQ-SB-10
    #[test]
    fn at_most_one_change_pending() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe(&ALICE, ITEM);
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));
            hooks();

            // An amendment cancels the pending replacement.
            assert_ok!(<Listings as MutateSubscription<AccountId>>::amend(
                &inv(),
                &ITEM,
                &ALICE,
                conditions(15, None, None),
            ));
            assert_eq!(sub(&ALICE, ITEM).unwrap().replacement, None);
            assert_eq!(
                hooks()[0],
                Hook::ReplacementDropped(
                    key(&ALICE, ITEM),
                    OTHER,
                    ReplacementDropReason::AmendmentEnacted
                )
            );
            // While it is pending: no replacement, no second amendment.
            assert_noop!(
                Listings::schedule_replacement(RuntimeOrigin::signed(ALICE), INV, ITEM, OTHER),
                ListingsError::ChangePending
            );
            assert_noop!(
                <Listings as MutateSubscription<AccountId>>::amend(
                    &inv(),
                    &ITEM,
                    &ALICE,
                    conditions(16, None, None)
                ),
                ListingsError::ChangePending
            );
        })
    }

    // AC-A5.4
    #[test]
    fn effective_boundary_is_one_full_billing_period_after_enactment() {
        // B = 30 days, boundaries on days 30, 60 and 90.
        assert_eq!(effective_boundary(0u64, B, 27 * DAYS, 1), Some(60 * DAYS));
        assert_eq!(effective_boundary(0u64, B, 30 * DAYS, 1), Some(60 * DAYS));
        assert_eq!(effective_boundary(0u64, B, 31 * DAYS, 1), Some(90 * DAYS));

        for (enacted, effective) in [(27, 60), (30, 60), (31, 90)] {
            new_test_ext().execute_with(|| {
                publish(ITEM, conditions(PRICE, None, None));
                subscribe(&ALICE, ITEM);
                run_to(enacted * DAYS);
                assert_eq!(
                    <Listings as MutateSubscription<AccountId>>::amend(
                        &inv(),
                        &ITEM,
                        &ALICE,
                        conditions(15, None, None)
                    ),
                    Ok(effective * DAYS)
                );
            })
        }
    }

    // INV-20
    #[test]
    fn no_amended_term_before_a_full_billing_period() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            for enacted in [1, B / 2, B - L, B - 1, B] {
                Clock::set(enacted);
                let s = sub(&ALICE, ITEM).unwrap();
                let b = effective_boundary(s.anchor, B, enacted, 1).unwrap();
                assert!(b - enacted >= B);
            }
        })
    }

    // REQ-SB-13
    #[test]
    fn amended_charge_never_attempted_before_its_boundary() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            Clock::set(10);
            assert_ok!(<Listings as MutateSubscription<AccountId>>::amend(
                &inv(),
                &ITEM,
                &ALICE,
                conditions(15, None, None),
            ));

            // Period 1 renews within its lead, at the old price.
            run_to(B - L);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE);
            // Period 2 starts at the boundary: no lead, not even through `charge_due`.
            run_to(2 * B - L);
            assert_noop!(charge_due(&ALICE, ITEM), ListingsError::NothingDue);
            run_to(2 * B - 1);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE);
            run_to(2 * B);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE - 15);
        })
    }

    // AC-A5.6
    #[test]
    fn subscription_ending_before_its_boundary_never_takes_the_amendment() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            Clock::set(10 * DAYS);
            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            Clock::set(15 * DAYS);
            assert_ok!(<Listings as MutateSubscription<AccountId>>::amend(
                &inv(),
                &ITEM,
                &ALICE,
                conditions(15, None, None),
            ));
            hooks();

            run_to(60 * DAYS);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(balance(&ALICE), 1_000 - PRICE);
            assert_eq!(hooks(), vec![ended(&ALICE, ITEM, EndReason::Cancelled)]);
        })
    }

    // AC-B9.3
    #[test]
    fn after_the_amendment_the_commitment_applies_as_amended() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            Clock::set(10);
            assert_ok!(<Listings as MutateSubscription<AccountId>>::amend(
                &inv(),
                &ITEM,
                &ALICE,
                conditions(15, None, Some(4)),
            ));
            run_to(2 * B);
            assert_eq!(
                sub(&ALICE, ITEM).unwrap().conditions.min_commitment,
                Some(4)
            );

            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            assert_eq!(
                sub(&ALICE, ITEM).unwrap().cancel_requested,
                Some(Cancellation::Ordinary)
            );
            run_to(4 * B - 1);
            assert_eq!(sub(&ALICE, ITEM).unwrap().paid_through, 4 * B);
            run_to(4 * B);
            assert_eq!(sub(&ALICE, ITEM), None);
        })
    }

    // REQ-SB-10
    #[test]
    fn amend_after_own_amendment_boundary_before_the_queue_runs() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            Clock::set(1);
            assert_ok!(Listings::amend_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE,
                conditions(15, None, None)
            ));
            run_to(2 * B - 1);
            // The first amendment is in force; nothing is pending any more.
            Clock::set(2 * B + 5);
            assert_ok!(<Listings as MutateSubscription<AccountId>>::amend(
                &inv(),
                &ITEM,
                &ALICE,
                conditions(20, None, None)
            )
            .map(|effective_at| assert_eq!(effective_at, 4 * B)));
            run_to(4 * B);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE - 2 * 15 - 20);
        })
    }
}

/// Amending all of an item's subscriptions at once, each at its own boundary.
mod item_amendments {
    use super::*;

    // CTR-SUB-6
    #[test]
    fn item_amendment_applies_to_each_subscription_at_its_own_boundary() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            Clock::set(10 * DAYS);
            subscribe(&BOB, ITEM);
            run_to(20 * DAYS);

            let amendment = <Listings as MutateSubscription<AccountId>>::amend_item(
                &inv(),
                &ITEM,
                conditions(15, None, None),
            )
            .unwrap();
            assert_eq!(amendment.seq, 1);
            assert_eq!(amendment.last_boundary(1), 80 * DAYS);
            // New subscriptions get the amended conditions at once.
            Clock::set(25 * DAYS);
            subscribe(&CHARLIE, ITEM);
            assert_eq!(balance(&CHARLIE), 1_000 - 15);
            assert_eq!(sub(&CHARLIE, ITEM).unwrap().amendment_seq, 1);
            // The free exit is open for the existing ones.
            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::pending_amendment(
                    &inv(),
                    &ITEM,
                    &ALICE,
                    25 * DAYS
                ),
                Some(PendingConditions {
                    conditions: conditions(15, None, None),
                    effective_at: 60 * DAYS
                })
            );

            // ALICE: 30 at the old price, 60 at the new one, never before 60.
            run_to(59 * DAYS + 23 * HOURS);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE);
            run_to(60 * DAYS);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE - 15);
            assert_eq!(sub(&ALICE, ITEM).unwrap().amendment_seq, 1);
            // BOB (anchor day 10): 40 at the old price, 70 at the new one.
            assert_eq!(balance(&BOB), 1_000 - 2 * PRICE);
            run_to(70 * DAYS - 1);
            assert_eq!(balance(&BOB), 1_000 - 2 * PRICE);
            run_to(70 * DAYS);
            assert_eq!(balance(&BOB), 1_000 - 2 * PRICE - 15);
        })
    }

    // CTR-SUB-6, DEC-34, 0009-A19.5
    #[test]
    fn second_item_amendment_waits_only_for_live_subscriptions() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            // With no subscription, nothing waits for the first amendment.
            Clock::set(20 * DAYS);
            assert_ok!(amend_item(ITEM, conditions(15, None, None)));
            assert_ok!(amend_item(ITEM, conditions(16, None, None)));
            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::item_amendment(&inv(), &ITEM)
                    .unwrap()
                    .seq,
                2
            );
            // A subscription that starts after an amendment takes it at once: nothing waits for
            // it either.
            subscribe(&ALICE, ITEM);
            assert_eq!(behind(ITEM), 0);
            assert_ok!(amend_item(ITEM, conditions(17, None, None)));
            assert_eq!(behind(ITEM), 1);
            assert_noop!(
                amend_item(ITEM, conditions(18, None, None)),
                ListingsError::ChangePending
            );
            // ALICE (anchor day 20) applies it at day 80.
            run_to(80 * DAYS);
            assert_eq!(sub(&ALICE, ITEM).unwrap().conditions.price.amount, 17);
            assert_eq!(behind(ITEM), 0);
            assert_ok!(amend_item(ITEM, conditions(18, None, None)));
        })
    }

    // CTR-SUB-6, REQ-CT-8, INV-10, 0009-A19.5
    #[test]
    fn second_item_amendment_before_the_queue_reaches_the_first_boundary_is_refused() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            Clock::set(5);
            subscribe(&ALICE, ITEM);
            run_to(6);
            let first = <Listings as MutateSubscription<AccountId>>::amend_item(
                &inv(),
                &ITEM,
                conditions(15, None, None),
            )
            .unwrap();
            let b1 = 5 + 2 * B;
            assert_eq!(first.last_boundary(1), b1 + 1);
            run_to(b1 - 1);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE);

            // Past every boundary of the first amendment, but the queue has not reached
            // ALICE's yet: she has not applied it, so the second waits.
            Clock::set(b1 + 1);
            assert_noop!(
                amend_item(ITEM, conditions(OTHER_PRICE, None, None)),
                ListingsError::ChangePending
            );

            // The period from b1 is charged at the first amendment's price.
            run_to(b1 + 2 * HOURS);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE - 15);
            assert_ok!(amend_item(ITEM, conditions(OTHER_PRICE, None, None)));
            // And the second applies at ALICE's own boundary for it, a full period later.
            run_to(b1 + 2 * B + HOURS);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE - 2 * 15 - OTHER_PRICE);
        })
    }

    // CTR-SUB-6, 0009-A19.5
    #[test]
    fn subscriptions_ending_or_defaulting_stop_holding_back_the_next_amendment() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(6)));
            subscribe(&ALICE, ITEM);
            subscribe(&BOB, ITEM);
            subscribe(&CHARLIE, ITEM);
            Clock::set(1);
            assert_ok!(amend_item(ITEM, conditions(15, None, Some(6))));
            assert_eq!(behind(ITEM), 3);

            // ALICE is terminated, BOB defaults within his commitment, CHARLIE pays.
            assert_ok!(Listings::terminate_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE
            ));
            assert_eq!(behind(ITEM), 2);
            set_funds(&BOB, 0);
            run_to(B + G);
            assert!(matches!(
                state(&BOB, ITEM),
                Some(SubscriptionState::Defaulted { .. })
            ));
            assert_eq!(behind(ITEM), 1);
            assert_noop!(
                amend_item(ITEM, conditions(16, None, Some(6))),
                ListingsError::ChangePending
            );
            run_to(2 * B);
            assert_eq!(behind(ITEM), 0);
            assert_ok!(amend_item(ITEM, conditions(16, None, Some(6))));
            // The *Defaulted* one is not waited for.
            assert_eq!(behind(ITEM), 1);
        })
    }

    // AC-E5.1, REQ-SB-7
    #[test]
    fn item_amendment_notifies_dependants() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            Clock::set(7);
            hooks();
            assert_ok!(amend_item(ITEM, conditions(15, None, None)));
            assert_eq!(hooks(), vec![Hook::ItemAmendmentEnacted(inv(), ITEM, 1, 7)]);
        })
    }

    // CTR-SUB-6, REQ-SB-10
    #[test]
    fn item_amendment_after_own_amendment_boundary_stays_pending() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            Clock::set(1);
            assert_ok!(Listings::amend_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE,
                conditions(15, None, None)
            ));
            assert_eq!(
                sub(&ALICE, ITEM)
                    .unwrap()
                    .pending_conditions
                    .unwrap()
                    .effective_at,
                2 * B
            );
            // The item is amended just after the subscription's own amendment's boundary,
            // before the queue reaches it.
            run_to(2 * B - 1);
            Clock::set(2 * B + 5);
            assert_ok!(Listings::amend_item_subscriptions(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                conditions(30, None, None)
            ));
            let pending = Some(PendingConditions {
                conditions: conditions(30, None, None),
                effective_at: 4 * B,
            });
            let pending_amendment = || {
                <Listings as InspectSubscription<AccountId>>::pending_amendment(
                    &inv(),
                    &ITEM,
                    &ALICE,
                    Clock::now(),
                )
            };
            assert_eq!(pending_amendment(), pending);
            idle();
            assert_eq!(pending_amendment(), pending);
            assert_eq!(sub(&ALICE, ITEM).unwrap().amendment_seq, 0);

            run_to(4 * B);
            assert_eq!(balance(&ALICE), 1_000 - 2 * PRICE - 2 * 15 - 30);
            assert_eq!(sub(&ALICE, ITEM).unwrap().amendment_seq, 1);
        })
    }
}

/// What a subscription exposes: the inspection reads and the notifications to dependants.
mod reads_and_notifications {
    use super::*;

    // CTR-SUB-3
    #[test]
    fn reads_report_a_subscription_schedule() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, Some(6), Some(3)));
            Clock::set(5);
            subscribe(&ALICE, ITEM);
            type S = Listings;

            assert_eq!(
                <S as InspectSubscription<AccountId>>::next_due(&inv(), &ITEM, &ALICE),
                Some(5 + B)
            );
            assert_eq!(
                <S as InspectSubscription<AccountId>>::commitment_end(&inv(), &ITEM, &ALICE),
                Some(5 + 3 * B)
            );
            assert_eq!(
                <S as InspectSubscription<AccountId>>::grace_end(&inv(), &ITEM, &ALICE),
                Some(5 + B + G)
            );
            assert_eq!(sub(&ALICE, ITEM).unwrap().term_remaining(), Some(5));
            assert_eq!(
                <S as InspectSubscription<AccountId>>::pending_amendment(&inv(), &ITEM, &ALICE, 5),
                None
            );
            assert!(
                <S as InspectSubscription<AccountId>>::blocks_new_subscription(
                    &inv(),
                    &ITEM,
                    &ALICE,
                    5
                )
            );
            assert_eq!(
                <S as InspectSubscription<AccountId>>::next_due(&inv(), &ITEM, &BOB),
                None
            );
        })
    }

    // CTR-CALL-2
    #[test]
    fn own_refusals_read_as_typed_subscription_errors() {
        new_test_ext().execute_with(|| {
            let typed = |error: DispatchError| {
                <Listings as InspectSubscription<AccountId>>::subscription_error(&error)
            };
            assert_eq!(
                typed(ListingsError::ChangePending.into()),
                Some(SubscriptionError::ChangePending)
            );
            assert_eq!(
                typed(ListingsError::NotSubscribable.into()),
                Some(SubscriptionError::NotSubscribable)
            );
            assert_eq!(
                typed(ListingsError::ZeroPriceDirectSubscription.into()),
                Some(SubscriptionError::ZeroPriceDirectSubscription)
            );
            // Not a subscription refusal, or not this pallet's.
            assert_eq!(typed(ListingsError::UnknownItem.into()), None);
            assert_eq!(typed(pallet_assets::Error::<Test>::Unknown.into()), None);
            assert_eq!(typed(DispatchError::BadOrigin), None);
        })
    }

    // CTR-SUB-4
    #[test]
    fn notifications_name_the_subscription() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            subscribe(&BOB, ITEM);
            assert_eq!(
                hooks(),
                vec![
                    Hook::Started(key(&ALICE, ITEM)),
                    Hook::Started(key(&BOB, ITEM))
                ]
            );
        })
    }

    // AC-E5.1, REQ-SB-7
    #[test]
    fn every_transition_notifies_in_the_same_block() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            set_funds(&ALICE, 100);
            assert_ok!(charge_due(&ALICE, ITEM));
            Clock::set(B + DAYS);
            assert_ok!(<Listings as MutateSubscription<AccountId>>::amend(
                &inv(),
                &ITEM,
                &ALICE,
                conditions(15, None, None),
            ));
            run_to(3 * B);
            assert_ok!(Listings::terminate_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE
            ));

            let k = key(&ALICE, ITEM);
            assert_eq!(
                hooks(),
                vec![
                    Hook::Started(k.clone()),
                    Hook::Suspended(k.clone(), B + G),
                    Hook::Charged(k.clone(), 1, 2 * B),
                    Hook::Restored(k.clone()),
                    Hook::AmendmentEnacted(k.clone(), 3 * B),
                    Hook::Charged(k.clone(), 2, 3 * B),
                    Hook::AmendmentInForce(k.clone(), 3 * B),
                    Hook::Charged(k.clone(), 3, 4 * B),
                    Hook::Ended(k, EndReason::Terminated),
                ]
            );
        })
    }
}

/// The due queue: bounded work per block, backlog order, weights, bucket overflow and storage
/// bounds, and a full queue that never stops, strands or refuses anything.
mod queue {
    use super::*;
    use crate::DueOverflow;

    const FILLER: u32 = 3;

    /// Fills the bucket of `tick` and the two after it (`MaxDuePerBucket` is 1), with
    /// subscriptions to another item whose next action falls at `tick`.
    fn fill_buckets_at(tick: u64) {
        MaxDuePerBucket::set(&1);
        let now = Clock::now();
        let mut filler = conditions(PRICE, None, None);
        filler.period = tick - now + L;
        publish(FILLER, filler);
        for i in 0..3u8 {
            let who = AccountId::new([100 + i; 32]);
            set_funds(&who, 1_000);
            subscribe(&who, FILLER);
        }
        let bucket = crate::Pallet::<Test>::bucket_of(tick);
        for offset in 0..3 {
            assert_eq!(DueQueue::<Test>::get(bucket + offset).len(), 1);
        }
    }

    fn fund_accounts(n: u8) -> Vec<AccountId> {
        (10..10 + n)
            .map(|i| {
                let who = AccountId::new([i; 32]);
                set_funds(&who, 1_000);
                who
            })
            .collect()
    }

    // NFR-3, AC-E3.1
    #[test]
    fn charges_per_block_are_bounded_and_the_rest_stay_queued() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            let accounts = fund_accounts(6);
            for who in &accounts {
                subscribe(who, ITEM);
            }
            Clock::set(B - L);
            idle();
            let renewed = |accounts: &[AccountId]| {
                accounts
                    .iter()
                    .filter(|who| sub(who, ITEM).unwrap().periods_charged == 2)
                    .count()
            };
            assert_eq!(renewed(&accounts), MaxChargesPerBlock::get() as usize);
            idle();
            assert_eq!(renewed(&accounts), 6);
        })
    }

    // NFR-3
    #[test]
    fn backlog_drains_oldest_first() {
        new_test_ext().execute_with(|| {
            MaxChargesPerBlock::set(&1);
            publish(ITEM, conditions(PRICE, None, None));
            let accounts = fund_accounts(3);
            for (i, who) in accounts.iter().enumerate().rev() {
                Clock::set(i as u64 * DAYS);
                subscribe(who, ITEM);
            }
            // Every renewal is due; the queue fell behind.
            Clock::set(3 * DAYS + B);
            for expected in 0..3 {
                idle();
                let renewed: Vec<usize> = accounts
                    .iter()
                    .enumerate()
                    .filter(|(_, who)| sub(who, ITEM).unwrap().periods_charged == 2)
                    .map(|(i, _)| i)
                    .collect();
                assert_eq!(renewed, (0..=expected).collect::<Vec<_>>());
            }
        })
    }

    // NFR-3
    #[test]
    fn queue_is_never_overweight() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            let accounts = fund_accounts(3);
            for who in &accounts {
                subscribe(who, ITEM);
            }
            // The walk has caught up with the clock, so the next call reads one bucket.
            run_to(B - L - HOURS);
            Clock::set(B - L);

            // Too little for the bookkeeping: nothing is done.
            let tiny = Weight::from_parts(1, 1);
            assert!(<Listings as Hooks<u64>>::on_idle(1, tiny).all_lte(tiny));
            assert_eq!(sub(&accounts[0], ITEM).unwrap().periods_charged, 1);

            // Enough for one entry: one is processed, the rest stay queued.
            let base = <<Test as frame_system::Config>::DbWeight as Get<
                frame_support::weights::RuntimeDbWeight,
            >>::get()
            .reads_writes(4, 1);
            let one = base
                .saturating_add(<() as crate::WeightInfo>::skip_empty_bucket())
                .saturating_add(crate::Pallet::<Test>::due_item_weight());
            let consumed = <Listings as Hooks<u64>>::on_idle(1, one);
            assert!(consumed.all_lte(one));
            let renewed = accounts
                .iter()
                .filter(|who| sub(who, ITEM).unwrap().periods_charged == 2)
                .count();
            assert_eq!(renewed, 1);
            assert_eq!(
                DueQueue::<Test>::get(crate::Pallet::<Test>::bucket_of(B - L)).len(),
                2
            );
        })
    }

    // NFR-3
    #[test]
    fn empty_buckets_are_weighed_and_walked() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            assert_eq!(DueCursor::<Test>::get(), Some(0));
            Clock::set(10 * HOURS);
            let consumed = idle();
            assert_eq!(DueCursor::<Test>::get(), Some(11));
            assert!(
                consumed.ref_time()
                    >= 11 * <() as crate::WeightInfo>::skip_empty_bucket().ref_time()
            );
        })
    }

    // NFR-5, REQ-SB-4, 0009-A19.2
    #[test]
    fn full_buckets_overflow_into_the_next_ones_then_their_overflow() {
        new_test_ext().execute_with(|| {
            MaxDuePerBucket::set(&1);
            publish(ITEM, conditions(PRICE, None, None));
            let accounts = fund_accounts(4);
            for who in &accounts[..3] {
                subscribe(who, ITEM);
            }
            let bucket = crate::Pallet::<Test>::bucket_of(B - L);
            for offset in 0..3 {
                assert_eq!(DueQueue::<Test>::get(bucket + offset).len(), 1);
            }
            // A new subscription is never refused for a full queue: it overflows, one record.
            subscribe(&accounts[3], ITEM);
            let key = (INV, ITEM, accounts[3].clone());
            assert!(DueOverflow::<Test>::contains_key(bucket, &key));
            run_to(B);
            assert!(!DueOverflow::<Test>::contains_key(bucket, &key));
            assert_eq!(sub(&accounts[3], ITEM).unwrap().periods_charged, 2);
        })
    }

    // REQ-SB-4, 0009-A5
    #[test]
    #[should_panic(expected = "`DueBucketSize` must not be longer than `RenewalLead`")]
    fn integrity_test_refuses_a_bucket_longer_than_the_lead() {
        new_test_ext().execute_with(|| {
            RenewalLead::set(&(HOURS - 1));
            <Listings as Hooks<u64>>::integrity_test();
        })
    }

    // NFR-5
    #[test]
    fn stored_records_are_bounded() {
        use codec::MaxEncodedLen;
        assert!(crate::SubscriptionRecordOf::<Test>::max_encoded_len() < 512);
        assert!(crate::ItemSubscriptionOf::<Test>::max_encoded_len() < 128);
        assert!(crate::ItemAmendmentOf::<Test>::max_encoded_len() < 128);
    }

    // REQ-SB-4
    #[test]
    fn full_queue_never_stops_renewals() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            fill_buckets_at(2 * B - L);
            run_to(B);
            assert_eq!(sub(&ALICE, ITEM).unwrap().periods_charged, 2);
            // Its renewal found no room in the buckets, and went to the overflow.
            let bucket = crate::Pallet::<Test>::bucket_of(2 * B - L);
            assert!(DueOverflow::<Test>::contains_key(
                bucket,
                (INV, ITEM, ALICE)
            ));
            run_to(2 * B);
            assert_eq!(sub(&ALICE, ITEM).unwrap().periods_charged, 3);
            assert!(is_paid(&ALICE, ITEM, 2 * B));
        })
    }

    // REQ-SB-5, INV-8
    #[test]
    fn full_queue_never_strands_a_suspended_subscription() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            fill_buckets_at(B + G);
            run_to(B);
            assert!(matches!(
                state(&ALICE, ITEM),
                Some(SubscriptionState::Suspended { .. })
            ));
            hooks();
            run_to(B + G + 3 * HOURS);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert!(hooks().contains(&ended(&ALICE, ITEM, EndReason::Lapsed)));
            subscribe(&BOB, ITEM);
        })
    }

    // REQ-CT-11, REQ-CT-12
    #[test]
    fn full_queue_never_strands_a_defaulted_subscription() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            fill_buckets_at(3 * B);
            run_to(B + G);
            assert_eq!(
                state(&ALICE, ITEM),
                Some(SubscriptionState::Defaulted { until: 3 * B })
            );
            hooks();
            run_to(3 * B + 3 * HOURS);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert!(hooks().contains(&ended(&ALICE, ITEM, EndReason::Lapsed)));
        })
    }

    // REQ-CT-5, REQ-SB-6
    #[test]
    fn full_queue_never_refuses_a_cancellation() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            fill_buckets_at(B);
            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            run_to(B);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(balance(&ALICE), 1_000 - PRICE);
        })
    }

    // REQ-CT-15, REQ-SB-13
    #[test]
    fn full_queue_never_refuses_the_free_exit() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(5)));
            subscribe(&ALICE, ITEM);
            Clock::set(1);
            assert_ok!(Listings::amend_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                ALICE,
                conditions(15, None, Some(5))
            ));
            fill_buckets_at(B);
            assert_ok!(Listings::cancel_subscription(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            assert_eq!(
                sub(&ALICE, ITEM).unwrap().cancel_requested,
                Some(Cancellation::FreeExit)
            );
            run_to(B);
            assert_eq!(sub(&ALICE, ITEM), None);
            assert_eq!(balance(&ALICE), 1_000 - PRICE);
        })
    }

    // REQ-SB-8, REQ-SB-10
    #[test]
    fn full_queue_never_refuses_a_replacement_or_an_amendment() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            publish(OTHER, conditions(OTHER_PRICE, None, None));
            subscribe(&ALICE, ITEM);
            Clock::set(HOURS);
            subscribe(&BOB, ITEM);
            fill_buckets_at(B);
            // Each moves the next action to a due tick whose buckets are full: `B` for ALICE's
            // replacement, `B + HOURS` for BOB's amendment.
            assert_ok!(Listings::schedule_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM,
                OTHER
            ));
            assert_ok!(Listings::amend_subscription(
                RuntimeOrigin::signed(ROOT),
                INV,
                ITEM,
                BOB,
                conditions(15, None, None)
            ));
            // And back to the lead.
            assert_ok!(Listings::cancel_replacement(
                RuntimeOrigin::signed(ALICE),
                INV,
                ITEM
            ));
            // BOB's entry overflowed past the two full buckets after `B + HOURS`.
            run_to(B + 3 * HOURS);
            assert_eq!(sub(&ALICE, ITEM).unwrap().periods_charged, 2);
            assert_eq!(balance(&BOB), 1_000 - PRICE - 15);
        })
    }

    // NFR-3
    #[test]
    fn hook_weight_is_charged_for_every_processed_entry() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            let accounts = fund_accounts(2);
            for who in &accounts {
                subscribe(who, ITEM);
            }
            run_to(B - L - HOURS);
            Clock::set(B - L);
            let renewed = |accounts: &[AccountId]| {
                accounts
                    .iter()
                    .filter(|who| sub(who, ITEM).unwrap().periods_charged == 2)
                    .count()
            };

            let bookkeeping = <<Test as frame_system::Config>::DbWeight as Get<
                frame_support::weights::RuntimeDbWeight,
            >>::get()
            .reads_writes(4, 1)
            .saturating_add(<() as crate::WeightInfo>::skip_empty_bucket());
            let entry_alone = <() as crate::WeightInfo>::process_due(1)
                .max(<() as crate::WeightInfo>::process_due_replacement())
                .max(<() as crate::WeightInfo>::process_due_overflow());
            assert_eq!(
                crate::Pallet::<Test>::due_item_weight(),
                entry_alone.saturating_add(HookWeight::get())
            );

            // Enough for an entry's own work, but not for the hooks it may run: nothing is
            // processed.
            let limit = bookkeeping.saturating_add(entry_alone);
            assert!(<Listings as Hooks<u64>>::on_idle(1, limit).all_lte(limit));
            assert_eq!(renewed(&accounts), 0);

            // With the hooks' worst case too, one entry is.
            let limit = limit.saturating_add(HookWeight::get());
            assert!(<Listings as Hooks<u64>>::on_idle(1, limit).all_lte(limit));
            assert_eq!(renewed(&accounts), 1);
        })
    }

    // NFR-3
    #[test]
    fn calls_that_can_notify_declare_the_hook_weight() {
        use crate::{Call, WeightInfo};
        use frame_support::dispatch::GetDispatchInfo;

        new_test_ext().execute_with(|| {
            let hooks = HookWeight::get();
            let weight = |call: Call<Test>| call.get_dispatch_info().call_weight;
            let with_hooks = |w: Weight| w.saturating_add(hooks);
            let who = ALICE;

            assert_eq!(
                weight(Call::subscribe {
                    inventory_id: INV,
                    id: ITEM
                }),
                with_hooks(<() as WeightInfo>::subscribe())
            );
            assert_eq!(
                weight(Call::charge_due {
                    inventory_id: INV,
                    id: ITEM,
                    who: who.clone()
                }),
                with_hooks(
                    <() as WeightInfo>::charge_due()
                        .max(<() as WeightInfo>::process_due_replacement())
                        .max(<() as WeightInfo>::process_due_overflow())
                )
            );
            assert_eq!(
                weight(Call::cancel_subscription {
                    inventory_id: INV,
                    id: ITEM
                }),
                with_hooks(<() as WeightInfo>::cancel_subscription())
            );
            assert_eq!(
                weight(Call::terminate_subscription {
                    inventory_id: INV,
                    id: ITEM,
                    who: who.clone()
                }),
                with_hooks(<() as WeightInfo>::terminate_subscription())
            );
            assert_eq!(
                weight(Call::schedule_replacement {
                    inventory_id: INV,
                    id: ITEM,
                    new_id: OTHER
                }),
                with_hooks(<() as WeightInfo>::schedule_replacement())
            );
            assert_eq!(
                weight(Call::amend_subscription {
                    inventory_id: INV,
                    id: ITEM,
                    who: who.clone(),
                    conditions: conditions(PRICE, None, None)
                }),
                with_hooks(<() as WeightInfo>::amend_subscription())
            );
            assert_eq!(
                weight(Call::cancel_replacement {
                    inventory_id: INV,
                    id: ITEM
                }),
                with_hooks(
                    <() as WeightInfo>::cancel_replacement()
                        .max(<() as WeightInfo>::process_due_overflow())
                )
            );
            assert_eq!(
                weight(Call::settle_subscription {
                    inventory_id: INV,
                    id: ITEM,
                    who: who.clone()
                }),
                with_hooks(
                    <() as WeightInfo>::settle_subscription()
                        .max(<() as WeightInfo>::process_due_overflow())
                )
            );
            // It notifies the item's amendment (`on_item_amendment_enacted`).
            assert_eq!(
                weight(Call::amend_item_subscriptions {
                    inventory_id: INV,
                    id: ITEM,
                    conditions: conditions(PRICE, None, None)
                }),
                with_hooks(<() as WeightInfo>::amend_item_subscriptions())
            );

            // Calls that notify nothing declare their own weight only.
            assert_eq!(
                weight(Call::set_subscription_conditions {
                    inventory_id: INV,
                    id: ITEM,
                    conditions: conditions(PRICE, None, None),
                    exclusive_to: None
                }),
                <() as WeightInfo>::set_subscription_conditions()
            );
        })
    }
}

/// Migration pauses: grace extended by the pause, the catch-up after it, and the billing
/// boundaries it leaves alone.
mod pauses {
    use super::*;

    // AC-D4.1
    #[test]
    fn charge_due_in_a_pause_goes_first_with_grace_extended_by_the_pause() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            // The lead attempt fails; the retry is due at B, inside the pause.
            run_to(B - 10);
            Listings::started();
            Clock::set(B + 100);
            Listings::completed();
            hooks();

            // The first block after the pause, before its transactions.
            poll();
            assert_eq!(
                state(&ALICE, ITEM),
                Some(SubscriptionState::Suspended { since: B + 100 })
            );
            // Grace end extended by the pause's whole length (110), not just past the due tick.
            assert_eq!(
                hooks(),
                vec![Hook::Suspended(key(&ALICE, ITEM), B + G + 110)]
            );
            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::grace_end(&inv(), &ITEM, &ALICE),
                Some(B + G + 110)
            );
            run_to(B + G + 109);
            assert!(sub(&ALICE, ITEM).is_some());
            Clock::set(B + G + 110);
            assert_noop!(charge_due(&ALICE, ITEM), ListingsError::GraceElapsed);
            run_to(B + G + 110 + HOURS);
            assert_eq!(sub(&ALICE, ITEM), None);
        })
    }

    // AC-D4.2
    #[test]
    fn suspended_whose_grace_ends_in_a_pause_does_not_lapse_early() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            Clock::set(B + G - 10);
            Listings::started();
            Clock::set(B + G + 50);
            Listings::completed();

            poll();
            idle();
            idle();
            assert!(matches!(
                state(&ALICE, ITEM),
                Some(SubscriptionState::Suspended { .. })
            ));
            // It can still pay.
            run_to(B + G + 55);
            set_funds(&ALICE, 100);
            assert_ok!(charge_due(&ALICE, ITEM));
            assert_eq!(state(&ALICE, ITEM), Some(SubscriptionState::Active));
        })
    }

    // AC-D4.2
    #[test]
    fn suspended_defaults_at_grace_end_plus_the_pause_length() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            Clock::set(B + G - 10);
            Listings::started();
            Clock::set(B + G + 50);
            Listings::completed();
            poll();
            idle();

            run_to(B + G + 59);
            assert!(matches!(
                state(&ALICE, ITEM),
                Some(SubscriptionState::Suspended { .. })
            ));
            run_to(B + G + HOURS);
            assert_eq!(
                state(&ALICE, ITEM),
                Some(SubscriptionState::Defaulted { until: 3 * B })
            );
        })
    }

    // AC-D4.3, INV-21
    #[test]
    fn pause_moves_no_billing_boundary_or_commitment_end() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, Some(3)));
            subscribe(&ALICE, ITEM);
            let before = sub(&ALICE, ITEM).unwrap();
            // A pause across the due tick, its lead included.
            Clock::set(B - L - 10);
            Listings::started();
            Clock::set(B + 100);
            Listings::completed();
            poll();

            let after = sub(&ALICE, ITEM).unwrap();
            assert_eq!(after.anchor, before.anchor);
            assert_eq!(after.paid_through, 2 * B);
            assert_eq!(after.commitment_end(), before.commitment_end());
            // INV-21: the pause alone suspended nothing.
            assert_eq!(after.state, SubscriptionState::Active);
        })
    }

    // INV-21
    #[test]
    fn pause_beginning_after_the_grace_end_does_not_extend_it() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            // The queue falls behind; a pause begins after the grace end has passed.
            Clock::set(B + G + 10);
            Listings::started();
            Clock::set(B + G + 1_000);
            Listings::completed();
            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::grace_end(&inv(), &ITEM, &ALICE),
                Some(B + G)
            );
            set_funds(&ALICE, 100);
            assert_noop!(charge_due(&ALICE, ITEM), ListingsError::GraceElapsed);
        })
    }

    // REQ-BL-8
    #[test]
    fn ongoing_pause_counts_until_it_ends() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            Clock::set(B + DAYS);
            Listings::started();
            Clock::set(B + DAYS + 40);
            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::grace_end(&inv(), &ITEM, &ALICE),
                Some(B + G + 40)
            );
            assert!(!Subscriptions::<Test>::iter().count().is_zero());
        })
    }

    // REQ-SB-14
    #[test]
    fn pause_never_counts_against_grace() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            // A pause of 600 ticks inside the grace.
            Clock::set(B + DAYS);
            Listings::started();
            Clock::set(B + DAYS + 600);
            Listings::completed();

            assert_eq!(
                <Listings as InspectSubscription<AccountId>>::grace_end(&inv(), &ITEM, &ALICE),
                Some(B + G + 600)
            );
        })
    }

    // CTR-SUB-7
    #[test]
    fn migration_pause_extends_grace() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&ALICE, ITEM);
            set_funds(&ALICE, 0);
            run_to(B);
            Clock::set(B + G - 10);
            Listings::started();
            Clock::set(B + G + 50);
            Listings::completed();
            poll();
            idle();
            assert!(matches!(
                state(&ALICE, ITEM),
                Some(SubscriptionState::Suspended { .. })
            ));
        })
    }

    // CTR-SUB-5
    #[test]
    fn catch_up_after_a_pause_runs_before_any_other_due_charge() {
        new_test_ext().execute_with(|| {
            publish(ITEM, conditions(PRICE, None, None));
            subscribe(&BOB, ITEM);
            Clock::set(2 * DAYS);
            subscribe(&ALICE, ITEM);
            // The queue falls behind: BOB's lead begins, unprocessed, before the pause.
            Clock::set(B - L + HOURS);
            Listings::started();
            // ALICE's lead begins during the pause.
            Clock::set(2 * DAYS + B - L + HOURS);
            Listings::completed();

            poll();
            assert_eq!(sub(&ALICE, ITEM).unwrap().paid_through, 2 * DAYS + 2 * B);
            assert_eq!(sub(&BOB, ITEM).unwrap().paid_through, B);
            // No regular work while the catch-up is open; it closes here.
            idle();
            assert_eq!(sub(&BOB, ITEM).unwrap().paid_through, B);
            assert!(CatchUpFrom::<Test>::get().is_none());
            idle();
            assert_eq!(sub(&BOB, ITEM).unwrap().paid_through, 2 * B);
        })
    }
}
