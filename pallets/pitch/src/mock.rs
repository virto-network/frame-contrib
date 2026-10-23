pub use crate::{self as fc_pallet_pitch, *};
use frame::{
    deps::{frame_support::parameter_types, sp_core::H256, sp_runtime::BuildStorage},
    testing_prelude::*,
};

pub type AccountId = u64;
pub type BlockNumber = u64;
pub type CommunityId = u32;

pub const HOLDER: AccountId = 1;
pub const MEMBER: AccountId = 2;
pub const OUTSIDER: AccountId = 3;
pub const CELL: u64 = 0x87283082bffffff;
pub const ROOT: H256 = H256::repeat_byte(42);

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
    #[runtime::pallet_index(20)]
    pub type Pitch = fc_pallet_pitch;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = MockBlock<Test>;
    type AccountData = pallet_balances::AccountData<u64>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

pub struct TestVerifier;
impl MembershipVerifier<AccountId, H256> for TestVerifier {
    fn verify(who: &AccountId, root: &H256, proof: &[u8]) -> bool {
        *who == MEMBER && *root == ROOT && proof == b"member"
    }
}

parameter_types! {
    pub PitchPalletId: PalletId = PalletId(*b"pitch___");
    pub const MaxPitchesPerCell: u32 = 4;
    pub const MaxProofLen: u32 = 32;
    pub const MaxResidueLen: u32 = 128;
    pub const GracePeriod: BlockNumber = 2;
}

impl Config for Test {
    type WeightInfo = ();
    type CommunityId = CommunityId;
    type Verifier = TestVerifier;
    type PalletId = PitchPalletId;
    type MaxPitchesPerCell = MaxPitchesPerCell;
    type MaxProofLen = MaxProofLen;
    type MaxResidueLen = MaxResidueLen;
    type GracePeriod = GracePeriod;
}

pub fn new_test_ext() -> TestExternalities {
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = TestExternalities::new(storage);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
