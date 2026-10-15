use super::*;

use fc_traits_nonfungibles_helpers::SelectNonFungibleItem;
use frame_support::{
    assert_ok, derive_impl, parameter_types,
    traits::{ConstU128, ConstU32},
    weights::Weight,
};
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::EnsureNever;
use impl_nonfungibles::{NonFungibleGasTank, WeightTank};
use sp_runtime::{
    traits::{IdentifyAccount, IdentityLookup, Verify},
    MultiSignature,
};

type Block = frame_system::mocking::MockBlock<Test>;
type BlockNumber = BlockNumberFor<Test>;

pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub type Balance = u128;

#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask
    )]
    pub struct Test;

    #[runtime::pallet_index(0)]
    pub type System = frame_system;

    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;

    #[runtime::pallet_index(20)]
    pub type Memberships = pallet_nfts;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
    type Balance = Balance;
    type ExistentialDeposit = ConstU128<1>;
}

impl pallet_nfts::Config for Test {
    type ApprovalsLimit = ();
    type AttributeDepositBase = ();
    type CollectionDeposit = ();
    type CollectionId = u16;
    type CreateOrigin = EnsureNever<AccountId>;
    type Currency = Balances;
    type DepositPerByte = ();
    type Features = ();
    type ForceOrigin = EnsureNever<AccountId>;
    type ItemAttributesApprovalsLimit = ();
    type ItemDeposit = ();
    type ItemId = u32;
    type KeyLimit = ConstU32<64>;
    type Locker = ();
    type MaxAttributesPerCall = ();
    type MaxDeadlineDuration = ();
    type MaxTips = ();
    type MetadataDepositBase = ();
    type OffchainPublic = AccountPublic;
    type OffchainSignature = MultiSignature;
    type RuntimeEvent = RuntimeEvent;
    type StringLimit = ();
    type ValueLimit = ConstU32<50>;
    type WeightInfo = ();

    #[cfg(feature = "runtime-benchmarks")]
    type Helper = ();
    type BlockNumberProvider = System;
}

#[frame_support::storage_alias]
type Toggle = StorageValue<Prefix, bool, frame_support::pallet_prelude::ValueQuery>;

parameter_types! {
     pub ToggleBasedSelector: Box<dyn SelectNonFungibleItem<u16, u32>> = Box::new(|_, _| Toggle::get());
}

pub type MembershipsGas =
    NonFungibleGasTank<Test, System, Memberships, pallet_nfts::ItemConfig, ToggleBasedSelector>;

parameter_types! {
    pub static TestMaxScan: u32 = 4;
}

/// The same tank, with a scan bound the test can move.
pub type ScanBoundedGas = NonFungibleGasTank<
    Test,
    System,
    Memberships,
    pallet_nfts::ItemConfig,
    ToggleBasedSelector,
    TestMaxScan,
>;

parameter_types! {
    const CollectionOwner: AccountId = AccountId::new([0u8;32]);

    const SmallMember: AccountId = AccountId::new([1u8;32]);
    const MediumMember: AccountId = AccountId::new([2u8;32]);
    const LargeMember: AccountId = AccountId::new([3u8;32]);
    const ExtraLargeMember: AccountId = AccountId::new([4u8;32]);

    SmallTank: Weight = <() as frame_system::WeightInfo>::remark(100);
    MediumTank: Weight = <() as frame_system::WeightInfo>::remark(1000);
    LargeTank: Weight = <() as frame_system::WeightInfo>::remark(10000);
    ExtraLargeTank: Weight = <() as frame_system::WeightInfo>::remark(100000);
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    use frame_support::traits::nonfungibles_v2::{Create, Mutate};

    let collection_id = 1;
    let mut ext = sp_io::TestExternalities::default();
    ext.execute_with(|| {
        Toggle::put(true);

        assert_ok!(Memberships::create_collection_with_id(
            collection_id,
            &CollectionOwner::get(),
            &CollectionOwner::get(),
            &Default::default(),
        ));

        for (item, who, tank) in [
            (
                1,
                SmallMember::get(),
                WeightTank::<BlockNumber> {
                    capacity_per_period: Some(SmallTank::get()),
                    ..Default::default()
                },
            ),
            (
                2,
                MediumMember::get(),
                WeightTank::<BlockNumber> {
                    capacity_per_period: Some(MediumTank::get()),
                    ..Default::default()
                },
            ),
            (
                3,
                LargeMember::get(),
                WeightTank::<BlockNumber> {
                    capacity_per_period: Some(LargeTank::get()),
                    ..Default::default()
                },
            ),
        ] {
            assert_ok!(Memberships::mint_into(
                &collection_id,
                &item,
                &who,
                &Default::default(),
                true,
            ));
            assert_ok!(
                tank.put::<Test, Memberships, pallet_nfts::ItemConfig>(&collection_id, &item)
            );
        }
    });
    ext
}

