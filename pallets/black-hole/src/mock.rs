pub use crate as fc_pallet_black_hole;

use frame::{
    deps::sp_runtime::{traits::Verify, MultiSignature},
    testing_prelude::*,
};

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
    pub type BlackHole = fc_pallet_black_hole;
}

pub type Block = MockBlock<Test>;
pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub type Balance = <Test as pallet_balances::Config>::Balance;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type Lookup = IdentityLookup<Self::AccountId>;
    type AccountId = AccountId;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

parameter_types! {
    pub BlackHolePalletId: PalletId = PalletId(*b"burn____");
}

impl fc_pallet_black_hole::Config for Test {
    type WeightInfo = ();
    type EventHorizonDispatchOrigin = EnsureRoot<AccountId>;
    type Balances = Balances;
    type BlockNumberProvider = System;
    type PalletId = BlackHolePalletId;
    type BurnPeriod = ConstU64<10>;
}

pub fn new_test_ext() -> TestExternalities {
    let mut t = TestExternalities::default();
    t.execute_with(|| System::set_block_number(1));
    t
}

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        BlackHole::on_idle(System::block_number(), Weight::MAX);
        System::on_finalize(System::block_number());

        System::set_block_number(System::block_number() + 1);

        System::on_initialize(System::block_number());
        BlackHole::on_initialize(System::block_number());
    }
}
