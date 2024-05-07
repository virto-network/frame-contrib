//! Test environment for pallet pass.

use crate::{self as pallet_pass, Config};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
    ensure, parameter_types,
    traits::{ConstU16, ConstU32, ConstU64, EqualPrivilegeOnly, OnInitialize},
    weights::Weight,
};
use frame_system::EnsureRoot;
use scale_info::TypeInfo;
use sp_core::{blake2_256, H256};
use sp_io::TestExternalities;
use sp_runtime::{
    traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
    MultiSignature,
};

use core::marker::PhantomData;

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

pub struct RandomessFromBlockNumber;
impl frame_support::traits::Randomness<H256, u64> for RandomessFromBlockNumber {
    fn random(subject: &[u8]) -> (H256, u64) {
        let block_number = System::block_number();
        let block_number_as_bytes = block_number.to_le_bytes();
        (
            H256(blake2_256(
                &vec![block_number_as_bytes.to_vec(), subject.to_vec()].concat()[..],
            )),
            block_number,
        )
    }
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

// pub struct DummyRegistrar<AccountId, AccountName>(PhantomData<(AccountId, AccountName)>);
// impl<AccountId, AccountName> pallet_pass::traits::Registrar<AccountId, AccountName>
//     for DummyRegistrar<AccountId, AccountName>
// {
//     fn claim(
//         _account_name: AccountName,
//         _claimer: AccountId,
//     ) -> Result<(), pallet_pass::traits::ClaimError> {
//         Ok(())
//     }

//     fn claimer_pays_fees(_account_name: AccountName, _claimer: AccountId) -> bool {
//         true
//     }
// }

pub struct EvenOddRegistrar<AccountId, AccountName>(PhantomData<(AccountId, AccountName)>);
impl<AccountId: AsRef<[u8]>, AccountName> EvenOddRegistrar<AccountId, AccountName> {
    // Function to determine if an account id is even
    fn is_even_account_id(account_id: &AccountId) -> bool {
        let bytes = account_id.as_ref();
        if let Some(last_byte) = bytes.last() {
            last_byte % 2 == 0
        } else {
            false
        }
    }
}

impl<AccountId: AsRef<[u8]>, AccountName> pallet_pass::traits::Registrar<AccountId, AccountName>
    for EvenOddRegistrar<AccountId, AccountName>
{
    fn claim(
        _account_name: AccountName,
        claimer: AccountId,
    ) -> Result<(), pallet_pass::traits::ClaimError> {
        if Self::is_even_account_id(&claimer) {
            Ok(())
        } else {
            Err(pallet_pass::traits::ClaimError::CannotClaim)
        }
    }

    fn claimer_pays_fees(_account_name: AccountName, _claimer: AccountId) -> bool {
        true
    }
}

#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, Debug, Clone, Eq, PartialEq)]
pub enum MockRegistrars {
    // DummyRegistrar,
    EvenOddRegistrar,
}

impl<AccountId: AsRef<[u8]>, AccountName> pallet_pass::traits::Registrar<AccountId, AccountName> for MockRegistrars {
    fn claim(
        account_name: AccountName,
        claimer: AccountId,
    ) -> Result<(), pallet_pass::traits::ClaimError> {
        EvenOddRegistrar::<AccountId, AccountName>::claim(account_name, claimer)
    }

    fn claimer_pays_fees(account_name: AccountName, claimer: AccountId) -> bool {
        EvenOddRegistrar::<AccountId, AccountName>::claimer_pays_fees(account_name, claimer)
    }
}


impl Config for Test {
    type WeightInfo = ();
    type RuntimeEvent = RuntimeEvent;
    type Authenticator = MockAuthenticators;
    type Randomness = RandomessFromBlockNumber;
    type Registrar = MockRegistrars;
    type RuntimeCall = RuntimeCall;
    type Scheduler = Scheduler;
    type PalletsOrigin = OriginCaller;
    type UninitializedTimeout = ConstU64<10>;
    type MaxAccountNameLen = ConstU32<64>;
    type MaxDeviceDescriptorLen = ConstU32<65535>;
    type MaxDevicesPerAccount = ConstU32<5>;
    type MaxSessionDuration = ConstU64<10>;
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
    System::set_block_number(System::block_number() + 1);
    log::info!("Starting block {:?}", System::block_number());
    Scheduler::on_initialize(System::block_number());
}
