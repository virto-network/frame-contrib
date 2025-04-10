//! Tests for orders pallet.

use super::{Error as ErrorT, Event as EventT, Pallet};
use crate::mock::*;
use fc_pallet_listings::{InventoryId, ItemType};
use frame_support::{assert_noop, assert_ok};

type Orders = Pallet<Test>;
type Error = ErrorT<Test>;
type Event = EventT<Test>;

mod create_cart {
    use super::*;

    /// Items added in a cart must exist in `Listings`. Otherwise, it should return
    /// [Error::ItemNotFound].
    #[test]
    fn fails_if_the_item_does_not_exist() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Orders::create_cart(
                    RuntimeOrigin::signed(BOB),
                    Some(vec![(
                        InventoryId(AliceStore::get(), 2),
                        ItemType::Unit(1),
                        None
                    )])
                ),
                Error::ItemNotFound
            );

            assert_noop!(
                Orders::create_cart(
                    RuntimeOrigin::signed(BOB),
                    Some(vec![(
                        InventoryId(AliceStore::get(), 1),
                        ItemType::Unit(4),
                        None
                    )])
                ),
                Error::ItemNotFound
            );
        })
    }

    /// BOB (`[2u8; 32]`) is allowed a maximum of 2 carts, per origin rules. They can create up to 2
    /// carts. Otherwise, it should return [Error::MaxCartsExceeded].
    #[test]
    fn fails_if_the_account_exceeds_the_number_of_allowed_carts() {
        new_test_ext().execute_with(|| {
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(BOB),
                Some(vec![(
                    InventoryId(AliceStore::get(), 1),
                    ItemType::Unit(1),
                    None
                ),]),
            ));
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(BOB),
                Some(vec![(
                    InventoryId(AliceStore::get(), 1),
                    ItemType::Unit(2),
                    None
                )]),
            ));
            assert_noop!(
                Orders::create_cart(
                    RuntimeOrigin::signed(BOB),
                    Some(vec![(
                        InventoryId(AliceStore::get(), 1),
                        ItemType::Unit(3),
                        None
                    )])
                ),
                Error::MaxCartsExceeded
            );
        })
    }

    /// Although EVE (`[5u8; 32]`) is allowed to have 5 carts, the maximum number of carts in the
    /// system is 4. They can create up to 4 carts. Otherwise, it should return
    /// [Error::MaxCartsExceeded].
    #[test]
    fn fails_if_account_exceeds_the_max_number_of_carts() {
        new_test_ext().execute_with(|| {
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(EVE),
                Some(vec![(
                    InventoryId(AliceStore::get(), 1),
                    ItemType::Unit(1),
                    None
                ),]),
            ));
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(EVE),
                Some(vec![(
                    InventoryId(AliceStore::get(), 1),
                    ItemType::Unit(2),
                    None
                )]),
            ));
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(EVE),
                Some(vec![(
                    InventoryId(AliceStore::get(), 1),
                    ItemType::Unit(3),
                    None
                ),]),
            ));
            assert_ok!(Orders::create_cart(
                RuntimeOrigin::signed(EVE),
                Some(vec![(
                    InventoryId(BobStore::get(), 1),
                    ItemType::Unit(1),
                    None
                ),]),
            ));
            assert_noop!(
                Orders::create_cart(
                    RuntimeOrigin::signed(EVE),
                    Some(vec![(
                        InventoryId(BobStore::get(), 1),
                        ItemType::Unit(2),
                        None
                    )])
                ),
                Error::MaxCartsExceeded
            );
        })
    }

    /// BOB (`[2u8; 32]`) is allowed a maximum of 2 items per cart, per origin rules. They're
    /// allowed to set carts with up to 2 items. Otherwise, it should return
    /// [Error::MaxItemsExceeded].
    #[test]
    fn fails_if_the_cart_exceeds_the_number_of_allowed_items() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Orders::create_cart(
                    RuntimeOrigin::signed(BOB),
                    Some(vec![
                        (InventoryId(AliceStore::get(), 1), ItemType::Unit(1), None),
                        (InventoryId(AliceStore::get(), 1), ItemType::Unit(2), None),
                        (InventoryId(AliceStore::get(), 1), ItemType::Unit(3), None)
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
                Some(vec![(
                    InventoryId(AliceStore::get(), 1),
                    ItemType::Unit(1),
                    None
                )])
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
