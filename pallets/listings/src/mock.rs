//! Test environment for template pallet.

use crate::{
    self as pallet_listings, test_utils::SignedMerchantId, EndReason, InventoryId, InventoryIdFor,
    InventoryIdTuple, ItemIdOf, ReplacementDropReason,
};
use core::cell::{Cell, RefCell};
use fc_traits_listings::OnSubscriptionChanged;
use mock_helpers::ExtHelper;

use frame_support::{
    derive_impl,
    traits::{AsEnsureOriginWithArg, EnsureOriginWithArg, EqualPrivilegeOnly, Get},
    weights::Weight,
    PalletId,
};
use frame_system::{
    pallet_prelude::BlockNumberFor, EnsureNever, EnsureRoot, EnsureSigned, RawOrigin,
};
use sp_core::{parameter_types, ConstU32, ConstU64};
use sp_io::TestExternalities;
use sp_runtime::{
    traits::{BlockNumberProvider, IdentifyAccount, IdentityLookup, Verify},
    BuildStorage, DispatchError, MultiSignature, Percent,
};

pub type Block = frame_system::mocking::MockBlock<Test>;
pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub type AssetId = <Test as pallet_assets::Config>::AssetId;
pub type Balance = <Test as pallet_balances::Config>::Balance;
type ExistentialDeposit = <Test as pallet_balances::Config>::ExistentialDeposit;

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
    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type Assets = pallet_assets;
    #[runtime::pallet_index(12)]
    pub type AssetsHolder = pallet_assets_holder;
    #[runtime::pallet_index(15)]
    pub type Payments = fc_pallet_payments;
    #[runtime::pallet_index(20)]
    pub type Listings = pallet_listings;
    #[runtime::pallet_index(21)]
    pub type ListingsCatalog = pallet_nfts;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = AccountId;
    type Block = Block;
    type Lookup = IdentityLookup<Self::AccountId>;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

#[derive_impl(pallet_assets::config_preludes::TestDefaultConfig)]
impl pallet_assets::Config for Test {
    type Balance = Balance;
    type Currency = Balances;
    type ForceOrigin = EnsureRoot<AccountId>;
    type CreateOrigin = EnsureSigned<AccountId>;
    type Freezer = ();
    type Holder = AssetsHolder;
}

impl pallet_assets_holder::Config for Test {
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeEvent = RuntimeEvent;
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
    static HOOKS: RefCell<Vec<Hook>> = const { RefCell::new(Vec::new()) };
    static REFUSE_REPLACEMENT: Cell<bool> = const { Cell::new(false) };
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

/// The chain clock of the tests, set by hand.
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

/// The key a hook receives.
pub type HookKey = (InventoryIdTuple<Test>, ItemIdOf<Test>, AccountId);

/// A recorded call to [`OnSubscriptionChanged`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hook {
    Started(HookKey),
    Charged(HookKey, u32, u64),
    Suspended(HookKey, u64),
    Restored(HookKey),
    Defaulted(HookKey, u64),
    AmendmentEnacted(HookKey, u64),
    ItemAmendmentEnacted(InventoryIdTuple<Test>, ItemIdOf<Test>, u32, u64),
    AmendmentInForce(HookKey, u64),
    Ended(HookKey, EndReason),
    Replaced(HookKey, ItemIdOf<Test>, u64),
    ReplacementDropped(HookKey, ItemIdOf<Test>, ReplacementDropReason),
}

/// The error a refused replacement reports.
pub const REPLACEMENT_REFUSED: DispatchError = DispatchError::Other("replacement refused");

/// Records every hook; refuses replacements while told to.
pub struct RecordHooks;
impl RecordHooks {
    fn push(hook: Hook) {
        HOOKS.with(|hooks| hooks.borrow_mut().push(hook));
    }

    /// Takes the hooks recorded so far.
    pub fn take() -> Vec<Hook> {
        HOOKS.with(|hooks| hooks.take())
    }

    /// Makes `allow_replacement` refuse (or allow) from now on.
    pub fn refuse_replacements(refuse: bool) {
        REFUSE_REPLACEMENT.with(|r| r.set(refuse));
    }
}

