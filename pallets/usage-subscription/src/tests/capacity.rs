//! The sales capacity (`US-A6`, `REQ-CP-1`–`REQ-CP-6`, `DEC-39`–`DEC-44`).

use super::*;
use fc_traits_listings::item::subscriptions::ReplacementDropReason;
use frame_support::traits::fungible;

/// What `allowance` per [`U`] commits per capacity period.
fn load_of(allowance: Weight) -> Weight {
    allowance * (CapacityPeriod::get() / U)
}

/// What one contract of [`ALLOWANCE`] commits.
fn one() -> Weight {
    load_of(ALLOWANCE)
}

/// `weight` less one unit of `ref_time`.
fn less_ref_time(weight: Weight) -> Weight {
    weight.saturating_sub(Weight::from_parts(1, 0))
}

/// `weight` less one unit of proof size.
fn less_proof_size(weight: Weight) -> Weight {
    weight.saturating_sub(Weight::from_parts(0, 1))
}

/// Half of [`ALLOWANCE`].
fn half() -> Weight {
    Weight::from_parts(ALLOWANCE.ref_time() / 2, ALLOWANCE.proof_size() / 2)
}

fn set_capacity(capacity: Option<Weight>) {
    assert_ok!(UsageSubscription::set_capacity(
        RuntimeOrigin::signed(CAPACITY_ADMIN),
        capacity,
        None
    ));
}

fn schedule_capacity(capacity: Option<Weight>, from: u64) {
    assert_ok!(UsageSubscription::set_capacity(
        RuntimeOrigin::signed(CAPACITY_ADMIN),
        capacity,
        Some(from)
    ));
}

fn load() -> Weight {
    CommittedLoad::<Test>::get()
}

/// The committed load is what the contracts count, together, and each offer's contract count is
/// how many of its contracts count.
fn assert_load_is_the_sum() {
    let sum = Contracts::<Test>::iter_values().fold(Weight::zero(), |sum, contract| {
        sum.saturating_add(UsageSubscription::committed_by(&contract))
    });
    assert_eq!(load(), sum);

    let mut counts = alloc::collections::BTreeMap::new();
    for contract in Contracts::<Test>::iter_values().filter(|contract| contract.load.is_some()) {
        *counts.entry(contract.offer).or_insert(0u32) += 1;
    }
    assert_eq!(
        ContractsOnOffer::<Test>::iter().collect::<alloc::collections::BTreeMap<_, _>>(),
        counts
    );
}

/// Moves the clock to `tick` an hour at a time, so each hook runs when it would on a chain, not
/// all at `tick`.
fn advance_hourly_to(tick: u64) {
    let mut now = Clock::now();
    while now < tick {
        now = (now + HOURS).min(tick);
        advance_to(now);
    }
}

/// Terms of [`terms`] with a weekly usage period: windows that do not line up with the 30-day
/// billing boundaries.
fn weekly(allowance: Weight) -> TermsOf<Test> {
    Terms {
        allowance,
        usage_period: 7 * DAYS,
        ..terms()
    }
}

/// Publishes a standard offer of `allowance`.
fn standard_of(allowance: Weight) -> u32 {
    publish(
        OfferKind::Standard,
        Terms {
            allowance,
            ..terms()
        },
    )
}

/// Publishes a one-period trial of [`ALLOWANCE`].
fn trial() -> u32 {
    publish(
        OfferKind::Trial,
        Terms {
            term: Some(1),
            ..terms()
        },
    )
}

fn amend_contract(group: u32, allowance: Weight) -> sp_runtime::DispatchResult {
    UsageSubscription::amend_contract(
        RuntimeOrigin::signed(AMENDER),
        group,
        Terms {
            allowance,
            ..terms()
        },
    )
}

fn amend_offer(offer: u32, allowance: Weight) -> sp_runtime::DispatchResult {
    UsageSubscription::amend_offer(
        RuntimeOrigin::signed(AMENDER),
        offer,
        Terms {
            allowance,
            ..terms()
        },
    )
}

/// How many `CapacityThresholdCrossed` events were emitted.
fn crossings() -> usize {
    events()
        .iter()
        .filter(|event| matches!(event, crate::Event::CapacityThresholdCrossed { .. }))
        .count()
}

