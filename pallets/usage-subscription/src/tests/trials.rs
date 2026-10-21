//! Trials and conversions (`US-B6`, `US-B8`, `REQ-OF-6`, `REQ-OF-7`, `REQ-CT-9`, `REQ-CT-13`).

use super::*;
use fc_traits_listings::item::subscriptions::ReplacementDropReason;

/// Publishes a trial of `term` periods at `price`.
fn trial(term: u32, price: Balance) -> u32 {
    publish(
        OfferKind::Trial,
        Terms {
            term: Some(term),
            price: ItemPrice {
                asset: ASSET,
                amount: price,
            },
            ..terms()
        },
    )
}

fn subscribe_converting(group: u32, offer: u32, conversion: u32) {
    assert_ok!(UsageSubscription::subscribe(
        group_origin(group),
        offer,
        Some(conversion)
    ));
}

// REQ-OF-6
#[test]
fn trial_offer_rules() {
    new_test_ext().execute_with(|| {
        let invalid = [
            // No term limit, a term limit beyond the maximum, a minimum commitment.
            Terms {
                term: None,
                ..terms()
            },
            Terms {
                term: Some(MaxTrialPeriods::get() + 1),
                ..terms()
            },
            Terms {
                term: Some(2),
                min_commitment: Some(1),
                ..terms()
            },
        ];
        for terms in invalid {
            assert_noop!(
                UsageSubscription::publish_offer(
                    RuntimeOrigin::signed(ADMIN),
                    OfferKind::Trial,
                    terms
                ),
                Error::<Test>::InvalidTerms
            );
        }
        // Free, or any price the table allows.
        trial(1, 0);
        trial(MaxTrialPeriods::get(), PRICE / 2);
    });
}

// AC-B6.1, REQ-BL-7
#[test]
fn free_trial_moves_nothing() {
    new_test_ext().execute_with(|| {
        let offer = trial(2, 0);
        subscribe(GROUP_A, offer);

        assert_eq!(
            subscription(GROUP_A).map(|s| s.state),
            Some(SubscriptionState::Active)
        );
        assert_eq!(funds(&group_account(GROUP_A)), GROUP_FUNDS);
        assert_eq!(funds(&PAYEE), 0);
        assert!(TrialUsed::<Test>::contains_key(GROUP_A));
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.kind),
            Some(OfferKind::Trial)
        );
    });
}

// §5.2: Active → Ended (completed: the trial's last period is paid); AC-B6.2, REQ-CT-9
#[test]
fn trial_without_conversion_completes() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, trial(2, 0));
        advance_to(B);
        assert_eq!(subscription(GROUP_A).map(|s| s.periods_charged), Some(2));
        clear_events();

        // It never renews: at its end it completes, and nothing is charged.
        advance_to(2 * B);
        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert_eq!(funds(&group_account(GROUP_A)), GROUP_FUNDS);
        assert_eq!(
            events(),
            vec![crate::Event::ContractEnded {
                group: GROUP_A,
                reason: EndReason::Completed
            }]
        );
    });
}

// AC-B6.3, REQ-OF-7
#[test]
fn one_trial_per_group_ever() {
    new_test_ext().execute_with(|| {
        let first = trial(1, 0);
        let second = trial(2, 0);
        subscribe(GROUP_A, first);
        advance_to(B);
        assert!(Contracts::<Test>::get(GROUP_A).is_none());

        // Any trial, after the first has ended.
        for offer in [first, second] {
            assert_noop!(
                UsageSubscription::subscribe(group_origin(GROUP_A), offer, None),
                Error::<Test>::TrialUsed
            );
        }
        // Another group may.
        subscribe(GROUP_B, second);
        // Nor is a switch into a trial possible once one was started.
        subscribe(GROUP_A, standard());
        assert_noop!(
            UsageSubscription::switch_offer(group_origin(GROUP_A), second),
            Error::<Test>::TrialUsed
        );
    });
}