impl OnSubscriptionChanged<InventoryIdTuple<Test>, ItemIdOf<Test>, AccountId, u64> for RecordHooks {
    fn max_hook_weight() -> Weight {
        HookWeight::get()
    }
    fn on_started(inventory: &InventoryIdTuple<Test>, item: &u32, who: &AccountId) {
        Self::push(Hook::Started((*inventory, *item, who.clone())));
    }
    fn on_charged(
        inventory: &InventoryIdTuple<Test>,
        item: &u32,
        who: &AccountId,
        period: u32,
        paid_through: u64,
    ) {
        Self::push(Hook::Charged(
            (*inventory, *item, who.clone()),
            period,
            paid_through,
        ));
    }
    fn on_suspended(inventory: &InventoryIdTuple<Test>, item: &u32, who: &AccountId, grace: u64) {
        Self::push(Hook::Suspended((*inventory, *item, who.clone()), grace));
    }
    fn on_restored(inventory: &InventoryIdTuple<Test>, item: &u32, who: &AccountId) {
        Self::push(Hook::Restored((*inventory, *item, who.clone())));
    }
    fn on_defaulted(inventory: &InventoryIdTuple<Test>, item: &u32, who: &AccountId, until: u64) {
        Self::push(Hook::Defaulted((*inventory, *item, who.clone()), until));
    }
    fn on_amendment_enacted(
        inventory: &InventoryIdTuple<Test>,
        item: &u32,
        who: &AccountId,
        at: u64,
    ) {
        Self::push(Hook::AmendmentEnacted((*inventory, *item, who.clone()), at));
    }
    fn on_item_amendment_enacted(
        inventory: &InventoryIdTuple<Test>,
        item: &u32,
        seq: u32,
        enacted_at: u64,
    ) {
        Self::push(Hook::ItemAmendmentEnacted(
            *inventory, *item, seq, enacted_at,
        ));
    }
    fn on_amendment_in_force(
        inventory: &InventoryIdTuple<Test>,
        item: &u32,
        who: &AccountId,
        at: u64,
    ) {
        Self::push(Hook::AmendmentInForce((*inventory, *item, who.clone()), at));
    }
    fn on_ended(inventory: &InventoryIdTuple<Test>, item: &u32, who: &AccountId, why: EndReason) {
        Self::push(Hook::Ended((*inventory, *item, who.clone()), why));
    }
    fn on_replaced(
        inventory: &InventoryIdTuple<Test>,
        item: &u32,
        who: &AccountId,
        new_item: &u32,
        new_anchor: u64,
    ) {
        Self::push(Hook::Replaced(
            (*inventory, *item, who.clone()),
            *new_item,
            new_anchor,
        ));
    }
    fn allow_replacement(
        _: &InventoryIdTuple<Test>,
        _: &u32,
        _: &AccountId,
        _: &u32,
    ) -> Result<(), DispatchError> {
        if REFUSE_REPLACEMENT.with(|r| r.get()) {
            Err(REPLACEMENT_REFUSED)
        } else {
            Ok(())
        }
    }
    fn on_replacement_dropped(
        inventory: &InventoryIdTuple<Test>,
        item: &u32,
        who: &AccountId,
        new_item: &u32,
        reason: ReplacementDropReason,
    ) {
        Self::push(Hook::ReplacementDropped(
            (*inventory, *item, who.clone()),
            *new_item,
            reason,
        ));
    }
}

/// Ticks of the chain clock (relay blocks, 6 s).
pub const MINUTES: u64 = 10;
pub const HOURS: u64 = 60 * MINUTES;
pub const DAYS: u64 = 24 * HOURS;

frame_support::parameter_types! {
    /// The worst-case weight the recording hooks declare: what a real dependant's hooks would.
    pub storage HookWeight: Weight = Weight::from_parts(7_000_000, 3_000);
    pub storage RenewalLead: u64 = DAYS;
    pub const DueBucketSize: u64 = HOURS;
    pub storage MaxDuePerBucket: u32 = 8;
    pub storage MaxChargesPerBlock: u32 = 4;
}

