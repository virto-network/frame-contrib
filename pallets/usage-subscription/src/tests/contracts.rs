//! Epic B and the contract half of Epics A and D: the lifecycle of a contract (SPEC §5.2, one
//! test per row at this layer), the one-contract rule, switches and terminations.

use super::*;
use fc_traits_listings::item::subscriptions::ReplacementDropReason;

/// Lets the charge at `paid_through` fail: the group's account cannot pay it.
fn underfund(group: u32) {
    set_funds(group, PRICE - 1);
}

// §5.2: — → Active (subscribe, and the first charge succeeds)
#[test]
fn contract_starts_active_when_the_first_charge_succeeds() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        Clock::set(5 * DAYS);
        clear_events();

        subscribe(GROUP_A, offer);

        // `AC-B1.1`: period 0 charged to the payee, *Active*, anchored now.
        assert_eq!(funds(&group_account(GROUP_A)), GROUP_FUNDS - PRICE);
        assert_eq!(funds(&PAYEE), PRICE);
        let sub = subscription(GROUP_A).expect("subscribed; qed");
        assert_eq!(sub.state, SubscriptionState::Active);
        assert_eq!(sub.anchor, 5 * DAYS);
        assert_eq!(sub.paid_through, 5 * DAYS + B);

        // `INV-9`: the contract copies the offer's weight terms.
        assert_eq!(
            Contracts::<Test>::get(GROUP_A),
            Some(Contract {
                offer,
                kind: OfferKind::Standard,
                allowance: ALLOWANCE,
                usage_period: U,
                anchor: 5 * DAYS,
                window_start: 5 * DAYS,
                used: Weight::zero(),
                pending: None,
                next_allowance: None,
                amendment_seq: 0,
            })
        );
        assert_eq!(
            GroupOfAccount::<Test>::get(group_account(GROUP_A)),
            Some(GROUP_A)
        );
        assert_eq!(
            events(),
            vec![crate::Event::ContractStarted {
                group: GROUP_A,
                offer
            }]
        );
    });
}

// AC-B1.2, REQ-SB-2
#[test]
fn insufficient_funds_create_nothing() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        underfund(GROUP_A);
        clear_events();

        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_A), offer, None),
            Error::<Test>::ChargeFailed
        );
        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert!(GroupOfAccount::<Test>::get(group_account(GROUP_A)).is_none());
    });
}

// REQ-GR-3, REQ-GR-4
#[test]
fn only_usable_groups_subscribe() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        // The collective's own group.
        assert_noop!(
            UsageSubscription::subscribe(group_origin(COLLECTIVE_GROUP), offer, None),
            Error::<Test>::GroupUnusable
        );
        UsableGroups::set_unusable(GROUP_B);
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_B), offer, None),
            Error::<Test>::GroupUnusable
        );
    });
}

#[test]
fn subscribe_refusals() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        assert_noop!(
            UsageSubscription::subscribe(RuntimeOrigin::signed(ALICE), offer, None),
            DispatchError::BadOrigin
        );
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_A), offer + 1, None),
            Error::<Test>::UnknownOffer
        );
        // No trial, so no conversion.
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_A), offer, Some(offer)),
            Error::<Test>::InvalidConversion
        );
    });
}

// AC-B1.3, REQ-CT-1, INV-8
#[test]
fn one_contract_per_group_in_any_state() {
    new_test_ext().execute_with(|| {
        let first = standard();
        let second = standard();
        subscribe(GROUP_A, first);

        // *Active*: to the same offer, or another one.
        for offer in [first, second] {
            assert_noop!(
                UsageSubscription::subscribe(group_origin(GROUP_A), offer, None),
                Error::<Test>::AlreadyContracted
            );
        }

        // *Suspended*.
        underfund(GROUP_A);
        advance_to(B);
        assert!(matches!(
            subscription(GROUP_A).map(|s| s.state),
            Some(SubscriptionState::Suspended { .. })
        ));
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_A), second, None),
            Error::<Test>::AlreadyContracted
        );
    });
}