/// Gives group `group` an account that can pay its deposit and `periods` billing periods.
fn fund_group(group: u32, periods: u128) {
    let account = group_account(group);
    assert_ok!(<Balances as fungible::Mutate<_>>::mint_into(
        &account, 1_000
    ));
    assert_ok!(Assets::mint_into(ASSET, &account, periods * PRICE + 1));
}

// AC-A6.1, REQ-CP-1, REQ-CP-3
#[test]
fn subscribing_within_the_capacity_raises_the_load_by_the_normalized_allowance() {
    new_test_ext().execute_with(|| {
        set_capacity(Some(one() * 2));
        let offer = standard();

        subscribe(GROUP_A, offer);

        assert_eq!(load(), one());
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).and_then(|c| c.load),
            Some(one())
        );
        assert_eq!(
            UsageSubscription::capacity(),
            CapacityInfo {
                load: one(),
                capacity: Some(one() * 2),
                scheduled: None,
            }
        );
    });
}

// AC-A6.2, REQ-CP-3
#[test]
fn subscription_beyond_the_capacity_is_refused_in_either_component() {
    for capacity in [less_ref_time(one() * 2), less_proof_size(one() * 2)] {
        new_test_ext().execute_with(|| {
            let offer = standard();
            set_capacity(Some(capacity));
            subscribe(GROUP_A, offer);

            assert_noop!(
                UsageSubscription::subscribe(group_origin(GROUP_B), offer, None),
                Error::<Test>::CapacityExceeded
            );
            assert_eq!(load(), one());
        });
    }
}

// AC-A6.2, REQ-CP-3
#[test]
fn switch_to_a_larger_allowance_is_checked() {
    for (capacity, fits) in [
        (less_ref_time(one() * 2), false),
        (less_proof_size(one() * 2), false),
        (one() * 2, true),
    ] {
        new_test_ext().execute_with(|| {
            let offer = standard();
            let larger = standard_of(ALLOWANCE * 2);
            subscribe(GROUP_A, offer);
            set_capacity(Some(capacity));

            let switch = UsageSubscription::switch_offer(group_origin(GROUP_A), larger);
            if fits {
                assert_ok!(switch);
            } else {
                assert_noop!(switch, Error::<Test>::CapacityExceeded);
            }
        });
    }
}

// AC-A6.2, REQ-CP-3
#[test]
fn switch_to_a_smaller_allowance_is_no_sale() {
    new_test_ext().execute_with(|| {
        let offer = standard_of(ALLOWANCE * 2);
        let smaller = standard();
        subscribe(GROUP_A, offer);
        set_capacity(Some(one() * 2));

        assert_ok!(UsageSubscription::switch_offer(
            group_origin(GROUP_A),
            smaller
        ));
        advance_to(B);
        assert_eq!(load(), one());
        assert_load_is_the_sum();
    });
}

// AC-A6.2, REQ-CP-3
#[test]
fn trial_conversion_is_checked_when_requested() {
    new_test_ext().execute_with(|| {
        let trial = trial();
        let larger = standard_of(ALLOWANCE * 3);
        // The trial fits, its conversion does not.
        set_capacity(Some(less_proof_size(one() * 3)));

        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_A), trial, Some(larger)),
            Error::<Test>::CapacityExceeded
        );

        set_capacity(Some(one() * 3));
        assert_ok!(UsageSubscription::subscribe(
            group_origin(GROUP_A),
            trial,
            Some(larger)
        ));
        // The trial counts until it converts.
        assert_eq!(load(), one());
    });
}

// AC-A6.2, REQ-CP-3
#[test]
fn custom_amendment_raising_the_allowance_is_checked() {
    for (capacity, fits) in [
        (less_ref_time(one() * 2), false),
        (less_proof_size(one() * 2), false),
        (one() * 2, true),
    ] {
        new_test_ext().execute_with(|| {
            subscribe(GROUP_A, custom(GROUP_A, None, None));
            set_capacity(Some(capacity));

            let amendment = amend_contract(GROUP_A, ALLOWANCE * 2);
            if fits {
                assert_ok!(amendment);
                // Counted from the enactment.
                assert_eq!(load(), one() * 2);
            } else {
                assert_noop!(amendment, Error::<Test>::CapacityExceeded);
            }
        });
    }
}

