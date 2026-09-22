//! Configuration of the pallets of the runtime.
//!
//! The configuration is kept plain and generic: `Root` for privileged origins, signed origins
//! elsewhere, and reasonable bounds. Where a benchmark depends on a helper, it's implemented in
//! [`benchmark_helpers`].

use super::*;

use alloc::boxed::Box;
use fc_pallet_communities::origin::{EnsureCommunity, EnsureSignedPays};
use fc_pallet_listings::{InventoryId, InventoryIdFor, ItemIdOf};
use fc_pallet_pass::FirstItemIsFree;
use frame_contrib_traits::{
    authn::util::{dummy::Dummy, AuthorityFromPalletId},
    gas_tank::{NonFungibleGasTank, SelectNonFungibleItem},
    memberships::NonFungiblesMemberships,
};
use frame_support::{
    derive_impl,
    dispatch::DispatchClass,
    parameter_types,
    traits::{
        fungible::HoldConsideration, tokens::imbalance::ResolveTo, AsEnsureOriginWithArg,
        ConstU128, ConstU32, EitherOf, EnsureOrigin, EnsureOriginWithArg, EqualPrivilegeOnly, Get,
        LinearStoragePrice, VariantCountOf,
    },
    weights::{
        constants::{RocksDbWeight, WEIGHT_REF_TIME_PER_SECOND},
        ConstantMultiplier, IdentityFee, Weight,
    },
    PalletId,
};
use frame_system::{
    limits::{BlockLength, BlockWeights},
    EnsureNever, EnsureRoot, EnsureRootWithSuccess, EnsureSigned,
};
use pallet_nfts::PalletFeatures;
use pallet_referenda::PalletsOriginOf;
use sp_runtime::{traits::AccountIdConversion, Perbill, Percent};

// Units

pub const UNITS: Balance = 1_000_000_000_000;
pub const CENTS: Balance = UNITS / 100;
pub const MILLICENTS: Balance = CENTS / 1_000;
pub const EXISTENTIAL_DEPOSIT: Balance = MILLICENTS;

pub const fn deposit(items: u32, bytes: u32) -> Balance {
    items as Balance * 20 * CENTS + (bytes as Balance) * 100 * MILLICENTS
}

pub const MINUTES: BlockNumber = 10;
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

// System

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
const MAXIMUM_BLOCK_WEIGHT: Weight =
    Weight::from_parts(WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2), u64::MAX);

parameter_types! {
    pub const Version: RuntimeVersion = VERSION;
    pub RuntimeBlockLength: BlockLength =
        BlockLength::builder()
            .max_length(5 * 1024 * 1024)
            .modify_max_length_for_class(DispatchClass::Normal, |max| {
                *max = NORMAL_DISPATCH_RATIO * *max
            })
            .build();
    pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
        .for_class(DispatchClass::all(), |weights| {
            weights.base_extrinsic = Weight::from_parts(125_000_000, 0);
        })
        .for_class(DispatchClass::Normal, |weights| {
            weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
        })
        .for_class(DispatchClass::Operational, |weights| {
            weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
            weights.reserved = Some(
                MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT,
            );
        })
        .avg_block_initialization(Perbill::from_percent(10))
        .build_or_panic();
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) *
        RuntimeBlockWeights::get().max_block;
}

#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
    type Block = Block;
    type AccountId = AccountId;
    type Nonce = Nonce;
    type Hash = Hash;
    type BlockWeights = RuntimeBlockWeights;
    type BlockLength = RuntimeBlockLength;
    type DbWeight = RocksDbWeight;
    type Version = Version;
    type AccountData = pallet_balances::AccountData<Balance>;
    type MaxConsumers = ConstU32<16>;
}

#[derive_impl(pallet_timestamp::config_preludes::TestDefaultConfig)]
impl pallet_timestamp::Config for Runtime {}

impl pallet_scheduler::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = ConstU32<512>;
    type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Runtime>;
    type OriginPrivilegeCmp = EqualPrivilegeOnly;
    type Preimages = Preimage;
    type BlockNumberProvider = System;
}

parameter_types! {
    pub const PreimageHoldReason: RuntimeHoldReason =
        RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);
}