// §5.2: Active → Active (a charge attempted at t ≥ d − L succeeds)
#[test]
fn active_contract_stays_active_when_charged_within_the_lead() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, standard());
        clear_events();

        advance_to(B - RenewalLead::get());

        // `AC-D1.1`: exactly the price, to the payee; `paid through` advances one period.
        assert_eq!(funds(&PAYEE), 2 * PRICE);
        assert_eq!(subscription(GROUP_A).map(|s| s.paid_through), Some(2 * B));
        assert_eq!(
            events(),
            vec![crate::Event::ContractCharged {
                group: GROUP_A,
                period: 1,
                paid_through: 2 * B
            }]
        );
    });
}

// §5.2: Active → Suspended (a charge attempted at t ≥ d fails)
#[test]
fn active_contract_is_suspended_when_the_charge_at_due_fails() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, standard());
        underfund(GROUP_A);
        clear_events();

        advance_to(B);

        assert_eq!(
            subscription(GROUP_A).map(|s| s.state),
            Some(SubscriptionState::Suspended { since: B })
        );
        assert_eq!(
            events(),
            vec![crate::Event::ContractSuspended {
                group: GROUP_A,
                grace_end: B + GRACE
            }]
        );
        // The contract is kept.
        assert!(Contracts::<Test>::contains_key(GROUP_A));
    });
}

// §5.2: Suspended → Active (a charge attempted at t < d + G succeeds)
#[test]
fn suspended_contract_is_restored_when_charged_within_grace() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        subscribe(GROUP_A, offer);
        underfund(GROUP_A);
        advance_to(B);
        set_funds(GROUP_A, GROUP_FUNDS);
        clear_events();

        Clock::set(B + GRACE - 1);
        assert_ok!(Listings::charge_due(
            RuntimeOrigin::signed(ALICE),
            fc_pallet_listings::InventoryId(0, 0),
            offer,
            group_account(GROUP_A)
        ));

        // `AC-B4.1`: *Active*, `paid through` = d + B, the same anchor.
        let sub = subscription(GROUP_A).expect("restored; qed");
        assert_eq!(sub.state, SubscriptionState::Active);
        assert_eq!(sub.paid_through, 2 * B);
        assert_eq!(sub.anchor, 0);
        assert_eq!(
            events(),
            vec![
                crate::Event::ContractCharged {
                    group: GROUP_A,
                    period: 1,
                    paid_through: 2 * B
                },
                crate::Event::ContractRestored { group: GROUP_A },
            ]
        );
    });
}

// §5.2: Suspended → Ended (lapsed: grace elapsed unpaid, and d ≥ E)
#[test]
fn suspended_contract_lapses_when_grace_ends_outside_the_commitment() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        subscribe(GROUP_A, offer);
        underfund(GROUP_A);
        advance_to(B);
        set_funds(GROUP_A, GROUP_FUNDS);

        // `AC-B4.2`: paying past grace is refused.
        Clock::set(B + GRACE);
        assert_noop!(
            Listings::charge_due(
                RuntimeOrigin::signed(ALICE),
                fc_pallet_listings::InventoryId(0, 0),
                offer,
                group_account(GROUP_A)
            ),
            listings_error(fc_pallet_listings::Error::<Test>::GraceElapsed)
        );

        clear_events();
        advance_to(B + GRACE);
        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert!(GroupOfAccount::<Test>::get(group_account(GROUP_A)).is_none());
        assert_eq!(
            events(),
            vec![crate::Event::ContractEnded {
                group: GROUP_A,
                reason: EndReason::Lapsed
            }]
        );

        // The group may subscribe again.
        subscribe(GROUP_A, offer);
    });
}

