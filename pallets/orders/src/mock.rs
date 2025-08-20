//! Test environment for template pallet.

use crate::{self as fc_pallet_orders, Config};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use core::cell::Cell;
use fc_pallet_payments::FeeHandler;
#[cfg(feature = "runtime-benchmarks")]
use {
    crate::types::{InventoryIdOf, MerchantIdOf},
    fc_pallet_listings::InventoryId,
};

use fc_pallet_listings::test_utils::SignedMerchantId;
use fc_pallet_listings::{InventoryIdFor, ItemIdOf, ItemPrice};
use frame_support::{
    derive_impl,
    pallet_prelude::{ConstU32, Hooks, Weight},
    traits::{ConstU64, EnsureOrigin, EqualPrivilegeOnly, Get},
    PalletId,
};
use frame_system::{
    pallet_prelude::BlockNumberFor, EnsureNever, EnsureRoot, EnsureRootWithSuccess, EnsureSigned,
};
use mock_helpers::ExtHelper;
use scale_info::TypeInfo;
use sp_core::{parameter_types, ByteArray};
use sp_io::TestExternalities;
use sp_runtime::{
    traits::{IdentifyAccount, IdentityLookup, Verify},
    BoundedVec, BuildStorage, MultiSignature, Percent,
};

pub type Block = frame_system::mocking::MockBlock<Test>;
pub type BlockNumber = BlockNumberFor<Test>;
pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub type AssetId = <Test as pallet_assets::Config>::AssetId;
pub type Balance = <Test as pallet_balances::Config>::Balance;
pub type ExistentialDeposit = <Test as pallet_balances::Config>::ExistentialDeposit;

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
    pub type Preimage = pallet_preimage;
    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type Assets = pallet_assets;
    #[runtime::pallet_index(12)]
    pub type AssetsHolder = pallet_assets_holder;
    #[runtime::pallet_index(20)]
    pub type Listings = fc_pallet_listings;
    #[runtime::pallet_index(21)]
    pub type ListingsCatalog = pallet_nfts;
    #[runtime::pallet_index(30)]
    pub type Payments = fc_pallet_payments;
    #[runtime::pallet_index(31)]
    pub type Orders = fc_pallet_orders;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = AccountId;
    type Block = Block;
    type Lookup = IdentityLookup<Self::AccountId>;
    type AccountData = pallet_balances::AccountData<Balance>;
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
    type Preimages = Preimage;
    type BlockNumberProvider = System;
}

impl pallet_preimage::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type Consideration = ();
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

impl pallet_nfts::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = InventoryIdFor<Self>;
    type ItemId = ItemIdOf<Self>;
    type Currency = Balances;
    type ForceOrigin = EnsureNever<AccountId>;
    type CreateOrigin = EnsureNever<AccountId>;
    type Locker = ();
    type CollectionDeposit = ();
    type ItemDeposit = ();
    type MetadataDepositBase = ();
    type AttributeDepositBase = ();
    type DepositPerByte = ();
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
    type Helper = OwnersCatalogBenchmarkHelper<Self>;
    type WeightInfo = ();
    type BlockNumberProvider = System;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct OwnersCatalogBenchmarkHelper<T, I = ()>(core::marker::PhantomData<(T, I)>);

#[cfg(feature = "runtime-benchmarks")]
impl<T, I: 'static>
    pallet_nfts::BenchmarkHelper<
        InventoryIdFor<Test>,
        ItemIdOf<Test>,
        sp_runtime::MultiSigner,
        sp_runtime::AccountId32,
        MultiSignature,
    > for OwnersCatalogBenchmarkHelper<T, I>
where
    T: pallet_nfts::Config<I>,
{
    fn collection(i: u16) -> InventoryIdFor<Test> {
        fn convert(i: u16) -> SignedMerchantId {
            let high = (i >> 8) as u8;
            let low = (i & 0xFF) as u8;
            let mut j = [0u8; 32];

            for idx in 0..16 {
                j[2 * idx] = high;
                j[2 * idx + 1] = low;
            }

            j.into()
        }

        InventoryId(convert(i), 1u16.into())
    }

    fn item(i: u16) -> ItemIdOf<Test> {
        i.into()
    }

    fn signer() -> (sp_runtime::MultiSigner, sp_runtime::AccountId32) {
        <() as pallet_nfts::BenchmarkHelper<
            u16,
            u16,
            sp_runtime::MultiSigner,
            sp_runtime::AccountId32,
            MultiSignature,
        >>::signer()
    }

    fn sign(signer: &sp_runtime::MultiSigner, message: &[u8]) -> MultiSignature {
        <() as pallet_nfts::BenchmarkHelper<
            u16,
            u16,
            sp_runtime::MultiSigner,
            sp_runtime::AccountId32,
            MultiSignature,
        >>::sign(signer, message)
    }
}

