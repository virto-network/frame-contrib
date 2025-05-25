//! Test environment for template pallet.

use crate::{
    self as pallet_listings, test_utils::SignedMerchantId, InventoryId, InventoryIdFor, ItemIdOf,
};
use mock_helpers::ExtHelper;

use frame_support::{
    derive_impl,
    traits::{EnsureOriginWithArg, Get},
};
use frame_system::{
    pallet_prelude::BlockNumberFor, EnsureNever, EnsureRoot, EnsureSigned, RawOrigin,
};
use sp_core::{parameter_types, ConstU32};
use sp_io::TestExternalities;
use sp_runtime::{
    traits::{IdentifyAccount, IdentityLookup, Verify},
    BuildStorage, MultiSignature,
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
    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type Assets = pallet_assets;
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
    type RuntimeEvent = RuntimeEvent;
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
        });
        ext
    }
}

pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);

pub fn new_test_ext() -> TestExternalities {
    ExtBuilder::default()
        .with_account(ROOT, Balance::MAX / 2)
        .with_account(ALICE, 2 * <ExistentialDeposit as Get<Balance>>::get())
        .with_account(BOB, 2 * <ExistentialDeposit as Get<Balance>>::get())
        .with_asset(mock_helpers::Asset::new(1, ROOT, 10, false))
        .build()
}