// §5.2: Suspended → Defaulted (grace elapsed unpaid, and d < E)
#[test]
fn suspended_contract_defaults_when_grace_ends_within_the_commitment() {
    new_test_ext().execute_with(|| {
        let offer = custom(GROUP_A, Some(3), None);
        subscribe(GROUP_A, offer);
        underfund(GROUP_A);
        advance_to(B);
        clear_events();

        advance_to(B + GRACE);

        // `AC-B7.2`: *Defaulted* until the commitment end, kept, blocking the group.
        assert_eq!(
            subscription(GROUP_A).map(|s| s.state),
            Some(SubscriptionState::Defaulted { until: 3 * B })
        );
        assert!(Contracts::<Test>::contains_key(GROUP_A));
        assert_eq!(
            events(),
            vec![crate::Event::ContractDefaulted {
                group: GROUP_A,
                until: 3 * B
            }]
        );
        let other = standard();
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_A), other, None),
            Error::<Test>::AlreadyContracted
        );
        // No arrears are collected.
        set_funds(GROUP_A, GROUP_FUNDS);
        advance_to(2 * B);
        assert_eq!(funds(&group_account(GROUP_A)), GROUP_FUNDS);
    });
}

// §5.2: Defaulted → Ended (lapsed: t ≥ E)
#[test]
fn defaulted_contract_lapses_at_the_commitment_end() {
    new_test_ext().execute_with(|| {
        let offer = custom(GROUP_A, Some(3), None);
        subscribe(GROUP_A, offer);
        underfund(GROUP_A);
        advance_to(B);
        advance_to(B + GRACE);
        set_funds(GROUP_A, GROUP_FUNDS);
        clear_events();

        // `AC-B7.3`: at the commitment end it ends (*lapsed*), and subscribing succeeds.
        advance_to(3 * B);
        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert_eq!(
            events(),
            vec![crate::Event::ContractEnded {
                group: GROUP_A,
                reason: EndReason::Lapsed
            }]
        );
        subscribe(GROUP_A, standard());
    });
}

// REQ-CT-12
#[test]
fn defaulted_contract_past_its_commitment_end_never_blocks() {
    new_test_ext().execute_with(|| {
        let offer = custom(GROUP_A, Some(3), None);
        subscribe(GROUP_A, offer);
        underfund(GROUP_A);
        advance_to(B);
        advance_to(B + GRACE);
        set_funds(GROUP_A, GROUP_FUNDS);
        let other = standard();
        clear_events();

        // The due queue has not run at the commitment end yet: the record is still there.
        Clock::set(3 * B);
        assert!(Contracts::<Test>::contains_key(GROUP_A));

        subscribe(GROUP_A, other);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.offer),
            Some(other)
        );
        assert_eq!(
            events(),
            vec![
                crate::Event::ContractEnded {
                    group: GROUP_A,
                    reason: EndReason::Lapsed
                },
                crate::Event::ContractStarted {
                    group: GROUP_A,
                    offer: other
                },
            ]
        );
    });
}

// REQ-CT-1, REQ-CT-12
#[test]
fn suspended_contract_past_its_grace_end_never_blocks() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        subscribe(GROUP_A, offer);
        underfund(GROUP_A);
        advance_to(B);
        set_funds(GROUP_A, GROUP_FUNDS);
        let other = standard();
        clear_events();

        // The due queue has not run past the grace end yet: the record is still *Suspended*.
        Clock::set(B + GRACE + 1);
        assert!(matches!(
            subscription(GROUP_A).map(|s| s.state),
            Some(SubscriptionState::Suspended { .. })
        ));

        subscribe(GROUP_A, other);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.offer),
            Some(other)
        );
        assert_eq!(
            events(),
            vec![
                crate::Event::ContractEnded {
                    group: GROUP_A,
                    reason: EndReason::Lapsed
                },
                crate::Event::ContractStarted {
                    group: GROUP_A,
                    offer: other
                },
            ]
        );
    });
}

