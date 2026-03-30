pub use crate::{types::*, Config};
use frame::{
    deps::{
        frame_support::{derive_impl, parameter_types, traits::AsEnsureOriginWithArg},
        frame_system::{EnsureRoot, EnsureSigned},
        sp_runtime::BuildStorage,
    },
    testing_prelude::*,
};

pub type AccountId = u64;
pub type Balance = u64;
pub type AssetId = u32;
pub type CommunityId = u64;

// Test accounts
pub const COMMUNITY_1_ADMIN: AccountId = 10;
pub const MEMBER_1A: AccountId = 100; // community 1
pub const MEMBER_1B: AccountId = 101; // community 1
pub const MEMBER_2A: AccountId = 200; // community 2
pub const NO_COMMUNITY: AccountId = 300;
pub const FEE_RECEIVER_PROTOCOL: AccountId = 50;
pub const FEE_RECEIVER_COMMUNITY: AccountId = 51;
pub const ASSET_ADMIN: AccountId = 3;

pub const ASSET_ID: AssetId = 1;
pub const INITIAL_BALANCE: Balance = 10_000;

#[frame_construct_runtime]
pub mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeTask,
        RuntimeHoldReason,
        RuntimeFreezeReason
    )]
    pub struct Test;

    #[runtime::pallet_index(0)]
    pub type System = frame_system;
    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type Assets = pallet_assets;
    #[runtime::pallet_index(20)]
    pub type Fees = crate;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = MockBlock<Test>;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

#[derive_impl(pallet_assets::config_preludes::TestDefaultConfig)]
impl pallet_assets::Config for Test {
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type ForceOrigin = EnsureRoot<AccountId>;
    type Freezer = ();
    type Holder = ();
}

/// Dummy community detector for tests.
pub struct DummyAccountCommunity;
impl AccountCommunity<AccountId, CommunityId> for DummyAccountCommunity {
    fn community_of(who: &AccountId) -> Option<CommunityId> {
        match who {
            100..=199 => Some(1), // community 1
            200..=299 => Some(2), // community 2
            _ => None,
        }
    }
}

/// Dummy call inspector that detects pallet-assets transfer calls.
pub struct MockCallInspector;
impl CallInspector<RuntimeCall, AssetId, Balance> for MockCallInspector {
    fn extract_asset_transfer(call: &RuntimeCall) -> Option<(AssetId, Balance)> {
        match call {
            RuntimeCall::Assets(pallet_assets::Call::transfer { id, amount, .. }) => {
                Some((*id, *amount))
            }
            RuntimeCall::Assets(pallet_assets::Call::transfer_keep_alive {
                id, amount, ..
            }) => Some((*id, *amount)),
            _ => None,
        }
    }
}

parameter_types! {
    pub const MaxFeeNameLen: u32 = 64;
    pub const MaxProtocolFees: u32 = 10;
    pub const MaxCommunityFees: u32 = 10;
}

impl Config for Test {
    type CommunityId = CommunityId;
    type MaxFeeNameLen = MaxFeeNameLen;
    type MaxProtocolFees = MaxProtocolFees;
    type MaxCommunityFees = MaxCommunityFees;
    type AdminOrigin = EnsureRoot<AccountId>;
    type CommunityOrigin = EnsureSigned<AccountId>; // signer = community id for tests
    type CommunityDetector = DummyAccountCommunity;
    type Assets = Assets;
    type CallInspector = MockCallInspector;
}

pub(crate) fn new_test_ext() -> TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (MEMBER_1A, INITIAL_BALANCE),
            (MEMBER_1B, INITIAL_BALANCE),
            (MEMBER_2A, INITIAL_BALANCE),
            (NO_COMMUNITY, INITIAL_BALANCE),
            (FEE_RECEIVER_PROTOCOL, INITIAL_BALANCE),
            (FEE_RECEIVER_COMMUNITY, INITIAL_BALANCE),
            (ASSET_ADMIN, INITIAL_BALANCE),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_assets::GenesisConfig::<Test> {
        assets: vec![(ASSET_ID, ASSET_ADMIN, true, 1)],
        metadata: vec![(ASSET_ID, "Test Token".into(), "TEST".into(), 0)],
        accounts: vec![
            (ASSET_ID, MEMBER_1A, INITIAL_BALANCE),
            (ASSET_ID, MEMBER_1B, INITIAL_BALANCE),
            (ASSET_ID, MEMBER_2A, INITIAL_BALANCE),
            (ASSET_ID, NO_COMMUNITY, INITIAL_BALANCE),
            (ASSET_ID, FEE_RECEIVER_PROTOCOL, 0),
            (ASSET_ID, FEE_RECEIVER_COMMUNITY, 0),
        ],
        next_asset_id: None,
        reserves: vec![],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
