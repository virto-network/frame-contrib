//! Amendments, with notice and a free exit (`US-A5`, `US-B9`, `REQ-CT-8`, `REQ-CT-14`,
//! `REQ-CT-15`, `INV-20`, `DEC-28`, `DEC-34`).

use super::*;
use fc_traits_listings::item::subscriptions::ReplacementDropReason;

/// Amended terms: twice the allowance, twice the price.
fn amended() -> TermsOf<Test> {
    Terms {
        allowance: ALLOWANCE * 2,
        price: ItemPrice {
            asset: ASSET,
            amount: 2 * PRICE,
        },
        ..terms()
    }
}

fn amend_contract(group: u32, terms: TermsOf<Test>) {
    assert_ok!(UsageSubscription::amend_contract(
        RuntimeOrigin::signed(AMENDER),
        group,
        terms
    ));
}

fn effective_at_of_last_amendment() -> Option<u64> {
    events().into_iter().rev().find_map(|event| match event {
        crate::Event::ContractAmended { effective_at, .. } => Some(effective_at),
        _ => None,
    })
}

// AC-A5.4
#[test]
fn effective_boundary_is_one_full_billing_period_after_enactment() {
    for (enacted_on, effective_on) in [(27, 60), (30, 60), (31, 90)] {
        new_test_ext().execute_with(|| {
            subscribe(GROUP_A, custom(GROUP_A, None, None));
            advance_to(enacted_on * DAYS);

            amend_contract(GROUP_A, Terms { ..terms() });

            assert_eq!(
                effective_at_of_last_amendment(),
                Some(effective_on * DAYS),
                "enacted on day {enacted_on}"
            );
        });
    }
}

// §5.2: Active → Active (an amendment that covers the contract is enacted); AC-A5.1
#[test]
fn amended_contract_takes_the_new_terms_at_its_boundary() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, custom(GROUP_A, None, None));
        advance_to(27 * DAYS);
        clear_events();

        // The group is never asked.
        amend_contract(GROUP_A, amended());
        assert_eq!(
            events(),
            vec![crate::Event::ContractAmended {
                group: GROUP_A,
                effective_at: 2 * B
            }]
        );
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).and_then(|c| c.pending),
            Some(PendingChange::Amend {
                allowance: ALLOWANCE * 2,
                effective_at: 2 * B,
                from_window: 2 * B,
            })
        );

        // The period from day 30 is at the old price; the one from b, not before b.
        advance_to(B);
        assert_eq!(funds(&PAYEE), 2 * PRICE);
        advance_to(2 * B - RenewalLead::get());
        assert_eq!(funds(&PAYEE), 2 * PRICE);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.allowance),
            Some(ALLOWANCE)
        );

        clear_events();
        advance_to(2 * B);
        assert_eq!(funds(&PAYEE), 4 * PRICE);
        let contract = Contracts::<Test>::get(GROUP_A).expect("live; qed");
        assert_eq!(contract.allowance, ALLOWANCE * 2);
        assert_eq!(contract.pending, None);
        assert_eq!(contract.anchor, 0);
        assert_eq!(
            events(),
            vec![
                crate::Event::ContractAmendmentInForce {
                    group: GROUP_A,
                    effective_at: 2 * B
                },
                crate::Event::ContractCharged {
                    group: GROUP_A,
                    period: 2,
                    paid_through: 3 * B
                },
            ]
        );
    });
}

// INV-20
#[test]
fn amended_allowance_applies_from_the_first_window_at_or_after_the_boundary() {
    new_test_ext().execute_with(|| {
        // A usage period that does not divide the billing period.
        const U7: u64 = 700;
        let offer = publish(
            OfferKind::Custom(GROUP_A),
            Terms {
                usage_period: U7,
                ..terms()
            },
        );
        subscribe(GROUP_A, offer);
        advance_to(27 * DAYS);
        amend_contract(
            GROUP_A,
            Terms {
                usage_period: U7,
                ..amended()
            },
        );
        let boundary = 2 * B;
        let from_window = boundary.div_ceil(U7) * U7;
        assert!(from_window > boundary);

        // In force at b, but the window that contains b keeps the old allowance.
        advance_to(boundary);
        let contract = Contracts::<Test>::get(GROUP_A).expect("live; qed");
        assert_eq!(contract.allowance, ALLOWANCE);
        assert_eq!(
            contract.next_allowance,
            Some(NextAllowance {
                allowance: ALLOWANCE * 2,
                from_window
            })
        );
        assert_eq!(contract.pending, None);
    });
}