parameter_types! {
    pub CollectionDeposit: Balance = 1000;
    pub ItemDeposit: Balance = 100;
    pub MetadataDepositBase: Balance = 10;
    pub AttributeDepositBase: Balance = 10;
    pub DepositPerByte: Balance = 1;
}

impl pallet_nfts::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = InventoryIdFor<Test>;
    type ItemId = ItemIdOf<Test>;
    type Currency = Balances;
    type ForceOrigin = EnsureNever<AccountId>;
    type CreateOrigin = EnsureNever<AccountId>;
    type Locker = ();
    type CollectionDeposit = CollectionDeposit;
    type ItemDeposit = ItemDeposit;
    type MetadataDepositBase = MetadataDepositBase;
    type AttributeDepositBase = AttributeDepositBase;
    type DepositPerByte = DepositPerByte;
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
    type Helper = benchmarks::OwnersCatalogBenchmarkHelper<Self>;
    type WeightInfo = ();
    type BlockNumberProvider = System;
}

pub struct EnsureAccountIdInventories;

impl<Id> EnsureOriginWithArg<RuntimeOrigin, InventoryId<SignedMerchantId, Id>>
    for EnsureAccountIdInventories
{
    type Success = AccountId;

    fn try_origin(
        o: RuntimeOrigin,
        InventoryId(account_bytes, _): &InventoryId<SignedMerchantId, Id>,
    ) -> Result<Self::Success, RuntimeOrigin> {
        match Into::<Result<RawOrigin<AccountId>, RuntimeOrigin>>::into(o.clone())? {
            RawOrigin::Signed(ref who)
                if account_bytes.eq(<AccountId as AsRef<[u8]>>::as_ref(who)) =>
            {
                Ok(who.clone())
            }
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin(
        InventoryId(public, _): &InventoryId<SignedMerchantId, Id>,
    ) -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::signed(AccountId::new(public.0)))
    }
}

