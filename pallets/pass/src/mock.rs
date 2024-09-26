//! Test environment for pallet pass.

use crate::{self as pallet_pass, Config};
use codec::{Decode, Encode, MaxEncodedLen};
use fc_traits_authn::Challenger;
use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstU32, ConstU64, EqualPrivilegeOnly, OnInitialize, Randomness},
    weights::Weight,
    DebugNoBound, EqNoBound, PalletId,
};
use frame_system::{EnsureRoot, EnsureSigned};
use scale_info::TypeInfo;
use sp_core::{blake2_256, H256};
use sp_io::TestExternalities;
use sp_runtime::{
    str_array as s,
    traits::{IdentifyAccount, IdentityLookup, Verify},
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

pub struct RandomnessFromBlockNumber;
impl frame_support::traits::Randomness<H256, u64> for RandomnessFromBlockNumber {
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

#[derive(TypeInfo, MaxEncodedLen, DebugNoBound, EqNoBound, PartialEq, Clone, Encode, Decode)]
pub struct MockDeviceAttestation {
    pub(crate) context: (),
    pub(crate) user_id: fc_traits_authn::HashedUserId,
    pub(crate) challenge: fc_traits_authn::Challenge,
    pub(crate) device_id: fc_traits_authn::DeviceId,
}

impl fc_traits_authn::ChallengeResponse<()> for MockDeviceAttestation {
    fn is_valid(&self) -> bool {
        self.challenge == MockChallenger::generate(&self.context)
    }

    fn used_challenge(&self) -> ((), fc_traits_authn::Challenge) {
        ((), MockChallenger::generate(&self.context))
    }

    fn authority(&self) -> fc_traits_authn::AuthorityId {
        s("DummyAuthenticator")
    }
}

impl fc_traits_authn::UserAuthenticator for MockDeviceAttestation {
    const AUTHORITY: fc_traits_authn::AuthorityId = s("MockDevice");
    type Challenger = MockChallenger;
    type Credential = Self;

    fn device_id(&self) -> fc_traits_authn::DeviceId {
        todo!()
    }
}

impl fc_traits_authn::DeviceChallengeResponse<()> for MockDeviceAttestation {
    fn device_id(&self) -> fc_traits_authn::DeviceId {
        self.device_id
    }
}

impl fc_traits_authn::UserChallengeResponse<()> for MockDeviceAttestation {
    fn user_id(&self) -> fc_traits_authn::HashedUserId {
        self.user_id
    }
}

pub struct MockChallenger;
impl fc_traits_authn::Challenger for MockChallenger {
    type Context = ();

    fn generate(_: &Self::Context) -> fc_traits_authn::Challenge {
        let (hash, _) = RandomnessFromBlockNumber::random_seed();
        hash.0
    }
}

pub struct InvalidAuthenticator;
impl fc_traits_authn::Authenticator for InvalidAuthenticator {
    const AUTHORITY: fc_traits_authn::AuthorityId = s("InvalidAuthenticator");
    type Challenger = MockChallenger;
    type DeviceAttestation = MockDeviceAttestation;
    type Device = MockDevice;

    fn verify_device(&self, _: &Self::DeviceAttestation) -> Option<Self::Device> {
        None
    }

    fn unpack_device(&self, _: &Self::DeviceAttestation) -> Self::Device {
        ()
    }
}

pub struct DummyAuthenticator;
impl fc_traits_authn::Authenticator for DummyAuthenticator {
    const AUTHORITY: fc_traits_authn::AuthorityId = s("DummyAuthenticator");
    type Challenger = MockChallenger;
    type DeviceAttestation = MockDeviceAttestation;
    type Device = MockDeviceAttestation;

    fn unpack_device(&self, verification: &Self::DeviceAttestation) -> Self::Device {
        todo!()
    }
    // fn get_device_id(&self, device: Vec<u8>) -> Option<pallet_pass::DeviceId> {
    //     let len = device.len();
    //     Some([(len as u8) + 1; 32])
    // }

    // fn authenticate(
    //     &self,
    //     _device: Vec<u8>,
    //     challenge: &[u8],
    //     payload: &[u8],
    // ) -> Result<(), fc_traits_authn::AuthenticateError> {
    //     ensure!(
    //         challenge == payload,
    //         fc_traits_authn::AuthenticateError::ChallengeFailed
    //     );
    //     Ok(())
    // }
}

#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, Debug, Clone, Eq, PartialEq)]
pub enum MockAuthenticationMethods {
    InvalidAuthenticationMethod,
    DummyAuthenticationMethod,
}

impl Into<Box<dyn fc_traits_authn::Authenticator>> for MockAuthenticationMethods {
    fn into(self) -> Box<dyn fc_traits_authn::Authenticator> {
        match self {
            MockAuthenticationMethods::InvalidAuthenticationMethod => {
                Box::new(InvalidAuthenticator)
            }
            MockAuthenticationMethods::DummyAuthenticationMethod => Box::new(DummyAuthenticator),
        }
    }
}

parameter_types! {
    pub PassPalletId: PalletId = PalletId(*b"py/pass_");
}

impl Config for Test {
    type WeightInfo = ();
    type RuntimeEvent = RuntimeEvent;
    type Authenticator = MockAuthenticationMethods;
    type RegisterOrigin = EnsureSigned<Self::AccountId>;
    type RuntimeCall = RuntimeCall;
    type PalletId = PassPalletId;
    type PalletsOrigin = OriginCaller;
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
    System::reset_events();
    System::set_block_number(System::block_number() + 1);
    log::info!("Starting block {:?}", System::block_number());
    Scheduler::on_initialize(System::block_number());
}