// AC-A5.2
#[test]
fn amendments_keep_periods_and_kinds() {
    new_test_ext().execute_with(|| {
        let custom_offer = custom(GROUP_A, None, None);
        subscribe(GROUP_A, custom_offer);
        let standard_offer = standard();
        subscribe(GROUP_B, standard_offer);
        let trial = publish(
            OfferKind::Trial,
            Terms {
                term: Some(2),
                ..terms()
            },
        );
        subscribe(GROUP_C, trial);

        // A different usage or billing period.
        for terms in [
            Terms {
                usage_period: 2 * U,
                ..terms()
            },
            Terms {
                billing_period: 2 * B,
                ..terms()
            },
        ] {
            assert_noop!(
                UsageSubscription::amend_contract(
                    RuntimeOrigin::signed(AMENDER),
                    GROUP_A,
                    terms.clone()
                ),
                Error::<Test>::InvalidTerms
            );
            assert_noop!(
                UsageSubscription::amend_offer(
                    RuntimeOrigin::signed(AMENDER),
                    standard_offer,
                    terms
                ),
                Error::<Test>::InvalidTerms
            );
        }
        // A term limit or a commitment on a standard offer.
        for terms in [
            Terms {
                term: Some(6),
                ..terms()
            },
            Terms {
                min_commitment: Some(2),
                ..terms()
            },
        ] {
            assert_noop!(
                UsageSubscription::amend_offer(
                    RuntimeOrigin::signed(AMENDER),
                    standard_offer,
                    terms
                ),
                Error::<Test>::InvalidTerms
            );
        }
        // A trial offer, a trial contract, or a standard contract on its own.
        assert_noop!(
            UsageSubscription::amend_offer(RuntimeOrigin::signed(AMENDER), trial, terms()),
            Error::<Test>::InvalidTerms
        );
        assert_noop!(
            UsageSubscription::amend_offer(RuntimeOrigin::signed(AMENDER), custom_offer, terms()),
            Error::<Test>::OfferWithdrawn
        );
        for group in [GROUP_B, GROUP_C] {
            assert_noop!(
                UsageSubscription::amend_contract(RuntimeOrigin::signed(AMENDER), group, terms()),
                Error::<Test>::InvalidTerms
            );
        }
        // A custom contract's own rules.
        assert_noop!(
            UsageSubscription::amend_contract(
                RuntimeOrigin::signed(AMENDER),
                GROUP_A,
                Terms {
                    min_commitment: Some(3),
                    term: Some(2),
                    ..terms()
                }
            ),
            Error::<Test>::InvalidTerms
        );
    });
}

// AC-A5.3
#[test]
fn amendment_cancels_a_pending_switch_and_blocks_new_ones() {
    new_test_ext().execute_with(|| {
        let target = standard();
        subscribe(GROUP_A, custom(GROUP_A, None, None));
        assert_ok!(UsageSubscription::switch_offer(
            group_origin(GROUP_A),
            target
        ));
        clear_events();

        amend_contract(GROUP_A, amended());
        assert_eq!(
            events(),
            vec![
                crate::Event::SwitchDropped {
                    group: GROUP_A,
                    offer: target,
                    reason: ReplacementDropReason::AmendmentEnacted
                },
                crate::Event::ContractAmended {
                    group: GROUP_A,
                    effective_at: B
                },
            ]
        );
        assert!(matches!(
            Contracts::<Test>::get(GROUP_A).and_then(|c| c.pending),
            Some(PendingChange::Amend { .. })
        ));

        assert_noop!(
            UsageSubscription::switch_offer(group_origin(GROUP_A), target),
            Error::<Test>::ChangePending
        );
        // `REQ-CT-8`: no second amendment while one is pending.
        assert_noop!(
            UsageSubscription::amend_contract(RuntimeOrigin::signed(AMENDER), GROUP_A, terms()),
            Error::<Test>::ChangePending
        );
    });
}

// AC-A5.5, REQ-OF-8
#[test]
fn only_the_amend_origin_amends() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        subscribe(GROUP_A, custom(GROUP_A, None, None));
        for origin in [
            RuntimeOrigin::signed(ADMIN),
            RuntimeOrigin::root(),
            group_origin(GROUP_A),
        ] {
            assert_noop!(
                UsageSubscription::amend_contract(origin.clone(), GROUP_A, amended()),
                DispatchError::BadOrigin
            );
            assert_noop!(
                UsageSubscription::amend_offer(origin, offer, amended()),
                DispatchError::BadOrigin
            );
        }
    });
}

