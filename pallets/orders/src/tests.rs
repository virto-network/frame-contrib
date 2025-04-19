//! Tests for orders pallet.

use super::{Error as ErrorT, Event as EventT, Order, Pallet};
use crate::mock::*;
use crate::NextOrderId;
use frame_support::{assert_noop, assert_ok, traits::schedule::v3::Named};

type Orders = Pallet<Test>;
type Error = ErrorT<Test>;
type Event = EventT<Test>;

mod create_cart {
    use super::*;

    /// Items added in a cart must exist in `Listings`. Otherwise, it should return
    /// [`Error::ItemNotFound`].
    #[test]
    fn fails_if_the_item_does_not_exist() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Orders::create_cart(
                    RuntimeOrigin::signed(BOB),
                    Some(vec![(((AliceStore::get(), 2), 1), None)])
                ),
                Error::ItemNotFound
            );

            assert_noop!(
                Orders::create_cart(
                    RuntimeOrigin::signed(BOB),
                    Some(vec![(((AliceStore::get(), 1), 4), None)])
                ),
                Error::ItemNotFound
            );
        })
    }

    /// BOB (`[2u8; 32]`) is allowed a maximum of 2 carts, per origin rules. They can create up to 2
    /// carts. Otherwise, it should return [`Error::MaxCartsExceeded`].
    #[test]
    fn fails_if_the_account_exceeds_the_number_of_allowed_carts() {
        new_test_ext().execute_with(|| {
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(BOB),
                Some(vec![(((AliceStore::get(), 1), 1), None),]),
            ));
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(BOB),
                Some(vec![(((AliceStore::get(), 1), 2), None)]),
            ));
            assert_noop!(
                Orders::create_cart(
                    RuntimeOrigin::signed(BOB),
                    Some(vec![(((AliceStore::get(), 1), 3), None)])
                ),
                Error::MaxCartsExceeded
            );
        })
    }

    /// Although EVE (`[5u8; 32]`) is allowed to have 5 carts, the maximum number of carts in the
    /// system is 4. They can create up to 4 carts. Otherwise, it should return
    /// [`Error::MaxCartsExceeded`].
    #[test]
    fn fails_if_account_exceeds_the_max_number_of_carts() {
        new_test_ext().execute_with(|| {
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(EVE),
                Some(vec![(((AliceStore::get(), 1), 1), None),]),
            ));
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(EVE),
                Some(vec![(((AliceStore::get(), 1), 2), None)]),
            ));
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(EVE),
                Some(vec![(((AliceStore::get(), 1), 3), None),]),
            ));
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(EVE),
                Some(vec![(((BobStore::get(), 1), 1), None),]),
            ));
            assert_noop!(
                Orders::create_cart(
                    RuntimeOrigin::signed(EVE),
                    Some(vec![(((BobStore::get(), 1), 2), None)])
                ),
                Error::MaxCartsExceeded
            );
        })
    }

    /// BOB (`[2u8; 32]`) is allowed a maximum of 2 items per cart, per origin rules. They're
    /// allowed to set carts with up to 2 items. Otherwise, it should return
    /// [`Error::MaxItemsExceeded`].
    #[test]
    fn fails_if_the_cart_exceeds_the_number_of_allowed_items() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Orders::create_cart(
                    RuntimeOrigin::signed(BOB),
                    Some(vec![
                        (((AliceStore::get(), 1), 1), None),
                        (((AliceStore::get(), 1), 2), None),
                        (((AliceStore::get(), 1), 3), None)
                    ])
                ),
                Error::MaxItemsExceeded
            );
        })
    }

    /// Creating a cart works correctly.
    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(BOB),
                Some(vec![(((AliceStore::get(), 1), 1), None)])
            ));

            System::assert_has_event(
                Event::CartCreated {
                    owner: BOB,
                    order_id: 0,
                }
                .into(),
            )
        })
    }
}

mod set_cart_items {
    use super::*;

    fn new_test_ext<T>(who: AccountId, f: impl FnOnce(u32) -> T) -> T {
        let mut t = super::new_test_ext();
        t.execute_with(|| {
            let order_id = NextOrderId::<Test>::get();
            assert_ok!(Orders::create_cart(RuntimeOrigin::signed(who).into(), None));
            f(order_id)
        })
    }