// REQ-CT-1, REQ-CT-12
#[test]
fn suspended_contract_past_its_commitment_end_never_blocks() {
    new_test_ext().execute_with(|| {
        let offer = custom(GROUP_A, Some(3), None);
        subscribe(GROUP_A, offer);
        underfund(GROUP_A);
        advance_to(B);
        set_funds(GROUP_A, GROUP_FUNDS);
        let other = standard();
        clear_events();

        // Neither the default at the grace end nor the end at the commitment end has run yet.
        Clock::set(3 * B);
        assert!(matches!(
            subscription(GROUP_A).map(|s| s.state),
            Some(SubscriptionState::Suspended { .. })
        ));

        subscribe(GROUP_A, other);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.offer),
            Some(other)
        );
    });
}

// REQ-CT-1
#[test]
fn suspended_contract_within_grace_still_blocks() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, standard());
        underfund(GROUP_A);
        advance_to(B);
        let other = standard();

        // Within grace, settling changes nothing, and the contract blocks.
        Clock::set(B + GRACE - 1);
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_A), other, None),
            Error::<Test>::AlreadyContracted
        );
    });
}

// §5.2: Active → Ended (completed: the term limit's last period is paid)
#[test]
fn active_contract_completes_once_its_term_limit_is_paid() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, custom(GROUP_A, None, Some(2)));
        advance_to(B - RenewalLead::get());
        assert_eq!(subscription(GROUP_A).map(|s| s.periods_charged), Some(2));
        clear_events();

        advance_to(2 * B);

        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert_eq!(funds(&PAYEE), 2 * PRICE);
        assert_eq!(
            events(),
            vec![crate::Event::ContractEnded {
                group: GROUP_A,
                reason: EndReason::Completed
            }]
        );
    });
}

// §5.2: Active → Ended (cancelled: t ≥ paid through, and paid through ≥ E)
#[test]
fn cancelled_contract_ends_at_paid_through_past_the_commitment() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, standard());
        clear_events();

        // `AC-B2.1`: the pool stays usable until `paid through`, and nothing more is charged.
        assert_ok!(UsageSubscription::cancel(group_origin(GROUP_A)));
        assert_eq!(
            events(),
            vec![crate::Event::CancellationRequested { group: GROUP_A }]
        );
        advance_to(B - RenewalLead::get());
        assert_eq!(funds(&PAYEE), PRICE);
        assert!(subscription(GROUP_A).is_some_and(|s| s.is_paid(B - 1)));

        clear_events();
        advance_to(B);
        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert_eq!(funds(&PAYEE), PRICE);
        assert_eq!(
            events(),
            vec![crate::Event::ContractEnded {
                group: GROUP_A,
                reason: EndReason::Cancelled
            }]
        );
    });
}

// AC-B7.1, REQ-CT-10
#[test]
fn cancelling_within_the_commitment_keeps_billing_to_its_end() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, custom(GROUP_A, Some(3), None));
        assert_ok!(UsageSubscription::cancel(group_origin(GROUP_A)));

        // Charges continue to the commitment end, then it ends (*cancelled*).
        advance_to(B - RenewalLead::get());
        advance_to(2 * B - RenewalLead::get());
        assert_eq!(funds(&PAYEE), 3 * PRICE);
        advance_to(3 * B - RenewalLead::get());
        assert_eq!(funds(&PAYEE), 3 * PRICE);
        clear_events();
        advance_to(3 * B);
        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert_eq!(
            events(),
            vec![crate::Event::ContractEnded {
                group: GROUP_A,
                reason: EndReason::Cancelled
            }]
        );
    });
}

// §5.2: Suspended → Ended (cancelled: the group cancels, and d ≥ E)
#[test]
fn cancelling_a_suspended_contract_outside_the_commitment_ends_it() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, standard());
        underfund(GROUP_A);
        advance_to(B);
        clear_events();

        assert_ok!(UsageSubscription::cancel(group_origin(GROUP_A)));

        // At once, and no arrears.
        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert_eq!(funds(&PAYEE), PRICE);
        assert_eq!(
            events(),
            vec![crate::Event::ContractEnded {
                group: GROUP_A,
                reason: EndReason::Cancelled
            }]
        );
    });
}

