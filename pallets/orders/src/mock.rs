//! Test environment for template pallet.

use crate::{self as fc_pallet_orders, Config};
use codec::{Decode, Encode, MaxEncodedLen};
use core::cell::Cell;
use fc_pallet_listings::{InventoryIdOf, ItemIdOf, ItemPrice, ItemType};
use frame_support::pallet_prelude::Weight;
use frame_support::traits::{ConstU64, EnsureOrigin, EqualPrivilegeOnly, Get};
use frame_support::{derive_impl, pallet_prelude::ConstU32, PalletId};
use frame_system::{EnsureNever, EnsureRoot, EnsureRootWithSuccess, EnsureSigned};
use mock_helpers::ExtHelper;
use scale_info::TypeInfo;
use sp_core::{parameter_types, ByteArray};
use sp_io::TestExternalities;
use sp_runtime::{
    traits::{IdentifyAccount, IdentityLookup, Verify},
    BuildStorage, MultiSignature, Percent,
};

#[cfg(feature = "runtime-benchmarks")]
use fc_pallet_listings::InventoryId;

pub type Block = frame_system::mocking::MockBlock<Test>;
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
}

pub type AccountIdBytes = [u8; 32];

impl pallet_nfts::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = InventoryIdOf<Self>;
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
}

#[cfg(feature = "runtime-benchmarks")]
pub struct OwnersCatalogBenchmarkHelper<T, I = ()>(core::marker::PhantomData<(T, I)>);

#[cfg(feature = "runtime-benchmarks")]
impl<T, I: 'static>
    pallet_nfts::BenchmarkHelper<
        InventoryIdOf<Test>,
        ItemIdOf<Test>,
        sp_runtime::MultiSigner,
        sp_runtime::AccountId32,
        MultiSignature,
    > for OwnersCatalogBenchmarkHelper<T, I>
where
    T: pallet_nfts::Config<I>,
{
    fn collection(i: u16) -> InventoryIdOf<Test> {
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

        InventoryId(convert(i), 1u16.into())
    }

    fn item(i: u16) -> ItemIdOf<Test> {
        ItemType::Unit(i.into())
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
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Balances = Balances;
    type Assets = Assets;
    type Nonfungibles = ListingsCatalog;
    type NonfungiblesKeyLimit = ();
    type NonfungiblesValueLimit = ();
    type CreateInventoryOrigin = EnsureSigned<AccountId>;
    type InventoryAdminOrigin = EnsureSigned<AccountId>;
    type MerchantId = [u8; 32];
    type InventoryId = u32;
    type ItemSKU = u32;

    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ListingsBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct ListingsBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl fc_pallet_listings::BenchmarkHelper<Test> for ListingsBenchmarkHelper {
    fn inventory_id() -> InventoryIdOf<Test> {
        InventoryId([0u8; 32], 0)
    }

    fn item_id() -> ItemIdOf<Test> {
        ItemType::Unit(0)
    }

    fn item_price() -> ItemPrice<AssetId, Balance> {
        ItemPrice {
            asset: 0,
            amount: 10,
        }
    }
}

#[derive(Clone, Copy, MaxEncodedLen, Encode, Decode, TypeInfo, Eq, PartialEq, Debug)]
pub struct PaymentId(u32);

thread_local! {
    pub static LAST_ID: Cell<u32>  = const { Cell::new(0) };
}

impl fc_pallet_payments::PaymentId<Test> for PaymentId {
    fn next(_sender: &AccountId, _beneficiary: &AccountId) -> Option<Self> {
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
    pub const IncentivePercentage: Percent = Percent::from_percent(0);
    pub const MaxRemarkLength: u32 = 256;
}

impl fc_pallet_payments::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type Assets = Assets;
    type AssetsHold = Assets;
    type FeeHandler = ();
    type SenderOrigin = EnsureSigned<AccountId>;
    type BeneficiaryOrigin = EnsureSigned<AccountId>;
    type DisputeResolver = EnsureRootWithSuccess<AccountId, RootAccount>;
    type PaymentId = PaymentId;
    type Scheduler = Scheduler;
    type Preimages = ();
    type RuntimeHoldReason = RuntimeHoldReason;
    type WeightInfo = ();
    type OnPaymentStatusChanged = Orders;
    type PalletId = PaymentPalletId;
    type IncentivePercentage = IncentivePercentage;
    type MaxRemarkLength = MaxRemarkLength;
    type MaxFees = ConstU32<50>;
    type MaxDiscounts = ConstU32<50>;
    type CancelBufferBlockLength = ConstU64<10>;
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
        Ok(RuntimeOrigin::signed(AccountId::new([1u8; 32])))
    }
}

impl Config for Test {
    type WeightInfo = ();
    type RuntimeEvent = RuntimeEvent;
    type CreateOrigin = LimitsPerAccountId;
    type SetItemsOrigin = LimitsPerAccountId;
    type OrderId = u32;
    type Listings = Listings;
    type Payments = Payments;
    type MaxCartLen = ConstU32<4>;
    type MaxItemLen = ConstU32<4>;
}

// Test helpers: public accounts, assets, stores and `ExtBuilder`

pub const ASSET_A: AssetId = 1;
pub const ASSET_B: AssetId = 2;

pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);

pub const EVE: AccountId = AccountId::new([5u8; 32]);

parameter_types! {
    pub AliceStore: AccountIdBytes = ALICE.to_raw_vec().try_into().unwrap();
    pub BobStore: AccountIdBytes = BOB.to_raw_vec().try_into().unwrap();
}

#[derive(Default)]
pub struct ExtBuilder {
    balances: mock_helpers::BalancesExtHelper<Test>,
    assets: mock_helpers::AssetsExtBuilder<Test>,
    listings: mock_helpers::ListingsExtBuilder<Test>,
}

impl ExtBuilder {
    fn with_account(mut self, account: AccountId, balance: Balance) -> Self {
        self.balances = self.balances.with_account(account, balance);
        self
    }

    fn with_asset(mut self, asset: mock_helpers::Asset<AccountId, AssetId, Balance>) -> Self {
        self.assets = self.assets.with_asset(asset);
        self
    }

    fn with_inventory(mut self, inventory: mock_helpers::InventoryOf<Test>) -> Self {
        self.listings = self.listings.with_inventory(inventory);
        self
    }

    fn build(&mut self) -> TestExternalities {
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
                    ItemType::Unit(1),
                    b"Alice Flowers - Red Roses".to_vec(),
                    Some(ItemPrice {
                        asset: ASSET_A,
                        amount: 10,
                    }),
                ))
                .with_item(mock_helpers::Item::new(
                    ItemType::Unit(2),
                    b"Alice Flowers - Blue Violets".to_vec(),
                    Some(ItemPrice {
                        asset: ASSET_B,
                        amount: 20,
                    }),
                ))
                .with_item(mock_helpers::Item::new(
                    ItemType::Unit(3),
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
                    ItemType::Unit(1),
                    b"Bob's Hardware - Hammer".to_vec(),
                    Some(ItemPrice {
                        asset: ASSET_B,
                        amount: 30,
                    }),
                ))
                .with_item(mock_helpers::Item::new(
                    ItemType::Unit(2),
                    b"Bob's Hardware - Ruler".to_vec(),
                    Some(ItemPrice {
                        asset: ASSET_B,
                        amount: 25,
                    }),
                ))
                .with_item(mock_helpers::Item::new(
                    ItemType::Unit(3),
                    b"Bob's Hardware - Screwdriver".to_vec(),
                    Some(ItemPrice {
                        asset: ASSET_A,
                        amount: 50,
                    }),
                )),
        )
        .build()
}
