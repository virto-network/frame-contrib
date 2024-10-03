use super::*;

use frame_support::{
    assert_ok, derive_impl, parameter_types,
    traits::{ConstU128, ConstU32},
};
use frame_system::EnsureNever;
use impl_nonfungibles::{
    GasSizeConfigMap, GasTankSize, NonFungibleGasBurner, ATTR_MEMBER_GAS_SIZE,
};
use sp_runtime::{
    traits::{IdentifyAccount, IdentityLookup, Verify},
    BoundedBTreeMap, MultiSignature,
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
    type ValueLimit = ConstU32<10>;
    type WeightInfo = ();

    #[cfg(feature = "runtime-benchmarks")]
    type Helper = ();
}

parameter_types! {
    pub GasSizeConfigs: GasSizeConfigMap = {
        let mut map = BoundedBTreeMap::new();

        map.try_insert(GasTankSize::Small, <() as frame_system::WeightInfo>::remark(13))
            .expect("given values are correct; qed");
        map.try_insert(GasTankSize::Medium, <() as frame_system::WeightInfo>::remark(26))
            .expect("given values are correct; qed");
        map.try_insert(GasTankSize::Large, <() as frame_system::WeightInfo>::remark(39))
            .expect("given values are correct; qed");

        map
    };
}

pub type MembershipsGas = NonFungibleGasBurner<Test, GasSizeConfigs, Memberships>;

parameter_types! {
    const CollectionOwner: AccountId = AccountId::new([0u8;32]);
    const SmallMember: AccountId = AccountId::new([1u8;32]);
    const MediumMember: AccountId = AccountId::new([2u8;32]);
    const LargeMember: AccountId = AccountId::new([3u8;32]);
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

        for (item, who, size) in [
            (1, SmallMember::get(), GasTankSize::Small),
            (2, MediumMember::get(), GasTankSize::Medium),
            (3, LargeMember::get(), GasTankSize::Large),
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
                &ATTR_MEMBER_GAS_SIZE,
                &size
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
                &<() as frame_system::WeightInfo>::remark(14),
            )
            .is_none());
            assert!(MembershipsGas::check_available_gas(
                &MediumMember::get(),
                &<() as frame_system::WeightInfo>::remark(27),
            )
            .is_none());
            assert!(MembershipsGas::check_available_gas(
                &LargeMember::get(),
                &<() as frame_system::WeightInfo>::remark(40),
            )
            .is_none());
        });
    }

    #[test]
    fn it_works_returning_which_item_was_used_to_burn_gas() {
        new_test_ext().execute_with(|| {
            assert!(MembershipsGas::check_available_gas(
                &SmallMember::get(),
                &<() as frame_system::WeightInfo>::remark(13),
            )
            .is_some_and(|w| w.eq(&Weight::from_parts(1, 1))));
            assert!(MembershipsGas::check_available_gas(
                &MediumMember::get(),
                &<() as frame_system::WeightInfo>::remark(26),
            )
            .is_some_and(|w| w.eq(&Weight::from_parts(1, 2))));
            assert!(MembershipsGas::check_available_gas(
                &LargeMember::get(),
                &<() as frame_system::WeightInfo>::remark(39),
            )
            .is_some_and(|w| w.eq(&Weight::from_parts(1, 3))));
        });
    }
}
