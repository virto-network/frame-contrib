//! Test environment: listings subscriptions over the real payments pallet, group memberships in
//! their own `pallet_nfts` collections, the transaction-payment pallet as the fee path, and a
//! settable chain clock.

use crate::{self as fc_pallet_usage_subscription};
use core::cell::{Cell, RefCell};
use fc_pallet_listings::{InventoryId, InventoryIdFor};
use fc_traits_memberships::GroupCollectionMemberships;
use frame_support::{
    derive_impl, ord_parameter_types, parameter_types,
    traits::{
        nonfungibles_v2::Create, AsEnsureOriginWithArg, ConstU32, ConstU64, Contains, EnsureOrigin,
        EnsureOriginWithArg, EqualPrivilegeOnly, Hooks,
    },
    weights::{FixedFee, Weight},
    PalletId,
};
use frame_system::{
    limits::BlockWeights, pallet_prelude::BlockNumberFor, EnsureNever, EnsureRoot, EnsureSigned,
    EnsureSignedBy, RawOrigin,
};
use sp_runtime::{
    traits::{BlockNumberProvider, Convert, IdentifyAccount, IdentityLookup, Verify},
    BuildStorage, MultiSignature, Perbill, Percent,
};

pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub type Balance = u128;
pub type AssetId = u32;

pub type TxExtensions = (
    frame_system::CheckWeight<Test>,
    pallet_transaction_payment::ChargeTransactionPayment<Test>,
);
pub type UncheckedExtrinsic =
    sp_runtime::generic::UncheckedExtrinsic<AccountId, RuntimeCall, MultiSignature, TxExtensions>;
pub type Block = sp_runtime::generic::Block<
    sp_runtime::generic::Header<u64, sp_runtime::traits::BlakeTwo256>,
    UncheckedExtrinsic,
>;

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
    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type Assets = pallet_assets;
    #[runtime::pallet_index(12)]
    pub type AssetsHolder = pallet_assets_holder;
    #[runtime::pallet_index(13)]
    pub type TransactionPayment = pallet_transaction_payment;
    #[runtime::pallet_index(15)]
    pub type Payments = fc_pallet_payments;
    #[runtime::pallet_index(20)]
    pub type Listings = fc_pallet_listings;
    #[runtime::pallet_index(21)]
    pub type ListingsCatalog = pallet_nfts<Instance2>;
    #[runtime::pallet_index(30)]
    pub type Memberships = pallet_nfts<Instance1>;
    #[runtime::pallet_index(40)]
    pub type UsageSubscription = fc_pallet_usage_subscription;
}

/// The base weight of every extrinsic, small so tests can state metered weights exactly.
pub const BASE_EXTRINSIC: Weight = Weight::from_parts(5, 0);

parameter_types! {
    pub MockBlockWeights: BlockWeights = {
        let mut weights = BlockWeights::with_sensible_defaults(
            Weight::from_parts(2_000_000_000_000, u64::MAX),
            Perbill::from_percent(75),
        );
        for class in frame_support::dispatch::DispatchClass::all() {
            weights.per_class.get_mut(*class).base_extrinsic = BASE_EXTRINSIC;
        }
        weights
    };
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = AccountId;
    type Block = Block;
    type BlockWeights = MockBlockWeights;
    type Lookup = IdentityLookup<Self::AccountId>;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = Balance;
    type AccountStore = System;
}

#[derive_impl(pallet_assets::config_preludes::TestDefaultConfig)]
impl pallet_assets::Config for Test {
    type Balance = Balance;
    type AssetId = AssetId;
    type AssetIdParameter = AssetId;
    type Currency = Balances;
    type ForceOrigin = EnsureRoot<AccountId>;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type Freezer = ();
    type Holder = AssetsHolder;
}

impl pallet_assets_holder::Config for Test {
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeEvent = RuntimeEvent;
}

