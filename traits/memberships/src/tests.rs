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
    type ApprovalsLimit = ConstU32<2>;
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

#[allow(deprecated)]
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

#[allow(deprecated)]
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

mod group_collection {
    //! `GroupCollectionMemberships` over `pallet-nfts` (F-04). A comment above each test names the
    //! `SPEC.md` identifiers it verifies.
    use super::{AccountId, Balances, Memberships, RuntimeOrigin, Test};
    use crate::{
        Attributes, Error, GenericRank, GroupCollectionMemberships, Inspect, InspectEnumerable,
        Issue, Manager, Rank, RankOnTransfer, Receivers, Transfer, TransferPolicy,
        ATTR_MEMBER_RANK,
    };
    use codec::Encode;
    use frame_support::{
        assert_noop, assert_ok, parameter_types,
        traits::{
            fungible::Mutate as _,
            tokens::nonfungibles_v2::{Create, Inspect as _},
            ConstU32,
        },
    };
    use pallet_nfts::{AttributeNamespace, PalletAttributes};
    use sp_runtime::{traits::Convert, DispatchError};

    const GROUP: u16 = 1;
    const OTHER_GROUP: u16 = 2;

    const ALICE: AccountId = AccountId::new([0x0a; 32]);
    const BOB: AccountId = AccountId::new([0x0b; 32]);
    const CHARLIE: AccountId = AccountId::new([0x0c; 32]);

    const RETIRING: AccountId = AccountId::new([0x0d; 32]);

    parameter_types! {
        pub const ManagerAccount: AccountId = AccountId::new([0xee; 32]);
        pub RetirementHolder: Option<AccountId> = Some(RETIRING);
    }

    /// Each group's account is derived from its id, with no read.
    pub struct GroupAccount;
    impl Convert<u16, AccountId> for GroupAccount {
        fn convert(group: u16) -> AccountId {
            let mut account = [0xa0; 32];
            account[..2].copy_from_slice(&group.to_le_bytes());
            AccountId::new(account)
        }
    }

    fn group_account(group: u16) -> AccountId {
        GroupAccount::convert(group)
    }

    type Mgr = GroupCollectionMemberships<
        Memberships,
        pallet_nfts::ItemConfig,
        GroupAccount,
        ManagerAccount,
    >;
    type SmallScanMgr = GroupCollectionMemberships<
        Memberships,
        pallet_nfts::ItemConfig,
        GroupAccount,
        ManagerAccount,
        ConstU32<4>,
    >;
    /// The same memberships, with a retirement holder.
    type RetiringMgr = GroupCollectionMemberships<
        Memberships,
        pallet_nfts::ItemConfig,
        GroupAccount,
        ManagerAccount,
        ConstU32<64>,
        RetirementHolder,
    >;

    /// Two groups, each collection owned and administered by the manager account. `GROUP` has
    /// memberships 1..=`stock` in its stock; `OTHER_GROUP` has membership 1000.
    fn new_test_ext(stock: u32) -> sp_io::TestExternalities {
        let mut ext = sp_io::TestExternalities::new(Default::default());
        ext.execute_with(|| {
            frame_system::Pallet::<Test>::set_block_number(1);
            for group in [GROUP, OTHER_GROUP] {
                assert_ok!(Memberships::create_collection_with_id(
                    group,
                    &ManagerAccount::get(),
                    &ManagerAccount::get(),
                    &Default::default(),
                ));
            }
            for m in 1..=stock {
                assert_ok!(Mgr::issue(&GROUP, &m));
            }
            assert_ok!(Mgr::issue(&OTHER_GROUP, &1000));
        });
        ext
    }

    fn policy(receivers: Receivers, rank: RankOnTransfer) -> TransferPolicy {
        TransferPolicy { receivers, rank }
    }

    fn is_locked(group: u16, m: u32) -> bool {
        !Memberships::can_transfer(&group, &m)
    }

    /// Every attribute an item carries, with its namespace.
    fn item_attributes(group: u16, m: u32) -> Vec<(AttributeNamespace<AccountId>, Vec<u8>)> {
        pallet_nfts::Attribute::<Test>::iter_prefix((group, Some(m)))
            .map(|((namespace, key), _)| (namespace, key.into_inner()))
            .collect()
    }