mod gas_burner {
    use frame_support::weights::Weight;

    use super::*;

    #[test]
    fn fail_if_selector_discards_a_membership() {
        new_test_ext().execute_with(|| {
            Toggle::put(false);
            assert!(MembershipsGas::check_available_gas(
                &SmallMember::get(),
                &<() as frame_system::WeightInfo>::remark(100),
            )
            .is_none());
        })
    }

    #[test]
    fn fail_if_gas_is_larger_than_membership_capacity() {
        new_test_ext().execute_with(|| {
            assert!(MembershipsGas::check_available_gas(
                &SmallMember::get(),
                &<() as frame_system::WeightInfo>::remark(101),
            )
            .is_none());
            assert!(MembershipsGas::check_available_gas(
                &MediumMember::get(),
                &<() as frame_system::WeightInfo>::remark(1001),
            )
            .is_none());
            assert!(MembershipsGas::check_available_gas(
                &LargeMember::get(),
                &<() as frame_system::WeightInfo>::remark(10001),
            )
            .is_none());
        });
    }

    #[test]
    fn it_works_returning_which_item_was_used_to_burn_gas() {
        new_test_ext().execute_with(|| {
            // Assert "small" tank membership
            let remaining = MembershipsGas::prepare_gas(
                &SmallMember::get(),
                &<() as frame_system::WeightInfo>::remark(100),
            )
            .expect("gas to burn equals tank capacity; qed");

            assert_eq!(
                MembershipsGas::burn_gas(
                    &SmallMember::get(),
                    &remaining,
                    &<() as frame_system::WeightInfo>::remark(100)
                ),
                Weight::zero()
            );

            // Assert "medium" tank membership
            let remaining = MembershipsGas::prepare_gas(
                &MediumMember::get(),
                &<() as frame_system::WeightInfo>::remark(1000),
            )
            .expect("gas to burn equals tank capacity; qed");

            assert_eq!(
                MembershipsGas::burn_gas(
                    &MediumMember::get(),
                    &remaining,
                    &<() as frame_system::WeightInfo>::remark(1000)
                ),
                Weight::zero()
            );

            // Assert "large" tank membership
            let remaining = MembershipsGas::prepare_gas(
                &LargeMember::get(),
                &<() as frame_system::WeightInfo>::remark(10000),
            )
            .expect("gas to burn equals tank capacity; qed");

            assert_eq!(
                MembershipsGas::burn_gas(
                    &LargeMember::get(),
                    &remaining,
                    &<() as frame_system::WeightInfo>::remark(10000)
                ),
                Weight::zero()
            );
        });
    }
}

mod gas_fueler {
    use super::*;

    #[test]
    fn it_works() {
        new_test_ext().execute_with(|| {
            // Burn gas on large tank
            let remaining = MembershipsGas::prepare_gas(
                &LargeMember::get(),
                &<() as frame_system::WeightInfo>::remark(1000),
            )
            .expect("gas to burn equals tank capacity; qed");

            assert_eq!(
                MembershipsGas::burn_gas(
                    &LargeMember::get(),
                    &remaining,
                    &<() as frame_system::WeightInfo>::remark(5000)
                ),
                LargeTank::get().saturating_sub(<() as frame_system::WeightInfo>::remark(5000))
            );

            // Refuels gas
            assert_eq!(
                MembershipsGas::refuel_gas(
                    &(1, 3),
                    &<() as frame_system::WeightInfo>::remark(5000)
                ),
                LargeTank::get()
            );
        })
    }
}

