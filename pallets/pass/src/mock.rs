//! Test environment for pallet pass.

use crate::{self as pallet_pass, ChargeTransactionToPassAccount, Config};
pub use authenticators::*;
use codec::{Decode, Encode, MaxEncodedLen};
use fc_traits_authn::{composite_authenticators, util::AuthorityFromPalletId, Challenger};
use frame_support::weights::FixedFee;
use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstU64, EitherOf, EqualPrivilegeOnly, OnInitialize},
    weights::Weight,
    DebugNoBound, EqNoBound, PalletId,
};
use frame_system::mocking::MockUncheckedExtrinsic;
use frame_system::{EnsureRoot, EnsureRootWithSuccess};
use scale_info::TypeInfo;
use sp_core::{blake2_256, H256};
use sp_io::TestExternalities;
use sp_runtime::{
    traits::{IdentifyAccount, IdentityLookup, Verify},
    MultiSignature,
};

mod authenticators;

pub type Extension = ChargeTransactionToPassAccount<
    pallet_transaction_payment::ChargeTransactionPayment<Test>,
    Test,
>;
pub type CheckedExtrinsic =
    sp_runtime::generic::CheckedExtrinsic<AccountId, RuntimeCall, Extension>;
pub type Block = sp_runtime::generic::Block<
    sp_runtime::generic::Header<u64, sp_runtime::traits::BlakeTwo256>,
    MockUncheckedExtrinsic<Test, (), Extension>,
>;

pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub type Balance = <Test as pallet_balances::Config>::Balance;

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
    #[runtime::pallet_index(1)]
    pub type Scheduler = pallet_scheduler;
    #[runtime::pallet_index(2)]
    pub type TransactionPayment = pallet_transaction_payment;
    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type Pass = pallet_pass;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = AccountId;
    type Lookup = IdentityLookup<AccountId>;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
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

parameter_types! {
    pub MaxScheduledPerBlock: u32 = u32::MAX;
    pub MaximumWeight: Weight = Weight::MAX;
}

impl pallet_scheduler::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = MaximumWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type OriginPrivilegeCmp = EqualPrivilegeOnly;
    type MaxScheduledPerBlock = MaxScheduledPerBlock;
    type WeightInfo = ();
    type Preimages = ();
    type BlockNumberProvider = System;
}

parameter_types! {
    pub const RootAccount: AccountId = AccountId::new([0u8; 32]);
    pub PassPalletId: PalletId = PalletId(*b"py/pass_");
    pub RootDoesNotPay: Option<pallet_pass::DepositInformation<Test>> = None;
}

composite_authenticators! {
    pub Pass<AuthorityFromPalletId<PassPalletId>> {
        authenticator_a::Authenticator,
        AuthenticatorB,
    };
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type WeightInfo = ();
    type Authenticator = PassAuthenticator;
    type PalletsOrigin = OriginCaller;
    type PalletId = PassPalletId;
    type MaxSessionDuration = ConstU64<10>;
    type RegisterOrigin = EitherOf<
        // Root does not pay
        EnsureRootWithSuccess<Self::AccountId, RootDoesNotPay>,
        // Anyone else pays
        pallet_pass::EnsureSignedPays<Test, ConstU64<1>, RootAccount>,
    >;
    type Scheduler = Scheduler;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = BenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
use frame_system::pallet_prelude::OriginFor;
#[cfg(feature = "runtime-benchmarks")]
use pallet_pass::{CredentialOf, DeviceAttestationOf};

#[cfg(feature = "runtime-benchmarks")]
pub struct BenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_pass::BenchmarkHelper<Test> for BenchmarkHelper {
    fn register_origin() -> OriginFor<Test> {
        RuntimeOrigin::root()
    }

    fn device_attestation(device_id: DeviceId) -> DeviceAttestationOf<Test, ()> {
        PassDeviceAttestation::AuthenticatorAAuthenticator(authenticator_a::DeviceAttestation {
            device_id,
            challenge: authenticator_a::Authenticator::generate(&()),
        })
    }

    fn credential(user_id: HashedUserId) -> CredentialOf<Test, ()> {
        PassCredential::AuthenticatorAAuthenticator(authenticator_a::Credential {
            user_id,
            challenge: authenticator_a::Authenticator::generate(&()),
        })
    }
}

pub fn new_test_ext() -> TestExternalities {
    let mut ext = TestExternalities::new(Default::default());
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}

pub fn run_to(n: u64) {
    while System::block_number() < n {
        next_block();
    }
}

pub fn next_block() {
    System::reset_events();
    System::set_block_number(System::block_number() + 1);
    log::info!("Starting block {:?}", System::block_number());
    Scheduler::on_initialize(System::block_number());
}
