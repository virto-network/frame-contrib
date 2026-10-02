//! The view functions (`CTR-QRY-1`, SPEC §8.3, `REQ-OB-1`, 0008).

use super::*;
use fc_traits_listings::item::subscriptions::Cancellation;
use fc_traits_memberships::{RankOnTransfer, Receivers, Transfer, TransferPolicy};
use sp_runtime::StateVersion;

fn storage_root() -> Vec<u8> {
    sp_io::storage::root(StateVersion::V1)
}

// REQ-OB-1
#[test]
fn every_query_reads_one_state_and_writes_nothing() {
    new_test_ext().execute_with(|| {
        let offer = standard();
        let target = standard();
        subscribe(GROUP_A, offer);
        add_member(GROUP_A, 1, &ALICE);
        assert_ok!(UsageSubscription::switch_offer(
            group_origin(GROUP_A),
            target
        ));
        Clock::set(U + 1);
        let before = storage_root();

        let _ = UsageSubscription::offer(offer);
        let _ = UsageSubscription::open_offers(None, 10);
        let _ = UsageSubscription::contract(GROUP_A);
        let _ = UsageSubscription::pool(GROUP_A);
        let _ = UsageSubscription::paying_group(ALICE);
        let _ = UsageSubscription::trial_used(GROUP_A);
        let _ = UsageSubscription::transfer_policy(GROUP_A);
        let _ = UsageSubscription::would_waive(ALICE, Weight::from_parts(10, 10));

        assert_eq!(storage_root(), before);
    });
}

// CTR-QRY-1
#[test]
fn offer_shows_its_terms_and_its_pending_amendment() {
    new_test_ext().execute_with(|| {
        assert_eq!(UsageSubscription::offer(0), None);
        let offer = standard();
        assert_eq!(
            UsageSubscription::offer(offer),
            Some(OfferInfo {
                kind: OfferKind::Standard,
                terms: terms(),
                status: OfferStatus::Open,
                amendment: None,
            })
        );

        // An amendment shows while it may be pending for some contract.
        Clock::set(DAYS);
        let amended = Terms {
            allowance: ALLOWANCE * 2,
            ..terms()
        };
        assert_ok!(UsageSubscription::amend_offer(
            RuntimeOrigin::signed(AMENDER),
            offer,
            amended.clone()
        ));
        let info = UsageSubscription::offer(offer).expect("exists; qed");
        assert_eq!(info.terms, amended);
        assert_eq!(
            info.amendment,
            Some(OfferAmendmentRecord {
                seq: 1,
                terms: amended,
                enacted_at: DAYS
            })
        );
        Clock::set(DAYS + 2 * B);
        assert_eq!(
            UsageSubscription::offer(offer).and_then(|o| o.amendment),
            None
        );
    });
}

// CTR-QRY-1
#[test]
fn open_offers_pages_by_cursor() {
    new_test_ext().execute_with(|| {
        let ids: Vec<u32> = (0..12).map(|_| standard()).collect();
        for withdrawn in [1, 2, 7] {
            assert_ok!(UsageSubscription::withdraw_offer(
                RuntimeOrigin::signed(ADMIN),
                ids[withdrawn]
            ));
        }
        let ids_of = |page: &OffersPageOf<Test>| -> Vec<u32> {
            page.offers.iter().map(|(id, _)| *id).collect()
        };

        // Ascending, withdrawn ones skipped, the cursor resuming exactly after the last id read.
        let page = UsageSubscription::open_offers(None, 4);
        assert_eq!(ids_of(&page), vec![0, 3]);
        assert_eq!(page.next, Some(3));
        let page = UsageSubscription::open_offers(page.next, 4);
        assert_eq!(ids_of(&page), vec![4, 5, 6]);
        assert_eq!(page.next, Some(7));

        // The rest, to the end.
        let page = UsageSubscription::open_offers(page.next, 100);
        assert_eq!(ids_of(&page), vec![8, 9, 10, 11]);
        assert_eq!(page.next, None);

        // A page reads at most its limit, whatever is beyond: four withdrawn offers in a row
        // give an empty page that still moves on.
        for withdrawn in [8, 9, 10] {
            assert_ok!(UsageSubscription::withdraw_offer(
                RuntimeOrigin::signed(ADMIN),
                ids[withdrawn]
            ));
        }
        let page = UsageSubscription::open_offers(Some(6), 3);
        assert_eq!(ids_of(&page), Vec::<u32>::new());
        assert_eq!(page.next, Some(9));
        let page = UsageSubscription::open_offers(page.next, 3);
        assert_eq!(ids_of(&page), vec![11]);
        assert_eq!(page.next, None);

        // Past the end.
        let page = UsageSubscription::open_offers(Some(11), 3);
        assert!(page.offers.is_empty());
        assert_eq!(page.next, None);
    });
}

