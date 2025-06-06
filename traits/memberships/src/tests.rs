use frame_support::{
    assert_ok, construct_runtime, derive_impl, parameter_types,
    traits::{AsEnsureOriginWithArg, ConstU32},
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess};
use sp_runtime::{
    traits::{IdentifyAccount, IdentityLookup, Verify},
    MultiSignature,
};

type Block = frame_system::mocking::MockBlock<Test>;

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

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = Balance;
    type AccountStore = System;
}

type CollectionId = <Test as pallet_nfts::Config>::CollectionId;
type ItemId = <Test as pallet_nfts::Config>::ItemId;

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
    type BlockNumberProvider = System;
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

    type MembershipsManager = NonFungiblesMemberships<Memberships, pallet_nfts::ItemConfig>;

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

mod with_hooks {
    use super::{new_test_ext, Memberships};
    use super::{AccountId, CollectionId, ItemId, Member, GROUP, MEMBERSHIP};
    use crate::{
        GenericRank, Manager, NonFungiblesMemberships, OnMembershipAssigned, OnMembershipReleased,
        OnRankSet, Rank, WithHooks,
    };
    use codec::{Decode, Encode};
    use frame_support::pallet_prelude::ValueQuery;
    use frame_support::{assert_ok, parameter_types, storage_alias};
    use sp_runtime::{traits::ConstU32, BoundedVec, DispatchError};

    #[derive(Debug, Encode, Decode, PartialEq)]
    enum Hook {
        MembershipAssigned(AccountId, CollectionId, ItemId),
        MembershipReleased(CollectionId, ItemId),
        RankSet(CollectionId, ItemId, GenericRank),
    }

    #[storage_alias]
    pub type Hooks = StorageValue<Prefix, BoundedVec<Hook, ConstU32<4>>, ValueQuery>;

    parameter_types! {
        pub AddMembershipAssignedHook: Box<dyn OnMembershipAssigned<AccountId, CollectionId, ItemId>> = Box::new(
            |who, g, m| {
                Hooks::try_append(Hook::MembershipAssigned(who, g, m)).map_err(|_| DispatchError::Other("MaxHooks"))
            }
        );
        pub AddMembershipReleasedHook: Box<dyn OnMembershipReleased<CollectionId, ItemId>> = Box::new(
            |g, m| Hooks::try_append(Hook::MembershipReleased(g, m)).map_err(|_| DispatchError::Other("MaxHooks"))
        );
        pub AddRankSetHook: Box<dyn OnRankSet<CollectionId, ItemId>> = Box::new(
            |g, m, r| Hooks::try_append(Hook::RankSet(g, m, r)).map_err(|_| DispatchError::Other("MaxHooks"))
        );
    }

    type NoHooksManager = WithHooks<NonFungiblesMemberships<Memberships, pallet_nfts::ItemConfig>>;

    #[test]
    fn noop_hooks_by_default_works() {
        new_test_ext().execute_with(|| {
            assert_ok!(NoHooksManager::assign(&GROUP, &MEMBERSHIP, &Member::get()));
            assert_ok!(NoHooksManager::set_rank(
                &GROUP,
                &MEMBERSHIP,
                GenericRank(1)
            ));
            assert_ok!(NoHooksManager::release(&GROUP, &MEMBERSHIP));

            assert_eq!(
                Hooks::get(),
                BoundedVec::<Hook, ConstU32<4>>::truncate_from(vec![])
            )
        })
    }

    type MembershipsManager = WithHooks<
        NonFungiblesMemberships<Memberships, pallet_nfts::ItemConfig>,
        AddMembershipAssignedHook,
        AddMembershipReleasedHook,
        AddRankSetHook,
    >;

    #[test]
    fn assigning_and_releasing_calls_hooks() {
        new_test_ext().execute_with(|| {
            assert_ok!(MembershipsManager::assign(
                &GROUP,
                &MEMBERSHIP,
                &Member::get()
            ));

            assert_eq!(
                Hooks::get(),
                BoundedVec::<Hook, ConstU32<4>>::truncate_from(vec![Hook::MembershipAssigned(
                    Member::get(),
                    GROUP,
                    MEMBERSHIP
                )])
            );

            assert_ok!(MembershipsManager::release(&GROUP, &MEMBERSHIP,));

            assert_eq!(
                Hooks::get(),
                BoundedVec::<Hook, ConstU32<4>>::truncate_from(vec![
                    Hook::MembershipAssigned(Member::get(), GROUP, MEMBERSHIP),
                    Hook::MembershipReleased(GROUP, MEMBERSHIP)
                ])
            );
        });
    }

    #[test]
    fn setting_rank_calls_hooks() {
        new_test_ext().execute_with(|| {
            assert_ok!(MembershipsManager::assign(
                &GROUP,
                &MEMBERSHIP,
                &Member::get()
            ));

            assert_ok!(MembershipsManager::set_rank(
                &GROUP,
                &MEMBERSHIP,
                GenericRank(1)
            ));

            assert_eq!(
                Hooks::get(),
                BoundedVec::<Hook, ConstU32<4>>::truncate_from(vec![
                    Hook::MembershipAssigned(Member::get(), GROUP, MEMBERSHIP),
                    Hook::RankSet(GROUP, MEMBERSHIP, GenericRank(1))
                ])
            );
        })
    }
}
