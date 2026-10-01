//! Test environment for pallet-gas-burn

use crate::ChargeTransactionPayment;
pub use crate::{self as fc_pallet_gas_transaction_payment, Config};
use frame::{
    deps::{
        frame_support::{derive_impl, dispatch::DispatchClass},
        frame_system::limits::BlockWeights,
        sp_runtime::{self, Perbill},
    },
    storage_alias,
    testing_prelude::*,
};
use frame_contrib_traits::gas_tank::GasBurner;
use frame_system::mocking::MockUncheckedExtrinsic;

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
    pub type GasTransactionPayment = fc_pallet_gas_transaction_payment;
    #[runtime::pallet_index(12)]
    pub type TransactionPayment = pallet_transaction_payment;
}

pub type TxExtensions =
    ChargeTransactionPayment<Test, pallet_transaction_payment::ChargeTransactionPayment<Test>>;
pub type UncheckedExtrinsic = MockUncheckedExtrinsic<Test, (), TxExtensions>;
pub type CheckedExtrinsic =
    sp_runtime::generic::CheckedExtrinsic<AccountId, RuntimeCall, TxExtensions>;
pub type Block =
    sp_runtime::generic::Block<sp_runtime::generic::Header<u64, BlakeTwo256>, UncheckedExtrinsic>;

pub type AccountId = <Test as frame_system::Config>::AccountId;
pub type Balance = <Test as pallet_balances::Config>::Balance;

/// The base weight of every extrinsic, small so tests can state metered weights exactly.
pub const BASE_EXTRINSIC: Weight = Weight::from_parts(5, 0);
/// The proof-size component of every tank set up by [`new_test_ext`].
pub const TANK_PROOF_SIZE: u64 = 1 << 20;

parameter_types! {
    pub MockBlockWeights: BlockWeights = {
        let mut weights = BlockWeights::with_sensible_defaults(
            Weight::from_parts(2_000_000_000_000, u64::MAX),
            Perbill::from_percent(75),
        );
        for class in DispatchClass::all() {
            weights.per_class.get_mut(*class).base_extrinsic = BASE_EXTRINSIC;
        }
        weights
    };
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type BlockWeights = MockBlockWeights;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<AccountId>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

#[derive_impl(pallet_transaction_payment::config_preludes::TestDefaultConfig)]
impl pallet_transaction_payment::Config for Test {
    type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
    type WeightToFee = FixedFee<1, Balance>;
    type LengthToFee = FixedFee<0, Balance>;
}

#[storage_alias]
pub type Tank = StorageMap<Prefix, Blake2_128, AccountId, Weight>;

/// Dummy gas burner. This is a benevolent burner, which means it'll always burn the least amount of
/// remarks for given expected and actual used gas.
pub struct DummyGasBurner;

impl GasBurner for DummyGasBurner {
    type AccountId = AccountId;
    type Gas = Weight;

    fn check_available_gas(who: &Self::AccountId, estimated: &Self::Gas) -> Option<Self::Gas> {
        Tank::get(who).and_then(|remaining| remaining.checked_sub(estimated))
    }

    fn burn_gas(who: &Self::AccountId, expected: &Self::Gas, used: &Self::Gas) -> Self::Gas {
        Tank::mutate(who, |gas| {
            let remaining = gas
                .and_then(|remaining| remaining.checked_sub(used))
                .unwrap_or_default()
                .max(*expected);
            *gas = Some(remaining);
            remaining
        })
    }
}

impl Config for Test {
    type WeightInfo = ();
    type GasTank = DummyGasBurner;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = Self;
}

#[cfg(feature = "runtime-benchmarks")]
impl fc_pallet_gas_transaction_payment::BenchmarkHelper<Test> for Test {
    type Ext = pallet_transaction_payment::ChargeTransactionPayment<Test>;

    fn ext() -> ChargeTransactionPayment<Test, Self::Ext> {
        TxExtensions::new(pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0))
    }

    fn setup_account(who: &AccountId, gas: Weight) -> DispatchResult {
        Tank::mutate(who, |tank| {
            *tank = Some(tank.unwrap_or_default().saturating_add(gas))
        });
        Ok(())
    }
}

/// Gives each account a tank of `ref_time` (and [`TANK_PROOF_SIZE`] of proof size).
pub fn new_test_ext(tank: Vec<(AccountId, u64)>) -> TestExternalities {
    let mut ext = TestExternalities::new(Default::default());
    ext.execute_with(|| {
        tank.iter().for_each(|(who, ref_time)| {
            Tank::insert(who, Weight::from_parts(*ref_time, TANK_PROOF_SIZE));
        });
        System::set_block_number(1);
    });
    ext
}
