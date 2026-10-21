//! Epic A: offers (`US-A1`–`US-A3`), and how they are kept in listings (`DEC-4`, `DEC-8`).

use super::*;
use fc_traits_listings::{
    item::subscriptions::{Eligibility, SubscriptionConditions},
    InspectInventory, InspectItem,
};

// AC-A1.1, REQ-OF-1
#[test]
fn standard_offer_is_open_for_every_group() {
    new_test_ext().execute_with(|| {
        let offer = standard();

        assert_eq!(
            Offers::<Test>::get(offer),
            Some(Offer {
                kind: OfferKind::Standard,
                allowance: ALLOWANCE,
                usage_period: U,
                status: OfferStatus::Open,
            })
        );
        assert_eq!(
            events(),
            vec![crate::Event::OfferPublished {
                offer,
                kind: OfferKind::Standard
            }]
        );

        // Any usable group may accept it.
        subscribe(GROUP_A, offer);
        subscribe(GROUP_B, offer);
    });
}

// DEC-4, DEC-8
#[test]
fn offer_is_an_item_of_the_collective_inventory_owned_by_the_payee() {
    new_test_ext().execute_with(|| {
        assert!(!<Listings as InspectInventory>::exists(&(0, 0)));
        let offer = standard();

        // The inventory was created on the first offer, owned by the payee.
        assert!(<Listings as InspectInventory>::exists(&(0, 0)));
        assert_eq!(
            <Listings as InspectItem<AccountId>>::creator(&(0, 0), &offer),
            Some(PAYEE)
        );
        // The money terms are the item's subscription conditions.
        assert_eq!(
            <Listings as SubscriptionsInspect<AccountId>>::subscription_conditions(&(0, 0), &offer),
            Some(SubscriptionConditions {
                price: ItemPrice {
                    asset: ASSET,
                    amount: PRICE
                },
                period: B,
                term: None,
                min_commitment: None,
                grace: GRACE,
            })
        );
        assert_eq!(
            <Listings as SubscriptionsInspect<AccountId>>::eligibility(&(0, 0), &offer),
            Some(Eligibility::Anyone)
        );

        // Later offers take the next ids.
        assert_eq!(standard(), offer + 1);
    });
}

// AC-A1.2, REQ-OF-2
#[test]
fn invalid_terms_are_refused() {
    new_test_ext().execute_with(|| {
        let invalid = [
            // A zero allowance component.
            Terms {
                allowance: Weight::from_parts(0, 100_000),
                ..terms()
            },
            Terms {
                allowance: Weight::from_parts(1_000_000, 0),
                ..terms()
            },
            // A grace of at least the billing period.
            Terms {
                grace: B,
                ..terms()
            },
            // Periods below the minimums.
            Terms {
                usage_period: MinUsagePeriod::get() - 1,
                ..terms()
            },
            Terms {
                billing_period: MinBillingPeriod::get() - 1,
                grace: 0,
                ..terms()
            },
            // A term limit or a commitment on a standard offer.
            Terms {
                term: Some(12),
                ..terms()
            },
            Terms {
                min_commitment: Some(1),
                ..terms()
            },
            // A zero price on a standard offer.
            Terms {
                price: ItemPrice {
                    asset: ASSET,
                    amount: 0,
                },
                ..terms()
            },
        ];
        for terms in invalid {
            assert_noop!(
                UsageSubscription::publish_offer(
                    RuntimeOrigin::signed(ADMIN),
                    OfferKind::Standard,
                    terms
                ),
                Error::<Test>::InvalidTerms
            );
        }

        // Listings' own rules come back as invalid terms too: an asset that does not exist.
        assert_noop!(
            UsageSubscription::publish_offer(
                RuntimeOrigin::signed(ADMIN),
                OfferKind::Standard,
                Terms {
                    price: ItemPrice {
                        asset: ASSET + 1,
                        amount: PRICE
                    },
                    ..terms()
                }
            ),
            Error::<Test>::InvalidTerms
        );
    });
}