impl pallet_preimage::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_preimage::weights::SubstrateWeight<Runtime>;
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type Consideration = HoldConsideration<
        AccountId,
        Balances,
        PreimageHoldReason,
        LinearStoragePrice<ConstU128<{ deposit(2, 64) }>, ConstU128<{ deposit(0, 1) }>, Balance>,
    >;
}

// Monetary

parameter_types! {
    pub TreasuryAccount: AccountId = PalletId(*b"fc/trsry").into_account_truncating();
}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type DustRemoval = ();
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ConstU32<50>;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
    type DoneSlashHandler = ();
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction =
        pallet_transaction_payment::FungibleAdapter<Balances, ResolveTo<TreasuryAccount, Balances>>;
    type OperationalFeeMultiplier = frame_support::traits::ConstU8<5>;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = ConstantMultiplier<Balance, ConstU128<MILLICENTS>>;
    type FeeMultiplierUpdate = ();
    type WeightInfo = pallet_transaction_payment::weights::SubstrateWeight<Runtime>;
}

pub type AssetId = u32;

#[derive_impl(pallet_assets::config_preludes::TestDefaultConfig)]
impl pallet_assets::Config for Runtime {
    type Balance = Balance;
    type AssetId = AssetId;
    type AssetIdParameter = codec::Compact<AssetId>;
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type ForceOrigin = EnsureRoot<AccountId>;
    type AssetDeposit = ConstU128<{ 10 * UNITS }>;
    type AssetAccountDeposit = ConstU128<{ deposit(1, 16) }>;
    type MetadataDepositBase = ConstU128<{ deposit(1, 68) }>;
    type MetadataDepositPerByte = ConstU128<{ deposit(0, 1) }>;
    type ApprovalDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
    type StringLimit = ConstU32<50>;
    type Freezer = AssetsFreezer;
    type Holder = AssetsHolder;
    type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

impl pallet_assets_freezer::Config for Runtime {
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type RuntimeEvent = RuntimeEvent;
}

impl pallet_assets_holder::Config for Runtime {
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeEvent = RuntimeEvent;
}

/// Selects any membership as a gas tank.
pub struct AnyMembership;
impl Get<Box<dyn SelectNonFungibleItem<CommunityId, MembershipId>>> for AnyMembership {
    fn get() -> Box<dyn SelectNonFungibleItem<CommunityId, MembershipId>> {
        Box::new(())
    }
}

/// Memberships can hold a gas tank, to pay for transactions with gas instead of fees.
pub type MembershipsGasTank =
    NonFungibleGasTank<Runtime, System, Memberships, pallet_nfts::ItemConfig, AnyMembership>;

impl fc_pallet_gas_transaction_payment::Config for Runtime {
    type WeightInfo = ();
    type GasTank = MembershipsGasTank;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = benchmark_helpers::GasTransactionPaymentBenchmarkHelper;
}

parameter_types! {
    pub const BlackHolePalletId: PalletId = PalletId(*b"fc/blkhl");
}

impl fc_pallet_black_hole::Config for Runtime {
    type WeightInfo = ();
    type EventHorizonDispatchOrigin = EnsureRoot<AccountId>;
    type Balances = Balances;
    type BlockNumberProvider = System;
    type PalletId = BlackHolePalletId;
    type BurnPeriod = ConstU32<DAYS>;
}

impl fc_pallet_fees::Config for Runtime {
    type CommunityId = CommunityId;
    type MaxFeeNameLen = ConstU32<32>;
    type MaxProtocolFees = ConstU32<8>;
    type MaxCommunityFees = ConstU32<8>;
    type AdminOrigin = EnsureRoot<AccountId>;
    type CommunityOrigin = EnsureCommunity<Runtime>;
    type CommunityDetector = ();
}

// Accounts

parameter_types! {
    pub const PassPalletId: PalletId = PalletId(*b"fc/pass_");
    pub const HoldAccountRegistration: RuntimeHoldReason =
        RuntimeHoldReason::Pass(fc_pallet_pass::HoldReason::AccountRegistration);
    pub const HoldAccountDevices: RuntimeHoldReason =
        RuntimeHoldReason::Pass(fc_pallet_pass::HoldReason::AccountDevices);
    pub const HoldSessionKeys: RuntimeHoldReason =
        RuntimeHoldReason::Pass(fc_pallet_pass::HoldReason::SessionKeys);
}

/// A dummy authenticator from `fc-traits-authn`: it accepts any attestation or credential, and
/// reports no verification weight. `fc_pallet_pass::WeightInfo` does not include the cost of
/// verifying attestations and credentials (each authenticator reports it separately), so
/// benchmarking with it measures exactly what the pallet charges for.
pub type PassAuthenticator = Dummy<AuthorityFromPalletId<PassPalletId>>;

pub type PassStoragePrice =
    LinearStoragePrice<ConstU128<{ deposit(1, 0) }>, ConstU128<{ deposit(0, 1) }>, Balance>;

impl fc_pallet_pass::Config for Runtime {
    type PalletsOrigin = OriginCaller;
    type WeightInfo = ();
    type RegisterOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type AddressGenerator = ();
    type Balances = Balances;
    type Authenticator = PassAuthenticator;
    type Scheduler = Scheduler;
    type BlockNumberProvider = System;
    type SpendMatcher = ();
    type CallMatcher = fc_pallet_pass::ScaleCallMatcher;
    type RegistrarConsideration =
        HoldConsideration<AccountId, Balances, HoldAccountRegistration, PassStoragePrice>;
    type DeviceConsideration = FirstItemIsFree<
        HoldConsideration<AccountId, Balances, HoldAccountDevices, PassStoragePrice>,
    >;
    type SessionKeyConsideration =
        FirstItemIsFree<HoldConsideration<AccountId, Balances, HoldSessionKeys, PassStoragePrice>>;
    type PalletId = PassPalletId;
    type MaxDevicesPerAccount = ConstU32<10>;
    type MaxSessionsPerAccount = ConstU32<10>;
    type MaxSessionDuration = ConstU32<DAYS>;
    type MaxFilteredCalls = ConstU32<32>;
    type MaxFilteredAssets = ConstU32<8>;
}

// Communities

pub type CommunityId = u16;
pub type MembershipId = u64;

pub type MembershipsInstance = pallet_nfts::Instance1;

impl pallet_nfts::Config<MembershipsInstance> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = CommunityId;
    type ItemId = MembershipId;
    type Currency = Balances;
    type ForceOrigin = EnsureRoot<AccountId>;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureNever<AccountId>>;
    type Locker = ();
    type CollectionDeposit = ();
    type ItemDeposit = ();
    type MetadataDepositBase = ();
    type AttributeDepositBase = ();
    type DepositPerByte = ();
    type StringLimit = ConstU32<256>;
    type KeyLimit = ConstU32<64>;
    type ValueLimit = ConstU32<256>;
    type ApprovalsLimit = ConstU32<20>;
    type ItemAttributesApprovalsLimit = ConstU32<20>;
    type MaxTips = ConstU32<10>;
    type MaxDeadlineDuration = ConstU32<{ 12 * 30 * DAYS }>;
    type MaxAttributesPerCall = ConstU32<10>;
    type Features = NftsFeatures;
    type OffchainSignature = Signature;
    type OffchainPublic = <Signature as sp_runtime::traits::Verify>::Signer;
    type WeightInfo = pallet_nfts::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type Helper = benchmark_helpers::MembershipsBenchmarkHelper;
    type BlockNumberProvider = System;
}

