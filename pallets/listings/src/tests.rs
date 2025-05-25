//! Tests for listings pallet

use crate as fc_pallet_listings;
use crate::{mock::*, InventoryId, ItemPrice};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::{BoundedVec, DispatchError};

type Listings = fc_pallet_listings::Pallet<Test>;
type ListingsError = fc_pallet_listings::Error<Test>;
type ListingsEvent = fc_pallet_listings::Event<Test>;

type Catalog = pallet_nfts::Pallet<Test>;
type CatalogError = pallet_nfts::Error<Test>;
type CatalogEvent = pallet_nfts::Event<Test>;

mod create_inventory {
    use super::*;
    use frame_support::traits::{fungible, fungible::Unbalanced, tokens::Precision};

    #[test]
    fn fails_if_create_origin_is_invalid() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::create_inventory(
                    RuntimeOrigin::signed(ALICE),
                    InventoryId([2u8; 32].into(), 1)
                ),
                DispatchError::BadOrigin,
            );
        })
    }

    #[test]
    fn fails_if_cannot_deposit() {
        new_test_ext().execute_with(|| {
            <Balances as fungible::Mutate<AccountId>>::set_balance(&ALICE, 2);
            assert_noop!(
                Listings::create_inventory(
                    RuntimeOrigin::signed(ALICE),
                    InventoryId([1u8; 32].into(), 1)
                ),
                pallet_balances::Error::<Test>::InsufficientBalance
            );
        })
    }

    #[test]
    fn fails_if_inventory_already_exists() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::create_inventory(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 1)
                ),
                ListingsError::AlreadyExistingInventory,
            );
        })
    }

    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            assert_ok!(Balances::increase_balance(
                &ALICE,
                CollectionDeposit::get(),
                Precision::Exact
            ));

            assert_ok!(Listings::create_inventory(
                RuntimeOrigin::signed(ALICE),
                InventoryId([1u8; 32].into(), 1),
            ));

            System::assert_has_event(
                ListingsEvent::InventoryCreated {
                    merchant: [1u8; 32].into(),
                    id: 1,
                    owner: ALICE,
                }
                .into(),
            );
        })
    }
}

mod archive_inventory {
    use super::*;

    #[test]
    fn fails_if_unknown_inventory() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::archive_inventory(
                    RuntimeOrigin::signed(ALICE),
                    InventoryId([2u8; 32].into(), 1)
                ),
                ListingsError::UnknownInventory,
            );
        })
    }

    #[test]
    fn fails_if_caller_is_not_the_inventory_admin() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::archive_inventory(
                    RuntimeOrigin::signed(ALICE),
                    InventoryId([0u8; 32].into(), 1)
                ),
                DispatchError::BadOrigin,
            );
        })
    }

    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            let inventory_id = InventoryId([0u8; 32].into(), 1);

            assert_ok!(Listings::archive_inventory(
                RuntimeOrigin::signed(ROOT),
                inventory_id,
            ));
            assert_noop!(
                Listings::ensure_active_inventory(&inventory_id),
                ListingsError::ArchivedInventory
            );
        })
    }

    #[test]
    fn fails_if_inventory_is_already_archived() {
        new_test_ext().execute_with(|| {
            let inventory_id = InventoryId([0u8; 32].into(), 1);

            assert_ok!(Listings::archive_inventory(
                RuntimeOrigin::signed(ROOT),
                inventory_id
            ));
            assert_noop!(
                Listings::archive_inventory(RuntimeOrigin::signed(ROOT), inventory_id),
                ListingsError::ArchivedInventory
            );
        })
    }
}

mod publish_item {
    use super::*;

