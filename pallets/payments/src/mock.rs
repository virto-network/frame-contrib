pub use crate::{self as pallet_payments, types::*, Config};
use frame_support::{
    derive_impl, parameter_types,
    traits::{
        AsEnsureOriginWithArg, ConstU32, ConstU64, EqualPrivilegeOnly, OnFinalize, OnInitialize,
    },
    weights::Weight,
    PalletId,
};

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_system::{EnsureRoot, EnsureSigned};
use scale_info::TypeInfo;
use sp_keystore::{testing::MemoryKeystore, KeystoreExt};
use sp_runtime::{BoundedVec, BuildStorage, Percent};

type Block = frame_system::mocking::MockBlock<Test>;
type AccountId = u64;
#[allow(unused)]
type AssetId = u32;

#[derive(
    Clone,
    Copy,
    Debug,
    Decode,
    DecodeWithMemTracking,
    Encode,
    Eq,
    MaxEncodedLen,
    PartialEq,
    TypeInfo,
)]
pub struct PaymentId(pub u32);

pub const SENDER_ACCOUNT: AccountId = 10;
pub const PAYMENT_BENEFICIARY: AccountId = 11;
pub const ASSET_ADMIN_ACCOUNT: AccountId = 3;
pub const ROOT_ACCOUNT: AccountId = 1;

pub const ASSET_ID: u32 = 1;
pub const INCENTIVE_PERCENTAGE: u8 = 10;
pub const MARKETPLACE_FEE_PERCENTAGE: u8 = 15;
pub const INITIAL_BALANCE: u64 = 100;
pub const PAYMENT_ID: PaymentId = PaymentId(1);

pub const FEE_SENDER_ACCOUNT: AccountId = 30;
pub const FEE_BENEFICIARY_ACCOUNT: AccountId = 31;
pub const FEE_SYSTEM_ACCOUNT: AccountId = 32;

pub const SYSTEM_FEE: u64 = 3;
pub const EXPECTED_SYSTEM_TOTAL_FEE: u64 = 6;
pub const EXPECTED_SYSTEM_SENDER_FEE: u64 = 3; // 15% of 20

pub const FEE_SENDER_AMOUNT: Balance = 2;
pub const FEE_BENEFICIARY_AMOUNT: Balance = 3;
pub const PAYMENT_AMOUNT: u64 = 20;
pub const INCENTIVE_AMOUNT: u64 = PAYMENT_AMOUNT / INCENTIVE_PERCENTAGE as u64;

// Configure a mock runtime to test the pallet.
#[frame_support::runtime]
mod runtime {
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
    #[runtime::pallet_index(5)]
    pub type Scheduler = pallet_scheduler;
    #[runtime::pallet_index(6)]
    pub type Preimage = pallet_preimage;

    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type Assets = pallet_assets;
    #[runtime::pallet_index(12)]
    pub type AssetsHolder = pallet_assets_holder;

    #[runtime::pallet_index(20)]
    pub type Payments = pallet_payments;
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
    pub MaxWeight: Weight = Weight::from_parts(2_000_000_000_000, u64::MAX);
}

pub type Balance = <Test as pallet_balances::Config>::Balance;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig
)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

#[derive_impl(pallet_assets::config_preludes::TestDefaultConfig as pallet_assets::DefaultConfig)]
impl pallet_assets::Config for Test {
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<u64>>;
    type ForceOrigin = EnsureRoot<u64>;
    type Freezer = ();
    type Holder = AssetsHolder;
}

impl pallet_assets_holder::Config for Test {
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeEvent = RuntimeEvent;
}

impl pallet_preimage::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<u64>;
    type Consideration = ();
    type WeightInfo = ();
}

impl pallet_scheduler::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = MaxWeight;
    type ScheduleOrigin = EnsureRoot<u64>;
    type OriginPrivilegeCmp = EqualPrivilegeOnly;
    type MaxScheduledPerBlock = ConstU32<100>;
    type WeightInfo = ();
    type Preimages = Preimage;
    type BlockNumberProvider = System;
}

pub struct MockFeeHandler;

const MANDATORY_FEE: bool = true;

impl crate::types::FeeHandler<Test> for MockFeeHandler {
    fn apply_fees(
        _asset: &AssetIdOf<Test>,
        _sender: &AccountId,
        _beneficiary: &AccountId,
        amount: &Balance,
        _remark: Option<&[u8]>,
    ) -> Fees<Test> {
        let sender_fees = vec![
            SubTypes::Fixed(FEE_SENDER_ACCOUNT, FEE_SENDER_AMOUNT, !MANDATORY_FEE),
            SubTypes::Percentage(
                FEE_SYSTEM_ACCOUNT,
                Percent::from_percent(MARKETPLACE_FEE_PERCENTAGE),
                MANDATORY_FEE,
            ),
        ];

        let beneficiary_fees = vec![
            SubTypes::Fixed(
                FEE_BENEFICIARY_ACCOUNT,
                FEE_BENEFICIARY_AMOUNT,
                !MANDATORY_FEE,
            ),
            SubTypes::Percentage(
                FEE_SYSTEM_ACCOUNT,
                Percent::from_percent(MARKETPLACE_FEE_PERCENTAGE),
                MANDATORY_FEE,
            ),
        ];

        let compute_fee = |fees: &Vec<SubTypes<Test>>| -> FeeDetails<Test> {
            let details = fees
                .iter()
                .map(|fee| match fee {
                    SubTypes::Fixed(account, amount_fixed, charged_disputes) => {
                        (*account, *amount_fixed, *charged_disputes)
                    }
                    SubTypes::Percentage(account, percent, charged_disputes) => {
                        (*account, percent.mul_floor(*amount), *charged_disputes)
                    }
                })
                .collect::<Vec<(AccountId, Balance, bool)>>();
            // This is a test, so i'm just unwrapping
            let bounded_details: FeeDetails<Test> = BoundedVec::try_from(details).unwrap();
            bounded_details
        };

        Fees {
            sender_pays: compute_fee(&sender_fees),
            beneficiary_pays: compute_fee(&beneficiary_fees),
        }
    }
}

