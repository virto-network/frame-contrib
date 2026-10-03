//! Tests, grouped by subject. The SPEC identifiers and §5.2 rows each test verifies are in a
//! comment above it.

use crate::{mock::*, *};
use fc_traits_listings::item::{
    subscriptions::{Inspect as SubscriptionsInspect, Subscription, SubscriptionState},
    ItemPrice,
};
use frame_support::{
    assert_noop, assert_ok,
    traits::{
        fungibles::Mutate as _,
        tokens::{Fortitude, Precision, Preservation},
    },
};
use sp_runtime::DispatchError;

mod contracts;
mod offers;

/// The allowance of the offers in the tests.
pub const ALLOWANCE: Weight = Weight::from_parts(1_000_000, 100_000);
/// The price of one billing period.
pub const PRICE: Balance = 100;
/// The billing period of the offers in the tests: 30 days.
pub const B: u64 = 30 * DAYS;
/// The grace of the offers in the tests.
pub const GRACE: u64 = 3 * DAYS;
/// The usage period of the offers in the tests.
pub const U: u64 = HOURS;

/// Valid terms for a standard offer.
pub fn terms() -> TermsOf<Test> {
    Terms {
        allowance: ALLOWANCE,
        usage_period: U,
        price: ItemPrice {
            asset: ASSET,
            amount: PRICE,
        },
        billing_period: B,
        term: None,
        min_commitment: None,
        grace: GRACE,
    }
}

/// The origin that publishes offers of `kind`.
pub fn offer_origin(kind: &OfferKindOf<Test>) -> RuntimeOrigin {
    match kind {
        OfferKind::Custom(_) => RuntimeOrigin::root(),
        _ => RuntimeOrigin::signed(ADMIN),
    }
}

/// Publishes an offer and returns its id.
pub fn publish(kind: OfferKindOf<Test>, terms: TermsOf<Test>) -> u32 {
    let offer = NextOfferId::<Test>::get().unwrap_or_default();
    assert_ok!(UsageSubscription::publish_offer(
        offer_origin(&kind),
        kind,
        terms
    ));
    offer
}

/// Publishes a standard offer with [`terms`].
pub fn standard() -> u32 {
    publish(OfferKind::Standard, terms())
}

/// Publishes a custom offer for `group` with a minimum commitment and term limit.
pub fn custom(group: u32, min_commitment: Option<u32>, term: Option<u32>) -> u32 {
    publish(
        OfferKind::Custom(group),
        Terms {
            min_commitment,
            term,
            ..terms()
        },
    )
}

/// Subscribes `group` to `offer`.
pub fn subscribe(group: u32, offer: u32) {
    assert_ok!(UsageSubscription::subscribe(
        group_origin(group),
        offer,
        None
    ));
}

/// The listings subscription of `group`'s contract.
pub fn subscription(group: u32) -> Option<Subscription<ItemPrice<AssetId, Balance>, u64, u32>> {
    let offer = Contracts::<Test>::get(group)?.offer;
    <Listings as SubscriptionsInspect<AccountId>>::subscription(
        &(0, 0),
        &offer,
        &group_account(group),
    )
}

/// What `who` holds of [`ASSET`].
pub fn funds(who: &AccountId) -> Balance {
    Assets::balance(ASSET, who)
}

/// Sets what `group`'s account holds of [`ASSET`].
pub fn set_funds(group: u32, amount: Balance) {
    let account = group_account(group);
    let held = funds(&account);
    assert_ok!(Assets::burn_from(
        ASSET,
        &account,
        held,
        Preservation::Expendable,
        Precision::BestEffort,
        Fortitude::Force,
    ));
    if amount > 0 {
        assert_ok!(Assets::mint_into(ASSET, &account, amount));
    }
}

/// This pallet's events, in order.
pub fn events() -> Vec<crate::Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::UsageSubscription(event) => Some(event),
            _ => None,
        })
        .collect()
}

/// Clears the events recorded so far.
pub fn clear_events() {
    System::reset_events();
}

/// A listings error, by variant.
pub fn listings_error(error: fc_pallet_listings::Error<Test>) -> DispatchError {
    error.into()
}