mod make_tank {
    use super::*;

    #[test]
    fn it_works() {
        use frame_support::traits::nonfungibles_v2::Mutate;

        new_test_ext().execute_with(|| {
            assert_ok!(Memberships::mint_into(
                &1,
                &4,
                &ExtraLargeMember::get(),
                &Default::default(),
                true,
            ));

            MembershipsGas::make_tank(&(1, 4), Some(ExtraLargeTank::get()), None)
                .expect("failed to register the tank");

            // Burn gas on large tank
            let remaining =
                MembershipsGas::prepare_gas(&ExtraLargeMember::get(), &ExtraLargeTank::get())
                    .expect("gas to burn equals tank capacity; qed");

            assert_eq!(
                MembershipsGas::burn_gas(
                    &ExtraLargeMember::get(),
                    &remaining,
                    &ExtraLargeTank::get(),
                ),
                Weight::zero()
            );

            // Refuels gas
            assert_eq!(
                MembershipsGas::refuel_gas(
                    &(1, 4),
                    &<() as frame_system::WeightInfo>::remark(100000)
                ),
                ExtraLargeTank::get()
            );
        })
    }
}

mod periodic_tanks {
    //! A tank's usage windows, its bounded scan, and the note `prepare_gas` leaves for the burn.

    use super::*;
    use frame_support::storage::unhashed;
    use frame_support::traits::nonfungibles_v2::{InspectEnumerable, Mutate};
    use impl_nonfungibles::paying_item_key;
    use sp_runtime::StateVersion;

    parameter_types! {
        const PeriodicMember: AccountId = AccountId::new([5u8;32]);
        const ScanMember: AccountId = AccountId::new([6u8;32]);
        Capacity: Weight = Weight::from_parts(1_000, 1_000);
    }

    const COLLECTION: u16 = 1;
    const PERIODIC_ITEM: u32 = 10;
    const PERIOD: BlockNumber = 10;

    fn storage_root() -> Vec<u8> {
        sp_io::storage::root(StateVersion::V1)
    }

    fn give_tank(item: u32, who: &AccountId, tank: WeightTank<BlockNumber>) {
        assert_ok!(Memberships::mint_into(
            &COLLECTION,
            &item,
            who,
            &Default::default(),
            true,
        ));
        assert_ok!(tank.put::<Test, Memberships, pallet_nfts::ItemConfig>(&COLLECTION, &item));
    }

    /// A member with a tank of `Capacity` per `PERIOD`, whose stored window starts at `since` and
    /// has used `used`.
    fn periodic_member(since: BlockNumber, used: Weight) {
        give_tank(
            PERIODIC_ITEM,
            &PeriodicMember::get(),
            WeightTank {
                since,
                used,
                period: Some(PERIOD),
                capacity_per_period: Some(Capacity::get()),
            },
        );
    }

    fn stored_tank(item: u32) -> WeightTank<BlockNumber> {
        WeightTank::<BlockNumber>::get::<Test, Memberships>(&COLLECTION, &item)
            .expect("the test gave this item a tank; qed")
    }

    /// Runs a whole transaction's worth of tank calls: preparation, then the burn.
    fn spend(who: &AccountId, estimated: Weight, used: Weight) -> Weight {
        let expected = MembershipsGas::prepare_gas(who, &estimated)
            .expect("the tank covers the estimate; qed");
        MembershipsGas::burn_gas(who, &expected, &used)
    }

