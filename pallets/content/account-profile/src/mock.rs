//! Test environment for the account-profile pallet.

use crate::{self as pallet_account_profile, Config};
use fc_pallet_content::{self as pallet_content};
use frame_support::derive_impl;
use sp_io::TestExternalities;

pub type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Content: pallet_content,
        AccountProfile: pallet_account_profile,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

#[derive_impl(
    pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig
)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

impl pallet_content::Config for Test {
    type WeightInfo = ();
}

impl Config for Test {
    type ContentStore = pallet_content::Pallet<Test>;
    type WeightInfo = ();
}

pub fn new_test_ext() -> TestExternalities {
    let mut ext = TestExternalities::new(Default::default());
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}
