//! Test environment for pallet pass.

use crate::{self as pallet_pass, Config};
pub use authenticators::*;
use codec::{Decode, Encode, MaxEncodedLen};
use fc_traits_authn::{composite_authenticators, util::AuthorityFromPalletId, Challenger};
use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstU32, ConstU64, EitherOf, EqualPrivilegeOnly, OnInitialize},
    weights::Weight,
    DebugNoBound, EqNoBound, PalletId,
};
use frame_system::{pallet_prelude::OriginFor, EnsureRoot, EnsureRootWithSuccess};
use scale_info::TypeInfo;
use sp_core::{blake2_256, H256};
use sp_io::TestExternalities;
use sp_runtime::{
    traits::{IdentifyAccount, IdentityLookup, Verify},
    MultiSignature,
};

mod authenticators;

pub type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Scheduler: pallet_scheduler,
        Pass: pallet_pass,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<AccountId>;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

impl pallet_balances::Config for Test {
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type MaxLocks = ConstU32<10>;
    type Balance = u64;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU64<1>;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeHoldReason = ();
    type RuntimeFreezeReason = ();
}

parameter_types! {
    pub MaxScheduledPerBlock: u32 = u32::MAX;
    pub MaximumWeight: Weight = Weight::MAX;
}

impl pallet_scheduler::Config for Test {
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = MaxScheduledPerBlock;
    type MaximumWeight = MaximumWeight;
    type OriginPrivilegeCmp = EqualPrivilegeOnly;
    type Preimages = ();
    type WeightInfo = ();
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
    type WeightInfo = ();
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Authenticator = PassAuthenticator;
    type RegisterOrigin = EitherOf<
        // Root does not pay
        EnsureRootWithSuccess<Self::AccountId, RootDoesNotPay>,
        // Anyone else pays
        pallet_pass::EnsureSignedPays<Test, ConstU64<1>, RootAccount>,
    >;
    type RuntimeCall = RuntimeCall;
    type PalletId = PassPalletId;
    type PalletsOrigin = OriginCaller;
    type MaxSessionDuration = ConstU64<10>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = BenchmarkHelper;
}

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

pub fn new_test_ext() -> sp_io::TestExternalities {
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
