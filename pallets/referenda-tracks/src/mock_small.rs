//! A second test environment, whose `TrackId` is a `u16`: its group and sub-track halves are `u8`,
//! so a runtime with this id has 255 allocatable groups and 255 sub-tracks per group, fewer than
//! `MaxTracks` (here, 300). It is meant to exercise the benchmarks on a runtime whose id space is
//! smaller than `MaxTracks`.

use crate::{self as pallet_referenda_tracks, mock::Tally, Config};
use frame_support::{
    derive_impl, parameter_types,
    traits::{AsEnsureOriginWithArg, ConstU32, ConstU64, ConstU8, EqualPrivilegeOnly},
    weights::Weight,
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess};
use sp_io::TestExternalities;
use sp_runtime::BuildStorage;

pub type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances,
        Preimage: pallet_preimage,
        Scheduler: pallet_scheduler,
        Referenda: pallet_referenda,
        Tracks: pallet_referenda_tracks,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

impl pallet_preimage::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<u64>;
    type Consideration = ();
}

parameter_types! {
    pub MaxWeight: Weight = Weight::from_parts(2_000_000_000_000, u64::MAX);
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

impl pallet_referenda::Config for Test {
    type WeightInfo = ();
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type Scheduler = Scheduler;
    type Currency = pallet_balances::Pallet<Self>;
    type SubmitOrigin = frame_system::EnsureSigned<u64>;
    type CancelOrigin = EnsureRoot<u64>;
    type KillOrigin = EnsureRoot<u64>;
    type Slash = ();
    type Votes = u32;
    type Tally = Tally;
    type SubmissionDeposit = ConstU64<2>;
    type MaxQueued = ConstU32<3>;
    type UndecidingTimeout = ConstU64<20>;
    type AlarmInterval = ConstU64<1>;
    type Tracks = Tracks;
    type Preimages = Preimage;
    type BlockNumberProvider = System;
}

parameter_types! {
    pub static MaxTracks: u32 = 300;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct BenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl crate::BenchmarkHelper<Test, ()> for BenchmarkHelper {
    fn track_id(id: u32) -> u16 {
        id as u16
    }
}

type Root = AsEnsureOriginWithArg<EnsureRoot<u64>>;

impl Config for Test {
    type WeightInfo = ();
    type CreateOrigin = Root;
    type GroupManagerCreateOrigin = AsEnsureOriginWithArg<EnsureRootWithSuccess<u64, ConstU8<0>>>;
    type GroupManagerOrigin = Root;
    type RemoveOrigin = Root;
    type RemoveGroupOrigin = Root;
    type TrackId = u16;
    type MaxTracks = MaxTracks;

    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = BenchmarkHelper;
}

pub fn new_test_ext() -> TestExternalities {
    let t = RuntimeGenesisConfig {
        system: Default::default(),
        balances: pallet_balances::GenesisConfig::<Test> {
            balances: vec![],
            dev_accounts: None,
        },
    }
    .build_storage()
    .unwrap();

    let mut ext = TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