    #[test]
    fn default_prepare_gas_is_check() {
        struct OnlyRequired;
        impl GasBurner for OnlyRequired {
            type AccountId = u64;
            type Gas = u64;

            fn check_available_gas(who: &u64, estimated: &u64) -> Option<u64> {
                who.checked_sub(*estimated)
            }

            fn burn_gas(_: &u64, expected: &u64, _: &u64) -> u64 {
                *expected
            }
        }

        assert_eq!(OnlyRequired::prepare_gas(&10, &3), Some(7));
        assert_eq!(OnlyRequired::prepare_gas(&2, &3), None);
        assert_eq!(
            OnlyRequired::prepare_gas(&10, &3),
            OnlyRequired::check_available_gas(&10, &3)
        );
    }

    // INV-1
    #[test]
    fn check_writes_nothing() {
        new_test_ext().execute_with(|| {
            System::set_block_number(3);
            periodic_member(0, Capacity::get());
            let before = storage_root();

            // Admitted, refused, and across a window boundary: no write in any of them.
            assert!(MembershipsGas::check_available_gas(
                &SmallMember::get(),
                &<() as frame_system::WeightInfo>::remark(100)
            )
            .is_some());
            assert!(MembershipsGas::check_available_gas(
                &SmallMember::get(),
                &<() as frame_system::WeightInfo>::remark(101)
            )
            .is_none());
            assert!(
                MembershipsGas::check_available_gas(&PeriodicMember::get(), &Capacity::get())
                    .is_none()
            );
            System::set_block_number(PERIOD + 3);
            let before_boundary = storage_root();
            assert!(
                MembershipsGas::check_available_gas(&PeriodicMember::get(), &Capacity::get())
                    .is_some()
            );
            assert_eq!(storage_root(), before_boundary);

            System::set_block_number(3);
            assert_eq!(storage_root(), before);
        });
    }

    // AC-G2.2
    #[test]
    fn admits_after_a_window_boundary() {
        new_test_ext().execute_with(|| {
            periodic_member(0, Weight::zero());

            // Window 0: use the whole capacity.
            System::set_block_number(3);
            assert_eq!(
                spend(&PeriodicMember::get(), Capacity::get(), Capacity::get()),
                Weight::zero()
            );
            assert!(MembershipsGas::check_available_gas(
                &PeriodicMember::get(),
                &Weight::from_parts(1, 0)
            )
            .is_none());

            // The window has just ended: admitted, and the check writes nothing.
            System::set_block_number(PERIOD);
            let before = storage_root();
            assert_eq!(
                MembershipsGas::check_available_gas(&PeriodicMember::get(), &Capacity::get()),
                Some(Weight::zero())
            );
            assert_eq!(storage_root(), before);

            // Prepared and burnt in the new window.
            let half = Weight::from_parts(500, 500);
            assert_eq!(spend(&PeriodicMember::get(), Capacity::get(), half), half);
            let tank = stored_tank(PERIODIC_ITEM);
            assert_eq!(tank.since, PERIOD);
            assert_eq!(tank.used, half);
        });
    }

    #[test]
    fn window_boundary_is_inclusive() {
        new_test_ext().execute_with(|| {
            periodic_member(0, Capacity::get());

            System::set_block_number(PERIOD - 1);
            assert!(
                MembershipsGas::check_available_gas(&PeriodicMember::get(), &Capacity::get())
                    .is_none()
            );

            System::set_block_number(PERIOD);
            assert!(
                MembershipsGas::check_available_gas(&PeriodicMember::get(), &Capacity::get())
                    .is_some()
            );
        });
    }

    // INV-14
    #[test]
    fn window_start_is_computed_never_in_the_future() {
        new_test_ext().execute_with(|| {
            periodic_member(0, Capacity::get());

            // Two and a half periods later, the current window started at 2 · PERIOD.
            System::set_block_number(2 * PERIOD + PERIOD / 2);
            let used = Weight::from_parts(10, 10);
            spend(&PeriodicMember::get(), used, used);

            let tank = stored_tank(PERIODIC_ITEM);
            assert_eq!(tank.since, 2 * PERIOD);
            assert!(tank.since <= System::block_number());
            assert_eq!(tank.used, used);
        });
    }