#[derive_impl(pallet_transaction_payment::config_preludes::TestDefaultConfig)]
impl pallet_transaction_payment::Config for Test {
    type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
    type WeightToFee = frame_support::weights::IdentityFee<Balance>;
    type LengthToFee = FixedFee<0, Balance>;
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
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type OriginPrivilegeCmp = EqualPrivilegeOnly;
    type MaxScheduledPerBlock = ConstU32<100>;
    type WeightInfo = ();
    type Preimages = ();
    type BlockNumberProvider = System;
}

thread_local! {
    static LAST_PAYMENT_ID: Cell<u32> = const { Cell::new(0) };
    static CLOCK: Cell<u64> = const { Cell::new(0) };
    static UNUSABLE: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

/// Payment ids from a counter.
pub struct PaymentIds;
impl fc_pallet_payments::GeneratePaymentId<AccountId> for PaymentIds {
    type PaymentId = u32;

    fn generate(_: &AccountId, _: &AccountId) -> Option<u32> {
        LAST_PAYMENT_ID.with(|id| {
            id.set(id.get() + 1);
            Some(id.get())
        })
    }
}

parameter_types! {
    pub const PaymentsPalletId: PalletId = PalletId(*b"payments");
    pub const IncentivePercentage: Percent = Percent::from_percent(0);
}

impl fc_pallet_payments::Config for Test {
    type PalletsOrigin = OriginCaller;
    type RuntimeHoldReason = RuntimeHoldReason;
    type WeightInfo = ();
    type SenderOrigin = EnsureSigned<AccountId>;
    type BeneficiaryOrigin = EnsureSigned<AccountId>;
    type DisputeResolver = EnsureNever<AccountId>;
    type PaymentId = u32;
    type Assets = Assets;
    type AssetsHold = AssetsHolder;
    type BlockNumberProvider = System;
    type FeeHandler = ();
    type Scheduler = Scheduler;
    type Preimages = ();
    type OnPaymentStatusChanged = ();
    type GeneratePaymentId = PaymentIds;
    type PalletId = PaymentsPalletId;
    type IncentivePercentage = IncentivePercentage;
    type MaxRemarkLength = ConstU32<64>;
    type MaxFees = ConstU32<10>;
    type MaxDiscounts = ConstU32<10>;
    type CancelBufferBlockLength = ConstU64<10>;
}

/// The chain clock of the tests (relay blocks), set by hand.
pub struct Clock;
impl Clock {
    /// Sets the current tick.
    pub fn set(tick: u64) {
        CLOCK.with(|clock| clock.set(tick));
    }

    /// The current tick.
    pub fn now() -> u64 {
        CLOCK.with(|clock| clock.get())
    }
}
impl BlockNumberProvider for Clock {
    type BlockNumber = u64;

    fn current_block_number() -> u64 {
        Self::now()
    }

    fn set_block_number(tick: u64) {
        Self::set(tick)
    }
}

/// Ticks of the chain clock (relay blocks, 6 s).
pub const MINUTES: u64 = 10;
pub const HOURS: u64 = 60 * MINUTES;
pub const DAYS: u64 = 24 * HOURS;

parameter_types! {
    pub const RenewalLead: u64 = HOURS;
    pub const DueBucketSize: u64 = 10 * MINUTES;
    pub const MaxDuePerBucket: u32 = 16;
    pub const MaxChargesPerBlock: u32 = 16;
}

// The listings catalog: offers are its items.

parameter_types! {
    pub const NoDeposit: Balance = 0;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct CatalogBenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl
    pallet_nfts::BenchmarkHelper<
        InventoryIdFor<Test>,
        u32,
        sp_runtime::MultiSigner,
        AccountId,
        MultiSignature,
    > for CatalogBenchmarkHelper
{
    fn collection(i: u16) -> InventoryIdFor<Test> {
        InventoryId(i.into(), 0)
    }
    fn item(i: u16) -> u32 {
        i.into()
    }
    fn signer() -> (sp_runtime::MultiSigner, AccountId) {
        <() as pallet_nfts::BenchmarkHelper<
            u16,
            u16,
            sp_runtime::MultiSigner,
            AccountId,
            MultiSignature,
        >>::signer()
    }
    fn sign(signer: &sp_runtime::MultiSigner, message: &[u8]) -> MultiSignature {
        <() as pallet_nfts::BenchmarkHelper<
            u16,
            u16,
            sp_runtime::MultiSigner,
            AccountId,
            MultiSignature,
        >>::sign(signer, message)
    }
}

impl pallet_nfts::Config<pallet_nfts::Instance2> for Test {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = InventoryIdFor<Test>;
    type ItemId = u32;
    type Currency = Balances;
    type ForceOrigin = EnsureRoot<AccountId>;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureNever<AccountId>>;
    type Locker = ();
    type CollectionDeposit = NoDeposit;
    type ItemDeposit = NoDeposit;
    type MetadataDepositBase = NoDeposit;
    type AttributeDepositBase = NoDeposit;
    type DepositPerByte = NoDeposit;
    type StringLimit = ConstU32<256>;
    type KeyLimit = ConstU32<64>;
    type ValueLimit = ConstU32<256>;
    type ApprovalsLimit = ();
    type ItemAttributesApprovalsLimit = ();
    type MaxTips = ();
    type MaxDeadlineDuration = ();
    type MaxAttributesPerCall = ();
    type Features = ();
    type OffchainSignature = MultiSignature;
    type OffchainPublic = AccountPublic;
    #[cfg(feature = "runtime-benchmarks")]
    type Helper = CatalogBenchmarkHelper;
    type WeightInfo = ();
    type BlockNumberProvider = System;
}

/// Direct subscription is closed on the collective's merchant, 0 (`DEC-8`): usage contracts only
/// go through the usage-subscription pallet.
pub struct SubscribeOutsideCollective;
impl EnsureOriginWithArg<RuntimeOrigin, InventoryIdFor<Test>> for SubscribeOutsideCollective {
    type Success = AccountId;

    fn try_origin(
        o: RuntimeOrigin,
        InventoryId(merchant, _): &InventoryIdFor<Test>,
    ) -> Result<AccountId, RuntimeOrigin> {
        match o.clone().into() {
            Ok(RawOrigin::Signed(who)) if *merchant != 0 => Ok(who),
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin(_: &InventoryIdFor<Test>) -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::signed(ALICE))
    }
}

impl fc_pallet_listings::Config for Test {
    type WeightInfo = ();
    type CreateInventoryOrigin = AsEnsureOriginWithArg<EnsureNever<AccountId>>;
    type InventoryAdminOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
    type MerchantId = u32;
    type InventoryId = u32;
    type ItemSKU = u32;
    type CollectionConfig =
        pallet_nfts::CollectionConfig<Balance, BlockNumberFor<Self>, InventoryIdFor<Self>>;
    type ItemConfig = pallet_nfts::ItemConfig;
    type Balances = Balances;
    type Assets = Assets;
    type Nonfungibles = ListingsCatalog;
    type NonfungiblesKeyLimit = ConstU32<64>;
    type NonfungiblesValueLimit = ConstU32<256>;
    type SubscribeOrigin = SubscribeOutsideCollective;
    type Payments = Payments;
    type BlockNumberProvider = Clock;
    type RenewalLead = RenewalLead;
    type DueBucketSize = DueBucketSize;
    type MaxDuePerBucket = MaxDuePerBucket;
    type MaxChargesPerBlock = MaxChargesPerBlock;
    type OnSubscriptionChanged = UsageSubscription;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = Self;
}

#[cfg(feature = "runtime-benchmarks")]
impl fc_pallet_listings::BenchmarkHelper<InventoryIdFor<Test>> for Test {
    fn inventory_id() -> InventoryIdFor<Test> {
        InventoryId(7, 0)
    }
}

#[cfg(feature = "runtime-benchmarks")]
impl fc_pallet_listings::SubscriptionsBenchmarkHelper<AccountId, AssetId, Balance> for Test {
    fn price() -> (AssetId, Balance) {
        (ASSET, 10)
    }

    fn fund(who: &AccountId, asset: &AssetId, amount: Balance) {
        use frame_support::traits::fungibles::Mutate;
        let _ = Assets::mint_into(*asset, who, amount.saturating_mul(1_000));
    }
}

// Group memberships: each group's own collection, owned by the memberships manager.

parameter_types! {
    pub MembershipsManagerAccount: AccountId = AccountId::new([0x4d; 32]);
}

impl pallet_nfts::Config<pallet_nfts::Instance1> for Test {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = u32;
    type ItemId = u32;
    type Currency = Balances;
    type ForceOrigin = EnsureRoot<AccountId>;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureNever<AccountId>>;
    type Locker = ();
    type CollectionDeposit = NoDeposit;
    type ItemDeposit = NoDeposit;
    type MetadataDepositBase = NoDeposit;
    type AttributeDepositBase = NoDeposit;
    type DepositPerByte = NoDeposit;
    type StringLimit = ConstU32<256>;
    type KeyLimit = ConstU32<64>;
    type ValueLimit = ConstU32<256>;
    type ApprovalsLimit = ();
    type ItemAttributesApprovalsLimit = ();
    type MaxTips = ();
    type MaxDeadlineDuration = ();
    type MaxAttributesPerCall = ();
    type Features = ();
    type OffchainSignature = MultiSignature;
    type OffchainPublic = AccountPublic;
    #[cfg(feature = "runtime-benchmarks")]
    type Helper = ();
    type WeightInfo = ();
    type BlockNumberProvider = System;
}

/// The account of group `g`: `[g as le bytes, 0xEE…]`. Naming it reads nothing.
pub struct GroupAccountOf;
impl Convert<u32, AccountId> for GroupAccountOf {
    fn convert(group: u32) -> AccountId {
        let mut bytes = [0xEE; 32];
        bytes[..4].copy_from_slice(&group.to_le_bytes());
        AccountId::new(bytes)
    }
}

/// The group a group account belongs to, if it is one.
pub fn group_of_account(who: &AccountId) -> Option<u32> {
    let bytes: &[u8; 32] = who.as_ref();
    (bytes[4..] == [0xEE; 28]).then(|| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

pub type MembershipsManager = GroupCollectionMemberships<
    Memberships,
    pallet_nfts::ItemConfig,
    GroupAccountOf,
    MembershipsManagerAccount,
>;

/// Group 0 is the collective's (`REQ-GR-4`); a test may mark any other group unusable.
pub struct UsableGroups;
impl UsableGroups {
    pub fn set_unusable(group: u32) {
        UNUSABLE.with(|u| u.borrow_mut().push(group));
    }
}
impl Contains<u32> for UsableGroups {
    fn contains(group: &u32) -> bool {
        *group != 0 && !UNUSABLE.with(|u| u.borrow().contains(group))
    }
}

/// A group's administrative origin: a signed origin from the group's account.
pub struct EnsureGroup;
impl EnsureOrigin<RuntimeOrigin> for EnsureGroup {
    type Success = u32;

    fn try_origin(o: RuntimeOrigin) -> Result<u32, RuntimeOrigin> {
        match o.clone().into() {
            Ok(RawOrigin::Signed(who)) => group_of_account(&who).ok_or(o),
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::signed(GroupAccountOf::convert(GROUP_A)))
    }
}

pub const ADMIN: AccountId = AccountId::new([0xAD; 32]);
pub const AMENDER: AccountId = AccountId::new([0xA3; 32]);
pub const PAYEE: AccountId = AccountId::new([0xBE; 32]);

ord_parameter_types! {
    pub const Admin: AccountId = ADMIN;
    pub const Amender: AccountId = AMENDER;
}

parameter_types! {
    pub const Payee: AccountId = PAYEE;
    pub const OfferInventory: (u32, u32) = (0, 0);
    pub const MinUsagePeriod: u64 = HOURS;
    pub const MinBillingPeriod: u64 = DAYS;
    pub const MaxTrialPeriods: u32 = 3;
}

impl fc_pallet_usage_subscription::Config for Test {
    type WeightInfo = ();
    type Memberships = MembershipsManager;
    type GroupAccount = GroupAccountOf;
    type UsableGroup = UsableGroups;
    type Subscriptions = Listings;
    type OfferInventory = OfferInventory;
    type Payee = Payee;
    type StandardOfferOrigin = EnsureSignedBy<Admin, AccountId>;
    type CustomOfferOrigin = EnsureRoot<AccountId>;
    type TerminateOrigin = EnsureRoot<AccountId>;
    type AmendOrigin = EnsureSignedBy<Amender, AccountId>;
    type GroupOrigin = EnsureGroup;
    type BlockNumberProvider = Clock;
    type MinUsagePeriod = MinUsagePeriod;
    type MinBillingPeriod = MinBillingPeriod;
    type MaxTrialPeriods = MaxTrialPeriods;
}

// Accounts, groups and assets of the tests.

pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);
pub const CHARLIE: AccountId = AccountId::new([3u8; 32]);

pub const GROUP_A: u32 = 1;
pub const GROUP_B: u32 = 2;
pub const GROUP_C: u32 = 3;
/// The collective's own group, never usable.
pub const COLLECTIVE_GROUP: u32 = 0;

/// The asset offers are priced in: sufficient, with a minimum balance of 1.
pub const ASSET: AssetId = 1;
/// What each group account holds of [`ASSET`] at genesis.
pub const GROUP_FUNDS: Balance = 1_000_000;

pub fn group_account(group: u32) -> AccountId {
    GroupAccountOf::convert(group)
}

pub fn group_origin(group: u32) -> RuntimeOrigin {
    RuntimeOrigin::signed(group_account(group))
}

/// Creates the collection of `group`, owned by the memberships manager.
pub fn create_group(group: u32) {
    let manager = MembershipsManagerAccount::get();
    assert!(
        <Memberships as Create<AccountId, _>>::create_collection_with_id(
            group,
            &manager,
            &manager,
            &Default::default(),
        )
        .is_ok()
    );
}

/// Moves the clock to `tick`, and lets listings process its due queue.
pub fn advance_to(tick: u64) {
    Clock::set(tick);
    <Listings as Hooks<BlockNumberFor<Test>>>::on_idle(
        System::block_number(),
        Weight::from_parts(u64::MAX / 4, u64::MAX / 4),
    );
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    let mut balances = mock_helpers::BalancesExtBuilder::<Test>::default()
        .with_account(PAYEE, 1_000)
        .with_account(ALICE, 1_000_000)
        .with_account(BOB, 1_000_000)
        .with_account(CHARLIE, 1_000_000);
    let mut asset = mock_helpers::Asset::new(ASSET, PAYEE, 1, true);
    for group in [GROUP_A, GROUP_B, GROUP_C, COLLECTIVE_GROUP] {
        balances = balances.with_account(group_account(group), 1_000);
        asset = asset.add_account(group_account(group), GROUP_FUNDS);
    }
    use mock_helpers::ExtHelper;
    balances
        .as_storage()
        .assimilate_storage(&mut storage)
        .unwrap();
    mock_helpers::AssetsExtBuilder::<Test>::default()
        .with_asset(asset)
        .as_storage()
        .assimilate_storage(&mut storage)
        .unwrap();

    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
        System::set_block_number(1);
        Clock::set(0);
        UNUSABLE.with(|u| u.borrow_mut().clear());
        for group in [COLLECTIVE_GROUP, GROUP_A, GROUP_B, GROUP_C] {
            create_group(group);
        }
    });
    ext
}