parameter_types! {
    pub NftsFeatures: PalletFeatures = PalletFeatures::all_enabled();
    pub const CommunitiesPalletId: PalletId = PalletId(*b"fc/comms");
    pub const NoPay: Option<(Balance, AccountId, AccountId)> = None;
    pub const CommunityDeposit: Balance = 10 * UNITS;
}

pub type MembershipsManager = NonFungiblesMemberships<Memberships, pallet_nfts::ItemConfig>;

impl fc_pallet_communities::Config for Runtime {
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type WeightInfo = ();
    type CreateOrigin = EitherOf<
        EnsureRootWithSuccess<AccountId, NoPay>,
        EnsureSignedPays<Runtime, CommunityDeposit, TreasuryAccount>,
    >;
    type AdminOrigin = EnsureCommunity<Self>;
    type MemberMgmtOrigin = EnsureCommunity<Self>;
    type CommunityId = CommunityId;
    type MembershipId = MembershipId;
    type MemberMgmt = MembershipsManager;
    type Polls = CommunityReferenda;
    type Assets = Assets;
    type AssetsFreezer = AssetsFreezer;
    type Balances = Balances;
    type BlockNumberProvider = System;
    type PalletId = CommunitiesPalletId;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = benchmark_helpers::CommunitiesBenchmarkHelper;
}