    // INV-14
    #[test]
    fn future_window_start_keeps_its_usage() {
        new_test_ext().execute_with(|| {
            // kreivo#505's reset wrote `since = now + period`. Before the clock reaches it, the stored
            // window stays current, with its usage.
            periodic_member(2 * PERIOD, Capacity::get());

            System::set_block_number(PERIOD + 5);
            assert!(MembershipsGas::check_available_gas(
                &PeriodicMember::get(),
                &Weight::from_parts(1, 0)
            )
            .is_none());
            assert!(MembershipsGas::prepare_gas(&PeriodicMember::get(), &Weight::zero()).is_some());

            // Once a whole period past it, the tank is usable again.
            System::set_block_number(3 * PERIOD);
            assert_eq!(
                MembershipsGas::check_available_gas(&PeriodicMember::get(), &Capacity::get()),
                Some(Weight::zero())
            );
        });
    }

    // NFR-1
    #[test]
    fn scan_reads_at_most_max_scan_items() {
        new_test_ext().execute_with(|| {
            let who = ScanMember::get();
            for item in 20..23 {
                assert_ok!(Memberships::mint_into(
                    &COLLECTION,
                    &item,
                    &who,
                    &Default::default(),
                    true,
                ));
            }
            // Only the item the scan reaches last holds a tank.
            let owned: Vec<_> = Memberships::owned(&who).collect();
            let (_, last) = *owned.last().expect("three items were minted; qed");
            assert_ok!(WeightTank::<BlockNumber> {
                capacity_per_period: Some(Capacity::get()),
                ..Default::default()
            }
            .put::<Test, Memberships, pallet_nfts::ItemConfig>(&COLLECTION, &last));

            let estimated = Weight::from_parts(1, 1);
            TestMaxScan::set(owned.len() as u32 - 1);
            assert!(ScanBoundedGas::check_available_gas(&who, &estimated).is_none());
            assert!(ScanBoundedGas::prepare_gas(&who, &estimated).is_none());

            TestMaxScan::set(0);
            assert!(ScanBoundedGas::check_available_gas(&who, &estimated).is_none());

            TestMaxScan::set(owned.len() as u32);
            assert!(ScanBoundedGas::check_available_gas(&who, &estimated).is_some());
            let expected = ScanBoundedGas::prepare_gas(&who, &estimated)
                .expect("the bound now reaches the tank; qed");
            assert_eq!(
                ScanBoundedGas::burn_gas(&who, &expected, &estimated),
                Capacity::get().saturating_sub(estimated)
            );
        });
    }

    /// `who`'s paying-item note, if any.
    fn note(who: &AccountId) -> Option<(u16, u32, Weight)> {
        unhashed::get(&paying_item_key(who))
    }

    #[test]
    fn prepare_gas_writes_the_note_and_burn_clears_it() {
        new_test_ext().execute_with(|| {
            let who = SmallMember::get();
            let estimated = <() as frame_system::WeightInfo>::remark(10);

            let checked = MembershipsGas::check_available_gas(&who, &estimated);
            assert_eq!(note(&who), None);

            let expected = MembershipsGas::prepare_gas(&who, &estimated);
            assert_eq!(expected, checked);
            assert_eq!(
                note(&who),
                expected.map(|remaining| (COLLECTION, 1, remaining))
            );

            MembershipsGas::burn_gas(
                &who,
                &expected.expect("the tank covers the estimate; qed"),
                &estimated,
            );
            assert_eq!(note(&who), None);
        });
    }