// AC-A6.2, REQ-CP-3
#[test]
fn offer_amendment_raising_the_allowance_is_checked_for_every_live_contract() {
    for (capacity, fits) in [
        (less_ref_time(one() * 4), false),
        (less_proof_size(one() * 4), false),
        (one() * 4, true),
    ] {
        new_test_ext().execute_with(|| {
            let offer = standard();
            subscribe(GROUP_A, offer);
            subscribe(GROUP_B, offer);
            set_capacity(Some(capacity));

            // Twice the allowance, for both contracts.
            let amendment = amend_offer(offer, ALLOWANCE * 2);
            if fits {
                assert_ok!(amendment);
                assert_eq!(load(), one() * 4);
                assert_load_is_the_sum();
            } else {
                assert_noop!(amendment, Error::<Test>::CapacityExceeded);
            }
        });
    }
}

// AC-A6.3, REQ-CP-1, REQ-CP-2
#[test]
fn with_no_capacity_nothing_is_capped_and_the_load_is_kept() {
    new_test_ext().execute_with(|| {
        // A week of it is a quarter of the whole range.
        let allowance = Weight::from_parts(u64::MAX / 4 / 168, u64::MAX / 4 / 168);
        let huge = standard_of(allowance);
        for group in [GROUP_A, GROUP_B, GROUP_C] {
            subscribe(group, huge);
        }

        assert_eq!(load(), load_of(allowance) * 3);
        assert_load_is_the_sum();
        assert_eq!(UsageSubscription::capacity().capacity, None);
        assert!(!crate::Capacity::<Test>::exists());
        assert_eq!(crossings(), 0);
    });
}

// AC-A6.3, REQ-CP-2
#[test]
fn removing_the_capacity_removes_the_cap() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        set_capacity(Some(one()));
        subscribe(GROUP_A, offer);
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_B), offer, None),
            Error::<Test>::CapacityExceeded
        );

        set_capacity(None);
        subscribe(GROUP_B, offer);
        assert_eq!(load(), one() * 2);
    });
}

// AC-A6.4, REQ-CP-3
#[test]
fn scheduled_decrease_binds_sales_before_its_moment() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        set_capacity(Some(one() * 3));
        schedule_capacity(Some(one()), 10 * DAYS);

        subscribe(GROUP_A, offer);
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_B), offer, None),
            Error::<Test>::CapacityExceeded
        );
    });
}

// AC-A6.4, REQ-CP-3
#[test]
fn scheduled_increase_does_not_count_before_its_moment() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        set_capacity(Some(one()));
        schedule_capacity(Some(one() * 3), 10 * DAYS);

        subscribe(GROUP_A, offer);
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_B), offer, None),
            Error::<Test>::CapacityExceeded
        );

        Clock::set(10 * DAYS);
        subscribe(GROUP_B, offer);
        subscribe(GROUP_C, offer);
        assert_eq!(load(), one() * 3);
    });
}

// REQ-CP-3
#[test]
fn scheduled_change_binds_each_component_on_its_own() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        // More `ref_time` later, less proof size later: the lower of each binds now.
        set_capacity(Some(Weight::from_parts(
            one().ref_time(),
            one().proof_size() * 3,
        )));
        schedule_capacity(
            Some(Weight::from_parts(one().ref_time() * 3, one().proof_size())),
            10 * DAYS,
        );
        assert_eq!(crate::Capacity::<Test>::get().binding(), Some(one()));

        subscribe(GROUP_A, offer);
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_B), offer, None),
            Error::<Test>::CapacityExceeded
        );
    });
}

// AC-A6.5, REQ-CP-2, REQ-CP-4
#[test]
fn capacity_below_the_load_is_refused_now_or_scheduled() {
    for below in [less_ref_time(one()), less_proof_size(one())] {
        new_test_ext().execute_with(|| {
            subscribe(GROUP_A, standard());

            assert_noop!(
                UsageSubscription::set_capacity(
                    RuntimeOrigin::signed(CAPACITY_ADMIN),
                    Some(below),
                    None
                ),
                Error::<Test>::CapacityBelowLoad
            );
            assert_noop!(
                UsageSubscription::set_capacity(
                    RuntimeOrigin::signed(CAPACITY_ADMIN),
                    Some(below),
                    Some(10 * DAYS)
                ),
                Error::<Test>::CapacityBelowLoad
            );
            // At the load, it fits.
            set_capacity(Some(one()));
        });
    }
}

