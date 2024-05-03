//! Test environment for pallet pass.

use crate::{self as pallet_pass, Config};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
    ensure,
    traits::{ConstU16, ConstU32, ConstU64},
};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_io::TestExternalities;
use sp_runtime::{
    traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
    MultiSignature,
};

pub type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Timestamp: pallet_timestamp,
        Babe: pallet_babe,
        Pass: pallet_pass,
    }
);

impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<u64>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ConstU16<42>;
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
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
    type MaxHolds = ();
}

parameter_types! {
    pub EpochDuration: u64 = prod_or_fast!(
        EPOCH_DURATION_IN_SLOTS as u64,
        2 * MINUTES as u64,
        "KSM_EPOCH_DURATION"
    );
    pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
    pub ReportLongevity: u64 =
        BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
}

impl pallet_babe::Config for Test {
    type EpochDuration = ConstU64<2>;
    type ExpectedBlockTime = ConstU64<2>;
}

pub struct InvalidAuthenticator;
impl pallet_pass::traits::Authenticator for InvalidAuthenticator {
    fn get_device_id(&self, _device: Vec<u8>) -> Option<pallet_pass::DeviceId> {
        None
    }

    fn authenticate(
        &self,
        _device: Vec<u8>,
        _challenge: &[u8],
        _payload: &[u8],
    ) -> Result<(), pallet_pass::traits::AuthenticateError> {
        Err(pallet_pass::traits::AuthenticateError::ChallengeFailed)
    }
}

pub struct DummyAuthenticator;
impl pallet_pass::traits::Authenticator for DummyAuthenticator {
    fn get_device_id(&self, _device: Vec<u8>) -> Option<pallet_pass::DeviceId> {
        Some([1u8; 32])
    }

    fn authenticate(
        &self,
        _device: Vec<u8>,
        challenge: &[u8],
        payload: &[u8],
    ) -> Result<(), pallet_pass::traits::AuthenticateError> {
        ensure!(
            challenge == payload,
            pallet_pass::traits::AuthenticateError::ChallengeFailed
        );
        Ok(())
    }
}

#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, Debug, Clone, Eq, PartialEq)]
pub enum MockAuthenticators {
    InvalidAuthenticator,
    DummyAuthenticator,
}

impl Into<Box<dyn pallet_pass::traits::Authenticator>> for MockAuthenticators {
    fn into(self) -> Box<dyn pallet_pass::traits::Authenticator> {
        match self {
            MockAuthenticators::InvalidAuthenticator => Box::new(InvalidAuthenticator),
            MockAuthenticators::DummyAuthenticator => Box::new(DummyAuthenticator),
        }
    }
}

impl Config for Test {
    type WeightInfo = ();
    type RuntimeEvent = RuntimeEvent;
    type Authenticator = MockAuthenticators;
    type Registrar = ();
    type MaxAccountNameLen = ConstU32<64>;
    type MaxDeviceDescriptorLen = ConstU32<65535>;
    type MaxSessionDuration = ConstU64<10>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut ext = TestExternalities::new(Default::default());
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}
