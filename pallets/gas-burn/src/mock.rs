//! Test environment for pallet-gas-burn

use crate::{self as fc_pallet_gas_burn, Config, GasBurner};
use frame_support::{
    derive_impl, parameter_types, storage_alias,
    weights::{IdentityFee, Weight},
    Blake2_128,
};
use frame_system::WeightInfo;
use sp_io::TestExternalities;

pub type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
  pub enum Test
  {
    System: frame_system,
    Balances: pallet_balances,
    GasBurn: fc_pallet_gas_burn,
    TransactionPayment: pallet_transaction_payment,
  }
);

pub type AccountId = <Test as frame_system::Config>::AccountId;
pub type Balance = <Test as pallet_balances::Config>::Balance;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<AccountId>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

#[derive_impl(pallet_transaction_payment::config_preludes::TestDefaultConfig as pallet_transaction_payment::DefaultConfig)]
impl pallet_transaction_payment::Config for Test {
    type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = IdentityFee<Balance>;
}

/// Dummy gas burner. Looks up the account and check how many `Remark`s (regardless of the size of the
/// remark) the account has left.
///
/// This is a benevolent burner, which means it'll always burn the least amount of remarks for
/// given expected and actual used gas.
pub struct RemarksBurner;

parameter_types! {
  pub RemarkUnit: Weight = frame_system::weights::SubstrateWeight::<Test>::remark(0);
}

#[storage_alias]
pub type Tank = StorageMap<Prefix, Blake2_128, AccountId, u64>;

impl GasBurner for RemarksBurner {
    type Gas = Weight;
    type AccountId = AccountId;

    fn check_available_gas(who: &Self::AccountId, estimated: &Self::Gas) -> Option<Self::Gas> {
        if let Some(remaining_remarks) = Tank::get(who) {
            estimated
                .checked_div_per_component(&RemarkUnit::get())
                .map(|remarks| RemarkUnit::get().mul(remaining_remarks.saturating_sub(remarks)))
        } else {
            None
        }
    }

    fn burn_gas(who: &Self::AccountId, expected: &Self::Gas, used: &Self::Gas) -> Self::Gas {
        Tank::mutate(who, |remarks| {
            let expected_remaining_remarks = expected
                .checked_div_per_component(&RemarkUnit::get())
                .unwrap_or_default();

            let remaining_remarks = remarks
                .unwrap_or_default()
                .saturating_sub(
                    used.checked_div_per_component(&RemarkUnit::get())
                        .unwrap_or_default(),
                )
                // For some "weird" reason, this is a benevolent burner
                .max(expected_remaining_remarks);

            *remarks = Some(remaining_remarks);

            RemarkUnit::get().mul(remaining_remarks)
        })
    }
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type GasBurner = RemarksBurner;
}

pub type SignedExtension = fc_pallet_gas_burn::ChargeTxBurningGas<
    Test,
    pallet_transaction_payment::ChargeTransactionPayment<Test>,
>;

pub fn new_test_ext(tank: Vec<(AccountId, u64)>) -> sp_io::TestExternalities {
    let mut ext = TestExternalities::new(Default::default());
    ext.execute_with(|| {
        tank.iter().for_each(|(who, remarks)| {
            Tank::insert(who, remarks);
        });
        System::set_block_number(1);
    });
    ext
}