impl pallet_referenda::Config for Runtime {
    type WeightInfo = pallet_referenda::weights::SubstrateWeight<Self>;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type Scheduler = Scheduler;
    type Currency = Balances;
    type SubmitOrigin = EnsureSigned<AccountId>;
    type CancelOrigin = EnsureRoot<AccountId>;
    type KillOrigin = EnsureRoot<AccountId>;
    type Slash = ();
    type Votes = fc_pallet_communities::types::VoteWeight;
    type Tally = fc_pallet_communities::types::Tally<Runtime>;
    type SubmissionDeposit = ConstU128<2>;
    type MaxQueued = ConstU32<3>;
    type UndecidingTimeout = ConstU32<20>;
    type AlarmInterval = ConstU32<1>;
    type Tracks = CommunityTracks;
    type Preimages = Preimage;
    type BlockNumberProvider = System;
}

/// `Root` creates groups (and sub-tracks) on the first group.
pub struct EnsureRootReturnsFirstGroup;
impl EnsureOriginWithArg<RuntimeOrigin, PalletsOriginOf<Runtime>> for EnsureRootReturnsFirstGroup {
    type Success = <CommunityId as fc_pallet_referenda_tracks::SplitId>::Half;

    fn try_origin(
        o: RuntimeOrigin,
        _: &PalletsOriginOf<Runtime>,
    ) -> Result<Self::Success, RuntimeOrigin> {
        <EnsureRoot<AccountId> as EnsureOrigin<RuntimeOrigin>>::try_origin(o)
            .map(|_| Default::default())
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin(_: &PalletsOriginOf<Runtime>) -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::root())
    }
}

impl fc_pallet_referenda_tracks::Config for Runtime {
    type WeightInfo = ();
    type CreateOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
    type GroupManagerCreateOrigin = EnsureRootReturnsFirstGroup;
    type GroupManagerOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
    type RemoveGroupOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
    type TrackId = CommunityId;
    type MaxTracks = ConstU32<{ u8::MAX as u32 }>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = benchmark_helpers::TracksBenchmarkHelper;
}

// Marketplace

pub type ListingsInstance = pallet_nfts::Instance2;

impl pallet_nfts::Config<ListingsInstance> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = InventoryIdFor<Runtime>;
    type ItemId = ItemIdOf<Runtime>;
    type Currency = Balances;
    type ForceOrigin = EnsureNever<AccountId>;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureNever<AccountId>>;
    type Locker = ();
    type CollectionDeposit = ConstU128<{ deposit(1, 0) }>;
    type ItemDeposit = ConstU128<{ deposit(1, 0) }>;
    type MetadataDepositBase = ConstU128<{ deposit(1, 0) }>;
    type AttributeDepositBase = ConstU128<{ deposit(1, 0) }>;
    type DepositPerByte = ConstU128<{ deposit(0, 1) }>;
    type StringLimit = ConstU32<256>;
    type KeyLimit = ConstU32<64>;
    type ValueLimit = ConstU32<256>;
    type ApprovalsLimit = ConstU32<20>;
    type ItemAttributesApprovalsLimit = ConstU32<20>;
    type MaxTips = ConstU32<10>;
    type MaxDeadlineDuration = ConstU32<{ 12 * 30 * DAYS }>;
    type MaxAttributesPerCall = ConstU32<10>;
    type Features = NftsFeatures;
    type OffchainSignature = Signature;
    type OffchainPublic = <Signature as sp_runtime::traits::Verify>::Signer;
    type WeightInfo = pallet_nfts::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type Helper = benchmark_helpers::ListingsCatalogBenchmarkHelper;
    type BlockNumberProvider = System;
}