// CTR-QRY-1, 0008-A1
#[test]
fn open_offers_limit_is_capped_at_max_offers_per_page() {
    new_test_ext().execute_with(|| {
        for _ in 0..12 {
            standard();
        }
        let page = UsageSubscription::open_offers(None, 100);
        assert_eq!(page.offers.len(), MaxOffersPerPage::get() as usize);
        assert_eq!(page.next, Some(4));
    });
}

// CTR-QRY-1, 0008-A1
#[test]
fn open_offers_page_cost_does_not_depend_on_the_number_of_offers() {
    let reads_of_a_page = |offers: u32| {
        new_test_ext().execute_with(|| {
            for _ in 0..offers {
                standard();
            }
            crate::views::OFFER_KEYS_READ.with(|reads| reads.set(0));
            let page = UsageSubscription::open_offers(Some(2), 4);
            assert_eq!(page.offers.len(), 4);
            assert_eq!(page.next, Some(6));
            crate::views::OFFER_KEYS_READ.with(|reads| reads.get())
        })
    };
    // Four ids, and the one after the last.
    assert_eq!(reads_of_a_page(10), 5);
    assert_eq!(reads_of_a_page(1_000), 5);
}

// CTR-QRY-1
#[test]
fn open_offers_on_empty_storage_is_an_empty_last_page() {
    new_test_ext().execute_with(|| {
        let page = UsageSubscription::open_offers(None, 10);
        assert!(page.offers.is_empty());
        assert_eq!(page.next, None);
    });
}