impl fc_pallet_listings::Config for Test {
    type WeightInfo = ();
    type CreateInventoryOrigin = EnsureSigned<AccountId>;
    type InventoryAdminOrigin = EnsureSigned<AccountId>;
    type MerchantId = SignedMerchantId;
    type InventoryId = u32;
    type ItemSKU = u32;
    type CollectionConfig =
        pallet_nfts::CollectionConfig<Balance, BlockNumberFor<Self>, InventoryIdFor<Self>>;
    type ItemConfig = pallet_nfts::ItemConfig;
    type Balances = Balances;
    type Assets = Assets;
    type Nonfungibles = ListingsCatalog;
    type NonfungiblesKeyLimit = ();
    type NonfungiblesValueLimit = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = Self;
}

#[cfg(feature = "runtime-benchmarks")]
impl fc_pallet_listings::BenchmarkHelper<InventoryIdFor<Test>> for Test {
    fn inventory_id() -> InventoryIdFor<Test> {
        InventoryId([0u8; 32].into(), 0)
    }
}

#[derive(
    Clone,
    Copy,
    MaxEncodedLen,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Eq,
    PartialEq,
    Debug,
)]
pub struct PaymentId(u32);

thread_local! {
    pub static LAST_ID: Cell<u32>  = const { Cell::new(0) };
}

impl PaymentId {
    pub fn last() -> Self {
        Self(LAST_ID.get())
    }
}

impl fc_pallet_payments::GeneratePaymentId<AccountId> for PaymentId {
    type PaymentId = Self;

    fn generate(_: &AccountId, _: &AccountId) -> Option<Self> {
        LAST_ID.with(|id| {
            let new_id = id.get() + 1;
            id.set(new_id);
            Some(PaymentId(new_id))
        })
    }
}

parameter_types! {
    pub const RootAccount: AccountId = AccountId::new([0u8; 32]);
    pub const PaymentPalletId: PalletId = PalletId(*b"fcp/pays");
    pub const IncentivePercentage: Percent = Percent::from_percent(10);
    pub const MaxRemarkLength: u32 = 256;
}

pub struct MinBalanceFeeHandler;

// Min balance as fees for both recipient and sender
impl FeeHandler<Test> for MinBalanceFeeHandler {
    fn apply_fees(
        asset: &fc_pallet_payments::AssetIdOf<Test>,
        _sender: &AccountId,
        _beneficiary: &AccountId,
        _amount: &fc_pallet_payments::BalanceOf<Test>,
        _remark: Option<&[u8]>,
    ) -> fc_pallet_payments::Fees<Test> {
        use frame_support::traits::fungibles::Inspect;
        fc_pallet_payments::Fees {
            sender_pays: BoundedVec::truncate_from(vec![(
                RootAccount::get(),
                Assets::minimum_balance(asset.clone()),
                true,
            )]),
            beneficiary_pays: BoundedVec::truncate_from(vec![(
                RootAccount::get(),
                Assets::minimum_balance(asset.clone()),
                true,
            )]),
        }
    }
}

impl fc_pallet_payments::Config for Test {
    type PalletsOrigin = OriginCaller;
    type RuntimeHoldReason = RuntimeHoldReason;
    type WeightInfo = ();
    type SenderOrigin = EnsureSigned<AccountId>;
    type BeneficiaryOrigin = EnsureSigned<AccountId>;
    type DisputeResolver = EnsureRootWithSuccess<AccountId, RootAccount>;
    type PaymentId = PaymentId;
    type Assets = Assets;
    type AssetsHold = AssetsHolder;
    type BlockNumberProvider = System;
    type FeeHandler = MinBalanceFeeHandler;
    type Scheduler = Scheduler;
    type Preimages = ();
    type OnPaymentStatusChanged = Orders;
    type GeneratePaymentId = PaymentId;
    type PalletId = PaymentPalletId;
    type IncentivePercentage = IncentivePercentage;
    type MaxRemarkLength = MaxRemarkLength;
    type MaxFees = ConstU32<50>;
    type MaxDiscounts = ConstU32<50>;
    type CancelBufferBlockLength = ConstU64<10>;
}

parameter_types! {
    pub const MaxLifetimeForCheckoutOrder: BlockNumber = 10;
    pub const MaxCartLen: u32 = 4;
    pub const MaxItemLen: u32 = 4;
}

pub struct LimitsPerAccountId;

impl EnsureOrigin<RuntimeOrigin> for LimitsPerAccountId {
    type Success = (AccountId, u32);

    fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
        match o.caller {
            OriginCaller::system(frame_system::RawOrigin::Signed(who)) => {
                Ok((who.clone(), AsRef::<[u8]>::as_ref(&who)[0] as u32))
            }
            _ => Err(o),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
        Ok(RuntimeOrigin::signed(AccountId::new(
            [MaxCartLen::get()
                .try_into()
                .expect("test `MaxCartLen` won't exceed 256"); 32],
        )))
    }
}