#[test]
fn cancel_refusals() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            UsageSubscription::cancel(group_origin(GROUP_A)),
            Error::<Test>::NoContract
        );
        // A *Defaulted* contract cannot be cancelled.
        subscribe(GROUP_A, custom(GROUP_A, Some(3), None));
        underfund(GROUP_A);
        advance_to(B);
        advance_to(B + GRACE);
        assert_noop!(
            UsageSubscription::cancel(group_origin(GROUP_A)),
            Error::<Test>::NoContract
        );
    });
}

// §5.2: Active → Ended (switched: a switch is pending, and the new first charge succeeds)
#[test]
fn active_contract_is_switched_when_the_new_first_charge_succeeds() {
    new_test_ext().execute_with(|| {
        let old = standard();
        let new = publish(
            OfferKind::Standard,
            Terms {
                allowance: ALLOWANCE * 2,
                price: ItemPrice {
                    asset: ASSET,
                    amount: 2 * PRICE,
                },
                ..terms()
            },
        );
        subscribe(GROUP_A, old);
        clear_events();

        assert_ok!(UsageSubscription::switch_offer(group_origin(GROUP_A), new));
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).and_then(|c| c.pending),
            Some(PendingChange::Switch(new))
        );
        assert_eq!(
            events(),
            vec![crate::Event::SwitchScheduled {
                group: GROUP_A,
                offer: new
            }]
        );

        // `AC-B3.1`: at `paid through`, the old one ends (*switched*) and the new one starts there,
        // with period 0 charged and the new offer's terms.
        clear_events();
        advance_to(B);
        assert_eq!(funds(&PAYEE), 3 * PRICE);
        let contract = Contracts::<Test>::get(GROUP_A).expect("switched; qed");
        assert_eq!(contract.offer, new);
        assert_eq!(contract.allowance, ALLOWANCE * 2);
        assert_eq!(contract.anchor, B);
        assert_eq!(contract.window_start, B);
        assert_eq!(contract.pending, None);
        assert_eq!(subscription(GROUP_A).map(|s| s.paid_through), Some(2 * B));
        assert_eq!(
            events(),
            vec![
                crate::Event::ContractEnded {
                    group: GROUP_A,
                    reason: EndReason::Switched
                },
                crate::Event::ContractStarted {
                    group: GROUP_A,
                    offer: new
                },
            ]
        );
    });
}

// AC-B3.2
#[test]
fn failed_first_charge_drops_the_switch_and_renews() {
    new_test_ext().execute_with(|| {
        let old = standard();
        let new = publish(
            OfferKind::Standard,
            Terms {
                price: ItemPrice {
                    asset: ASSET,
                    amount: 10 * PRICE,
                },
                ..terms()
            },
        );
        subscribe(GROUP_A, old);
        assert_ok!(UsageSubscription::switch_offer(group_origin(GROUP_A), new));
        set_funds(GROUP_A, 5 * PRICE);
        clear_events();

        advance_to(B);

        let contract = Contracts::<Test>::get(GROUP_A).expect("kept; qed");
        assert_eq!(contract.offer, old);
        assert_eq!(contract.pending, None);
        assert_eq!(subscription(GROUP_A).map(|s| s.paid_through), Some(2 * B));
        assert_eq!(
            events(),
            vec![
                crate::Event::SwitchDropped {
                    group: GROUP_A,
                    offer: new,
                    reason: ReplacementDropReason::ChargeFailed
                },
                crate::Event::ContractCharged {
                    group: GROUP_A,
                    period: 1,
                    paid_through: 2 * B
                },
            ]
        );
    });
}

// REQ-CT-7
#[test]
fn switch_into_a_withdrawn_offer_is_refused_at_its_tick() {
    new_test_ext().execute_with(|| {
        let old = standard();
        let new = standard();
        subscribe(GROUP_A, old);
        assert_ok!(UsageSubscription::switch_offer(group_origin(GROUP_A), new));
        assert_ok!(UsageSubscription::withdraw_offer(
            RuntimeOrigin::signed(ADMIN),
            new
        ));
        clear_events();

        advance_to(B);

        assert_eq!(Contracts::<Test>::get(GROUP_A).map(|c| c.offer), Some(old));
        assert_eq!(
            events().first(),
            Some(&crate::Event::SwitchDropped {
                group: GROUP_A,
                offer: new,
                reason: ReplacementDropReason::Refused(Error::<Test>::OfferWithdrawn.into())
            })
        );
    });
}