    /// Items must be added to a cart that exists. Otherwise, it should return
    /// [`Error::OrderNotFound`].
    #[test]
    fn fails_if_the_order_does_not_exist() {
        super::new_test_ext().execute_with(|| {
            assert_noop!(
                Orders::set_cart_items(
                    RuntimeOrigin::signed(BOB),
                    1,
                    vec![(((AliceStore::get(), 2), 1), None)]
                ),
                Error::OrderNotFound
            );
        })
    }

    /// The caller of this method must be the owner of the cart. Otherwise, it should return
    /// [`Error::NoPermission`].
    #[test]
    fn fails_if_no_permission() {
        new_test_ext(ALICE, |order_id| {
            assert_noop!(
                Orders::set_cart_items(
                    RuntimeOrigin::signed(BOB),
                    order_id,
                    vec![(((AliceStore::get(), 2), 1), None)]
                ),
                Error::NoPermission
            );
        })
    }

    /// Items added in a cart must exist in `Listings`. Otherwise, it should return
    /// [`Error::ItemNotFound`].
    #[test]
    fn fails_if_the_item_does_not_exist() {
        new_test_ext(BOB, |order_id| {
            assert_noop!(
                Orders::set_cart_items(
                    RuntimeOrigin::signed(BOB),
                    order_id,
                    vec![(((AliceStore::get(), 2), 1), None)]
                ),
                Error::ItemNotFound
            );
        })
    }

    /// BOB (`[2u8; 32]`) is allowed a maximum of 2 items per cart, per origin rules. They're
    /// allowed to set carts with up to 2 items. Otherwise, it should return
    /// [`Error::MaxItemsExceeded`].
    #[test]
    fn fails_if_the_cart_exceeds_the_number_of_allowed_items() {
        new_test_ext(ALICE, |order_id| {
            assert_noop!(
                Orders::set_cart_items(
                    RuntimeOrigin::signed(ALICE),
                    order_id,
                    vec![
                        (((BobStore::get(), 1), 1), None),
                        (((BobStore::get(), 1), 2), None)
                    ]
                ),
                Error::MaxItemsExceeded
            );
        })
    }

    /// Although EVE (`[5u8; 32]`) is allowed to have 5 carts, the maximum number of carts in the
    /// system is 4. They can create up to 4 carts. Otherwise, it should return
    /// [`Error::MaxCartsExceeded`].
    #[test]
    fn fails_if_account_exceeds_the_max_number_of_items() {
        new_test_ext(EVE, |order_id| {
            assert_noop!(
                Orders::set_cart_items(
                    RuntimeOrigin::signed(EVE),
                    order_id,
                    vec![
                        (((AliceStore::get(), 1), 1), None),
                        (((AliceStore::get(), 1), 2), None),
                        (((AliceStore::get(), 1), 3), None),
                        (((BobStore::get(), 1), 1), None),
                        (((BobStore::get(), 1), 2), None)
                    ]
                ),
                Error::MaxItemsExceeded
            );
        })
    }

    /// Correctly sets the items of a cart.
    #[test]
    fn it_works() {
        new_test_ext(ALICE, |order_id| {
            assert_ok!(Orders::set_cart_items(
                RuntimeOrigin::signed(ALICE),
                order_id,
                vec![(((AliceStore::get(), 1), 1), None)]
            ));
        })
    }
}

mod checkout {
    use super::*;
    use crate::types::OrderStatus;
    use frame_contrib_traits::listings::MutateItem;

