use super::*;

use frame_support::{
    assert_ok, derive_impl, parameter_types,
    traits::{ConstU128, ConstU32},
    weights::Weight,
};
use frame_system::EnsureNever;
use impl_nonfungibles::{MembershipWeightTank, NonFungibleGasBurner, ATTR_MEMBERSHIP_GAS};
use sp_runtime::{
    traits::{IdentifyAccount, IdentityLookup, Verify},
    MultiSignature,
};

type Block = frame_system::mocking::MockBlock<Test>;

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

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
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
}

pub type MembershipsGas = NonFungibleGasBurner<Test, Memberships, pallet_nfts::ItemConfig>;

parameter_types! {
    const CollectionOwner: AccountId = AccountId::new([0u8;32]);

    const SmallMember: AccountId = AccountId::new([1u8;32]);
    const MediumMember: AccountId = AccountId::new([2u8;32]);
    const LargeMember: AccountId = AccountId::new([3u8;32]);

    SmallTank: Weight = <() as frame_system::WeightInfo>::remark(100);
    MediumTank: Weight = <() as frame_system::WeightInfo>::remark(1000);
    LargeTank: Weight = <() as frame_system::WeightInfo>::remark(10000);
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    use frame_support::traits::nonfungibles_v2::{Create, Mutate};

    let collection_id = 1;
    let mut ext = sp_io::TestExternalities::default();
    ext.execute_with(|| {
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
                MembershipWeightTank::<Test> {
                    max_per_period: Some(SmallTank::get()),
                    ..Default::default()
                },
            ),
            (
                2,
                MediumMember::get(),
                MembershipWeightTank::<Test> {
                    max_per_period: Some(MediumTank::get()),
                    ..Default::default()
                },
            ),
            (
                3,
                LargeMember::get(),
                MembershipWeightTank::<Test> {
                    max_per_period: Some(LargeTank::get()),
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
            assert_ok!(Memberships::set_typed_attribute(
                &collection_id,
                &item,
                &ATTR_MEMBERSHIP_GAS,
                &tank
            ));
        }
    });
    ext
}

mod gas_burner {
    use frame_support::weights::Weight;

    use super::*;

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
            let Some(remaining) = MembershipsGas::check_available_gas(
                &SmallMember::get(),
                &<() as frame_system::WeightInfo>::remark(100),
            ) else {
                return assert!(false);
            };

            assert_eq!(
                MembershipsGas::burn_gas(
                    &SmallMember::get(),
                    &remaining,
                    &<() as frame_system::WeightInfo>::remark(100)
                ),
                Weight::zero()
            );

            // Assert "medium" tank membership
            let Some(remaining) = MembershipsGas::check_available_gas(
                &MediumMember::get(),
                &<() as frame_system::WeightInfo>::remark(100),
            ) else {
                return assert!(false);
            };

            assert_eq!(
                MembershipsGas::burn_gas(
                    &SmallMember::get(),
                    &remaining,
                    &<() as frame_system::WeightInfo>::remark(100)
                ),
                Weight::zero()
            );

            // Assert "large" tank membership
            let Some(remaining) = MembershipsGas::check_available_gas(
                &LargeMember::get(),
                &<() as frame_system::WeightInfo>::remark(1000),
            ) else {
                return assert!(false);
            };

            assert_eq!(
                MembershipsGas::burn_gas(
                    &SmallMember::get(),
                    &remaining,
                    &<() as frame_system::WeightInfo>::remark(1000)
                ),
                Weight::zero()
            );
        });
    }
}