// §5.2: Active (trial) → Ended (converted: a conversion is pending, and its first charge succeeds)
#[test]
fn trial_contract_is_converted_when_the_new_first_charge_succeeds() {
    new_test_ext().execute_with(|| {
        let target = standard();
        let offer = trial(3, 0);
        clear_events();

        subscribe_converting(GROUP_A, offer, target);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).and_then(|c| c.pending),
            Some(PendingChange::Convert(target))
        );
        assert_eq!(
            events(),
            vec![
                crate::Event::ContractStarted {
                    group: GROUP_A,
                    offer
                },
                crate::Event::ConversionScheduled {
                    group: GROUP_A,
                    offer: target
                },
            ]
        );

        // The trial runs its three periods.
        advance_to(B);
        advance_to(2 * B);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.offer),
            Some(offer)
        );

        // `AC-B8.1`: at its end, it converts, with the target's period 0 charged.
        clear_events();
        advance_to(3 * B);
        let contract = Contracts::<Test>::get(GROUP_A).expect("converted; qed");
        assert_eq!(contract.offer, target);
        assert_eq!(contract.kind, OfferKind::Standard);
        assert_eq!(contract.anchor, 3 * B);
        assert_eq!(funds(&PAYEE), PRICE);
        assert_eq!(
            events(),
            vec![
                crate::Event::ContractEnded {
                    group: GROUP_A,
                    reason: EndReason::Converted
                },
                crate::Event::ContractStarted {
                    group: GROUP_A,
                    offer: target
                },
            ]
        );
    });
}

// AC-B8.1
#[test]
fn conversion_takes_the_targets_terms_as_they_stand_then() {
    new_test_ext().execute_with(|| {
        let target = standard();
        subscribe_converting(GROUP_A, trial(1, 0), target);

        // The target is amended during the trial.
        Clock::set(B / 2);
        assert_ok!(UsageSubscription::amend_offer(
            RuntimeOrigin::signed(AMENDER),
            target,
            Terms {
                allowance: ALLOWANCE * 3,
                ..terms()
            }
        ));

        advance_to(B);
        let contract = Contracts::<Test>::get(GROUP_A).expect("converted; qed");
        assert_eq!(contract.offer, target);
        assert_eq!(contract.allowance, ALLOWANCE * 3);
    });
}

// AC-B8.2
#[test]
fn failed_first_charge_completes_the_trial() {
    new_test_ext().execute_with(|| {
        let target = standard();
        subscribe_converting(GROUP_A, trial(1, 0), target);
        set_funds(GROUP_A, PRICE - 1);
        clear_events();

        advance_to(B);

        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert!(subscription(GROUP_A).is_none());
        assert_eq!(
            events(),
            vec![
                crate::Event::ConversionDropped {
                    group: GROUP_A,
                    offer: target,
                    reason: ReplacementDropReason::ChargeFailed
                },
                crate::Event::ContractEnded {
                    group: GROUP_A,
                    reason: EndReason::Completed
                },
            ]
        );
    });
}

// AC-B8.3
#[test]
fn withdrawn_target_completes_the_trial_with_no_charge() {
    new_test_ext().execute_with(|| {
        let target = standard();
        subscribe_converting(GROUP_A, trial(1, 0), target);
        assert_ok!(UsageSubscription::withdraw_offer(
            RuntimeOrigin::signed(ADMIN),
            target
        ));
        clear_events();

        advance_to(B);

        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert_eq!(funds(&group_account(GROUP_A)), GROUP_FUNDS);
        assert_eq!(
            events(),
            vec![
                crate::Event::ConversionDropped {
                    group: GROUP_A,
                    offer: target,
                    reason: ReplacementDropReason::Refused(Error::<Test>::OfferWithdrawn.into())
                },
                crate::Event::ContractEnded {
                    group: GROUP_A,
                    reason: EndReason::Completed
                },
            ]
        );
    });
}