impl pallet_listings::Config for Test {
    type WeightInfo = ();
    type CreateInventoryOrigin = EnsureAccountIdInventories;
    type InventoryAdminOrigin = EnsureAccountIdInventories;
    type MerchantId = SignedMerchantId;
    type InventoryId = u32;
    type ItemSKU = u32;
    type CollectionConfig =
        pallet_nfts::CollectionConfig<Balance, BlockNumberFor<Self>, InventoryIdFor<Self>>;
    type ItemConfig = pallet_nfts::ItemConfig;
    type Balances = Balances;
    type Assets = Assets;
    type Nonfungibles = ListingsCatalog;
    type NonfungiblesKeyLimit = <Self as pallet_nfts::Config>::KeyLimit;
    type NonfungiblesValueLimit = <Self as pallet_nfts::Config>::ValueLimit;
    type SubscribeOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type Payments = Payments;
    type BlockNumberProvider = Clock;
    type RenewalLead = RenewalLead;
    type DueBucketSize = DueBucketSize;
    type MaxDuePerBucket = MaxDuePerBucket;
    type MaxChargesPerBlock = MaxChargesPerBlock;
    type OnSubscriptionChanged = RecordHooks;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = Self;
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks {
    use super::*;
    use pallet_nfts::BenchmarkHelper;
    use sp_runtime::{AccountId32, MultiSignature, MultiSigner};

    pub struct OwnersCatalogBenchmarkHelper<T, I = ()>(core::marker::PhantomData<(T, I)>);

    impl<T, I: 'static>
        BenchmarkHelper<
            InventoryIdFor<Test>,
            ItemIdOf<Test>,
            MultiSigner,
            AccountId32,
            MultiSignature,
        > for OwnersCatalogBenchmarkHelper<T, I>
    where
        T: pallet_nfts::Config<I>,
    {
        fn collection(i: u16) -> InventoryIdFor<Test> {
            fn convert(i: u16) -> [u8; 32] {
                let high = (i >> 8) as u8;
                let low = (i & 0xFF) as u8;
                let mut j = [0u8; 32];

                for idx in 0..16 {
                    j[2 * idx] = high;
                    j[2 * idx + 1] = low;
                }

                j
            }

            InventoryId(convert(i).into(), 1u16.into())
        }

        fn item(i: u16) -> ItemIdOf<Test> {
            i.into()
        }

        fn signer() -> (sp_runtime::MultiSigner, sp_runtime::AccountId32) {
            <() as BenchmarkHelper<
                u16,
                u16,
                sp_runtime::MultiSigner,
                sp_runtime::AccountId32,
                MultiSignature,
            >>::signer()
        }

        fn sign(signer: &sp_runtime::MultiSigner, message: &[u8]) -> MultiSignature {
            <() as BenchmarkHelper<
                u16,
                u16,
                sp_runtime::MultiSigner,
                sp_runtime::AccountId32,
                MultiSignature,
            >>::sign(signer, message)
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl crate::BenchmarkHelper<InventoryIdFor<Test>> for Test {
        fn inventory_id() -> InventoryIdFor<Test> {
            InventoryId([0u8; 32].into(), 0)
        }
    }

    impl crate::SubscriptionsBenchmarkHelper<AccountId, AssetId, Balance> for Test {
        fn price() -> (AssetId, Balance) {
            use frame_support::traits::fungibles::{Create, Inspect};
            if !Assets::asset_exists(SUBSCRIPTION_ASSET) {
                assert!(
                    <Assets as Create<AccountId>>::create(SUBSCRIPTION_ASSET, ROOT, true, 1)
                        .is_ok()
                );
            }
            (SUBSCRIPTION_ASSET, 10)
        }

        fn fund(who: &AccountId, asset: &AssetId, amount: Balance) {
            use frame_support::traits::fungibles::Mutate;
            assert!(Assets::mint_into(*asset, who, amount.saturating_mul(1_000)).is_ok());
        }
    }
}

pub const ROOT: AccountId = AccountId::new([0u8; 32]);

#[derive(Default)]
pub struct ExtBuilder {
    balances: mock_helpers::BalancesExtBuilder<Test>,
    assets: mock_helpers::AssetsExtBuilder<Test>,
}

impl ExtBuilder {
    pub(crate) fn with_account(mut self, account: AccountId, balance: Balance) -> Self {
        self.balances = self.balances.with_account(account, balance);
        self
    }

    pub(crate) fn with_asset(
        mut self,
        asset: mock_helpers::Asset<AccountId, AssetId, Balance>,
    ) -> Self {
        self.assets = self.assets.with_asset(asset);
        self
    }

    pub(crate) fn build(&mut self) -> TestExternalities {
        let mut storage = frame_system::GenesisConfig::<Test>::default()
            .build_storage()
            .unwrap();

        self.balances
            .as_storage()
            .assimilate_storage(&mut storage)
            .unwrap();

        self.assets
            .as_storage()
            .assimilate_storage(&mut storage)
            .unwrap();

        pallet_listings::GenesisConfig::<Test> {
            inventories: vec![(([0u8; 32].into(), 1), ROOT)],
            items: vec![],
        }
        .assimilate_storage(&mut storage)
        .unwrap();

        let mut ext = TestExternalities::new(storage);
        ext.execute_with(|| {
            System::set_block_number(1);
            Clock::set(0);
            RecordHooks::take();
            RecordHooks::refuse_replacements(false);
        });
        ext
    }
}

pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);

pub const CHARLIE: AccountId = AccountId::new([3u8; 32]);

/// The asset subscriptions are priced in: sufficient, with a minimum balance of 1.
pub const SUBSCRIPTION_ASSET: AssetId = 2;

pub fn new_test_ext() -> TestExternalities {
    ExtBuilder::default()
        .with_account(ROOT, Balance::MAX / 2)
        .with_account(ALICE, 2 * <ExistentialDeposit as Get<Balance>>::get())
        .with_account(BOB, 2 * <ExistentialDeposit as Get<Balance>>::get())
        .with_asset(mock_helpers::Asset::new(1, ROOT, 10, false))
        .with_asset(
            mock_helpers::Asset::new(SUBSCRIPTION_ASSET, ROOT, 1, true)
                .add_account(ALICE, 1_000)
                .add_account(BOB, 1_000)
                .add_account(CHARLIE, 1_000),
        )
        .build()
}