// CTR-QRY-1
#[test]
fn open_offers_page_ending_exactly_at_the_last_offer_is_the_last() {
    new_test_ext().execute_with(|| {
        for _ in 0..4 {
            standard();
        }
        let page = UsageSubscription::open_offers(None, 4);
        assert_eq!(page.offers.len(), 4);
        assert_eq!(page.next, None);
        let page = UsageSubscription::open_offers(Some(1), 2);
        assert_eq!(
            page.offers.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert_eq!(page.next, None);
    });
}

// CTR-QRY-1
#[test]
fn open_offers_with_a_zero_limit_keeps_the_cursor() {
    new_test_ext().execute_with(|| {
        for _ in 0..3 {
            standard();
        }
        let page = UsageSubscription::open_offers(Some(0), 0);
        assert!(page.offers.is_empty());
        assert_eq!(page.next, Some(0));
        // Nothing examined from the start: the next page starts from the first offer too.
        assert_eq!(UsageSubscription::open_offers(None, 0).next, None);
        assert_eq!(UsageSubscription::open_offers(None, 1).next, Some(0));
    });
}

// CTR-QRY-1
#[test]
fn open_offers_skips_an_offer_whose_conditions_are_missing() {
    new_test_ext().execute_with(|| {
        for _ in 0..4 {
            standard();
        }
        // Listings never removes conditions today; an offer without them is skipped, not the end.
        let inventory = OfferInventory::get();
        fc_pallet_listings::ItemConditions::<Test>::remove((
            fc_pallet_listings::InventoryId(inventory.0, inventory.1),
            1u32,
        ));
        let page = UsageSubscription::open_offers(None, 3);
        assert_eq!(
            page.offers.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            vec![0, 2]
        );
        assert_eq!(page.next, Some(2));
    });
}

// CTR-QRY-1, 0008-A1
#[test]
#[should_panic(expected = "`MaxOffersPerPage` must be non-zero")]
fn integrity_test_refuses_a_zero_max_offers_per_page() {
    use frame_support::traits::Hooks;
    MaxOffersPerPage::set(0);
    new_test_ext().execute_with(|| {
        <UsageSubscription as Hooks<u64>>::integrity_test();
    });
}

// AC-B5.1, CTR-QRY-1
#[test]
fn contract_shows_its_terms_dates_and_pending_changes() {
    new_test_ext().execute_with(|| {
        assert_eq!(UsageSubscription::contract(GROUP_A), None);
        let offer = custom(GROUP_A, Some(6), None);
        subscribe(GROUP_A, offer);
        Clock::set(DAYS);

        let info = UsageSubscription::contract(GROUP_A).expect("live; qed");
        assert_eq!(info.offer, offer);
        assert_eq!(info.kind, OfferKind::Custom(GROUP_A));
        assert_eq!(info.state, SubscriptionState::Active);
        assert_eq!(
            info.terms,
            Terms {
                min_commitment: Some(6),
                ..terms()
            }
        );
        assert_eq!(info.anchor, 0);
        assert_eq!(info.paid_through, B);
        assert_eq!(info.next_due, Some(B));
        assert_eq!(info.grace_end, Some(B + GRACE));
        assert_eq!(info.commitment_end, Some(6 * B));
        assert_eq!(info.periods_charged, 1);
        assert_eq!(info.cancel_requested, None);
        assert_eq!(info.pending_switch, None);
        assert_eq!(info.pending_amendment, None);

        // A pending amendment, with its boundary and the free exit open.
        assert_ok!(UsageSubscription::amend_contract(
            RuntimeOrigin::signed(AMENDER),
            GROUP_A,
            Terms {
                allowance: ALLOWANCE * 2,
                min_commitment: Some(6),
                ..terms()
            }
        ));
        let pending = UsageSubscription::contract(GROUP_A)
            .and_then(|info| info.pending_amendment)
            .expect("pending; qed");
        assert_eq!(pending.effective_at, 2 * B);
        assert_eq!(pending.terms.allowance, ALLOWANCE * 2);
        assert!(pending.free_exit);

        // A pending cancellation.
        assert_ok!(UsageSubscription::cancel(group_origin(GROUP_A)));
        assert_eq!(
            UsageSubscription::contract(GROUP_A).and_then(|info| info.cancel_requested),
            Some(Cancellation::FreeExit)
        );
    });
}

// CTR-QRY-1
#[test]
fn contract_shows_a_pending_switch() {
    new_test_ext().execute_with(|| {
        let target = standard();
        subscribe(GROUP_A, standard());
        assert_ok!(UsageSubscription::switch_offer(
            group_origin(GROUP_A),
            target
        ));
        let info = UsageSubscription::contract(GROUP_A).expect("live; qed");
        assert_eq!(info.pending_switch, Some(PendingChange::Switch(target)));
        assert_eq!(info.commitment_end, None);
    });
}

// CTR-QRY-1
#[test]
fn pool_shows_the_current_window_and_whether_it_is_usable() {
    new_test_ext().execute_with(|| {
        assert_eq!(UsageSubscription::pool(GROUP_A), None);
        subscribe(GROUP_A, standard());
        add_member(GROUP_A, 1, &ALICE);
        Clock::set(2 * U + 5);
        let ticket =
            UsageSubscription::check(&ALICE, Weight::from_parts(1_000, 10)).expect("pool; qed");
        UsageSubscription::charge(&ticket, Weight::from_parts(1_000, 10));

        assert_eq!(
            UsageSubscription::pool(GROUP_A),
            Some(PoolInfo {
                allowance: ALLOWANCE,
                window_start: 2 * U,
                window_end: 3 * U,
                used: Weight::from_parts(1_000, 10),
                remaining: ALLOWANCE.saturating_sub(Weight::from_parts(1_000, 10)),
                usable: Ok(()),
            })
        );

        // A new window has no usage; an unpaid contract is not usable, and says why.
        Clock::set(B);
        let pool = UsageSubscription::pool(GROUP_A).expect("live; qed");
        assert_eq!(pool.used, Weight::zero());
        assert_eq!(pool.usable, Err(FeePathReason::NotPaid));
        UsableGroups::set_unusable(GROUP_A);
        assert_eq!(
            UsageSubscription::pool(GROUP_A).map(|pool| pool.usable),
            Some(Err(FeePathReason::GroupUnusable))
        );
    });
}

// CTR-QRY-1
#[test]
fn paying_group_and_trial_used() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            UsageSubscription::paying_group(ALICE),
            PayingGroupResolution::NoMembership
        );
        add_member(GROUP_A, 1, &ALICE);
        assert_eq!(
            UsageSubscription::paying_group(ALICE),
            PayingGroupResolution::Only(GROUP_A)
        );
        add_member(GROUP_B, 2, &ALICE);
        assert_eq!(
            UsageSubscription::paying_group(ALICE),
            PayingGroupResolution::SeveralGroups
        );
        assert_ok!(UsageSubscription::set_paying_group(
            RuntimeOrigin::signed(ALICE),
            Some(GROUP_B)
        ));
        assert_eq!(
            UsageSubscription::paying_group(ALICE),
            PayingGroupResolution::Named(GROUP_B)
        );

        assert!(!UsageSubscription::trial_used(GROUP_A));
        subscribe(
            GROUP_A,
            publish(
                OfferKind::Trial,
                Terms {
                    term: Some(1),
                    ..terms()
                },
            ),
        );
        assert!(UsageSubscription::trial_used(GROUP_A));
    });
}