#[derive(Encode, Decode, PartialEq, Debug)]
pub enum PaymentStatusHooks {
    Created(PaymentId),
    Charged(PaymentId, Balance, Balance),
    Released(PaymentId, Balance, Balance),
    Cancelled(PaymentId),
}

parameter_types! {
    pub storage Hooks: Vec<PaymentStatusHooks> = vec![];
}

pub struct OnPaymentStatusHooks;

impl fc_traits_payments::OnPaymentStatusChanged<PaymentId, Balance> for OnPaymentStatusHooks {
    fn on_payment_created(id: &PaymentId) {
        let mut hooks = Hooks::get();
        hooks.push(PaymentStatusHooks::Created(id.clone()));
        Hooks::set(&hooks);
    }

    fn on_payment_charge_success(id: &PaymentId, fees: Balance, resulting_amount: Balance) {
        let mut hooks = Hooks::get();
        hooks.push(PaymentStatusHooks::Charged(
            id.clone(),
            fees,
            resulting_amount,
        ));
        Hooks::set(&hooks);
    }

    fn on_payment_cancelled(_id: &PaymentId) {
        let mut hooks = Hooks::get();
        hooks.push(PaymentStatusHooks::Cancelled(_id.clone()));
        Hooks::set(&hooks);
    }

    fn on_payment_released(id: &PaymentId, fees: Balance, resulting_amount: Balance) {
        let mut hooks = Hooks::get();
        hooks.push(PaymentStatusHooks::Released(
            id.clone(),
            fees,
            resulting_amount,
        ));
        Hooks::set(&hooks);
    }
}

parameter_types! {
    pub const MaxRemarkLength: u8 = 50;
    pub const IncentivePercentage: Percent = Percent::from_percent(INCENTIVE_PERCENTAGE);
    pub const PaymentPalletId: PalletId = PalletId(*b"payments");
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type Assets = Assets;
    type AssetsHold = AssetsHolder;
    type BlockNumberProvider = System;
    type FeeHandler = MockFeeHandler;
    type SenderOrigin = EnsureSigned<AccountId>;
    type BeneficiaryOrigin = EnsureSigned<AccountId>;
    type DisputeResolver = frame_system::EnsureRootWithSuccess<u64, ConstU64<ROOT_ACCOUNT>>;
    type PaymentId = PaymentId;
    type Scheduler = Scheduler;
    type Preimages = ();
    type RuntimeHoldReason = RuntimeHoldReason;
    type WeightInfo = ();
    type OnPaymentStatusChanged = OnPaymentStatusHooks;
    type PalletId = PaymentPalletId;
    type IncentivePercentage = IncentivePercentage;
    type MaxRemarkLength = MaxRemarkLength;
    type MaxFees = ConstU32<50>;
    type MaxDiscounts = ConstU32<50>;
    type CancelBufferBlockLength = ConstU64<10>;
}

// Build genesis storage according to the mock runtime.
pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            // id, owner, is_sufficient, min_balance
            (FEE_SENDER_ACCOUNT, INITIAL_BALANCE),
            (FEE_BENEFICIARY_ACCOUNT, INITIAL_BALANCE),
            (FEE_SYSTEM_ACCOUNT, INITIAL_BALANCE),
            (PAYMENT_BENEFICIARY, INITIAL_BALANCE),
        ],
        dev_accounts: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_assets::GenesisConfig::<Test> {
        assets: vec![
            // id, owner, is_sufficient, min_balance
            (ASSET_ID, ASSET_ADMIN_ACCOUNT, true, 1),
        ],
        metadata: vec![
            // id, name, symbol, decimals
            (ASSET_ID, "Token Name".into(), "TOKEN".into(), 10),
        ],
        accounts: vec![
            // id, account_id, balance
            (ASSET_ID, SENDER_ACCOUNT, 100),
        ],
        next_asset_id: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.register_extension(KeystoreExt::new(MemoryKeystore::new()));
    ext.execute_with(|| System::set_block_number(1));
    ext
}

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        Scheduler::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        Scheduler::on_initialize(System::block_number());
    }
}

use core::cell::Cell;
thread_local! {
    pub static LAST_ID: Cell<u32>  = const { Cell::new(0) };
}
impl pallet_payments::PaymentId<Test> for PaymentId {
    fn next(_sender: &AccountId, _beneficiary: &AccountId) -> Option<Self> {
        LAST_ID.with(|id| {
            let new_id = id.get() + 1;
            id.set(new_id);
            Some(PaymentId(new_id))
        })
    }
}