// AC-B8.3
#[test]
fn group_no_longer_eligible_completes_the_trial() {
    new_test_ext().execute_with(|| {
        let target = standard();
        subscribe_converting(GROUP_A, trial(1, 0), target);
        UsableGroups::set_unusable(GROUP_A);
        clear_events();

        advance_to(B);

        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert_eq!(funds(&group_account(GROUP_A)), GROUP_FUNDS);
        assert_eq!(
            events().first(),
            Some(&crate::Event::ConversionDropped {
                group: GROUP_A,
                offer: target,
                reason: ReplacementDropReason::Refused(Error::<Test>::GroupUnusable.into())
            })
        );
    });
}

// AC-B8.4
#[test]
fn cancelled_conversion_does_not_convert() {
    new_test_ext().execute_with(|| {
        let target = standard();
        subscribe_converting(GROUP_A, trial(1, 0), target);
        clear_events();

        assert_ok!(UsageSubscription::cancel_switch(group_origin(GROUP_A)));
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).and_then(|c| c.pending),
            None
        );
        assert_eq!(
            events(),
            vec![crate::Event::ConversionCancelled {
                group: GROUP_A,
                offer: target
            }]
        );
        assert_noop!(
            UsageSubscription::cancel_switch(group_origin(GROUP_A)),
            Error::<Test>::NoPendingSwitch
        );

        advance_to(B);
        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert_eq!(funds(&PAYEE), 0);
    });
}

// AC-B8.5
#[test]
fn invalid_conversions_are_refused_and_nothing_is_created() {
    new_test_ext().execute_with(|| {
        let offer = trial(2, 0);
        let other_trial = trial(1, 0);
        let withdrawn = standard();
        assert_ok!(UsageSubscription::withdraw_offer(
            RuntimeOrigin::signed(ADMIN),
            withdrawn
        ));
        let foreign = custom(GROUP_B, None, None);

        for conversion in [other_trial, withdrawn, foreign, 99] {
            assert_noop!(
                UsageSubscription::subscribe(group_origin(GROUP_A), offer, Some(conversion)),
                Error::<Test>::InvalidConversion
            );
        }
        // Only a trial converts.
        let target = standard();
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_A), target, Some(target)),
            Error::<Test>::InvalidConversion
        );
        assert!(!TrialUsed::<Test>::contains_key(GROUP_A));

        // A custom offer for the group itself is a valid target.
        let own = custom(GROUP_A, None, None);
        subscribe_converting(GROUP_A, offer, own);
    });
}

// AC-B8.5
#[test]
fn trial_naming_nothing_never_converts() {
    new_test_ext().execute_with(|| {
        let target = standard();
        subscribe(GROUP_A, trial(1, 0));
        advance_to(B);
        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert_ok!(UsageSubscription::subscribe(
            group_origin(GROUP_A),
            target,
            None
        ));
    });
}

// REQ-CT-7
#[test]
fn conversion_counts_as_the_pending_switch() {
    new_test_ext().execute_with(|| {
        let target = standard();
        let other = standard();
        subscribe_converting(GROUP_A, trial(2, 0), target);
        assert_noop!(
            UsageSubscription::switch_offer(group_origin(GROUP_A), other),
            Error::<Test>::SwitchPending
        );
    });
}

// REQ-CT-9
#[test]
fn switch_scheduled_during_a_trial_takes_effect_at_its_end() {
    new_test_ext().execute_with(|| {
        let target = standard();
        subscribe(GROUP_A, trial(3, 0));
        assert_ok!(UsageSubscription::switch_offer(
            group_origin(GROUP_A),
            target
        ));

        advance_to(B);
        advance_to(2 * B);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.kind),
            Some(OfferKind::Trial)
        );
        advance_to(3 * B);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.offer),
            Some(target)
        );
        assert!(events().contains(&crate::Event::ContractEnded {
            group: GROUP_A,
            reason: EndReason::Switched
        }));
    });
}