    fn stock(group: u16) -> Vec<u32> {
        let mut stock = Mgr::group_available_memberships(&group).collect::<Vec<_>>();
        stock.sort();
        stock
    }

    #[test]
    fn issue_puts_locked_memberships_in_the_stock() {
        new_test_ext(2).execute_with(|| {
            assert_eq!(stock(GROUP), vec![1, 2]);
            assert_eq!(Memberships::owner(GROUP, 1), Some(group_account(GROUP)));
            assert!(is_locked(GROUP, 1) && is_locked(GROUP, 2));
            assert_eq!(Mgr::members_total(&GROUP), 0);
            assert_eq!(
                Mgr::manager_account::<AccountId>(),
                Memberships::collection_owner(GROUP).unwrap()
            );
        });
    }

    #[test]
    fn assign_takes_from_the_stock_with_rank_zero() {
        new_test_ext(2).execute_with(|| {
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));

            assert_eq!(Memberships::owner(GROUP, 1), Some(ALICE));
            assert!(is_locked(GROUP, 1));
            assert_eq!(Mgr::rank_of(&GROUP, &1), Some(GenericRank::MIN));
            assert_eq!(Mgr::members_total(&GROUP), 1);
            assert_eq!(stock(GROUP), vec![2]);
            assert!(Mgr::is_member_of(&GROUP, &ALICE));
            assert!(Mgr::holds(&GROUP, &ALICE, &1));
            assert_eq!(Mgr::check_membership(&ALICE, &1), Some(GROUP));
        });
    }

    #[test]
    fn assign_refuses_a_membership_not_in_the_stock() {
        new_test_ext(1).execute_with(|| {
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));
            assert_noop!(Mgr::assign(&GROUP, &1, &BOB), Error::NotInStock);
            assert_noop!(Mgr::assign(&GROUP, &99, &BOB), Error::NotInStock);
            assert_noop!(Mgr::assign(&GROUP, &1000, &BOB), Error::NotInStock);
        });
    }

    #[test]
    fn release_refuses_a_membership_in_the_stock() {
        new_test_ext(1).execute_with(|| {
            assert_noop!(Mgr::release(&GROUP, &1), Error::NotAMember);
            assert_noop!(Mgr::release(&GROUP, &99), Error::NotAMember);
        });
    }

    // AC-F3.1, REQ-MI-4
    #[test]
    fn release_returns_the_membership_to_the_stock_with_its_rank_reset() {
        new_test_ext(2).execute_with(|| {
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));
            assert_ok!(Mgr::assign(&GROUP, &2, &BOB));
            assert_ok!(Mgr::set_rank(&GROUP, &1, 3));
            assert_ok!(Mgr::set_rank(&GROUP, &2, 2));
            assert_eq!(Mgr::ranks_total(&GROUP), 5);

            assert_ok!(Mgr::release(&GROUP, &1));

            assert_eq!(Memberships::owner(GROUP, 1), Some(group_account(GROUP)));
            assert_eq!(stock(GROUP), vec![1]);
            assert!(is_locked(GROUP, 1));
            assert_eq!(Mgr::rank_of(&GROUP, &1), None);
            assert_eq!(Mgr::ranks_total(&GROUP), 2);
            assert_eq!(Mgr::members_total(&GROUP), 1);
            assert!(!Mgr::is_member_of(&GROUP, &ALICE));
            // REQ-MI-4: nothing the manager set for the former holder is left; only the lock.
            assert_eq!(
                item_attributes(GROUP, 1),
                vec![(
                    AttributeNamespace::Pallet,
                    PalletAttributes::<u16>::TransferDisabled.encode()
                )]
            );
            // ... and it can be assigned again, from the stock.
            assert_ok!(Mgr::assign(&GROUP, &1, &CHARLIE));
            assert_eq!(Mgr::rank_of(&GROUP, &1), Some(GenericRank::MIN));
        });
    }

    // AC-F3.2
    #[test]
    fn burn_leaves_no_attribute_on_the_item() {
        new_test_ext(1).execute_with(|| {
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));
            assert_ok!(Mgr::set_rank(&GROUP, &1, 4));
            assert_ok!(Mgr::release(&GROUP, &1));

            assert_ok!(Mgr::burn(&GROUP, &1));

            assert_eq!(Memberships::owner(GROUP, 1), None);
            assert_eq!(item_attributes(GROUP, 1), vec![]);
            assert_eq!(Mgr::ranks_total(&GROUP), 0);
            assert_eq!(stock(GROUP), Vec::<u32>::new());
        });
    }

    // AC-F6.1
    #[test]
    fn a_group_that_never_set_a_policy_reads_disabled_and_reset() {
        new_test_ext(0).execute_with(|| {
            assert_eq!(Mgr::transfer_policy(&GROUP), TransferPolicy::default());
            assert_eq!(
                TransferPolicy::default(),
                policy(Receivers::Disabled, RankOnTransfer::Reset)
            );
        });
    }

    #[test]
    fn set_transfer_policy_is_read_back_for_its_group_only() {
        new_test_ext(0).execute_with(|| {
            let p = policy(Receivers::ToAnyAccount, RankOnTransfer::Keep);
            assert_ok!(Mgr::set_transfer_policy(&GROUP, p));
            assert_eq!(Mgr::transfer_policy(&GROUP), p);
            assert_eq!(
                Mgr::transfer_policy(&OTHER_GROUP),
                TransferPolicy::default()
            );
            // The policy is a system attribute of the collection, in the `Pallet` namespace.
            assert_eq!(
                Memberships::typed_system_attribute(&GROUP, None, &crate::ATTR_TRANSFER_POLICY),
                Some(p)
            );
        });
    }

    // AC-F4.4
    #[test]
    fn the_default_policy_and_disabled_refuse_a_transfer() {
        new_test_ext(2).execute_with(|| {
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));
            assert_ok!(Mgr::assign(&GROUP, &2, &BOB));
            assert_noop!(Mgr::transfer(&GROUP, &1, &BOB), Error::TransferDisabled);

            assert_ok!(Mgr::set_transfer_policy(
                &GROUP,
                policy(Receivers::Disabled, RankOnTransfer::Keep)
            ));
            assert_noop!(Mgr::transfer(&GROUP, &1, &CHARLIE), Error::TransferDisabled);
        });
    }

    // AC-F4.1
    #[test]
    fn transfer_to_an_existing_member_resets_the_rank() {
        new_test_ext(2).execute_with(|| {
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));
            assert_ok!(Mgr::assign(&GROUP, &2, &BOB));
            assert_ok!(Mgr::set_rank(&GROUP, &1, 3));
            assert_ok!(Mgr::set_transfer_policy(
                &GROUP,
                policy(Receivers::ToExistingMembers, RankOnTransfer::Reset)
            ));

            assert_ok!(Mgr::transfer(&GROUP, &1, &BOB));

            assert_eq!(Memberships::owner(GROUP, 1), Some(BOB));
            assert!(is_locked(GROUP, 1));
            assert_eq!(Mgr::rank_of(&GROUP, &1), Some(GenericRank::MIN));
            assert_eq!(Mgr::ranks_total(&GROUP), 0);
            assert_eq!(Mgr::members_total(&GROUP), 2);
            assert!(!Mgr::is_member_of(&GROUP, &ALICE));
            assert_eq!(Mgr::check_membership(&BOB, &1), Some(GROUP));
        });
    }

    // AC-F4.2
    #[test]
    fn transfer_to_existing_members_refuses_a_non_member_and_the_group_account() {
        new_test_ext(2).execute_with(|| {
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));
            assert_ok!(Mgr::set_transfer_policy(
                &GROUP,
                policy(Receivers::ToExistingMembers, RankOnTransfer::Reset)
            ));
            // CHARLIE holds a membership, but of another group.
            assert_ok!(Mgr::assign(&OTHER_GROUP, &1000, &CHARLIE));

            assert_noop!(Mgr::transfer(&GROUP, &1, &BOB), Error::NotSameGroup);
            assert_noop!(Mgr::transfer(&GROUP, &1, &CHARLIE), Error::NotSameGroup);
            assert_noop!(
                Mgr::transfer(&GROUP, &1, &group_account(GROUP)),
                Error::NotSameGroup
            );
        });
    }

    // AC-F4.5
    #[test]
    fn transfer_to_any_account_makes_the_recipient_a_member() {
        new_test_ext(1).execute_with(|| {
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));
            assert_ok!(Mgr::set_transfer_policy(
                &GROUP,
                policy(Receivers::ToAnyAccount, RankOnTransfer::Reset)
            ));

            assert_ok!(Mgr::transfer(&GROUP, &1, &CHARLIE));

            assert_eq!(Memberships::owner(GROUP, 1), Some(CHARLIE));
            assert!(Mgr::is_member_of(&GROUP, &CHARLIE));
            assert_eq!(
                Mgr::user_memberships(&CHARLIE, Some(GROUP)).collect::<Vec<_>>(),
                vec![(GROUP, 1)]
            );
            assert_eq!(Mgr::members_total(&GROUP), 1);
            // Never the group account.
            assert_noop!(
                Mgr::transfer(&GROUP, &1, &group_account(GROUP)),
                Error::NotSameGroup
            );
        });
    }

    // AC-F4.6
    #[test]
    fn a_kept_rank_travels_with_the_membership() {
        new_test_ext(2).execute_with(|| {
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));
            assert_ok!(Mgr::assign(&GROUP, &2, &BOB));
            assert_ok!(Mgr::set_rank(&GROUP, &1, 5));
            assert_ok!(Mgr::set_transfer_policy(
                &GROUP,
                policy(Receivers::ToExistingMembers, RankOnTransfer::Keep)
            ));

            assert_ok!(Mgr::transfer(&GROUP, &1, &BOB));

            assert_eq!(Memberships::owner(GROUP, 1), Some(BOB));
            assert_eq!(Mgr::rank_of(&GROUP, &1), Some(GenericRank::from(5)));
            assert_eq!(Mgr::ranks_total(&GROUP), 5);
            assert_eq!(Mgr::members_total(&GROUP), 2);
        });
    }

    #[test]
    fn transfer_refuses_a_membership_not_held_by_a_member() {
        new_test_ext(1).execute_with(|| {
            assert_ok!(Mgr::set_transfer_policy(
                &GROUP,
                policy(Receivers::ToAnyAccount, RankOnTransfer::Keep)
            ));
            // In the stock.
            assert_noop!(Mgr::transfer(&GROUP, &1, &ALICE), Error::NotAMember);
            // Nonexistent.
            assert_noop!(Mgr::transfer(&GROUP, &99, &ALICE), Error::NotAMember);
        });
    }

    // AC-F4.3, INV-18
    #[test]
    fn memberships_cannot_be_transferred_burnt_or_bought_directly() {
        new_test_ext(2).execute_with(|| {
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));
            let locked: DispatchError = pallet_nfts::Error::<Test>::ItemLocked.into();

            // By the holder, through the pallet's calls.
            assert_noop!(
                Memberships::transfer(RuntimeOrigin::signed(ALICE), GROUP, 1, BOB),
                locked
            );
            assert_noop!(
                Memberships::burn(RuntimeOrigin::signed(ALICE), GROUP, 1),
                locked
            );
            // By a delegate.
            assert_ok!(Memberships::approve_transfer(
                RuntimeOrigin::signed(ALICE),
                GROUP,
                1,
                BOB,
                None
            ));
            assert_noop!(
                Memberships::transfer(RuntimeOrigin::signed(BOB), GROUP, 1, BOB),
                locked
            );
            // By a buyer.
            assert_ok!(Memberships::set_price(
                RuntimeOrigin::signed(ALICE),
                GROUP,
                1,
                Some(1),
                None
            ));
            assert_ok!(Balances::mint_into(&CHARLIE, 1_000));
            assert_noop!(
                Memberships::buy_item(RuntimeOrigin::signed(CHARLIE), GROUP, 1, 1),
                locked
            );
            // By the group account, on its stock.
            assert_noop!(
                Memberships::transfer(RuntimeOrigin::signed(group_account(GROUP)), GROUP, 2, BOB),
                locked
            );
            assert_noop!(
                Memberships::burn(RuntimeOrigin::signed(group_account(GROUP)), GROUP, 2),
                locked
            );
            // Through the trait, by anything but the manager.
            assert_noop!(
                <Memberships as frame_support::traits::tokens::nonfungibles_v2::Transfer<
                    AccountId,
                >>::transfer(&GROUP, &1, &BOB),
                locked
            );

            assert_eq!(Memberships::owner(GROUP, 1), Some(ALICE));
            assert_eq!(Memberships::owner(GROUP, 2), Some(group_account(GROUP)));
        });
    }

    // INV-18
    #[test]
    fn attribute_writes_cannot_unlock_an_item_or_touch_the_rank() {
        new_test_ext(1).execute_with(|| {
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));
            // `pallet-nfts` keys the lock as `PalletAttributes::TransferDisabled`, encoded `[1]`.
            assert_eq!(
                PalletAttributes::<u16>::TransferDisabled.encode(),
                vec![1u8]
            );

            assert_noop!(
                Mgr::clear_membership_attribute(&GROUP, &1, &1u8),
                Error::ReservedAttribute
            );
            assert_noop!(
                Mgr::clear_membership_attribute(&GROUP, &1, &ATTR_MEMBER_RANK),
                Error::ReservedAttribute
            );
            assert_noop!(
                Mgr::set_membership_attribute(&GROUP, &1, &ATTR_MEMBER_RANK, &GenericRank::MAX),
                Error::ReservedAttribute
            );
            assert!(is_locked(GROUP, 1));

            // Other attributes still work, and are read back from the `Pallet` namespace.
            assert_ok!(Mgr::set_membership_attribute(&GROUP, &1, &b"note", &7u32));
            assert_eq!(
                Mgr::membership_attribute::<_, u32>(&GROUP, &1, &b"note"),
                Some(7)
            );
            assert_ok!(Mgr::clear_membership_attribute(&GROUP, &1, &b"note"));
            assert_eq!(
                Mgr::membership_attribute::<_, u32>(&GROUP, &1, &b"note"),
                None
            );
        });
    }

    // INV-18
    #[test]
    fn the_manager_relocks_an_item_that_reached_it_unlocked() {
        new_test_ext(0).execute_with(|| {
            use frame_support::traits::tokens::nonfungibles_v2::Mutate;
            // An issuer that forgot to lock.
            assert_ok!(Memberships::mint_into(
                &GROUP,
                &7,
                &group_account(GROUP),
                &Default::default(),
                false
            ));
            assert!(!is_locked(GROUP, 7));

            assert_ok!(Mgr::assign(&GROUP, &7, &ALICE));
            assert!(is_locked(GROUP, 7));

            // `lock` is idempotent.
            assert_ok!(Mgr::lock(&GROUP, &7));
            assert!(is_locked(GROUP, 7));
        });
    }

    // INV-16
    #[test]
    fn the_manager_writes_no_usage_or_expiration_state_on_an_item() {
        new_test_ext(2).execute_with(|| {
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));
            assert_ok!(Mgr::assign(&GROUP, &2, &BOB));
            assert_ok!(Mgr::set_rank(&GROUP, &1, 2));
            assert_ok!(Mgr::set_transfer_policy(
                &GROUP,
                policy(Receivers::ToExistingMembers, RankOnTransfer::Keep)
            ));
            assert_ok!(Mgr::transfer(&GROUP, &1, &BOB));

            let attributes = item_attributes(GROUP, 1);
            assert_eq!(attributes.len(), 2);
            assert!(attributes
                .iter()
                .all(|(namespace, _)| namespace == &AttributeNamespace::Pallet));
            let mut keys = attributes
                .into_iter()
                .map(|(_, key)| key)
                .collect::<Vec<_>>();
            keys.sort();
            let mut expected = vec![
                PalletAttributes::<u16>::TransferDisabled.encode(),
                ATTR_MEMBER_RANK.encode(),
            ];
            expected.sort();
            assert_eq!(keys, expected);
        });
    }

    // INV-17
    #[test]
    fn memberships_stay_in_their_group_collection() {
        new_test_ext(2).execute_with(|| {
            assert_ok!(Mgr::set_transfer_policy(
                &GROUP,
                policy(Receivers::ToAnyAccount, RankOnTransfer::Reset)
            ));
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));
            assert_ok!(Mgr::transfer(&GROUP, &1, &BOB));
            assert_ok!(Mgr::release(&GROUP, &1));
            assert_ok!(Mgr::assign(&GROUP, &2, &CHARLIE));

            for (m, holder) in [(1, group_account(GROUP)), (2, CHARLIE)] {
                assert_eq!(Memberships::owner(GROUP, m), Some(holder));
                assert_eq!(Memberships::owner(OTHER_GROUP, m), None);
            }
            let mut items = pallet_nfts::Item::<Test>::iter_key_prefix(GROUP).collect::<Vec<_>>();
            items.sort();
            assert_eq!(items, vec![1, 2]);
        });
    }

    // REQ-MI-13
    #[test]
    fn the_group_account_is_never_a_member_of_its_own_group() {
        new_test_ext(2).execute_with(|| {
            let account = group_account(GROUP);
            // GROUP's account is a member of OTHER_GROUP.
            assert_ok!(Mgr::assign(&OTHER_GROUP, &1000, &account));

            assert!(!Mgr::is_member_of(&GROUP, &account));
            assert!(!Mgr::holds(&GROUP, &account, &1));
            assert_eq!(Mgr::user_memberships(&account, Some(GROUP)).count(), 0);
            assert_eq!(Mgr::memberships_of(&account, Some(GROUP)).count(), 0);
            assert_eq!(
                Mgr::user_memberships(&account, None).collect::<Vec<_>>(),
                vec![(OTHER_GROUP, 1000)]
            );
            assert_eq!(Mgr::check_membership(&account, &1), None);
            assert_eq!(Mgr::check_membership(&account, &1000), Some(OTHER_GROUP));
            assert!(Mgr::is_member_of(&OTHER_GROUP, &account));

            assert_noop!(Mgr::assign(&GROUP, &1, &account), Error::NotSameGroup);
            assert_eq!(Mgr::members_total(&GROUP), 0);
        });
    }

    /// A state where ALICE holds 200 memberships of `GROUP`, committed so reads can be proven.
    fn alice_holds_200() -> sp_io::TestExternalities {
        let mut ext = new_test_ext(200);
        ext.execute_with(|| {
            for m in 1..=200 {
                assert_ok!(Mgr::assign(&GROUP, &m, &ALICE));
            }
        });
        ext.commit_all().unwrap();
        ext
    }

    // CTR-MEM-1
    #[test]
    fn the_validity_check_reads_a_bounded_number_of_items() {
        let mut ext = alice_holds_200();

        let (member, member_proof) = ext.execute_and_prove(|| Mgr::is_member_of(&GROUP, &ALICE));
        let (holds, holds_proof) = ext.execute_and_prove(|| Mgr::holds(&GROUP, &ALICE, &1));
        let (all, every) =
            ext.execute_and_prove(|| Mgr::user_memberships(&ALICE, Some(GROUP)).count());

        assert!(member && holds);
        assert_eq!(all, 200);
        // Each check reads one item, not the 200 ALICE holds.
        for check in [member_proof, holds_proof] {
            assert!(
                check.encoded_size() * 4 < every.encoded_size(),
                "{} vs {}",
                check.encoded_size(),
                every.encoded_size()
            );
        }
    }

    // CTR-MEM-2
    #[test]
    fn enumerations_are_lazy_and_deterministic() {
        let mut ext = alice_holds_200();

        let (first, few) = ext.execute_and_prove(|| {
            Mgr::user_memberships(&ALICE, None)
                .take(3)
                .collect::<Vec<_>>()
        });
        let (all, every) =
            ext.execute_and_prove(|| Mgr::user_memberships(&ALICE, None).collect::<Vec<_>>());
        let (again, _) = ext.execute_and_prove(|| {
            Mgr::user_memberships(&ALICE, None)
                .take(3)
                .collect::<Vec<_>>()
        });

        assert_eq!(first.len(), 3);
        assert_eq!(all.len(), 200);
        assert_eq!(first, again);
        assert_eq!(first[..], all[..3]);
        // Taking 3 reads a fraction of what taking all 200 does.
        assert!(
            few.encoded_size() * 4 < every.encoded_size(),
            "{} vs {}",
            few.encoded_size(),
            every.encoded_size()
        );
    }

    #[test]
    fn check_membership_reads_at_most_max_check_scan_items() {
        let mut ext = new_test_ext(40);
        ext.execute_with(|| {
            for m in 1..=40 {
                assert_ok!(Mgr::assign(&GROUP, &m, &ALICE));
            }
            // The first item the scan meets is found; with a bound of 4, some are not.
            let first = Mgr::user_memberships(&ALICE, None).next().unwrap().1;
            assert_eq!(SmallScanMgr::check_membership(&ALICE, &first), Some(GROUP));
            let found = (1..=40)
                .filter(|m| SmallScanMgr::check_membership(&ALICE, m).is_some())
                .count();
            assert_eq!(found, 4);
            // The default bound covers them all.
            assert!((1..=40).all(|m| Mgr::check_membership(&ALICE, &m) == Some(GROUP)));
        });
        ext.commit_all().unwrap();

        let (_, small) = ext.execute_and_prove(|| SmallScanMgr::check_membership(&ALICE, &999));
        let (_, large) = ext.execute_and_prove(|| Mgr::check_membership(&ALICE, &999));
        assert!(small.encoded_size() < large.encoded_size());
    }

    #[test]
    fn set_rank_refuses_a_membership_in_the_stock() {
        new_test_ext(1).execute_with(|| {
            assert_noop!(Mgr::set_rank(&GROUP, &1, 1), Error::NotAMember);
        });
    }

    #[test]
    fn errors_round_trip_through_dispatch_error() {
        for e in [
            Error::NotAMember,
            Error::TransferDisabled,
            Error::NotSameGroup,
            Error::NotInStock,
            Error::ReservedAttribute,
            Error::NotRetirementHolder,
            Error::Assigned,
        ] {
            assert_eq!(Error::try_from(DispatchError::from(e)), Ok(e));
        }
        assert_eq!(
            Error::try_from(DispatchError::BadOrigin),
            Err(DispatchError::BadOrigin)
        );
        assert_eq!(
            Error::try_from(DispatchError::Other("Unrelated")),
            Err(DispatchError::Other("Unrelated"))
        );
    }

    #[test]
    fn with_hooks_forwards_transfers() {
        type Hooked = crate::WithHooks<Mgr>;
        new_test_ext(2).execute_with(|| {
            assert_ok!(Hooked::assign(&GROUP, &1, &ALICE));
            assert_ok!(Hooked::assign(&GROUP, &2, &BOB));
            assert_ok!(Hooked::set_transfer_policy(
                &GROUP,
                policy(Receivers::ToExistingMembers, RankOnTransfer::Reset)
            ));
            assert_eq!(
                Hooked::transfer_policy(&GROUP),
                policy(Receivers::ToExistingMembers, RankOnTransfer::Reset)
            );
            assert_ok!(Hooked::transfer(&GROUP, &1, &BOB));
            assert!(Hooked::holds(&GROUP, &BOB, &1));
        });
    }

    // REQ-MI-D2
    #[test]
    fn retire_takes_a_membership_out_of_the_stock_without_counting_a_member() {
        new_test_ext(2).execute_with(|| {
            assert_ok!(RetiringMgr::assign(&GROUP, &1, &ALICE));
            assert_ok!(RetiringMgr::set_rank(&GROUP, &1, 3));

            assert_ok!(RetiringMgr::retire(&GROUP, &2, &RETIRING));

            assert_eq!(Memberships::owner(GROUP, 2), Some(RETIRING));
            assert!(is_locked(GROUP, 2));
            assert_eq!(stock(GROUP), Vec::<u32>::new());
            assert_eq!(RetiringMgr::members_total(&GROUP), 1);
            assert_eq!(RetiringMgr::ranks_total(&GROUP), 3);
            assert_eq!(RetiringMgr::rank_of(&GROUP, &2), None);
            // Not assignable from then on.
            assert_noop!(RetiringMgr::assign(&GROUP, &2, &BOB), Error::NotInStock);
        });
    }

    // REQ-MI-D2
    #[test]
    fn retire_refuses_a_membership_not_in_the_stock_and_any_other_holder() {
        new_test_ext(2).execute_with(|| {
            assert_ok!(RetiringMgr::assign(&GROUP, &1, &ALICE));

            // An assigned membership, one of another group, and one that doesn't exist.
            assert_noop!(
                RetiringMgr::retire(&GROUP, &1, &RETIRING),
                Error::NotInStock
            );
            assert_noop!(
                RetiringMgr::retire(&GROUP, &1000, &RETIRING),
                Error::NotInStock
            );
            assert_noop!(
                RetiringMgr::retire(&GROUP, &99, &RETIRING),
                Error::NotInStock
            );
            // Only to the retirement holder; a manager with none retires nothing.
            assert_noop!(
                RetiringMgr::retire(&GROUP, &2, &BOB),
                Error::NotRetirementHolder
            );
            assert_noop!(
                Mgr::retire(&GROUP, &2, &RETIRING),
                Error::NotRetirementHolder
            );
        });
    }

    // REQ-MI-D2
    #[test]
    fn the_retirement_holder_is_never_a_member() {
        new_test_ext(3).execute_with(|| {
            assert_ok!(RetiringMgr::assign(&GROUP, &1, &ALICE));
            assert_ok!(RetiringMgr::retire(&GROUP, &2, &RETIRING));
            assert_ok!(RetiringMgr::set_transfer_policy(
                &GROUP,
                policy(Receivers::ToAnyAccount, RankOnTransfer::Keep)
            ));

            assert!(!RetiringMgr::is_member_of(&GROUP, &RETIRING));
            assert!(!RetiringMgr::holds(&GROUP, &RETIRING, &2));
            assert_eq!(
                RetiringMgr::user_memberships(&RETIRING, Some(GROUP)).count(),
                0
            );
            assert_eq!(RetiringMgr::user_memberships(&RETIRING, None).count(), 0);
            assert_eq!(RetiringMgr::memberships_of(&RETIRING, None).count(), 0);
            assert_eq!(RetiringMgr::check_membership(&RETIRING, &2), None);

            // It cannot be released back to the stock, transferred, or given a membership.
            assert_noop!(RetiringMgr::release(&GROUP, &2), Error::NotAMember);
            assert_noop!(RetiringMgr::transfer(&GROUP, &2, &BOB), Error::NotAMember);
            assert_noop!(
                RetiringMgr::transfer(&GROUP, &1, &RETIRING),
                Error::NotSameGroup
            );
            assert_noop!(
                RetiringMgr::assign(&GROUP, &3, &RETIRING),
                Error::NotSameGroup
            );
            assert_eq!(RetiringMgr::members_total(&GROUP), 1);
        });
    }

    // REQ-MI-D2
    #[test]
    fn burn_refuses_an_assigned_membership() {
        new_test_ext(3).execute_with(|| {
            assert_ok!(RetiringMgr::assign(&GROUP, &1, &ALICE));
            assert_ok!(RetiringMgr::set_rank(&GROUP, &1, 2));

            assert_noop!(RetiringMgr::burn(&GROUP, &1), Error::Assigned);
            assert_noop!(Mgr::burn(&GROUP, &1), Error::Assigned);
            assert_eq!(Memberships::owner(GROUP, 1), Some(ALICE));
            assert_eq!(RetiringMgr::rank_of(&GROUP, &1), Some(GenericRank::from(2)));

            // A retiring membership and one in the stock are burnt.
            assert_ok!(RetiringMgr::retire(&GROUP, &2, &RETIRING));
            assert_ok!(RetiringMgr::burn(&GROUP, &2));
            assert_ok!(RetiringMgr::burn(&GROUP, &3));
            assert_eq!(Memberships::owner(GROUP, 2), None);
            assert_eq!(Memberships::owner(GROUP, 3), None);
            assert_eq!(item_attributes(GROUP, 2), vec![]);
            assert_eq!(RetiringMgr::members_total(&GROUP), 1);
        });
    }

    // REQ-MI-4
    #[test]
    fn the_manager_clears_only_its_own_attributes() {
        // Documented limit: an attribute set through `Attributes` outlives a release, a transfer
        // and a burn; whoever sets it clears it.
        new_test_ext(1).execute_with(|| {
            assert_ok!(Mgr::assign(&GROUP, &1, &ALICE));
            assert_ok!(Mgr::set_membership_attribute(&GROUP, &1, &b"note", &7u32));
            assert_ok!(Mgr::set_transfer_policy(
                &GROUP,
                policy(Receivers::ToAnyAccount, RankOnTransfer::Reset)
            ));

            assert_ok!(Mgr::transfer(&GROUP, &1, &BOB));
            assert_ok!(Mgr::release(&GROUP, &1));
            assert_eq!(
                Mgr::membership_attribute::<_, u32>(&GROUP, &1, &b"note"),
                Some(7)
            );

            assert_ok!(Mgr::burn(&GROUP, &1));
            assert_eq!(
                item_attributes(GROUP, 1),
                vec![(AttributeNamespace::Pallet, b"note".encode())]
            );
        });
    }
}