// REQ-OF-2
#[test]
fn custom_offer_rules() {
    new_test_ext().execute_with(|| {
        // A commitment longer than the term limit.
        assert_noop!(
            UsageSubscription::publish_offer(
                RuntimeOrigin::root(),
                OfferKind::Custom(GROUP_A),
                Terms {
                    min_commitment: Some(3),
                    term: Some(2),
                    ..terms()
                }
            ),
            Error::<Test>::InvalidTerms
        );
        // A zero term limit or commitment.
        for (min_commitment, term) in [(Some(0), None), (None, Some(0))] {
            assert_noop!(
                UsageSubscription::publish_offer(
                    RuntimeOrigin::root(),
                    OfferKind::Custom(GROUP_A),
                    Terms {
                        min_commitment,
                        term,
                        ..terms()
                    }
                ),
                Error::<Test>::InvalidTerms
            );
        }
        // With or without a commitment and a term limit.
        custom(GROUP_A, Some(3), Some(12));
        custom(GROUP_A, None, None);
        custom(GROUP_A, Some(3), None);
    });
}

// AC-A1.3, REQ-OF-1
#[test]
fn other_origins_cannot_publish() {
    new_test_ext().execute_with(|| {
        for origin in [RuntimeOrigin::signed(ALICE), group_origin(GROUP_A)] {
            assert_noop!(
                UsageSubscription::publish_offer(origin, OfferKind::Standard, terms()),
                DispatchError::BadOrigin
            );
        }
        // The administrative origin does not publish custom offers, which take a referendum.
        assert_noop!(
            UsageSubscription::publish_offer(
                RuntimeOrigin::signed(ADMIN),
                OfferKind::Custom(GROUP_A),
                terms()
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            UsageSubscription::publish_offer(RuntimeOrigin::root(), OfferKind::Standard, terms()),
            DispatchError::BadOrigin
        );
    });
}

// AC-A2.1, REQ-OF-4
#[test]
fn custom_offer_is_acceptable_only_by_its_group() {
    new_test_ext().execute_with(|| {
        let offer = custom(GROUP_A, Some(3), Some(12));

        // Made exclusive to the group's account in listings too.
        assert_eq!(
            <Listings as SubscriptionsInspect<AccountId>>::eligibility(&(0, 0), &offer),
            Some(Eligibility::Once(group_account(GROUP_A)))
        );

        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_B), offer, None),
            Error::<Test>::NotEligible
        );
        subscribe(GROUP_A, offer);
    });
}

// AC-A2.2, REQ-OF-4
#[test]
fn accepted_custom_offer_reads_as_withdrawn() {
    new_test_ext().execute_with(|| {
        let offer = custom(GROUP_A, None, None);
        subscribe(GROUP_A, offer);

        assert_eq!(
            Offers::<Test>::get(offer).map(|o| o.status),
            Some(OfferStatus::Withdrawn)
        );
        assert!(events().contains(&crate::Event::OfferWithdrawn { offer }));

        // Accepting it again is refused, even after the contract ends.
        assert_ok!(UsageSubscription::terminate_contract(
            RuntimeOrigin::root(),
            GROUP_A
        ));
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_A), offer, None),
            Error::<Test>::OfferWithdrawn
        );
    });
}

// AC-A3.1, REQ-OF-3
#[test]
fn withdrawing_leaves_contracts_as_they_are() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        subscribe(GROUP_A, offer);
        let contract = Contracts::<Test>::get(GROUP_A);
        let conditions = subscription(GROUP_A).map(|s| s.conditions);

        assert_ok!(UsageSubscription::withdraw_offer(
            RuntimeOrigin::signed(ADMIN),
            offer
        ));
        assert!(events().contains(&crate::Event::OfferWithdrawn { offer }));

        // New subscriptions are refused.
        assert_noop!(
            UsageSubscription::subscribe(group_origin(GROUP_B), offer, None),
            Error::<Test>::OfferWithdrawn
        );

        // The existing contract keeps its terms, and renews as before (`INV-9`).
        advance_to(B - RenewalLead::get());
        let renewed = subscription(GROUP_A).expect("still live; qed");
        assert_eq!(renewed.paid_through, 2 * B);
        assert_eq!(Some(renewed.conditions), conditions);
        assert_eq!(Contracts::<Test>::get(GROUP_A), contract);
    });
}