    /// The paying-item note lives from preparation to the end of the transaction, whichever way it
    /// ends: a burn, a burn that does not match it, or a transaction that pays no fee.
    // REQ-RT-8
    #[test]
    fn paying_item_note_never_outlives_the_transaction() {
        new_test_ext().execute_with(|| {
            let who = SmallMember::get();
            let estimated = <() as frame_system::WeightInfo>::remark(10);
            let before = storage_root();

            // No fee: the note is dropped and nothing else changed.
            let expected = MembershipsGas::prepare_gas(&who, &estimated)
                .expect("the tank covers the estimate; qed");
            assert!(note(&who).is_some());
            MembershipsGas::cancel_gas(&who, &expected);
            assert_eq!(note(&who), None);
            assert_eq!(storage_root(), before);

            // A burn that does not match the note charges nothing, and still drops it.
            MembershipsGas::prepare_gas(&who, &estimated)
                .expect("the tank covers the estimate; qed");
            assert_eq!(
                MembershipsGas::burn_gas(&who, &Weight::from_parts(1, 1), &estimated),
                Weight::zero()
            );
            assert_eq!(note(&who), None);
            assert_eq!(storage_root(), before);

            // A burn charges the tank and drops the note.
            assert_eq!(
                spend(&who, estimated, estimated),
                SmallTank::get().saturating_sub(estimated)
            );
            assert_eq!(note(&who), None);
        });
    }

    /// `REQ-RT-8` (`DEC-23` fix 3): a call that gives the account an item the scan reaches before the
    /// tank (a purchase, a swap, a new membership) does not push the tank out of the burn's reach.
    // REQ-RT-8
    #[test]
    fn burn_charges_the_noted_tank_after_an_earlier_item_is_added() {
        new_test_ext().execute_with(|| {
            let who = ScanMember::get();
            give_tank(
                30,
                &who,
                WeightTank {
                    capacity_per_period: Some(Capacity::get()),
                    ..Default::default()
                },
            );
            TestMaxScan::set(1);
            let estimated = Weight::from_parts(100, 100);
            let used = Weight::from_parts(60, 60);
            let expected = ScanBoundedGas::prepare_gas(&who, &estimated)
                .expect("the tank is the account's only item; qed");

            // Dispatch: the account gains items until one sorts before the tank, so the bounded scan
            // no longer reaches it.
            let mut item = 31;
            while Memberships::owned(&who).next() == Some((COLLECTION, 30)) {
                assert_ok!(Memberships::mint_into(
                    &COLLECTION,
                    &item,
                    &who,
                    &Default::default(),
                    true,
                ));
                item += 1;
            }
            assert!(ScanBoundedGas::check_available_gas(&who, &Weight::zero()).is_none());

            assert_eq!(
                ScanBoundedGas::burn_gas(&who, &expected, &used),
                Capacity::get().saturating_sub(used)
            );
            assert_eq!(stored_tank(30).used, used);
        });
    }

    /// `REQ-RT-8` (`DEC-23` fix 3): the tank that admitted a transaction pays for it, even when the
    /// call moves its item to another account during dispatch.
    // REQ-RT-8
    #[test]
    fn burn_charges_the_noted_tank_after_its_item_moves() {
        new_test_ext().execute_with(|| {
            let who = SmallMember::get();
            let estimated = <() as frame_system::WeightInfo>::remark(10);
            let expected = MembershipsGas::prepare_gas(&who, &estimated)
                .expect("the tank covers the estimate; qed");

            // Dispatch: the item goes to another account.
            assert_ok!(
                <Memberships as frame_support::traits::nonfungibles_v2::Transfer<_>>::transfer(
                    &COLLECTION,
                    &1,
                    &MediumMember::get()
                )
            );

            assert_eq!(
                MembershipsGas::burn_gas(&who, &expected, &estimated),
                SmallTank::get().saturating_sub(estimated)
            );
            assert_eq!(stored_tank(1).used, estimated);
        });
    }

    #[test]
    fn burn_without_preparation_charges_nothing() {
        new_test_ext().execute_with(|| {
            let who = SmallMember::get();
            let estimated = <() as frame_system::WeightInfo>::remark(10);
            let expected = MembershipsGas::check_available_gas(&who, &estimated)
                .expect("the tank covers the estimate; qed");

            assert_eq!(
                MembershipsGas::burn_gas(&who, &expected, &estimated),
                Weight::zero()
            );
            assert_eq!(stored_tank(1).used, Weight::zero());
        });
    }
}