    #[test]
    fn fails_if_unknown_inventory() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::publish_item(
                    RuntimeOrigin::signed(ALICE),
                    InventoryId([0u8; 32].into(), 2),
                    1,
                    BoundedVec::truncate_from(b"Item name".to_vec()),
                    None,
                ),
                ListingsError::UnknownInventory,
            );
        })
    }

    #[test]
    fn fails_if_inventory_is_archived() {
        new_test_ext().execute_with(|| {
            assert_ok!(Listings::archive_inventory(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1)
            ));

            assert_noop!(
                Listings::publish_item(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    BoundedVec::truncate_from(b"Item name".to_vec()),
                    None,
                ),
                ListingsError::ArchivedInventory,
            );
        })
    }

    #[test]
    fn fails_if_caller_is_not_the_inventory_admin() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::publish_item(
                    RuntimeOrigin::signed(ALICE),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    BoundedVec::truncate_from(b"Item name".to_vec()),
                    None,
                ),
                DispatchError::BadOrigin,
            );
        })
    }

    #[test]
    fn it_works() {
        // Reports published event
        new_test_ext().execute_with(|| {
            assert_ok!(Listings::publish_item(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1),
                1,
                BoundedVec::truncate_from(b"Item name".to_vec()),
                None,
            ));

            System::assert_has_event(
                ListingsEvent::ItemPublished {
                    inventory_id: InventoryId([0u8; 32].into(), 1),
                    id: 1,
                }
                .into(),
            )
        });

        // Reports published event
        new_test_ext().execute_with(|| {
            assert_ok!(Listings::publish_item(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1),
                1,
                BoundedVec::truncate_from(b"Item name".to_vec()),
                Some(ItemPrice {
                    asset: 1,
                    amount: 1,
                }),
            ));

            System::assert_has_event(
                ListingsEvent::ItemPriceSet {
                    inventory_id: InventoryId([0u8; 32].into(), 1),
                    id: 1,
                    price: ItemPrice {
                        asset: 1,
                        amount: 1,
                    },
                }
                .into(),
            )
        })
    }

    #[test]
    fn fails_publishing_an_already_existing_item() {
        // Reports published event
        new_test_ext().execute_with(|| {
            assert_ok!(Listings::publish_item(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1),
                1,
                BoundedVec::truncate_from(b"Item name".to_vec()),
                None,
            ));

            assert_noop!(
                Listings::publish_item(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    BoundedVec::truncate_from(b"Item name".to_vec()),
                    None,
                ),
                CatalogError::AlreadyExists
            );
        })
    }
}

fn new_test_ext_with_item() -> sp_io::TestExternalities {
    let mut t = new_test_ext();
    t.execute_with(|| {
        assert_ok!(Listings::publish_item(
            RuntimeOrigin::signed(ROOT),
            InventoryId([0u8; 32].into(), 1),
            1,
            BoundedVec::truncate_from(b"Item name".to_vec()),
            None,
        ));
    });
    t
}

mod set_item_price {
    use super::{new_test_ext_with_item as new_test_ext, *};

    #[test]
    fn fails_if_unknown_inventory_or_item() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::set_item_price(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 2),
                    1,
                    ItemPrice {
                        asset: 1,
                        amount: 1,
                    }
                ),
                ListingsError::UnknownInventory,
            );

            assert_noop!(
                Listings::set_item_price(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 1),
                    2,
                    ItemPrice {
                        asset: 1,
                        amount: 1,
                    }
                ),
                ListingsError::UnknownItem,
            );
        })
    }

    #[test]
    fn fails_if_inventory_is_archived() {
        new_test_ext().execute_with(|| {
            assert_ok!(Listings::archive_inventory(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1),
            ));

            assert_noop!(
                Listings::set_item_price(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    ItemPrice {
                        asset: 1,
                        amount: 1,
                    }
                ),
                ListingsError::ArchivedInventory,
            );
        })
    }

    #[test]
    fn fails_if_caller_is_not_the_inventory_admin() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::set_item_price(
                    RuntimeOrigin::signed(ALICE),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    ItemPrice {
                        asset: 1,
                        amount: 1,
                    }
                ),
                ListingsError::NoPermission,
            );
        })
    }

    #[test]
    fn fails_if_caller_is_the_inventory_admin_but_the_item_has_already_been_transferred() {
        new_test_ext().execute_with(|| {
            assert_ok!(Catalog::transfer(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1),
                1,
                BOB,
            ));

            assert_noop!(
                Listings::set_item_price(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    ItemPrice {
                        asset: 1,
                        amount: 1,
                    }
                ),
                ListingsError::NoPermission,
            );
        })
    }

    #[test]
    fn fails_if_item_is_non_transferable() {
        new_test_ext().execute_with(|| {
            assert_ok!(Catalog::transfer(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1),
                1,
                BOB,
            ));
            assert_ok!(Listings::mark_item_can_transfer(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1),
                1,
                false,
            ));

            assert_noop!(
                Listings::set_item_price(
                    RuntimeOrigin::signed(BOB),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    ItemPrice {
                        asset: 1,
                        amount: 1,
                    }
                ),
                ListingsError::ItemNonTransferable,
            );
        })
    }

    #[test]
    fn fails_if_item_is_marked_as_not_for_resale() {
        new_test_ext().execute_with(|| {
            assert_ok!(Listings::mark_item_not_for_resale(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1),
                1,
                true
            ));
            assert_ok!(Catalog::transfer(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1),
                1,
                BOB,
            ));

            assert_noop!(
                Listings::set_item_price(
                    RuntimeOrigin::signed(BOB),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    ItemPrice {
                        asset: 1,
                        amount: 1,
                    }
                ),
                ListingsError::NotForResale,
            );
        })
    }

    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            assert_ok!(Listings::set_item_price(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1),
                1,
                ItemPrice {
                    asset: 1,
                    amount: 1,
                }
            ));

            System::assert_has_event(
                ListingsEvent::ItemPriceSet {
                    inventory_id: InventoryId([0u8; 32].into(), 1),
                    id: 1,
                    price: ItemPrice {
                        asset: 1,
                        amount: 1,
                    },
                }
                .into(),
            )
        })
    }
}