// REQ-CT-7
#[test]
fn switch_waits_for_the_commitment_end() {
    new_test_ext().execute_with(|| {
        let old = custom(GROUP_A, Some(2), None);
        let new = standard();
        subscribe(GROUP_A, old);
        assert_ok!(UsageSubscription::switch_offer(group_origin(GROUP_A), new));

        advance_to(B);
        assert_eq!(Contracts::<Test>::get(GROUP_A).map(|c| c.offer), Some(old));
        advance_to(2 * B);
        assert_eq!(Contracts::<Test>::get(GROUP_A).map(|c| c.offer), Some(new));
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.anchor),
            Some(2 * B)
        );
    });
}

#[test]
fn switch_offer_refusals() {
    new_test_ext().execute_with(|| {
        let old = standard();
        let new = standard();
        let other = custom(GROUP_B, None, None);
        assert_noop!(
            UsageSubscription::switch_offer(group_origin(GROUP_A), new),
            Error::<Test>::NoContract
        );
        subscribe(GROUP_A, old);
        assert_noop!(
            UsageSubscription::switch_offer(group_origin(GROUP_A), 99),
            Error::<Test>::UnknownOffer
        );
        assert_noop!(
            UsageSubscription::switch_offer(group_origin(GROUP_A), other),
            Error::<Test>::NotEligible
        );
        assert_noop!(
            UsageSubscription::switch_offer(group_origin(GROUP_A), old),
            Error::<Test>::AlreadyContracted
        );
        assert_ok!(UsageSubscription::switch_offer(group_origin(GROUP_A), new));
        assert_noop!(
            UsageSubscription::switch_offer(group_origin(GROUP_A), new),
            Error::<Test>::SwitchPending
        );

        // A withdrawn target, and a contract that is not *Active*.
        let withdrawn = standard();
        assert_ok!(UsageSubscription::withdraw_offer(
            RuntimeOrigin::signed(ADMIN),
            withdrawn
        ));
        subscribe(GROUP_B, old);
        assert_noop!(
            UsageSubscription::switch_offer(group_origin(GROUP_B), withdrawn),
            Error::<Test>::OfferWithdrawn
        );
        underfund(GROUP_B);
        advance_to(B);
        assert_noop!(
            UsageSubscription::switch_offer(group_origin(GROUP_B), new),
            Error::<Test>::NoContract
        );
    });
}

// REQ-CT-7
#[test]
fn cancel_switch_drops_the_pending_switch() {
    new_test_ext().execute_with(|| {
        let old = standard();
        let new = standard();
        subscribe(GROUP_A, old);
        assert_noop!(
            UsageSubscription::cancel_switch(group_origin(GROUP_A)),
            Error::<Test>::NoPendingSwitch
        );
        assert_noop!(
            UsageSubscription::cancel_switch(group_origin(GROUP_B)),
            Error::<Test>::NoPendingSwitch
        );
        assert_ok!(UsageSubscription::switch_offer(group_origin(GROUP_A), new));
        clear_events();

        assert_ok!(UsageSubscription::cancel_switch(group_origin(GROUP_A)));
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).and_then(|c| c.pending),
            None
        );
        assert_eq!(
            events(),
            vec![crate::Event::SwitchCancelled {
                group: GROUP_A,
                offer: new
            }]
        );

        // The old contract renews.
        advance_to(B);
        assert_eq!(Contracts::<Test>::get(GROUP_A).map(|c| c.offer), Some(old));
    });
}