// AC-A6.5, REQ-CP-2
#[test]
fn only_the_capacity_origin_sets_the_capacity() {
    new_test_ext().execute_with(|| {
        for origin in [
            RuntimeOrigin::root(),
            RuntimeOrigin::signed(ADMIN),
            RuntimeOrigin::signed(AMENDER),
            group_origin(GROUP_A),
        ] {
            assert_noop!(
                UsageSubscription::set_capacity(origin, Some(one()), None),
                DispatchError::BadOrigin
            );
        }
    });
}

// AC-A6.6, REQ-CP-1
#[test]
fn minutes_and_hours_a_week_commit_the_same_load() {
    const WEEK: u64 = 7 * DAYS;
    // A minute and an hour of execution, with as many units of proof size as seconds.
    const SECOND: u64 = 1_000_000_000_000;
    let minute = Weight::from_parts(60 * SECOND, 60);
    let hour = Weight::from_parts(3_600 * SECOND, 3_600);

    let committed = |allowance: Weight, contracts: u32| {
        let mut committed = Weight::zero();
        new_test_ext().execute_with(|| {
            let offer = publish(
                OfferKind::Standard,
                Terms {
                    allowance,
                    usage_period: WEEK,
                    ..terms()
                },
            );
            for group in 100..100 + contracts {
                fund_group(group, 1);
                subscribe(group, offer);
            }
            committed = load();
        });
        committed
    };

    let minutes = committed(minute, 2_520);
    assert_eq!(minutes, committed(hour, 42));
    assert_eq!(minutes, Weight::from_parts(151_200 * SECOND, 151_200));
}

// AC-A6.7, REQ-CP-5
#[test]
fn threshold_event_fires_once_per_upward_crossing() {
    new_test_ext().execute_with(|| {
        // Mark: 80 % of ten contracts' worth.
        set_capacity(Some(one() * 10));
        let small = standard();
        let large = standard_of(ALLOWANCE * 4);

        subscribe(GROUP_A, large);
        assert_eq!(crossings(), 0);

        subscribe(GROUP_B, large);
        assert!(events().contains(&crate::Event::CapacityThresholdCrossed {
            load: one() * 8,
            capacity: one() * 10,
        }));
        assert_eq!(crossings(), 1);

        // Further updates at or above the mark emit nothing.
        subscribe(GROUP_C, small);
        assert_ok!(UsageSubscription::terminate_contract(
            RuntimeOrigin::root(),
            GROUP_C
        ));
        assert_eq!(load(), one() * 8);
        assert_eq!(crossings(), 1);

        // Below the mark, a new crossing emits again.
        assert_ok!(UsageSubscription::terminate_contract(
            RuntimeOrigin::root(),
            GROUP_B
        ));
        assert_eq!(crossings(), 1);
        subscribe(GROUP_B, large);
        assert_eq!(crossings(), 2);
    });
}

// AC-A6.7, REQ-CP-5
#[test]
fn threshold_is_crossed_in_either_component() {
    for (threshold, allowance) in [
        // `ref_time` crosses, proof size does not.
        (
            Perbill::from_percent(50),
            Weight::from_parts(ALLOWANCE.ref_time(), 1),
        ),
        // Proof size crosses, `ref_time` does not.
        (
            Perbill::from_percent(50),
            Weight::from_parts(1, ALLOWANCE.proof_size()),
        ),
    ] {
        new_test_ext().execute_with(|| {
            CapacityAlertThreshold::set(threshold);
            set_capacity(Some(one() * 2));
            subscribe(GROUP_A, standard_of(allowance));
            assert_eq!(crossings(), 1);
        });
    }
    CapacityAlertThreshold::set(Perbill::from_percent(80));
}

// REQ-CP-5
#[test]
fn capacity_change_can_cross_the_threshold() {
    new_test_ext().execute_with(|| {
        set_capacity(Some(one() * 10));
        subscribe(GROUP_A, standard_of(ALLOWANCE * 4));
        assert_eq!(crossings(), 0);

        // 80 % of five contracts' worth is four.
        set_capacity(Some(one() * 5));
        assert_eq!(
            events().last(),
            Some(&crate::Event::CapacityThresholdCrossed {
                load: one() * 4,
                capacity: one() * 5,
            })
        );
    });
}