mod mark_item_can_transfer {
    use super::{new_test_ext_with_item as new_test_ext, *};
    use fc_traits_listings::InspectItem;

    #[test]
    fn fails_if_unknown_inventory_or_item() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::mark_item_can_transfer(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 2),
                    1,
                    false
                ),
                ListingsError::UnknownInventory,
            );

            assert_noop!(
                Listings::mark_item_can_transfer(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 1),
                    2,
                    false
                ),
                ListingsError::UnknownItem,
            );
        })
    }

    #[test]
    fn fails_if_inventory_is_archived() {
        new_test_ext().execute_with(|| {
            assert_ok!(Listings::archive_inventory(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1),
            ));

            assert_noop!(
                Listings::mark_item_can_transfer(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    false,
                ),
                ListingsError::ArchivedInventory
            );
        })
    }

    #[test]
    fn fails_if_caller_is_not_the_inventory_admin() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::mark_item_can_transfer(
                    RuntimeOrigin::signed(ALICE),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    false,
                ),
                DispatchError::BadOrigin,
            );
        })
    }

    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            let caller = RuntimeOrigin::signed(ROOT);
            let inventory_id = InventoryId([0u8; 32].into(), 1);
            let id = 1;

            assert_ok!(Listings::mark_item_can_transfer(
                caller.clone(),
                inventory_id,
                id,
                false,
            ));

            assert!(!Listings::transferable(&inventory_id.into(), &id));

            assert_ok!(Listings::mark_item_can_transfer(
                caller,
                inventory_id,
                id,
                true,
            ));

            assert!(Listings::transferable(&inventory_id.into(), &id));
        })
    }
}

mod mark_item_not_for_resale {
    use super::{new_test_ext_with_item as new_test_ext, *};
    use fc_traits_listings::InspectItem;

    #[test]
    fn fails_if_unknown_inventory_or_item() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::mark_item_not_for_resale(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 2),
                    1,
                    true
                ),
                ListingsError::UnknownInventory,
            );

            assert_noop!(
                Listings::mark_item_not_for_resale(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 1),
                    2,
                    true
                ),
                ListingsError::UnknownItem,
            );
        })
    }

    #[test]
    fn fails_if_inventory_is_archived() {
        new_test_ext().execute_with(|| {
            assert_ok!(Listings::archive_inventory(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1),
            ));

            assert_noop!(
                Listings::mark_item_not_for_resale(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    false,
                ),
                ListingsError::ArchivedInventory
            );
        })
    }

    #[test]
    fn fails_if_caller_is_not_the_inventory_admin() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::mark_item_not_for_resale(
                    RuntimeOrigin::signed(ALICE),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    true,
                ),
                DispatchError::BadOrigin,
            );
        })
    }

    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            let caller = RuntimeOrigin::signed(ROOT);
            let inventory_id = InventoryId([0u8; 32].into(), 1);
            let id = 1;

            assert_ok!(Listings::mark_item_not_for_resale(
                caller.clone(),
                inventory_id,
                id,
                true,
            ));

            assert!(!Listings::can_resell(&inventory_id.into(), &id));

            assert_ok!(Listings::mark_item_not_for_resale(
                caller,
                inventory_id,
                id,
                false,
            ));

            assert!(Listings::can_resell(&inventory_id.into(), &id));
        })
    }
}