// AC-A5.6
#[test]
fn contract_ending_before_its_boundary_never_takes_the_amendment() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, custom(GROUP_A, None, None));
        advance_to(27 * DAYS);
        amend_contract(GROUP_A, amended());

        // The group leaves: it ends at `paid through`, before b.
        Clock::set(28 * DAYS);
        assert_ok!(UsageSubscription::cancel(group_origin(GROUP_A)));
        advance_to(B);
        assert!(Contracts::<Test>::get(GROUP_A).is_none());

        advance_to(3 * B);
        assert_eq!(funds(&PAYEE), PRICE);
    });
}

// AC-A5.7
#[test]
fn offer_amendment_takes_effect_at_each_contracts_own_boundary() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        subscribe(GROUP_A, offer);
        advance_to(10 * DAYS);
        subscribe(GROUP_B, offer);
        advance_to(27 * DAYS);
        clear_events();

        assert_ok!(UsageSubscription::amend_offer(
            RuntimeOrigin::signed(AMENDER),
            offer,
            amended()
        ));
        assert_eq!(
            events(),
            vec![crate::Event::OfferAmended {
                offer,
                enacted_at: 27 * DAYS,
                last_boundary: 27 * DAYS + 2 * B
            }]
        );
        let amendment = OfferAmendment::<Test>::get(offer).expect("recorded; qed");
        assert_eq!(amendment.seq, 1);
        assert_eq!(amendment.terms, amended());
        assert_eq!(
            Offers::<Test>::get(offer).map(|o| o.allowance),
            Some(ALLOWANCE * 2)
        );

        // Each group has the free exit until its own boundary: day 60 for A, day 70 for B.
        for (group, boundary) in [(GROUP_A, 60 * DAYS), (GROUP_B, 70 * DAYS)] {
            assert_eq!(
                <Listings as SubscriptionsInspect<AccountId>>::pending_amendment(
                    &(0, 0),
                    &offer,
                    &group_account(group),
                    Clock::now()
                )
                .map(|p| p.effective_at),
                Some(boundary)
            );
        }

        // A group that subscribes now takes the new terms.
        subscribe(GROUP_C, offer);
        assert_eq!(
            Contracts::<Test>::get(GROUP_C).map(|c| (c.allowance, c.amendment_seq)),
            Some((ALLOWANCE * 2, 1))
        );
        assert_eq!(
            subscription(GROUP_C).map(|s| s.conditions.price.amount),
            Some(2 * PRICE)
        );

        advance_to(60 * DAYS);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| (c.allowance, c.amendment_seq)),
            Some((ALLOWANCE * 2, 1))
        );
        assert_eq!(
            Contracts::<Test>::get(GROUP_B).map(|c| (c.allowance, c.amendment_seq)),
            Some((ALLOWANCE, 0))
        );
        advance_to(70 * DAYS);
        assert_eq!(
            Contracts::<Test>::get(GROUP_B).map(|c| (c.allowance, c.amendment_seq)),
            Some((ALLOWANCE * 2, 1))
        );
        assert_eq!(
            subscription(GROUP_B).map(|s| s.conditions.price.amount),
            Some(2 * PRICE)
        );

        // `DEC-34`, `DEC-3`: a second amendment waits for every contract's boundary, and for the
        // allowance it brought to be in force: from the first usage window at or after it.
        assert_noop!(
            UsageSubscription::amend_offer(RuntimeOrigin::signed(AMENDER), offer, terms()),
            Error::<Test>::ChangePending
        );
        advance_to(27 * DAYS + 2 * B);
        assert_noop!(
            UsageSubscription::amend_offer(RuntimeOrigin::signed(AMENDER), offer, terms()),
            Error::<Test>::ChangePending
        );
        advance_to(27 * DAYS + 2 * B + U);
        assert_ok!(UsageSubscription::amend_offer(
            RuntimeOrigin::signed(AMENDER),
            offer,
            terms()
        ));
    });
}

