//! Test environment for pallet-gas-burn

pub use crate as fc_pallet_gas_transaction_payment;
use crate::{Config, GasBurner};
use frame_support::{
    derive_impl, parameter_types, storage_alias,
    weights::{FixedFee, Weight},
    Blake2_128,
};
use frame_system::{mocking::MockUncheckedExtrinsic, WeightInfo};
use sp_io::TestExternalities;

#[frame_support::runtime]
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
    #[runtime::pallet_index(1)]
    pub type Utility = pallet_utility;

    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type GasTransactionPayment = fc_pallet_gas_transaction_payment;
    #[runtime::pallet_index(12)]
    pub type TransactionPayment = pallet_transaction_payment;
}

pub type SignedExtra = fc_pallet_gas_transaction_payment::ChargeTransactionPayment<
    Test,
    pallet_transaction_payment::ChargeTransactionPayment<Test>,
>;
pub type UncheckedExtrinsic = MockUncheckedExtrinsic<Test, (), SignedExtra>;
pub type CheckedExtrinsic =
    sp_runtime::generic::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra>;
pub type Block = sp_runtime::generic::Block<
    sp_runtime::generic::Header<u64, sp_runtime::traits::BlakeTwo256>,
    UncheckedExtrinsic,
>;

pub type AccountId = <Test as frame_system::Config>::AccountId;
pub type Balance = <Test as pallet_balances::Config>::Balance;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<AccountId>;
}

/// Test weights for `utility` pallet.
///
/// These weights are intentionally set as units of remarks, to ease
/// up counting the amount of _"remarks called"_ in tests.
pub struct TestUtilityWeightInfo;
impl pallet_utility::WeightInfo for TestUtilityWeightInfo {
    fn batch(_: u32) -> Weight {
        RemarkUnit::get()
    }

    fn as_derivative() -> Weight {
        RemarkUnit::get()
    }

    fn batch_all(_: u32) -> Weight {
        RemarkUnit::get()
    }

    fn dispatch_as() -> Weight {
        RemarkUnit::get()
    }

    fn force_batch(_: u32) -> Weight {
        RemarkUnit::get()
    }
}

impl pallet_utility::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PalletsOrigin = OriginCaller;
    type WeightInfo = TestUtilityWeightInfo;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

#[derive_impl(pallet_transaction_payment::config_preludes::TestDefaultConfig as pallet_transaction_payment::DefaultConfig)]
impl pallet_transaction_payment::Config for Test {
    type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
    type WeightToFee = FixedFee<1, Balance>;
    type LengthToFee = FixedFee<0, Balance>;
}

/// Dummy gas burner. Looks up the account and check how many `Remark`s (regardless of the size of the
/// remark) the account has left.
///
/// This is a benevolent burner, which means it'll always burn the least amount of remarks for
/// given expected and actual used gas.
pub struct RemarksBurner;

parameter_types! {
  pub RemarkUnit: Weight = frame_system::weights::SubstrateWeight::<Test>::remark(11);
}

#[storage_alias]
pub type Tank = StorageMap<Prefix, Blake2_128, AccountId, u64>;

impl GasBurner for RemarksBurner {
    type Gas = Weight;
    type AccountId = AccountId;

    fn check_available_gas(who: &Self::AccountId, estimated: &Self::Gas) -> Option<Self::Gas> {
        if let Some(remaining_remarks) = Tank::get(who) {
            let remark_unit = RemarkUnit::get();
            let required_remarks = estimated
                .checked_div_per_component(&remark_unit)
                .unwrap_or_default();

            if required_remarks <= remaining_remarks {
                Some(RemarkUnit::get().mul(remaining_remarks.saturating_sub(required_remarks)))
            } else {
                None
            }
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
