use frame_support::{
    derive_impl, parameter_types,
    traits::{
        fungible::HoldConsideration, AsEnsureOriginWithArg, ConstU32, ConstU64,
        EitherOf, EnsureOriginWithArg, EqualPrivilegeOnly, Footprint,
        OriginTrait, VariantCountOf,
    },
    weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
    PalletId,
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess, EnsureSigned};
use pallet_referenda::{TrackIdOf, TracksInfo};
use sp_io::TestExternalities;
use sp_runtime::{
    traits::{BlakeTwo256, Convert, IdentifyAccount, IdentityLookup, Verify},
    BuildStorage, MultiSignature,
};

pub type CommunityId = u32;

use crate::{
    self as pallet_communities,
    origin::{EnsureCommunity, EnsureSignedPays},
    types::{Tally, VoteWeight},
    Config, PrivacyLevel,
};

// Weights constants
pub const MAX_BLOCK_REF_TIME: u64 = WEIGHT_REF_TIME_PER_SECOND.saturating_div(2);
pub const MAX_BLOCK_POV_SIZE: u64 = 5 * 1024 * 1024;
type Block = frame_system::mocking::MockBlock<Test>;
type WeightInfo = ();

pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub type Balance = <Test as pallet_balances::Config>::Balance;

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
    pub type Preimage = pallet_preimage;

    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type Assets = pallet_assets;
    #[runtime::pallet_index(12)]
    pub type AssetsFreezer = pallet_assets_freezer;

    #[runtime::pallet_index(21)]
    pub type Referenda = pallet_referenda;
    #[runtime::pallet_index(31)]
    pub type Communities = pallet_communities;
    #[runtime::pallet_index(32)]
    pub type Tracks = fc_pallet_referenda_tracks;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<AccountId>;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
    type FreezeIdentifier = RuntimeFreezeReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
}

#[derive_impl(pallet_assets::config_preludes::TestDefaultConfig as pallet_assets::DefaultConfig)]
impl pallet_assets::Config for Test {
    type Balance = Balance;
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<Self::AccountId>>;
    type ForceOrigin = EnsureRoot<Self::AccountId>;
    type Freezer = AssetsFreezer;
}

impl pallet_assets_freezer::Config for Test {
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type RuntimeEvent = RuntimeEvent;
}

// Governance
parameter_types! {
    pub MaximumSchedulerWeight: Weight = Weight::from_parts(MAX_BLOCK_REF_TIME, MAX_BLOCK_POV_SIZE);
    pub const MaxScheduledPerBlock: u32 = 512;
}

pub struct ConvertDeposit;
impl Convert<Footprint, u64> for ConvertDeposit {
    fn convert(a: Footprint) -> u64 {
        a.count * 2 + a.size
    }
}

parameter_types! {
    pub PreimageHoldReason: RuntimeHoldReason = pallet_preimage::HoldReason::Preimage.into();
}

impl pallet_preimage::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Currency = Balances;
    type ManagerOrigin = EnsureSigned<AccountId>;
    type Consideration = HoldConsideration<AccountId, Balances, PreimageHoldReason, ConvertDeposit>;
}

impl pallet_scheduler::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type OriginPrivilegeCmp = EqualPrivilegeOnly;
    type MaxScheduledPerBlock = MaxScheduledPerBlock;
    type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Self>;
    type Preimages = Preimage;
    type BlockNumberProvider = System;
}

pub struct EnsureOriginToTrack;
impl EnsureOriginWithArg<RuntimeOrigin, TrackIdOf<Test, ()>> for EnsureOriginToTrack {
    type Success = ();

    fn try_origin(
        o: RuntimeOrigin,
        id: &TrackIdOf<Test, ()>,
    ) -> Result<Self::Success, RuntimeOrigin> {
        let track_id_for_origin: TrackIdOf<Test, ()> =
            Tracks::track_for(&o.clone().caller).map_err(|_| o.clone())?;
        frame_support::ensure!(&track_id_for_origin == id, o);
        Ok(())
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin(id: &TrackIdOf<Test, ()>) -> Result<RuntimeOrigin, ()> {
        Ok(pallet_communities::Origin::<Test>::new(id.clone()).into())
    }
}

parameter_types! {
    pub const MaxTracks: u32 = u32::MAX;
}

pub struct EnsureRootReturnGroupId;
impl EnsureOriginWithArg<RuntimeOrigin, pallet_referenda::PalletsOriginOf<Test>>
    for EnsureRootReturnGroupId
{
    type Success = <CommunityId as fc_pallet_referenda_tracks::SplitId>::Half;

    fn try_origin(
        o: RuntimeOrigin,
        _arg: &pallet_referenda::PalletsOriginOf<Test>,
    ) -> Result<Self::Success, RuntimeOrigin> {
        <EnsureRoot<AccountId> as frame_support::traits::EnsureOrigin<RuntimeOrigin>>::try_origin(o)
            .map(|_| Default::default())
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin(_arg: &pallet_referenda::PalletsOriginOf<Test>) -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::root())
    }
}