    pub fn new_test_ext<T>(who: AccountId, f: impl FnOnce(u32) -> T) -> T {
        super::new_test_ext().execute_with(|| {
            let order_id = NextOrderId::<Test>::get();
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(who).into(),
                Some(vec![(((AliceStore::get(), 1), 1), None)])
            ));
            f(order_id)
        })
    }

    /// Only an order that exists can be checked-out. Otherwise, it should return
    /// [`Error::OrderNotFound`].
    #[test]
    fn fails_if_the_order_does_not_exist() {
        new_test_ext(ALICE, |_| {
            assert_noop!(
                Orders::checkout(RuntimeOrigin::signed(ALICE), 1),
                Error::OrderNotFound
            );
        })
    }

    /// The caller of this method must be the owner of the cart. Otherwise, it should return
    /// [`Error::NoPermission`].
    #[test]
    fn fails_if_no_permission() {
        new_test_ext(ALICE, |order_id| {
            assert_noop!(
                Orders::checkout(RuntimeOrigin::signed(BOB), order_id),
                Error::NoPermission
            );
        })
    }

    /// A checkout will be correctly executed if all the items in the list are for sale, meaning
    /// each one of them have a price set. Otherwise, it should return [`Error::ItemNotForSale`].
    #[test]
    fn fails_if_the_item_is_not_for_sale() {
        super::new_test_ext().execute_with(|| {
            assert_ok!(Listings::publish(
                &(AliceStore::get(), 1),
                &4,
                b"Alice Flowers - White Tulips".to_vec(),
                None
            ));

            let order_id = NextOrderId::<Test>::get();
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(BOB),
                Some(vec![
                    (((AliceStore::get(), 1), 3), None),
                    (((AliceStore::get(), 1), 4), None)
                ])
            ));
            assert_noop!(
                Orders::checkout(RuntimeOrigin::signed(BOB), order_id),
                Error::ItemNotForSale
            );
        })
    }

    /// Asserts that the method works correctly
    #[test]
    fn it_works() {
        new_test_ext(ALICE, |order_id| {
            assert_ok!(Orders::checkout(RuntimeOrigin::signed(ALICE), order_id));
            System::assert_has_event(Event::OrderCheckout { order_id }.into());

            let (id, _) = Orders::schedule_cancel_params(&order_id);

            // Run until cancellation block
            let block_number = Scheduler::next_dispatch_time(id).unwrap() + 1;
            run_to_block(block_number);

            let (_, details) = Order::<Test>::get(order_id).expect("order exists");
            assert_eq!(details.status, OrderStatus::Cancelled);
        })
    }

    /// A checkout will be correctly executed if the order is a cart. Otherwise, it should return
    /// [`Error::InvalidState`].
    #[test]
    fn fails_if_invalid_state() {
        new_test_ext(ALICE, |order_id| {
            assert_ok!(Orders::checkout(RuntimeOrigin::signed(ALICE), order_id));
            assert_noop!(
                Orders::checkout(RuntimeOrigin::signed(ALICE), order_id),
                Error::InvalidState
            );
        })
    }

    /// A checkout will be correctly executed if none of the items within its list are already
    /// locked. Otherwise, it should return [`Error::ItemAlreadyLocked`].
    #[test]
    fn fails_if_the_item_is_already_locked() {
        new_test_ext(ALICE, |order_id| {
            assert_ok!(Orders::checkout(RuntimeOrigin::signed(ALICE), order_id));

            let order_id = NextOrderId::<Test>::get();
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(BOB),
                Some(vec![(((AliceStore::get(), 1), 1), None)])
            ));
            assert_noop!(
                Orders::checkout(RuntimeOrigin::signed(BOB), order_id),
                Error::ItemAlreadyLocked
            );
        })
    }
}

mod post_checkout {
    use super::*;

    pub fn new_test_ext<T>(who: AccountId, f: impl FnOnce(u32) -> T) -> T {
        checkout::new_test_ext(who.clone(), |order_id| {
            assert_ok!(Orders::checkout(
                RuntimeOrigin::signed(who),
                order_id.clone()
            ));
            f(order_id)
        })
    }
}

mod cancel {
    use super::*;
    use post_checkout::new_test_ext;
    use sp_runtime::DispatchError;

    /// Only an order that exists can be cancelled. Otherwise, it should return [`Error::OrderNotFound`].
    #[test]
    fn fails_if_the_order_does_not_exist() {
        new_test_ext(ALICE, |_| {
            assert_noop!(
                Orders::cancel(RuntimeOrigin::signed(ALICE), 1,),
                Error::OrderNotFound
            );
        })
    }