impl Config for Test {
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = ();
    type CreateOrigin = LimitsPerAccountId;
    type OrderAdminOrigin = LimitsPerAccountId;
    type PaymentOrigin = EnsureSigned<AccountId>;
    type OrderId = u32;
    type Listings = Listings;
    type Payments = Payments;
    type Scheduler = Scheduler;
    type BlockNumberProvider = System;
    type MaxLifetimeForCheckoutOrder = MaxLifetimeForCheckoutOrder;
    type MaxCartLen = MaxCartLen;
    type MaxItemLen = MaxItemLen;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = Self;
}

#[cfg(feature = "runtime-benchmarks")]
impl fc_pallet_orders::BenchmarkHelper<Test> for Test {
    type Balances = Balances;
    type Assets = Assets;
    type InventoryDeposit = <Test as pallet_nfts::Config>::CollectionDeposit;
    type ItemDeposit = <Test as pallet_nfts::Config>::ItemDeposit;

    fn inventory_id() -> (MerchantIdOf<Test>, InventoryIdOf<Test>) {
        (AliceStore::get(), 1)
    }

    fn item_id(i: usize) -> ItemIdOf<Test> {
        i as u32
    }
}

// Test helpers: public accounts, assets, stores and `ExtBuilder`

pub const ASSET_A: AssetId = 1;
pub const ASSET_B: AssetId = 2;

pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);

pub const EVE: AccountId = AccountId::new([5u8; 32]);

parameter_types! {
    pub AliceStore: SignedMerchantId = ALICE.to_raw_vec().into();
    pub BobStore: SignedMerchantId = BOB.to_raw_vec().into();
}

#[derive(Default)]
pub struct ExtBuilder {
    balances: mock_helpers::BalancesExtBuilder<Test>,
    assets: mock_helpers::AssetsExtBuilder<Test>,
    listings: mock_helpers::ListingsExtBuilder<Test>,
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

    pub(crate) fn with_inventory(mut self, inventory: mock_helpers::InventoryOf<Test>) -> Self {
        self.listings = self.listings.with_inventory(inventory);
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

        self.listings
            .as_storage()
            .assimilate_storage(&mut storage)
            .unwrap();

        let mut ext = TestExternalities::new(storage);
        ext.execute_with(|| {
            System::set_block_number(1);
        });
        ext
    }
}

pub fn new_ext_builder() -> ExtBuilder {
    ExtBuilder::default()
        .with_account(ALICE, ExistentialDeposit::get())
        .with_account(BOB, ExistentialDeposit::get())
        .with_account(EVE, ExistentialDeposit::get())
        .with_asset(
            mock_helpers::Asset::new(ASSET_A, RootAccount::get(), 1, false)
                .add_account(ALICE, 100)
                .add_account(BOB, 100)
                .add_account(EVE, 100),
        )
        .with_asset(
            mock_helpers::Asset::new(ASSET_B, RootAccount::get(), 5, false)
                .add_account(ALICE, 100)
                .add_account(BOB, 100)
                .add_account(EVE, 100),
        )
}

pub fn new_test_ext() -> TestExternalities {
    new_ext_builder()
        .with_inventory(
            mock_helpers::Inventory::new((AliceStore::get(), 1), ALICE)
                .with_item(mock_helpers::Item::new(
                    1,
                    b"Alice Flowers - Red Roses".to_vec(),
                    Some(ItemPrice {
                        asset: ASSET_A,
                        amount: 10,
                    }),
                ))
                .with_item(mock_helpers::Item::new(
                    2,
                    b"Alice Flowers - Blue Violets".to_vec(),
                    Some(ItemPrice {
                        asset: ASSET_B,
                        amount: 20,
                    }),
                ))
                .with_item(mock_helpers::Item::new(
                    3,
                    b"Alice Flowers - Yellow Sunflowers".to_vec(),
                    Some(ItemPrice {
                        asset: ASSET_A,
                        amount: 30,
                    }),
                )),
        )
        .with_inventory(
            mock_helpers::Inventory::new((BobStore::get(), 1), BOB)
                .with_item(mock_helpers::Item::new(
                    1,
                    b"Bob's Hardware - Hammer".to_vec(),
                    Some(ItemPrice {
                        asset: ASSET_B,
                        amount: 30,
                    }),
                ))
                .with_item(mock_helpers::Item::new(
                    2,
                    b"Bob's Hardware - Ruler".to_vec(),
                    Some(ItemPrice {
                        asset: ASSET_B,
                        amount: 25,
                    }),
                ))
                .with_item(mock_helpers::Item::new(
                    3,
                    b"Bob's Hardware - Screwdriver".to_vec(),
                    Some(ItemPrice {
                        asset: ASSET_A,
                        amount: 50,
                    }),
                )),
        )
        .build()
}

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        Scheduler::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        Scheduler::on_initialize(System::block_number());
    }
}