impl fc_pallet_referenda_tracks::Config for Test {
    type WeightInfo = ();
    type CreateOrigin = frame_support::traits::AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
    type GroupManagerCreateOrigin = EnsureRootReturnGroupId;
    type GroupManagerOrigin = EnsureOriginToTrack;
    type RemoveGroupOrigin = frame_support::traits::AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
    type TrackId = CommunityId;
    type MaxTracks = MaxTracks;

    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = TracksBenchmarkHelper;
}

parameter_types! {
    pub static AlarmInterval: u64 = 1;
}

impl pallet_referenda::Config for Test {
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Scheduler = Scheduler;
    type Currency = pallet_balances::Pallet<Self>;
    type SubmitOrigin = EnsureSigned<AccountId>;
    type CancelOrigin = EnsureRoot<AccountId>;
    type KillOrigin = EnsureRoot<AccountId>;
    type Slash = ();
    type Votes = VoteWeight;
    type Tally = Tally<Test>;
    type SubmissionDeposit = ConstU64<2>;
    type MaxQueued = ConstU32<3>;
    type UndecidingTimeout = ConstU64<20>;
    type AlarmInterval = AlarmInterval;
    type Tracks = Tracks;
    type Preimages = Preimage;
    type BlockNumberProvider = System;
}

// Communities

parameter_types! {
    pub const CommunitiesPalletId: PalletId = PalletId(*b"kv/comms");
    pub const NoPay: Option<(Balance, AccountId, AccountId)> = None;
}

type RootCreatesCommunitiesForFree = EnsureRootWithSuccess<AccountId, NoPay>;
type AnyoneElsePays = EnsureSignedPays<Test, ConstU64<10>, RootAccount>;

parameter_types! {
    pub const RootAccount: AccountId = AccountId::new([0xff; 32]);
}

impl Config for Test {
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type WeightInfo = WeightInfo;

    type CreateOrigin = EitherOf<RootCreatesCommunitiesForFree, AnyoneElsePays>;
    type AdminOrigin = EnsureCommunity<Self>;
    type MemberMgmtOrigin = EnsureCommunity<Self>;

    type CommunityId = CommunityId;
    type Hasher = BlakeTwo256;
    type MembershipVerifier = crate::verifier::MerkleVerifier<BlakeTwo256>;
    type MaxMembers = ConstU32<100>;

    type Polls = Referenda;
    type Assets = Assets;
    type AssetsFreezer = AssetsFreezer;
    type Balances = Balances;
    type BlockNumberProvider = System;

    type PalletId = CommunitiesPalletId;

    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = CommunityBenchmarkHelper;
}

pub const COMMUNITY: CommunityId = 1;

pub fn community_origin(id: CommunityId) -> RuntimeOrigin {
    pallet_communities::Origin::<Test>::new(id).into()
}

pub(crate) struct TestEnvBuilder {
    communities: Vec<(CommunityId, PrivacyLevel)>,
    members: Vec<(CommunityId, AccountId)>,
}

impl TestEnvBuilder {
    pub(crate) fn new() -> Self {
        Self {
            communities: Vec::new(),
            members: Vec::new(),
        }
    }

    pub(crate) fn add_community(mut self, id: CommunityId, privacy: PrivacyLevel) -> Self {
        self.communities.push((id, privacy));
        self
    }

    pub(crate) fn add_member(mut self, community_id: CommunityId, who: AccountId) -> Self {
        self.members.push((community_id, who));
        self
    }

    pub(crate) fn build(self) -> TestExternalities {
        let t = RuntimeGenesisConfig {
            assets: Default::default(),
            balances: Default::default(),
            system: Default::default(),
        }
        .build_storage()
        .unwrap();

        let mut ext = TestExternalities::new(t);

        ext.execute_with(|| {
            System::set_block_number(1);

            for (community_id, privacy) in &self.communities {
                let origin = community_origin(*community_id);

                Communities::create(
                    RuntimeOrigin::root(),
                    origin.caller().clone(),
                    *community_id,
                )
                .expect("can create community");

                // Set the privacy level
                if *privacy != PrivacyLevel::Public {
                    crate::Info::<Test>::mutate(community_id, |info| {
                        if let Some(ref mut info) = info {
                            info.privacy = privacy.clone();
                        }
                    });
                }

                for (cid, who) in self.members.iter().filter(|(cid, _)| cid == community_id) {
                    Communities::add_member(
                        community_origin(*cid),
                        who.clone(),
                        None,
                        None,
                    )
                    .expect("can add member");
                }
            }
        });

        ext
    }
}

pub fn account(n: u8) -> AccountId {
    AccountId::new([n; 32])
}