// AC-A6.8, REQ-CP-3
#[test]
fn pending_switch_that_no_longer_fits_is_dropped_at_its_boundary() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        let larger = standard_of(ALLOWANCE * 2);
        set_capacity(Some(one() * 3));
        subscribe(GROUP_A, offer);
        assert_ok!(UsageSubscription::switch_offer(
            group_origin(GROUP_A),
            larger
        ));
        // Sales made since leave no room for it.
        subscribe(GROUP_B, offer);
        subscribe(GROUP_C, offer);
        clear_events();

        advance_to(B);

        assert!(events().contains(&crate::Event::SwitchDropped {
            group: GROUP_A,
            offer: larger,
            reason: ReplacementDropReason::Refused(Error::<Test>::CapacityExceeded.into()),
        }));
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.offer),
            Some(offer)
        );
        assert_eq!(load(), one() * 3);
        assert_load_is_the_sum();
    });
}

// AC-A6.8, REQ-CP-3
#[test]
fn pending_conversion_that_no_longer_fits_is_dropped_at_its_boundary() {
    new_test_ext().execute_with(|| {
        let trial = trial();
        let larger = standard_of(ALLOWANCE * 2);
        set_capacity(Some(one() * 2));
        assert_ok!(UsageSubscription::subscribe(
            group_origin(GROUP_A),
            trial,
            Some(larger)
        ));
        subscribe(GROUP_B, standard());
        clear_events();

        advance_to(B);

        assert!(events().contains(&crate::Event::ConversionDropped {
            group: GROUP_A,
            offer: larger,
            reason: ReplacementDropReason::Refused(Error::<Test>::CapacityExceeded.into()),
        }));
        assert!(!Contracts::<Test>::contains_key(GROUP_A));
        assert_eq!(load(), one());
        assert_load_is_the_sum();
    });
}

// REQ-CP-2, REQ-CP-6
#[test]
fn capacity_is_declared_or_scheduled_with_its_event() {
    new_test_ext().execute_with(|| {
        set_capacity(Some(one()));
        assert_eq!(
            events().last(),
            Some(&crate::Event::CapacitySet {
                capacity: Some(one())
            })
        );

        schedule_capacity(Some(one() * 2), 10 * DAYS);
        assert_eq!(
            events().last(),
            Some(&crate::Event::CapacityScheduled {
                capacity: Some(one() * 2),
                from: 10 * DAYS,
            })
        );
        let scheduled = Some(ScheduledCapacity {
            capacity: Some(one() * 2),
            from: 10 * DAYS,
        });
        assert_eq!(UsageSubscription::capacity().scheduled, scheduled);

        // A declaration for now leaves the schedule pending.
        set_capacity(Some(one() * 3));
        assert_eq!(
            UsageSubscription::capacity(),
            CapacityInfo {
                load: Weight::zero(),
                capacity: Some(one() * 3),
                scheduled,
            }
        );

        // A new schedule replaces it.
        schedule_capacity(None, 20 * DAYS);
        assert_eq!(
            UsageSubscription::capacity().scheduled,
            Some(ScheduledCapacity {
                capacity: None,
                from: 20 * DAYS,
            })
        );

        // A schedule from a moment already reached replaces it, and applies now.
        Clock::set(5 * DAYS);
        schedule_capacity(Some(one() * 4), 5 * DAYS);
        assert_eq!(
            events().last(),
            Some(&crate::Event::CapacitySet {
                capacity: Some(one() * 4)
            })
        );
        assert_eq!(
            UsageSubscription::capacity(),
            CapacityInfo {
                load: Weight::zero(),
                capacity: Some(one() * 4),
                scheduled: None,
            }
        );
    });
}

// REQ-CP-2
#[test]
fn scheduled_change_applies_at_the_first_read_from_its_moment() {
    new_test_ext().execute_with(|| {
        set_capacity(Some(one()));
        schedule_capacity(Some(one() * 5), 10 * DAYS);
        let large = standard_of(ALLOWANCE * 4);

        Clock::set(10 * DAYS - 1);
        assert_eq!(UsageSubscription::capacity().capacity, Some(one()));

        Clock::set(10 * DAYS);
        // The view reads it applied, though nothing was written yet.
        assert_eq!(
            UsageSubscription::capacity(),
            CapacityInfo {
                load: Weight::zero(),
                capacity: Some(one() * 5),
                scheduled: None,
            }
        );
        assert!(crate::Capacity::<Test>::get().scheduled.is_some());

        // A sale fits it, and the load update stores it.
        subscribe(GROUP_A, large);
        let stored = crate::Capacity::<Test>::get();
        assert_eq!(stored.current, Some(one() * 5));
        assert_eq!(stored.scheduled, None);
    });
}