mod set_item_attribute {
    use super::{new_test_ext_with_item as new_test_ext, *};
    use codec::Encode;
    use pallet_nfts::AttributeNamespace;

    #[test]
    fn fails_if_unknown_inventory_or_item() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::set_item_attribute(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 2),
                    1,
                    BoundedVec::truncate_from(b"ATTRIBUTE_KEY".to_vec()),
                    Some(BoundedVec::truncate_from(b"ATTRIBUTE_VALUE".to_vec())),
                ),
                ListingsError::UnknownInventory
            );
        });

        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::set_item_attribute(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 2),
                    1,
                    BoundedVec::truncate_from(b"ATTRIBUTE_KEY".to_vec()),
                    Some(BoundedVec::truncate_from(b"ATTRIBUTE_VALUE".to_vec())),
                ),
                ListingsError::UnknownInventory
            );
        })
    }

    #[test]
    fn fails_if_inventory_is_archived() {
        new_test_ext().execute_with(|| {
            assert_ok!(Listings::archive_inventory(
                RuntimeOrigin::signed(ROOT),
                InventoryId([0u8; 32].into(), 1),
            ));

            assert_noop!(
                Listings::set_item_attribute(
                    RuntimeOrigin::signed(ROOT),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    BoundedVec::truncate_from(b"ATTRIBUTE_KEY".to_vec()),
                    Some(BoundedVec::truncate_from(b"ATTRIBUTE_VALUE".to_vec())),
                ),
                ListingsError::ArchivedInventory
            );
        });
    }

    #[test]
    fn fails_if_caller_is_not_the_inventory_admin() {
        new_test_ext().execute_with(|| {
            assert_noop!(
                Listings::set_item_attribute(
                    RuntimeOrigin::signed(ALICE),
                    InventoryId([0u8; 32].into(), 1),
                    1,
                    BoundedVec::truncate_from(b"ATTRIBUTE_KEY".to_vec()),
                    Some(BoundedVec::truncate_from(b"ATTRIBUTE_VALUE".to_vec())),
                ),
                DispatchError::BadOrigin,
            );
        })
    }

    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            let caller = RuntimeOrigin::signed(ROOT);
            let inventory_id = InventoryId([0u8; 32].into(), 1);
            let id = 1;

            let key = BoundedVec::truncate_from(b"ATTRIBUTE_KEY".to_vec());
            let value = BoundedVec::truncate_from(b"ATTRIBUTE_VALUE".to_vec());

            assert_ok!(Listings::set_item_attribute(
                caller.clone(),
                inventory_id,
                id,
                key.clone(),
                Some(value.clone()),
            ));

            System::assert_last_event(
                CatalogEvent::AttributeSet {
                    collection: inventory_id,
                    maybe_item: Some(id),
                    key: BoundedVec::truncate_from(key.clone().encode()),
                    value: BoundedVec::truncate_from(value.encode()),
                    namespace: AttributeNamespace::Pallet,
                }
                .into(),
            );

            assert_ok!(Listings::set_item_attribute(
                caller,
                inventory_id,
                id,
                key.clone(),
                None,
            ));

            System::assert_last_event(
                CatalogEvent::AttributeCleared {
                    collection: inventory_id,
                    maybe_item: Some(id),
                    key: BoundedVec::truncate_from(key.clone().encode()),
                    namespace: AttributeNamespace::Pallet,
                }
                .into(),
            );
        })
    }
}
