//! Test environment for template pallet.

use crate::{self as pallet_listings, InventoryId};

use fc_traits_listings::InventoryLifecycle;
use frame_support::traits::fungible::Unbalanced;
use frame_support::traits::tokens::Precision;
use frame_support::{
	derive_impl,
	dispatch::RawOrigin,
	traits::{EnsureOriginWithArg, Get},
};
use frame_system::{EnsureNever, EnsureRoot, EnsureSigned};
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
	pub type Listings = pallet_listings<Instance1>;
	#[runtime::pallet_index(21)]
	pub type ListingsCatalog = pallet_nfts<Instance1>;
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

pub type ListingsInstance = pallet_listings::Instance1;

pub type AccountIdBytes = [u8; 32];

parameter_types! {
    pub CollectionDeposit: Balance = 1000;
    pub ItemDeposit: Balance = 100;
    pub MetadataDepositBase: Balance = 10;
    pub AttributeDepositBase: Balance = 10;
    pub DepositPerByte: Balance = 1;
}

impl pallet_nfts::Config<ListingsInstance> for Test {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = InventoryId<AccountIdBytes, u32>;
	type ItemId = pallet_listings::ItemType<u32>;
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
	type Helper = ();
	type WeightInfo = ();
}

pub struct EnsureInventoryCreator;

impl<Id> EnsureOriginWithArg<RuntimeOrigin, InventoryId<AccountIdBytes, Id>>
for EnsureInventoryCreator
{
	type Success = AccountId;

	fn try_origin(
		o: RuntimeOrigin,
		InventoryId(public, _): &InventoryId<AccountIdBytes, Id>,
	) -> Result<Self::Success, RuntimeOrigin> {
		match o.clone().into() {
			Ok(RawOrigin::Signed(ref who))
			if <AccountId as AsRef<[u8]>>::as_ref(who) == &*public =>
				{
					Ok(who.clone())
				}
			_ => Err(o),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(
		InventoryId(public, _): &InventoryId<AccountIdBytes, Id>,
	) -> Result<RuntimeOrigin, ()> {
		Ok(RuntimeOrigin::signed(AccountId::new(*public)))
	}
}

pub struct EnsureInventoryOwnedByCaller;

impl<Id> EnsureOriginWithArg<RuntimeOrigin, InventoryId<AccountIdBytes, Id>>
for EnsureInventoryOwnedByCaller
{
	type Success = ();

	fn try_origin(
		o: RuntimeOrigin,
		InventoryId(owner, _): &InventoryId<AccountIdBytes, Id>,
	) -> Result<(), RuntimeOrigin> {
		match Into::<Result<RawOrigin<AccountId>, RuntimeOrigin>>::into(o.clone())? {
			RawOrigin::Signed(ref who) if <AccountId as AsRef<[u8]>>::as_ref(who) == &*owner => {
				Ok(())
			}
			_ => Err(o),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(
		InventoryId(a, _): &InventoryId<AccountId, Id>,
	) -> Result<RuntimeOrigin, ()> {
		Ok(RawOrigin::Signed(a.clone()).into())
	}
}

impl pallet_listings::Config<ListingsInstance> for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Assets = Assets;
	type CreateInventoryOrigin = EnsureInventoryCreator;
	type InventoryAdminOrigin = EnsureInventoryOwnedByCaller;
	type MerchantId = AccountIdBytes;
	type InventoryId = u32;
	type ItemSKU = u32;
}

pub const ROOT: AccountId = AccountId::new([0u8; 32]);

pub struct ExtBuilder {
	accounts: Vec<AccountId>,
	assets: Vec<(AssetId, Balance)>,
	inventories: Vec<(InventoryId<AccountIdBytes, u32>, AccountId)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			accounts: Default::default(),
			assets: Default::default(),
			inventories: vec![],
		}
	}
}

impl ExtBuilder {
	pub(crate) fn builder() -> ExtBuilder {
		ExtBuilder::default()
	}

	pub(crate) fn with_accounts(mut self, accounts: Vec<AccountId>) -> Self {
		self.accounts = accounts;
		self
	}

	pub(crate) fn with_assets(mut self, assets: Vec<(AssetId, Balance)>) -> Self {
		self.assets = assets;
		self
	}

	pub(crate) fn with_inventories(
		mut self,
		inventories: Vec<(InventoryId<AccountIdBytes, u32>, AccountId)>,
	) -> Self {
		self.inventories = inventories;
		self
	}

	pub(crate) fn build(self) -> TestExternalities {
		let mut storage = frame_system::GenesisConfig::<Test>::default()
			.build_storage()
			.unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: [
				self.accounts
					.iter()
					.map(|who| {
						(
							who.clone(),
							2 * <<Test as pallet_balances::Config>::ExistentialDeposit as Get<
								Balance,
							>>::get(),
						)
					})
					.collect::<Vec<_>>(),
				vec![(ROOT, Balance::MAX / 2)],
			]
				.concat(),
		}
			.assimilate_storage(&mut storage)
			.unwrap();

		pallet_assets::GenesisConfig::<Test> {
			assets: self
				.assets
				.iter()
				.map(|(id, min_balance)| (*id, ROOT, false, *min_balance))
				.collect(),
			metadata: vec![],
			accounts: vec![],
			next_asset_id: None,
		}
			.assimilate_storage(&mut storage)
			.unwrap();

		let mut t: TestExternalities = storage.into();
		t.execute_with(|| {
			System::set_block_number(1);

			for (InventoryId(ref merchant_id, ref id), ref owner) in self.inventories {
				Balances::increase_balance(owner, CollectionDeposit::get(), Precision::Exact)
					.unwrap();
				Listings::create(merchant_id, id, owner).unwrap();
			}
		});
		t
	}
}

pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);

pub fn new_test_ext() -> TestExternalities {
	ExtBuilder::builder()
		.with_assets(vec![(1, 10)])
		.with_accounts(vec![ALICE, BOB])
		.with_inventories(vec![(InventoryId([0u8; 32], 1), ROOT)])
		.build()
}