// REQ-CP-1, DEC-42
#[test]
fn load_counts_both_components_apart() {
    new_test_ext().execute_with(|| {
        let heavy = standard_of(Weight::from_parts(ALLOWANCE.ref_time() * 2, 1));
        let tall = standard_of(Weight::from_parts(1, ALLOWANCE.proof_size() * 2));
        subscribe(GROUP_A, heavy);
        subscribe(GROUP_B, tall);

        let per_week = CapacityPeriod::get() / U;
        assert_eq!(
            load(),
            Weight::from_parts(
                (ALLOWANCE.ref_time() * 2 + 1) * per_week,
                (ALLOWANCE.proof_size() * 2 + 1) * per_week,
            )
        );
    });
}

// REQ-CP-1
#[test]
fn normalization_rounds_up() {
    // Seven units an hour is 1 176 a week. One unit every three days is a third of a unit a
    // day, which rounds up to one; two units, to one as well.
    assert_eq!(
        normalized_allowance(Weight::from_parts(7, 7), HOURS, CapacityPeriod::get()),
        Weight::from_parts(7 * 168, 7 * 168)
    );
    assert_eq!(
        normalized_allowance(Weight::from_parts(1, 2), 3 * DAYS, DAYS),
        Weight::from_parts(1, 1)
    );
    assert_eq!(
        normalized_allowance(Weight::MAX, HOURS, CapacityPeriod::get()),
        Weight::MAX
    );
}

// REQ-CP-1, DEC-39, DEC-42
#[test]
fn defaulted_contract_releases_its_share_once() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, custom(GROUP_A, Some(3), None));
        subscribe(GROUP_B, standard());
        set_funds(GROUP_A, PRICE - 1);
        advance_to(B);
        assert_eq!(load(), one() * 2);

        advance_to(B + GRACE);
        assert_eq!(
            subscription(GROUP_A).map(|s| s.state),
            Some(SubscriptionState::Defaulted { until: 3 * B })
        );
        assert_eq!(load(), one());
        assert_load_is_the_sum();

        // It ends at its commitment end, releasing nothing more.
        advance_to(3 * B);
        assert!(!Contracts::<Test>::contains_key(GROUP_A));
        assert_eq!(load(), one());
    });
}

// REQ-CP-1, DEC-42
#[test]
fn ended_contracts_release_their_share() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        for group in [GROUP_A, GROUP_B, GROUP_C] {
            subscribe(group, offer);
        }
        assert_eq!(load(), one() * 3);

        assert_ok!(UsageSubscription::terminate_contract(
            RuntimeOrigin::root(),
            GROUP_A
        ));
        assert_eq!(load(), one() * 2);

        assert_ok!(UsageSubscription::cancel(group_origin(GROUP_B)));
        advance_to(B);
        assert!(!Contracts::<Test>::contains_key(GROUP_B));
        assert_eq!(load(), one());
        assert_load_is_the_sum();
    });
}

// REQ-CP-1, DEC-42
#[test]
fn switch_and_conversion_replace_the_load() {
    new_test_ext().execute_with(|| {
        let larger = standard_of(ALLOWANCE * 2);
        subscribe(GROUP_A, standard());
        assert_ok!(UsageSubscription::switch_offer(
            group_origin(GROUP_A),
            larger
        ));
        assert_ok!(UsageSubscription::subscribe(
            group_origin(GROUP_B),
            trial(),
            Some(larger)
        ));
        assert_eq!(load(), one() * 2);

        advance_to(B);

        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.offer),
            Some(larger)
        );
        assert_eq!(
            Contracts::<Test>::get(GROUP_B).map(|c| c.offer),
            Some(larger)
        );
        assert_eq!(load(), one() * 4);
        assert_load_is_the_sum();
    });
}