pub type MerchantId = u32;

parameter_types! {
    pub const MerchantsPalletId: PalletId = PalletId(*b"fc/mrcht");
}

/// Each merchant has an account, derived from its id. Only that account can create and manage the
/// merchant's inventories.
pub struct EnsureMerchantAccount;
impl EnsureMerchantAccount {
    pub fn merchant_account(merchant: &MerchantId) -> AccountId {
        MerchantsPalletId::get().into_sub_account_truncating(merchant)
    }
}
impl<Id> EnsureOriginWithArg<RuntimeOrigin, InventoryId<MerchantId, Id>> for EnsureMerchantAccount {
    type Success = AccountId;

    fn try_origin(
        o: RuntimeOrigin,
        InventoryId(merchant, _): &InventoryId<MerchantId, Id>,
    ) -> Result<Self::Success, RuntimeOrigin> {
        match o.clone().into() {
            Ok(frame_system::RawOrigin::Signed(who)) if who == Self::merchant_account(merchant) => {
                Ok(who)
            }
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin(
        InventoryId(merchant, _): &InventoryId<MerchantId, Id>,
    ) -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::signed(Self::merchant_account(merchant)))
    }
}

impl fc_pallet_listings::Config for Runtime {
    type WeightInfo = ();
    type CreateInventoryOrigin = EnsureMerchantAccount;
    type InventoryAdminOrigin = EnsureMerchantAccount;
    type MerchantId = MerchantId;
    type InventoryId = u32;
    type ItemSKU = u32;
    type CollectionConfig =
        pallet_nfts::CollectionConfig<Balance, BlockNumber, InventoryIdFor<Runtime>>;
    type ItemConfig = pallet_nfts::ItemConfig;
    type Balances = Balances;
    type Assets = Assets;
    type Nonfungibles = ListingsCatalog;
    type NonfungiblesKeyLimit = <Runtime as pallet_nfts::Config<ListingsInstance>>::KeyLimit;
    type NonfungiblesValueLimit = <Runtime as pallet_nfts::Config<ListingsInstance>>::ValueLimit;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = benchmark_helpers::ListingsBenchmarkHelper;
}

pub type PaymentId = u32;

/// Payment ids are taken from a sequential counter.
pub struct SequentialPaymentId;

#[frame_support::storage_alias]
pub type NextPaymentId =
    StorageValue<Payments, PaymentId, frame_support::pallet_prelude::ValueQuery>;

impl fc_pallet_payments::GeneratePaymentId<AccountId> for SequentialPaymentId {
    type PaymentId = PaymentId;

    fn generate(_: &AccountId, _: &AccountId) -> Option<Self::PaymentId> {
        NextPaymentId::try_mutate(|id| {
            let current = *id;
            *id = id.checked_add(1).ok_or(())?;
            Ok::<_, ()>(current)
        })
        .ok()
    }
}

parameter_types! {
    pub const PaymentsPalletId: PalletId = PalletId(*b"fc/pays_");
    pub const MarketplaceFee: Percent = Percent::from_percent(1);
}

/// Both parties pay a fee (a percentage of the amount) to the treasury.
pub struct MarketplaceFeeHandler;
impl fc_pallet_payments::FeeHandler<Runtime> for MarketplaceFeeHandler {
    fn apply_fees(
        _: &fc_pallet_payments::AssetIdOf<Runtime>,
        _: &AccountId,
        _: &AccountId,
        amount: &fc_pallet_payments::BalanceOf<Runtime>,
        _: Option<&[u8]>,
    ) -> fc_pallet_payments::Fees<Runtime> {
        let fee = MarketplaceFee::get().mul_floor(*amount);
        let fees = || {
            frame_support::BoundedVec::truncate_from(alloc::vec![(
                TreasuryAccount::get(),
                fee,
                true
            )])
        };
        fc_pallet_payments::Fees {
            sender_pays: fees(),
            beneficiary_pays: fees(),
        }
    }
}