#[test]
fn cancelling_drops_a_pending_switch() {
    new_test_ext().execute_with(|| {
        let new = standard();
        subscribe(GROUP_A, standard());
        assert_ok!(UsageSubscription::switch_offer(group_origin(GROUP_A), new));
        clear_events();

        assert_ok!(UsageSubscription::cancel(group_origin(GROUP_A)));
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).and_then(|c| c.pending),
            None
        );
        assert_eq!(
            events(),
            vec![
                crate::Event::SwitchDropped {
                    group: GROUP_A,
                    offer: new,
                    reason: ReplacementDropReason::SubscriptionCancelled
                },
                crate::Event::CancellationRequested { group: GROUP_A },
            ]
        );
        // A cancelling contract takes no switch.
        assert_noop!(
            UsageSubscription::switch_offer(group_origin(GROUP_A), new),
            Error::<Test>::NoContract
        );
    });
}

fn terminate_and_check(group: u32) {
    clear_events();
    let deposits = Balances::reserved_balance(group_account(group));
    let held = funds(&group_account(group));

    assert_ok!(UsageSubscription::terminate_contract(
        RuntimeOrigin::root(),
        group
    ));

    // `AC-A4.1`: at once, nothing refunded, deposits untouched, and the group may subscribe again.
    assert!(Contracts::<Test>::get(group).is_none());
    assert!(GroupOfAccount::<Test>::get(group_account(group)).is_none());
    assert_eq!(funds(&group_account(group)), held);
    assert_eq!(Balances::reserved_balance(group_account(group)), deposits);
    assert_eq!(
        events(),
        vec![crate::Event::ContractEnded {
            group,
            reason: EndReason::Terminated
        }]
    );
    set_funds(group, GROUP_FUNDS);
    subscribe(group, standard());
}

// §5.2: Active → Ended (terminated)
#[test]
fn terminating_an_active_contract_ends_it() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, standard());
        Clock::set(B / 2);
        terminate_and_check(GROUP_A);
    });
}

// §5.2: Suspended → Ended (terminated)
#[test]
fn terminating_a_suspended_contract_ends_it() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, standard());
        underfund(GROUP_A);
        advance_to(B);
        terminate_and_check(GROUP_A);
    });
}

// §5.2: Defaulted → Ended (terminated)
#[test]
fn terminating_a_defaulted_contract_ends_it() {
    new_test_ext().execute_with(|| {
        // `AC-B7.4`: the block of a *Defaulted* contract lifts at once.
        subscribe(GROUP_A, custom(GROUP_A, Some(3), None));
        underfund(GROUP_A);
        advance_to(B);
        advance_to(B + GRACE);
        terminate_and_check(GROUP_A);
    });
}

#[test]
fn terminate_contract_refusals() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            UsageSubscription::terminate_contract(RuntimeOrigin::root(), GROUP_A),
            Error::<Test>::NoContract
        );
        subscribe(GROUP_A, standard());
        for origin in [RuntimeOrigin::signed(ADMIN), group_origin(GROUP_A)] {
            assert_noop!(
                UsageSubscription::terminate_contract(origin, GROUP_A),
                DispatchError::BadOrigin
            );
        }
    });
}

#[test]
fn hooks_ignore_subscriptions_that_are_not_contracts() {
    new_test_ext().execute_with(|| {
        // A subscription of another merchant's inventory, by a group's account.
        assert_ok!(<Listings as fc_traits_listings::InventoryLifecycle<
            AccountId,
        >>::create((1, 0), &PAYEE));
        assert_ok!(Listings::publish_item(
            RuntimeOrigin::root(),
            fc_pallet_listings::InventoryId(1, 0),
            0,
            Default::default(),
            None
        ));
        assert_ok!(Listings::set_subscription_conditions(
            RuntimeOrigin::root(),
            fc_pallet_listings::InventoryId(1, 0),
            0,
            terms().conditions(),
            None
        ));
        clear_events();
        assert_ok!(Listings::subscribe(
            group_origin(GROUP_A),
            fc_pallet_listings::InventoryId(1, 0),
            0
        ));
        advance_to(2 * B);

        assert!(events().is_empty());
        assert!(Contracts::<Test>::get(GROUP_A).is_none());
    });
}