// AC-A5.8
#[test]
fn withdrawn_offer_keeps_its_pending_amendment() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        subscribe(GROUP_A, offer);
        advance_to(27 * DAYS);
        assert_ok!(UsageSubscription::amend_offer(
            RuntimeOrigin::signed(AMENDER),
            offer,
            amended()
        ));
        assert_ok!(UsageSubscription::withdraw_offer(
            RuntimeOrigin::signed(ADMIN),
            offer
        ));

        advance_to(2 * B);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.allowance),
            Some(ALLOWANCE * 2)
        );
        assert_eq!(
            subscription(GROUP_A).map(|s| s.conditions.price.amount),
            Some(2 * PRICE)
        );

        // A withdrawn offer is not amended.
        assert_noop!(
            UsageSubscription::amend_offer(RuntimeOrigin::signed(AMENDER), offer, terms()),
            Error::<Test>::OfferWithdrawn
        );
        assert_noop!(
            UsageSubscription::amend_offer(RuntimeOrigin::signed(AMENDER), 99, terms()),
            Error::<Test>::UnknownOffer
        );
    });
}

// §5.2: Active → Ended (cancelled while an amendment was pending); AC-B9.1, REQ-CT-15
#[test]
fn cancelling_before_the_boundary_waives_the_commitment() {
    new_test_ext().execute_with(|| {
        // A commitment end E = day 180, after b = day 60.
        subscribe(GROUP_A, custom(GROUP_A, Some(6), None));
        advance_to(27 * DAYS);
        amend_contract(GROUP_A, amended());

        Clock::set(28 * DAYS);
        assert_ok!(UsageSubscription::cancel(group_origin(GROUP_A)));
        clear_events();

        // It ends at `paid through`, never *Defaulted*, nothing charged after it.
        advance_to(B);
        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert_eq!(
            events(),
            vec![crate::Event::ContractEnded {
                group: GROUP_A,
                reason: EndReason::Cancelled
            }]
        );
        assert_eq!(funds(&PAYEE), PRICE);
        // The group may subscribe again.
        subscribe(GROUP_A, standard());
    });
}

// §5.2: Suspended → Suspended (an amendment is enacted), then → Ended (cancelled); AC-B9.2
#[test]
fn cancelling_a_suspended_contract_with_an_amendment_pending_ends_it() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, custom(GROUP_A, Some(6), None));
        set_funds(GROUP_A, PRICE - 1);
        advance_to(B);
        assert!(matches!(
            subscription(GROUP_A).map(|s| s.state),
            Some(SubscriptionState::Suspended { .. })
        ));
        Clock::set(B + DAYS);
        amend_contract(GROUP_A, amended());
        clear_events();

        assert_ok!(UsageSubscription::cancel(group_origin(GROUP_A)));
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

// AC-B9.3
#[test]
fn after_the_boundary_the_amended_commitment_applies() {
    new_test_ext().execute_with(|| {
        subscribe(GROUP_A, custom(GROUP_A, Some(6), None));
        advance_to(27 * DAYS);
        amend_contract(
            GROUP_A,
            Terms {
                min_commitment: Some(4),
                ..terms()
            },
        );
        advance_to(B);
        advance_to(2 * B);

        Clock::set(2 * B + DAYS);
        assert_ok!(UsageSubscription::cancel(group_origin(GROUP_A)));
        // Billing continues to the amended commitment end, 4 periods.
        advance_to(3 * B);
        assert!(Contracts::<Test>::contains_key(GROUP_A));
        advance_to(4 * B);
        assert!(Contracts::<Test>::get(GROUP_A).is_none());
        assert_eq!(funds(&PAYEE), 4 * PRICE);
    });
}

#[test]
fn amend_contract_refusals() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            UsageSubscription::amend_contract(RuntimeOrigin::signed(AMENDER), GROUP_A, terms()),
            Error::<Test>::NoContract
        );
        // A *Defaulted* contract is not amended.
        subscribe(GROUP_A, custom(GROUP_A, Some(3), None));
        set_funds(GROUP_A, PRICE - 1);
        advance_to(B);
        advance_to(B + GRACE);
        assert_noop!(
            UsageSubscription::amend_contract(RuntimeOrigin::signed(AMENDER), GROUP_A, terms()),
            Error::<Test>::NoContract
        );
    });
}