// CTR-QRY-1
#[test]
fn transfer_policy_reads_the_memberships_manager() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            UsageSubscription::transfer_policy(GROUP_A),
            TransferPolicy::default()
        );
        for receivers in [
            Receivers::Disabled,
            Receivers::ToExistingMembers,
            Receivers::ToAnyAccount,
        ] {
            for rank in [RankOnTransfer::Reset, RankOnTransfer::Keep] {
                let policy = TransferPolicy { receivers, rank };
                assert_ok!(MembershipsManager::set_transfer_policy(&GROUP_A, policy));
                assert_eq!(UsageSubscription::transfer_policy(GROUP_A), policy);
                assert_eq!(
                    UsageSubscription::transfer_policy(GROUP_A),
                    MembershipsManager::transfer_policy(&GROUP_A)
                );
            }
        }
        // Each group has its own.
        assert_eq!(
            UsageSubscription::transfer_policy(GROUP_B),
            TransferPolicy::default()
        );
    });
}

// CTR-QRY-1
#[test]
fn would_waive_gives_the_decision_with_its_reason() {
    new_test_ext().execute_with(|| {
        let estimate = Weight::from_parts(1_000, 10);
        assert_eq!(
            UsageSubscription::would_waive(ALICE, estimate),
            Waiver::Fee(FeePathReason::NoPayingGroup)
        );
        add_member(GROUP_A, 1, &ALICE);
        assert_eq!(
            UsageSubscription::would_waive(ALICE, estimate),
            Waiver::Fee(FeePathReason::NoContract)
        );
        subscribe(GROUP_A, standard());
        assert_eq!(
            UsageSubscription::would_waive(ALICE, estimate),
            Waiver::Pool {
                group: GROUP_A,
                remaining: ALLOWANCE.saturating_sub(estimate)
            }
        );
        assert_eq!(
            UsageSubscription::would_waive(ALICE, ALLOWANCE.saturating_add(estimate)),
            Waiver::Fee(FeePathReason::AllowanceExceeded)
        );
        Clock::set(B);
        assert_eq!(
            UsageSubscription::would_waive(ALICE, estimate),
            Waiver::Fee(FeePathReason::NotPaid)
        );
        UsableGroups::set_unusable(GROUP_A);
        assert_eq!(
            UsageSubscription::would_waive(ALICE, estimate),
            Waiver::Fee(FeePathReason::GroupUnusable)
        );
    });
}