    /// The caller of this method must be the owner of the owner. Otherwise, it should return
    /// [`Error::NoPermission`].
    #[test]
    fn fails_if_no_permission() {
        new_test_ext(ALICE, |order_id| {
            assert_noop!(
                Orders::cancel(RuntimeOrigin::signed(BOB), order_id),
                Error::NoPermission
            );
        })
    }

    /// Asserts that cancelling an order works.
    #[test]
    fn it_works() {
        checkout::new_test_ext(ALICE, |order_id| {
            assert_ok!(Orders::cancel(RuntimeOrigin::signed(ALICE), order_id));
            System::assert_has_event(Event::OrderCancelled { order_id }.into())
        });

        new_test_ext(ALICE, |order_id| {
            assert_ok!(Orders::cancel(RuntimeOrigin::signed(ALICE), order_id));
            let (id, _) = Orders::schedule_cancel_params(&order_id);
            assert_noop!(
                Scheduler::next_dispatch_time(id),
                DispatchError::Unavailable
            );
        });

        // Root can do it as well
        new_test_ext(ALICE, |order_id| {
            assert_ok!(Orders::cancel(RuntimeOrigin::root(), order_id));
        })
    }

    /// The order to be cancelled must be in either `Cart` or `Checkout` state. Otherwise, it should
    /// return [`Error::InvalidState`].
    #[test]
    fn fails_if_invalid_state() {
        checkout::new_test_ext(ALICE, |order_id| {
            assert_ok!(Orders::cancel(RuntimeOrigin::signed(ALICE), order_id));
            assert_noop!(
                Orders::cancel(RuntimeOrigin::signed(ALICE), order_id),
                Error::InvalidState
            );
        });

        new_test_ext(ALICE, |order_id| {
            assert_ok!(Orders::cancel(RuntimeOrigin::signed(ALICE), order_id));
            assert_noop!(
                Orders::cancel(RuntimeOrigin::signed(ALICE), order_id),
                Error::InvalidState
            );
        })
    }
}

mod pay {
    use super::*;
    use crate::types::{MerchantIdOf, OrderDetails, OrderStatus};
    use crate::InventoryIdOf;
    use fc_pallet_listings::{ItemIdOf, ItemPrice};
    use frame_contrib_traits::listings::item::Item;
    use frame_contrib_traits::listings::InspectItem;
    use frame_support::pallet_prelude::Get;
    use frame_support::traits::fungibles::InspectHold;
    use sp_runtime::DispatchError;

    pub const CHARLIE: AccountId = AccountId::new([3u8; 32]);

    fn new_test_ext<T>(
        who: AccountId,
        items: Vec<(
            ((MerchantIdOf<Test>, InventoryIdOf<Test>), ItemIdOf<Test>),
            Option<AccountId>,
        )>,
        f: impl FnOnce(u32) -> T,
    ) -> T {
        ExtBuilder::default()
            .with_account(ALICE, ExistentialDeposit::get())
            .with_account(BOB, ExistentialDeposit::get())
            .with_account(CHARLIE, ExistentialDeposit::get())
            .with_account(EVE, ExistentialDeposit::get())
            .with_asset(
                mock_helpers::Asset::new(ASSET_A, RootAccount::get(), 1, false)
                    .add_account(ALICE, 50)
                    .add_account(BOB, 40)
                    .add_account(CHARLIE, 30),
            )
            .with_asset(
                mock_helpers::Asset::new(ASSET_B, RootAccount::get(), 10, false)
                    .add_account(ALICE, 50)
                    .add_account(BOB, 40)
                    .add_account(CHARLIE, 30),
            )
            .with_inventory(
                mock_helpers::Inventory::new((AliceStore::get(), 1), ALICE)
                    .with_item(mock_helpers::Item::new(
                        1,
                        b"Item 1 (ASSET_A)".to_vec(),
                        Some(ItemPrice {
                            asset: ASSET_A,
                            amount: 35,
                        }),
                    ))
                    .with_item(mock_helpers::Item::new(
                        2,
                        b"Item 1 (ASSET_B)".to_vec(),
                        Some(ItemPrice {
                            asset: ASSET_B,
                            amount: 35,
                        }),
                    ))
                    .with_item(mock_helpers::Item::new(
                        3,
                        b"Item 2 (ASSET_A)".to_vec(),
                        Some(ItemPrice {
                            asset: ASSET_A,
                            amount: 15,
                        }),
                    ))
                    .with_item(mock_helpers::Item::new(
                        4,
                        b"Item 3 (ASSET_A)".to_vec(),
                        Some(ItemPrice {
                            asset: ASSET_A,
                            amount: 25,
                        }),
                    ))
                    .with_item(mock_helpers::Item::new(
                        10,
                        b"Special Item (Not For Sale)".to_vec(),
                        None,
                    )),
            )
            .build()
            .execute_with(|| {
                let order_id = NextOrderId::<Test>::get();
                assert_ok!(Orders::create_cart(
                    RuntimeOrigin::signed(who.clone()),
                    Some(items)
                ));
                assert_ok!(Orders::checkout(RuntimeOrigin::signed(who), order_id,));
                f(order_id)
            })
    }

