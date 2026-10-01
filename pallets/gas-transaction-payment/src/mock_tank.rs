//! A second test environment, integrated as the crate guide says: `NonFungibleGasTank` over a
//! `pallet_nfts` instance, and the extension placed after `CheckWeight`, wrapping
//! `pallet_transaction_payment`.

extern crate alloc;

use crate::ChargeTransactionPayment;
pub use crate::{self as fc_pallet_gas_transaction_payment, Config};
use alloc::boxed::Box;
use frame::{
    deps::{
        frame_support::{derive_impl, traits::nonfungibles_v2::Create},
        sp_runtime::{
            self,
            traits::{IdentifyAccount, IdentityLookup, Verify},
            MultiSignature,
        },
    },
    testing_prelude::*,
};
use frame_contrib_traits::gas_tank::{NonFungibleGasTank, SelectNonFungibleItem};
use frame_system::{mocking::MockUncheckedExtrinsic, EnsureNever};

#[frame_construct_runtime]
pub mod runtime {
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
    pub type GasTransactionPayment = fc_pallet_gas_transaction_payment;
    #[runtime::pallet_index(12)]
    pub type TransactionPayment = pallet_transaction_payment;
    #[runtime::pallet_index(20)]
    pub type Memberships = pallet_nfts;
}

/// The gas extension, wrapping the fee extension.
pub type GasExtension =
    ChargeTransactionPayment<Test, pallet_transaction_payment::ChargeTransactionPayment<Test>>;
/// `CheckWeight`, then the gas extension.
pub type TxExtensions = (frame_system::CheckWeight<Test>, GasExtension);
pub type UncheckedExtrinsic = MockUncheckedExtrinsic<Test, (), TxExtensions>;
pub type CheckedExtrinsic =
    sp_runtime::generic::CheckedExtrinsic<AccountId, RuntimeCall, TxExtensions>;
pub type Block =
    sp_runtime::generic::Block<sp_runtime::generic::Header<u64, BlakeTwo256>, UncheckedExtrinsic>;

pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
pub type Balance = u128;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = AccountId;
    type Lookup = IdentityLookup<AccountId>;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
    type Balance = Balance;
}

#[derive_impl(pallet_transaction_payment::config_preludes::TestDefaultConfig)]
impl pallet_transaction_payment::Config for Test {
    type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
    type WeightToFee = FixedFee<1, Balance>;
    type LengthToFee = FixedFee<0, Balance>;
}

impl pallet_nfts::Config for Test {
    type ApprovalsLimit = ();
    type AttributeDepositBase = ();
    type CollectionDeposit = ();
    type CollectionId = u16;
    type CreateOrigin = EnsureNever<AccountId>;
    type Currency = Balances;
    type DepositPerByte = ();
    type Features = ();
    type ForceOrigin = EnsureNever<AccountId>;
    type ItemAttributesApprovalsLimit = ();
    type ItemDeposit = ();
    type ItemId = u32;
    type KeyLimit = ConstU32<64>;
    type Locker = ();
    type MaxAttributesPerCall = ();
    type MaxDeadlineDuration = ();
    type MaxTips = ();
    type MetadataDepositBase = ();
    type OffchainPublic = AccountPublic;
    type OffchainSignature = MultiSignature;
    type RuntimeEvent = RuntimeEvent;
    type StringLimit = ();
    type ValueLimit = ConstU32<50>;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type Helper = ();
    type BlockNumberProvider = System;
}

/// Any membership may hold a tank.
pub struct AnyMembership;
impl Get<Box<dyn SelectNonFungibleItem<u16, u32>>> for AnyMembership {
    fn get() -> Box<dyn SelectNonFungibleItem<u16, u32>> {
        Box::new(())
    }
}

/// The tank, with the default scan bound.
pub type MembershipsGasTank =
    NonFungibleGasTank<Test, System, Memberships, pallet_nfts::ItemConfig, AnyMembership>;

impl Config for Test {
    type WeightInfo = ();
    type GasTank = MembershipsGasTank;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = Self;
}

pub const COLLECTION: u16 = 1;

#[cfg(feature = "runtime-benchmarks")]
impl fc_pallet_gas_transaction_payment::BenchmarkHelper<Test> for Test {
    type Ext = pallet_transaction_payment::ChargeTransactionPayment<Test>;

    fn ext() -> ChargeTransactionPayment<Test, Self::Ext> {
        GasExtension::new(pallet_transaction_payment::ChargeTransactionPayment::<Test>::from(0))
    }

    /// The worst case for the scan: `who` holds as many items as the bound, and the tank is on the
    /// one the scan reaches last. The burn reads the paying-item note and that one tank, whatever
    /// `who` holds.
    fn setup_account(who: &AccountId, gas: Weight) -> DispatchResult {
        use frame::deps::frame_support::traits::nonfungibles_v2::{InspectEnumerable, Mutate};
        use frame_contrib_traits::gas_tank::{DefaultMaxScan, MakeTank};

        for item in 0..<DefaultMaxScan as Get<u32>>::get() {
            Memberships::mint_into(&COLLECTION, &item, who, &Default::default(), true)?;
        }
        let last = Memberships::owned(who)
            .last()
            .ok_or(DispatchError::Other("no item was minted"))?;
        MembershipsGasTank::make_tank(&last, Some(gas), None)
    }
}

/// An environment with one membership collection, at block 1.
pub fn new_test_ext() -> TestExternalities {
    let mut ext = TestExternalities::new(Default::default());
    ext.execute_with(|| {
        System::set_block_number(1);
        let owner = AccountId::new([0u8; 32]);
        Memberships::create_collection_with_id(COLLECTION, &owner, &owner, &Default::default())
            .expect("the collection does not exist yet; qed");
    });
    ext
}