impl fc_pallet_payments::Config for Runtime {
    type PalletsOrigin = OriginCaller;
    type RuntimeHoldReason = RuntimeHoldReason;
    type WeightInfo = ();
    type SenderOrigin = EnsureSigned<AccountId>;
    type BeneficiaryOrigin = EnsureSigned<AccountId>;
    type DisputeResolver = EnsureRootWithSuccess<AccountId, TreasuryAccount>;
    type PaymentId = PaymentId;
    type Assets = Assets;
    type AssetsHold = AssetsHolder;
    type BlockNumberProvider = System;
    type FeeHandler = MarketplaceFeeHandler;
    type Scheduler = Scheduler;
    type Preimages = Preimage;
    type OnPaymentStatusChanged = Orders;
    type GeneratePaymentId = SequentialPaymentId;
    type PalletId = PaymentsPalletId;
    type IncentivePercentage = MarketplaceFee;
    type MaxRemarkLength = ConstU32<256>;
    type MaxFees = ConstU32<50>;
    type MaxDiscounts = ConstU32<50>;
    type CancelBufferBlockLength = ConstU32<DAYS>;
}

parameter_types! {
    pub const MaxCartLen: u32 = 10;
    pub const MaxItemLen: u32 = 64;
}

/// A signed origin, allowed to have up to `Limit` carts (or items per cart).
pub struct EnsureSignedWithLimit<Limit>(core::marker::PhantomData<Limit>);
impl<Limit: Get<u32>> EnsureOrigin<RuntimeOrigin> for EnsureSignedWithLimit<Limit> {
    type Success = (AccountId, u32);

    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        <EnsureSigned<AccountId> as EnsureOrigin<RuntimeOrigin>>::try_origin(o)
            .map(|who| (who, Limit::get()))
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        <EnsureSigned<AccountId> as EnsureOrigin<RuntimeOrigin>>::try_successful_origin()
    }
}