    /// Since the `pay` extrinsic is permissionless, anyone can pay for an order. However, the
    /// caller must be a signed origin. Otherwise, return [`DispatchError::BadOrigin`].
    #[test]
    fn fails_if_the_caller_is_not_signed() {
        new_test_ext(
            CHARLIE,
            vec![(((AliceStore::get(), 1), 2), None)],
            |order_id| {
                assert_noop!(
                    Orders::pay(RuntimeOrigin::root(), order_id),
                    DispatchError::BadOrigin
                );
            },
        )
    }

    /// An order can be paid if it exists. Otherwise, return [`Error::OrderNotFound`]
    #[test]
    fn fails_if_the_order_does_not_exist() {
        new_test_ext(BOB, vec![(((AliceStore::get(), 1), 2), None)], |_| {
            assert_noop!(
                Orders::pay(RuntimeOrigin::signed(CHARLIE), 1),
                Error::OrderNotFound
            );
        })
    }

    /// The caller must have enough funds to pay for the entire order. Otherwise, return
    /// [`Error::BalanceLow`][pallet_assets::Error::BalanceLow].
    #[test]
    fn fails_if_the_caller_does_not_have_enough_funds() {
        new_test_ext(
            ALICE,
            vec![(((AliceStore::get(), 1), 1), None)],
            |order_id| {
                assert_noop!(
                    Orders::pay(RuntimeOrigin::signed(CHARLIE), order_id),
                    pallet_assets::Error::<Test>::BalanceLow,
                );
            },
        );

        new_test_ext(
            ALICE,
            vec![(((AliceStore::get(), 1), 2), None)],
            |order_id| {
                assert_noop!(
                    Orders::pay(RuntimeOrigin::signed(BOB), order_id),
                    pallet_assets::Error::<Test>::BalanceLow,
                );
            },
        );

        new_test_ext(
            BOB,
            vec![
                (((AliceStore::get(), 1), 1), None),
                (((AliceStore::get(), 1), 3), None),
            ],
            |order_id| {
                assert_noop!(
                    Orders::pay(RuntimeOrigin::signed(ALICE), order_id),
                    pallet_assets::Error::<Test>::BalanceLow,
                );
            },
        )
    }

    /// The order must be in `Checkout` state to be paid. Otherwise, return [`Error::InvalidState`].
    #[test]
    fn fails_if_invalid_state() {
        // If not yet checked-out, cannot pay for it.
        checkout::new_test_ext(BOB, |order_id| {
            assert_noop!(
                Orders::pay(RuntimeOrigin::signed(BOB), order_id),
                Error::InvalidState,
            );
        });

        // If already paid, cannot pay for it again.
        new_test_ext(
            CHARLIE,
            vec![(((AliceStore::get(), 1), 3), None)],
            |order_id| {
                assert_ok!(Orders::pay(RuntimeOrigin::signed(ALICE), order_id));
                assert_noop!(
                    Orders::pay(RuntimeOrigin::signed(ALICE), order_id),
                    Error::InvalidState,
                );
            },
        );
    }