// REQ-CP-1, DEC-42
#[test]
fn custom_amendment_counts_an_increase_at_once_and_a_decrease_once_in_force() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, custom(GROUP_A, None, None));
        advance_to(27 * DAYS);
        assert_ok!(amend_contract(GROUP_A, ALLOWANCE * 2));
        assert_eq!(load(), one() * 2);
        advance_to(2 * B);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.allowance),
            Some(ALLOWANCE * 2)
        );
        assert_eq!(load(), one() * 2);
        assert_load_is_the_sum();

        // Down to half: counted until it is in force.
        advance_to(2 * B + DAYS);
        assert_ok!(amend_contract(GROUP_A, half()));
        assert_eq!(load(), one() * 2);
        advance_to(4 * B - DAYS);
        assert_eq!(load(), one() * 2);
        advance_to(4 * B);
        assert_eq!(load(), load_of(half()));
        assert_load_is_the_sum();
    });
}

// REQ-CP-1, DEC-42
#[test]
fn custom_amendment_raising_one_component_and_lowering_the_other() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, custom(GROUP_A, None, None));
        advance_to(27 * DAYS);
        let mixed = Weight::from_parts(ALLOWANCE.ref_time() * 2, ALLOWANCE.proof_size() / 2);

        assert_ok!(amend_contract(GROUP_A, mixed));
        // The increase counts now, the decrease not yet.
        assert_eq!(
            load(),
            Weight::from_parts(one().ref_time() * 2, one().proof_size())
        );

        advance_to(2 * B);
        assert_eq!(load(), load_of(mixed));
        assert_load_is_the_sum();
    });
}

// REQ-CP-1, DEC-42
#[test]
fn offer_amendment_counts_an_increase_at_once_and_a_decrease_once_in_force() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        subscribe(GROUP_A, offer);
        advance_to(DAYS);
        subscribe(GROUP_B, offer);
        advance_to(27 * DAYS);

        assert_ok!(amend_offer(offer, ALLOWANCE * 2));
        assert_eq!(load(), one() * 4);
        assert_load_is_the_sum();
        // A contract made from it now takes the new allowance.
        subscribe(GROUP_C, offer);
        assert_eq!(load(), one() * 6);
        assert_load_is_the_sum();

        // Each contract applies it at its own boundary; the load does not move.
        advance_to(2 * B);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).and_then(|c| c.load),
            Some(one() * 2)
        );
        assert_eq!(load(), one() * 6);
        assert_load_is_the_sum();
        advance_to(2 * B + DAYS);
        assert_eq!(
            Contracts::<Test>::get(GROUP_B).and_then(|c| c.load),
            Some(one() * 2)
        );
        assert_eq!(load(), one() * 6);
        assert_load_is_the_sum();

        // Down to half, after the last one is in force: nothing moves at the enactment.
        advance_to(3 * B);
        assert_ok!(amend_offer(offer, half()));
        assert_eq!(load(), one() * 6);
        assert_load_is_the_sum();
        // GROUP_A's boundary, then GROUP_B's, then GROUP_C's, anchored at day 27.
        advance_to(4 * B - DAYS);
        assert_eq!(load(), one() * 6);
        advance_to(4 * B);
        assert_eq!(load(), one() * 4 + load_of(half()));
        advance_to(4 * B + DAYS);
        assert_eq!(load(), one() * 2 + load_of(half()) * 2);
        advance_to(4 * B + 27 * DAYS);
        assert_eq!(load(), load_of(half()) * 3);
        assert_load_is_the_sum();
    });
}

// REQ-CP-1, DEC-42
#[test]
fn contract_that_ends_before_applying_an_offer_increase_releases_it() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        subscribe(GROUP_A, offer);
        subscribe(GROUP_B, offer);
        advance_to(27 * DAYS);
        assert_ok!(amend_offer(offer, ALLOWANCE * 2));
        assert_eq!(load(), one() * 4);

        assert_ok!(UsageSubscription::terminate_contract(
            RuntimeOrigin::root(),
            GROUP_A
        ));
        assert_eq!(load(), one() * 2);
        assert_load_is_the_sum();
    });
}

// REQ-CP-1, DEC-40, DEC-42
#[test]
fn offer_decrease_is_released_at_a_later_charge_when_its_window_starts_after_the_boundary() {
    new_test_ext().execute_with(|| {
        let offer = publish(OfferKind::Standard, weekly(ALLOWANCE));
        subscribe(GROUP_A, offer);
        assert_eq!(load(), ALLOWANCE);

        // Halved on day 1: effective at day 60, in force from the window of day 63.
        advance_to(DAYS);
        assert_ok!(UsageSubscription::amend_offer(
            RuntimeOrigin::signed(AMENDER),
            offer,
            weekly(half())
        ));

        // At the boundary, the halved allowance is not in force yet.
        advance_hourly_to(2 * B);
        assert_eq!(load(), ALLOWANCE);

        // Four renewals later, the halved allowance is in force, and so is its load.
        advance_hourly_to(4 * B);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| (c.allowance, c.next_allowance)),
            Some((half(), None))
        );
        assert_eq!(load(), half());
        assert_load_is_the_sum();
        set_capacity(Some(half()));
    });
}