impl fc_pallet_orders::Config for Runtime {
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = ();
    type CreateOrigin = EnsureSignedWithLimit<MaxCartLen>;
    type OrderAdminOrigin = EnsureSignedWithLimit<MaxItemLen>;
    type PaymentOrigin = EnsureSigned<AccountId>;
    type OrderId = u32;
    type Listings = Listings;
    type Payments = Payments;
    type Scheduler = Scheduler;
    type BlockNumberProvider = System;
    type MaxLifetimeForCheckoutOrder = ConstU32<HOURS>;
    type MaxCartLen = MaxCartLen;
    type MaxItemLen = MaxItemLen;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = benchmark_helpers::OrdersBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmark_helpers {
    use super::*;

    use codec::Encode;
    use fc_pallet_communities::types::{AssetIdOf, CommunityIdOf, MembershipIdOf, PollIndexOf};
    use frame_benchmarking::BenchmarkError;
    use frame_contrib_traits::gas_tank::MakeTank;
    use frame_support::{
        traits::{
            schedule::DispatchTime,
            tokens::nonfungible_v2::{ItemOf, Mutate as _},
        },
        BoundedVec,
    };
    use frame_system::pallet_prelude::{OriginFor, RuntimeCallFor};
    use pallet_referenda::{BoundedCallOf, Curve, TrackIdOf, TrackInfo};
    use sp_runtime::{traits::IdentifyAccount, DispatchResult, MultiSigner, SaturatedConversion};

    type AccountPublic = <Signature as sp_runtime::traits::Verify>::Signer;

    /// Produces valid keys and signatures for `pallet-nfts`' benchmarks.
    fn nfts_signer() -> (AccountPublic, AccountId) {
        let public = sp_io::crypto::sr25519_generate(0.into(), None);
        let account = MultiSigner::Sr25519(public).into_account();
        (public.into(), account)
    }

    fn nfts_sign(signer: &AccountPublic, message: &[u8]) -> Signature {
        MultiSignature::Sr25519(
            sp_io::crypto::sr25519_sign(0.into(), &signer.clone().try_into().unwrap(), message)
                .unwrap(),
        )
    }

    // Gas transaction payment

    pub struct GasTransactionPaymentBenchmarkHelper;
    impl fc_pallet_gas_transaction_payment::BenchmarkHelper<Runtime>
        for GasTransactionPaymentBenchmarkHelper
    {
        type Ext = pallet_transaction_payment::ChargeTransactionPayment<Runtime>;

        fn ext() -> fc_pallet_gas_transaction_payment::ChargeTransactionPayment<Runtime, Self::Ext>
        {
            fc_pallet_gas_transaction_payment::ChargeTransactionPayment::new(
                pallet_transaction_payment::ChargeTransactionPayment::from(0),
            )
        }

        /// Gives `who` a membership with a gas tank that holds `gas`.
        fn setup_account(who: &AccountId, gas: Weight) -> DispatchResult {
            const GAS_COLLECTION: CommunityId = 0;
            const GAS_MEMBERSHIP: MembershipId = 0;

            Memberships::do_create_collection(
                GAS_COLLECTION,
                TreasuryAccount::get(),
                TreasuryAccount::get(),
                Default::default(),
                0,
                pallet_nfts::Event::ForceCreated {
                    collection: GAS_COLLECTION,
                    owner: TreasuryAccount::get(),
                },
            )?;
            <Memberships as frame_support::traits::tokens::nonfungibles_v2::Mutate<
                AccountId,
                pallet_nfts::ItemConfig,
            >>::mint_into(
                &GAS_COLLECTION,
                &GAS_MEMBERSHIP,
                who,
                &Default::default(),
                true,
            )?;
            MembershipsGasTank::make_tank(&(GAS_COLLECTION, GAS_MEMBERSHIP), Some(gas), None)
        }
    }

    // Communities

    pub struct MembershipsBenchmarkHelper;
    impl
        pallet_nfts::BenchmarkHelper<CommunityId, MembershipId, AccountPublic, AccountId, Signature>
        for MembershipsBenchmarkHelper
    {
        fn collection(i: u16) -> CommunityId {
            i
        }
        fn item(i: u16) -> MembershipId {
            i.into()
        }
        fn signer() -> (AccountPublic, AccountId) {
            nfts_signer()
        }
        fn sign(signer: &AccountPublic, message: &[u8]) -> Signature {
            nfts_sign(signer, message)
        }
    }

    type MembershipCollection = ItemOf<Memberships, frame_support::traits::ConstU16<0>, AccountId>;

    fn create_memberships_collection(id: CommunityId, owner: AccountId) -> DispatchResult {
        Memberships::do_create_collection(
            id,
            owner.clone(),
            owner.clone(),
            Default::default(),
            0,
            pallet_nfts::Event::ForceCreated {
                collection: id,
                owner,
            },
        )
    }

    pub struct CommunitiesBenchmarkHelper;
    impl fc_pallet_communities::BenchmarkHelper<Runtime> for CommunitiesBenchmarkHelper {
        fn community_id() -> CommunityIdOf<Runtime> {
            1
        }

        fn community_asset_id() -> AssetIdOf<Runtime> {
            1
        }

        fn community_desired_size() -> u32 {
            u8::MAX as u32
        }

        fn initialize_memberships_collection() -> Result<(), BenchmarkError> {
            // The memberships manager collection, which holds every membership.
            create_memberships_collection(0, TreasuryAccount::get())?;
            // The collection of the community's memberships.
            let community_id = Self::community_id();
            create_memberships_collection(
                community_id,
                Communities::community_account(&community_id),
            )?;
            Ok(())
        }

        fn issue_membership(
            community_id: CommunityIdOf<Runtime>,
            membership_id: MembershipIdOf<Runtime>,
        ) -> Result<(), BenchmarkError> {
            let community_account = Communities::community_account(&community_id);
            MembershipCollection::mint_into(
                &membership_id,
                &community_account,
                &Default::default(),
                true,
            )?;
            Ok(())
        }

        fn prepare_track(track_origin: PalletsOriginOf<Runtime>) -> Result<(), BenchmarkError> {
            let info = TrackInfo {
                name: sp_runtime::str_array("Community"),
                max_deciding: 1,
                decision_deposit: 5,
                prepare_period: 1,
                decision_period: 5,
                confirm_period: 1,
                min_enactment_period: 1,
                min_approval: Curve::LinearDecreasing {
                    length: Perbill::from_percent(100),
                    floor: Perbill::from_percent(50),
                    ceil: Perbill::from_percent(100),
                },
                min_support: Curve::LinearDecreasing {
                    length: Perbill::from_percent(100),
                    floor: Perbill::from_percent(0),
                    ceil: Perbill::from_percent(100),
                },
            };
            CommunityTracks::do_insert(Self::community_id(), info, track_origin)?;
            Ok(())
        }

        fn prepare_poll(
            origin: OriginFor<Runtime>,
            proposal_origin: PalletsOriginOf<Runtime>,
            proposal_call: RuntimeCallFor<Runtime>,
        ) -> Result<PollIndexOf<Runtime>, BenchmarkError> {
            let proposal = BoundedCallOf::<Runtime, ()>::Inline(BoundedVec::truncate_from(
                proposal_call.encode(),
            ));
            CommunityReferenda::submit(
                origin.clone(),
                Box::new(proposal_origin),
                proposal,
                DispatchTime::After(1),
            )?;
            let index = pallet_referenda::ReferendumCount::<Runtime>::get() - 1;
            CommunityReferenda::place_decision_deposit(origin, index)?;

            System::set_block_number(System::block_number() + 1);
            CommunityReferenda::nudge_referendum(RuntimeOrigin::root(), index)?;

            Ok(index)
        }

        fn finish_poll(index: PollIndexOf<Runtime>) -> Result<(), BenchmarkError> {
            // Past the decision period (see `prepare_track`), the poll is decided.
            System::set_block_number(System::block_number() + 6);
            CommunityReferenda::nudge_referendum(RuntimeOrigin::root(), index)?;
            System::set_block_number(System::block_number() + 1);
            CommunityReferenda::nudge_referendum(RuntimeOrigin::root(), index)?;

            CommunityReferenda::ensure_ongoing(index)
                .err()
                .map(|_| ())
                .ok_or(BenchmarkError::Stop("poll is still ongoing"))
        }
    }

    pub struct TracksBenchmarkHelper;
    impl fc_pallet_referenda_tracks::BenchmarkHelper<Runtime> for TracksBenchmarkHelper {
        fn track_id(id: u32) -> TrackIdOf<Runtime, ()> {
            id.saturated_into()
        }
    }

    // Marketplace

    pub struct ListingsCatalogBenchmarkHelper;
    impl
        pallet_nfts::BenchmarkHelper<
            InventoryIdFor<Runtime>,
            ItemIdOf<Runtime>,
            AccountPublic,
            AccountId,
            Signature,
        > for ListingsCatalogBenchmarkHelper
    {
        fn collection(i: u16) -> InventoryIdFor<Runtime> {
            InventoryId(i.into(), 1)
        }
        fn item(i: u16) -> ItemIdOf<Runtime> {
            i.into()
        }
        fn signer() -> (AccountPublic, AccountId) {
            nfts_signer()
        }
        fn sign(signer: &AccountPublic, message: &[u8]) -> Signature {
            nfts_sign(signer, message)
        }
    }

    pub struct ListingsBenchmarkHelper;
    impl fc_pallet_listings::BenchmarkHelper<InventoryIdFor<Runtime>> for ListingsBenchmarkHelper {
        fn inventory_id() -> InventoryIdFor<Runtime> {
            InventoryId(1, 1)
        }
    }

    pub struct OrdersBenchmarkHelper;
    impl fc_pallet_orders::BenchmarkHelper<Runtime> for OrdersBenchmarkHelper {
        type Balances = Balances;
        type Assets = Assets;
        type InventoryDeposit =
            <Runtime as pallet_nfts::Config<ListingsInstance>>::CollectionDeposit;
        type ItemDeposit = <Runtime as pallet_nfts::Config<ListingsInstance>>::ItemDeposit;

        fn inventory_id() -> (MerchantId, u32) {
            (1, 1)
        }

        fn item_id(i: usize) -> ItemIdOf<Runtime> {
            i as u32
        }
    }
}
