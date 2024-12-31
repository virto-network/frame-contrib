use frame_support::{
    assert_ok, construct_runtime, derive_impl, parameter_types,
    traits::{AsEnsureOriginWithArg, ConstU128, ConstU32},
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess};
use sp_runtime::{
    traits::{IdentifyAccount, IdentityLookup, Verify},
    MultiSignature,
};

type Block = frame_system::mocking::MockBlock<Test>;
type WeightInfo = ();

pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub type Balance = u128;

parameter_types! {
  pub const RootAccount: AccountId = AccountId::new([0xff; 32]);
}

construct_runtime! {
  pub enum Test {
    System: frame_system,
    Balances: pallet_balances,
    Memberships: pallet_nfts,
  }
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
}

impl pallet_balances::Config for Test {
    type Balance = Balance;
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type WeightInfo = WeightInfo;
    type MaxLocks = ConstU32<10>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = RuntimeHoldReason;
    type MaxFreezes = ConstU32<10>;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
}

impl pallet_nfts::Config for Test {
    type ApprovalsLimit = ();
    type AttributeDepositBase = ();
    type CollectionDeposit = ();
    type CollectionId = u16;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureRootWithSuccess<AccountId, RootAccount>>;
    type Currency = Balances;
    type DepositPerByte = ();
    type Features = ();
    type ForceOrigin = EnsureRoot<AccountId>;
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

use frame_support::traits::nonfungibles_v2::{Create, Mutate};

parameter_types! {
    pub const GroupOwner: AccountId = AccountId::new([0x01; 32]);
    pub const Member: AccountId = AccountId::new([0x10; 32]);
}
const MEMBERSHIPS_MANAGER_GROUP: u16 = 0;
const GROUP: u16 = 1;
const MEMBERSHIP: u32 = 1;

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    let mut ext = sp_io::TestExternalities::new(Default::default());
    ext.execute_with(|| {
        System::set_block_number(1);
        assert_ok!(Memberships::create_collection_with_id(
            MEMBERSHIPS_MANAGER_GROUP,
            &RootAccount::get(),
            &RootAccount::get(),
            &Default::default(),
        ));
        assert_ok!(Memberships::mint_into(
            &MEMBERSHIPS_MANAGER_GROUP,
            &MEMBERSHIP,
            &GroupOwner::get(),
            &Default::default(),
            false,
        ));
        assert_ok!(Memberships::create_collection_with_id(
            GROUP,
            &GroupOwner::get(),
            &GroupOwner::get(),
            &Default::default(),
        ));
    });
    ext
}

mod manager {
    use super::{new_test_ext, Memberships};
    use super::{GroupOwner, Member, GROUP, MEMBERSHIP, MEMBERSHIPS_MANAGER_GROUP};
    use crate::{impl_nonfungibles, Manager, NonFungiblesMemberships};
    use frame_support::assert_ok;

    type MembershipsManager = NonFungiblesMemberships<Memberships>;

    #[test]
    fn assigning_and_releasing_moves_membership_to_special_account() {
        new_test_ext().execute_with(|| {
            assert_ok!(MembershipsManager::assign(
                &GROUP,
                &MEMBERSHIP,
                &Member::get()
            ));
            assert_eq!(
                Memberships::owner(MEMBERSHIPS_MANAGER_GROUP, MEMBERSHIP),
                Some(impl_nonfungibles::ASSIGNED_MEMBERSHIPS_ACCOUNT.into())
            );
            assert_ok!(MembershipsManager::release(&GROUP, &MEMBERSHIP));
            assert_eq!(
                Memberships::owner(MEMBERSHIPS_MANAGER_GROUP, MEMBERSHIP),
                Some(GroupOwner::get())
            );
        });
    }
}