    /// Asserts that the payment works.
    #[test]
    fn it_works() {
        // CHARLIE creates an order. BOB pays for it, and there are no extra beneficiaries for the
        // articles, so they belong to CHARLIE now. Additionally, ALICE should be able to release
        // the funds BOB transferred as part of the payment.

        let inventory_id = (AliceStore::get(), 1);
        let item_id = 3;

        new_test_ext(CHARLIE, vec![((inventory_id, item_id), None)], |order_id| {
            let bob_asset_b_balance = Assets::balance(ASSET_A, &BOB);

            assert_ok!(Orders::pay(RuntimeOrigin::signed(BOB), order_id));

            // The scheduled `cancel` call is now removed.
            let (schedule_id, _) = Orders::schedule_cancel_params(&order_id);
            assert_noop!(
                Scheduler::next_dispatch_time(schedule_id),
                DispatchError::Unavailable
            );

            // The order is now in progress. The corresponding event is emitted.
            System::assert_has_event(Event::OrderInProgress { order_id }.into());
            assert!(matches!(
                Order::<Test>::get(order_id),
                Some((
                    _,
                    OrderDetails {
                        status: OrderStatus::InProgress,
                        ..
                    }
                ))
            ));

            // The item belongs to CHARLIE (though it's still non-transferable, hence not resellable).
            assert!(matches!(
                Listings::item(&inventory_id, &item_id),
                Some(Item { owner: CHARLIE, .. })
            ));
            assert!(!Listings::transferable(&inventory_id, &item_id));

            // The balances hold (pun intended).
            let price = 15;
            assert_eq!(Assets::balance(ASSET_A, &BOB), bob_asset_b_balance - price);
            assert_eq!(Assets::total_balance_on_hold(ASSET_A, &ALICE), price);

            // Once BOB releases the payment (since CHARLIE received the payment), the item will be
            // unlocked.
            let alice_balance = Assets::balance(ASSET_A, &ALICE);
            assert_ok!(Payments::release(
                RuntimeOrigin::signed(BOB),
                PaymentId::last()
            ));
            assert!(Listings::transferable(&inventory_id, &item_id));
            assert_eq!(Assets::balance(ASSET_A, &ALICE), alice_balance + price);
            assert_eq!(Assets::total_balance_on_hold(ASSET_A, &ALICE), 0);

            // Since all the items have been delivered, the order is now completed
            assert!(matches!(
                Order::<Test>::get(order_id),
                Some((
                    _,
                    OrderDetails {
                        status: OrderStatus::Completed,
                        ..
                    }
                ))
            ));
            System::assert_has_event(Event::OrderCompleted { order_id }.into());
        });

        // BOB creates an order. CHARLIE pays for it, and the article is given to EVE, belonging to
        // them now. Additionally, ALICE should be able to release the funds BOB transferred as part
        // of the payment.
        //
        // ALICE cancels the sale, hence returning the item back to them.
        let inventory_id = (AliceStore::get(), 1);
        let item_id = 3;

        new_test_ext(
            CHARLIE,
            vec![((inventory_id, item_id), Some(EVE))],
            |order_id| {
                let bob_asset_b_balance = Assets::balance(ASSET_A, BOB);
                assert_ok!(Orders::pay(RuntimeOrigin::signed(BOB), order_id));

                // The item belongs to EVE (though it's still non-transferable, hence not resellable).
                assert!(matches!(
                    Listings::item(&inventory_id, &item_id),
                    Some(Item { owner: EVE, .. })
                ));

                // ALICE cancels the payment, now the payment for this item is rolled-back.
                assert_ok!(Payments::cancel(
                    RuntimeOrigin::signed(ALICE),
                    PaymentId::last()
                ));
                assert!(matches!(
                    Listings::item(&inventory_id, &item_id),
                    Some(Item { owner: ALICE, .. })
                ));
                assert_eq!(Assets::balance(ASSET_A, BOB), bob_asset_b_balance);

                // Still, the order is `Completed` since no items are pending.
                assert!(matches!(
                    Order::<Test>::get(order_id),
                    Some((
                        _,
                        OrderDetails {
                            status: OrderStatus::Completed,
                            ..
                        }
                    ))
                ));
            },
        );
    }
}