#[test]
fn withdraw_offer_refusals() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            UsageSubscription::withdraw_offer(RuntimeOrigin::signed(ADMIN), 7),
            Error::<Test>::UnknownOffer
        );

        let offer = standard();
        assert_noop!(
            UsageSubscription::withdraw_offer(RuntimeOrigin::signed(ALICE), offer),
            DispatchError::BadOrigin
        );
        assert_ok!(UsageSubscription::withdraw_offer(
            RuntimeOrigin::signed(ADMIN),
            offer
        ));
        assert_noop!(
            UsageSubscription::withdraw_offer(RuntimeOrigin::signed(ADMIN), offer),
            Error::<Test>::OfferWithdrawn
        );

        // A custom offer is withdrawn by the origin that publishes it.
        let offer = custom(GROUP_A, None, None);
        assert_noop!(
            UsageSubscription::withdraw_offer(RuntimeOrigin::signed(ADMIN), offer),
            DispatchError::BadOrigin
        );
        assert_ok!(UsageSubscription::withdraw_offer(
            RuntimeOrigin::root(),
            offer
        ));
    });
}

// DEC-8
#[test]
fn direct_subscription_to_the_collective_inventory_is_closed() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        assert_noop!(
            Listings::subscribe(
                RuntimeOrigin::signed(group_account(GROUP_A)),
                fc_pallet_listings::InventoryId(0, 0),
                offer
            ),
            DispatchError::BadOrigin
        );
    });
}

// 0009-A17, REQ-CT-6, REQ-CT-8
#[test]
fn every_offer_lets_the_collective_amend_and_terminate() {
    new_test_ext().execute_with(|| {
        for offer in [standard(), custom(GROUP_A, Some(2), None)] {
            assert_eq!(
                <Listings as SubscriptionsInspect<AccountId>>::policy(&(0, 0), &offer),
                Some(offer_policy())
            );
        }
    });
}

// CTR-SUB-1, REQ-OF-3
#[test]
fn withdrawing_an_offer_withdraws_its_listings_conditions() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        assert_ok!(UsageSubscription::withdraw_offer(
            RuntimeOrigin::signed(ADMIN),
            offer
        ));
        assert!(
            <Listings as SubscriptionsInspect<AccountId>>::conditions_withdrawn(&(0, 0), &offer)
        );
        // Listings refuses a subscription to it from any other path too.
        assert_noop!(
            <Listings as fc_traits_listings::item::subscriptions::Mutate<AccountId>>::subscribe(
                &(0, 0),
                &offer,
                &group_account(GROUP_B),
            ),
            listings_error(fc_pallet_listings::Error::<Test>::NotSubscribable)
        );
    });
}

// CTR-CALL-2
#[test]
fn listings_refusals_map_to_this_pallets_errors() {
    new_test_ext().execute_with(|| {
        use fc_pallet_listings::Error as L;
        for (refusal, ours) in [
            (L::<Test>::InvalidConditions, Error::<Test>::InvalidTerms),
            (L::NotEligible, Error::NotEligible),
            (L::NotSubscribable, Error::OfferWithdrawn),
            (L::AlreadySubscribed, Error::AlreadyContracted),
            (L::NoSubscription, Error::NoContract),
            (L::CancelPending, Error::NoContract),
            (L::ChargeFailed, Error::ChargeFailed),
            (L::ReplacementPending, Error::SwitchPending),
            (L::ChangePending, Error::ChangePending),
            (L::NoPendingReplacement, Error::NoPendingSwitch),
        ] {
            assert_eq!(
                UsageSubscription::listings_error(listings_error(refusal), Error::UnknownOffer),
                ours.into()
            );
        }
        // Anything else is the caller's fallback.
        assert_eq!(
            UsageSubscription::listings_error(listings_error(L::NothingDue), Error::UnknownOffer),
            Error::<Test>::UnknownOffer.into()
        );
        assert_eq!(
            UsageSubscription::listings_error(DispatchError::BadOrigin, Error::NoContract),
            Error::<Test>::NoContract.into()
        );
    });
}