// REQ-CP-1, DEC-40, DEC-42
#[test]
fn custom_decrease_is_released_at_a_later_charge_when_its_window_starts_after_the_boundary() {
    new_test_ext().execute_with(|| {
        subscribe(
            GROUP_A,
            publish(OfferKind::Custom(GROUP_A), weekly(ALLOWANCE)),
        );

        advance_to(DAYS);
        assert_ok!(UsageSubscription::amend_contract(
            RuntimeOrigin::signed(AMENDER),
            GROUP_A,
            weekly(half())
        ));

        advance_hourly_to(2 * B);
        assert_eq!(load(), ALLOWANCE);
        advance_hourly_to(4 * B);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| (c.allowance, c.next_allowance)),
            Some((half(), None))
        );
        assert_eq!(load(), half());
        assert_load_is_the_sum();
        set_capacity(Some(half()));
    });
}

// REQ-CP-1, REQ-CP-3, INV-14
#[test]
fn sale_that_would_overflow_the_load_is_refused_without_a_capacity() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        subscribe(GROUP_A, offer);

        // A week of it is the whole range: added to any load, it overflows.
        let whole = standard_of(Weight::from_parts(u64::MAX / 2, u64::MAX / 2));
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_B), whole, None),
            Error::<Test>::CapacityExceeded
        );
        assert_eq!(load(), one());

        // An offer's increase, times its two contracts, would overflow too.
        subscribe(GROUP_B, offer);
        let big = Weight::from_parts(u64::MAX / 200, u64::MAX / 200);
        assert_noop!(amend_offer(offer, big), Error::<Test>::CapacityExceeded);

        // With one contract it fits, and each end releases exactly what was added.
        assert_ok!(UsageSubscription::terminate_contract(
            RuntimeOrigin::root(),
            GROUP_B
        ));
        assert_ok!(amend_offer(offer, big));
        assert_eq!(load(), load_of(big));
        assert_load_is_the_sum();
        assert_ok!(UsageSubscription::terminate_contract(
            RuntimeOrigin::root(),
            GROUP_A
        ));
        assert_eq!(load(), Weight::zero());
        assert_load_is_the_sum();
    });
}

// REQ-CP-3, REQ-CP-5, DEC-41, DEC-44
#[test]
fn alert_measures_the_binding_capacity() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        set_capacity(Some(one() * 4));
        schedule_capacity(Some(one() * 3), 10 * DAYS);

        subscribe(GROUP_A, offer);
        subscribe(GROUP_B, offer);
        assert_eq!(crossings(), 0);

        // Three of the binding three: sales stop, and the alert has fired.
        subscribe(GROUP_C, offer);
        assert!(events().contains(&crate::Event::CapacityThresholdCrossed {
            load: one() * 3,
            capacity: one() * 3,
        }));
        fund_group(100, 1);
        assert_noop!(
            UsageSubscription::subscribe(group_origin(100), offer, None),
            Error::<Test>::CapacityExceeded
        );
        assert_eq!(crossings(), 1);
    });
}

// REQ-CP-1, REQ-CP-3
#[test]
fn direct_subscription_to_an_offer_item_does_not_count_for_an_offer_amendment() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        subscribe(GROUP_A, offer);
        // A runtime that leaves listings' own subscription open on the offers' inventory.
        assert_ok!(Assets::mint_into(ASSET, &ALICE, 10 * PRICE));
        assert_ok!(
            <Listings as fc_traits_listings::item::subscriptions::Mutate<AccountId>>::subscribe(
                &(0, 0),
                &offer,
                &ALICE,
            )
        );
        assert_eq!(ContractsOnOffer::<Test>::get(offer), 1);

        // Doubling the allowance adds one contract's increase: it fits the capacity exactly.
        set_capacity(Some(one() * 2));
        assert_ok!(amend_offer(offer, ALLOWANCE * 2));
        assert_eq!(load(), one() * 2);
        assert_load_is_the_sum();
    });
}