// DEC-34
#[test]
fn offer_amendment_drops_pending_switches_and_blocks_new_ones() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        let target = standard();
        subscribe(GROUP_A, offer);
        subscribe(GROUP_B, offer);
        // A's switch is pending when the amendment is enacted.
        assert_ok!(UsageSubscription::switch_offer(
            group_origin(GROUP_A),
            target
        ));
        advance_to(27 * DAYS);
        assert_ok!(UsageSubscription::amend_offer(
            RuntimeOrigin::signed(AMENDER),
            offer,
            amended()
        ));

        // B may not request one while the amendment is pending for it.
        assert_noop!(
            UsageSubscription::switch_offer(group_origin(GROUP_B), target),
            Error::<Test>::ChangePending
        );

        // A's switch is dropped at its boundary, with the reason, and A renews.
        clear_events();
        advance_to(B);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.offer),
            Some(offer)
        );
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).and_then(|c| c.pending),
            None
        );
        assert!(events().contains(&crate::Event::SwitchDropped {
            group: GROUP_A,
            offer: target,
            reason: ReplacementDropReason::AmendmentEnacted
        }));
    });
}

// DEC-34, INV-10, 0009-A19.5
#[test]
fn second_offer_amendment_waits_for_a_contract_behind_the_first() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        subscribe(GROUP_A, offer);
        advance_to(DAYS);
        assert_ok!(UsageSubscription::amend_offer(
            RuntimeOrigin::signed(AMENDER),
            offer,
            amended()
        ));

        // Every bound of time has passed, but the due queue has not reached GROUP_A's
        // boundary: its contract has not applied the first amendment.
        Clock::set(DAYS + 2 * B + U);
        assert_noop!(
            UsageSubscription::amend_offer(RuntimeOrigin::signed(AMENDER), offer, terms()),
            Error::<Test>::ChangePending
        );

        advance_to(DAYS + 2 * B + U + HOURS);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A).map(|c| c.amendment_seq),
            Some(1)
        );
        assert_ok!(UsageSubscription::amend_offer(
            RuntimeOrigin::signed(AMENDER),
            offer,
            terms()
        ));
    });
}

// DEC-3, 0006-A3, 0009-A20.4
#[test]
fn second_offer_amendment_waits_for_the_first_ones_allowance_in_force() {
    new_test_ext().execute_with(|| {
        // Weekly usage windows: an allowance amended at day 60 is in force from day 63.
        let weekly = |terms: TermsOf<Test>| Terms {
            usage_period: 7 * DAYS,
            ..terms
        };
        let offer = publish(OfferKind::Standard, weekly(terms()));
        subscribe(GROUP_A, offer);
        advance_to(DAYS);
        assert_ok!(UsageSubscription::amend_offer(
            RuntimeOrigin::signed(AMENDER),
            offer,
            weekly(amended())
        ));
        advance_to(2 * B + HOURS);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A)
                .and_then(|c| c.next_allowance)
                .map(|next| next.from_window),
            Some(63 * DAYS)
        );

        // The contract has applied the amendment, but its allowance is not in force yet.
        advance_to(DAYS + 2 * B);
        assert_noop!(
            UsageSubscription::amend_offer(RuntimeOrigin::signed(AMENDER), offer, weekly(terms())),
            Error::<Test>::ChangePending
        );
        advance_to(DAYS + 2 * B + 7 * DAYS);
        assert_ok!(UsageSubscription::amend_offer(
            RuntimeOrigin::signed(AMENDER),
            offer,
            weekly(terms())
        ));
    });
}

// DEC-3, 0006-A3, 0009-A20.4
#[test]
fn contract_amendment_waits_for_the_last_amended_allowance_in_force() {
    new_test_ext().execute_with(|| {
        let weekly = |terms: TermsOf<Test>| Terms {
            usage_period: 7 * DAYS,
            ..terms
        };
        let offer = publish(OfferKind::Custom(GROUP_A), weekly(terms()));
        subscribe(GROUP_A, offer);
        advance_to(DAYS);
        amend_contract(GROUP_A, weekly(amended()));

        // In force at day 60, for the weekly window from day 63.
        advance_to(2 * B + HOURS);
        assert_eq!(
            Contracts::<Test>::get(GROUP_A)
                .and_then(|c| c.next_allowance)
                .map(|next| next.from_window),
            Some(63 * DAYS)
        );
        assert_noop!(
            UsageSubscription::amend_contract(
                RuntimeOrigin::signed(AMENDER),
                GROUP_A,
                weekly(terms())
            ),
            Error::<Test>::ChangePending
        );
        advance_to(63 * DAYS);
        amend_contract(GROUP_A, weekly(terms()));
    });
}
