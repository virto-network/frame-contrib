//! Test environment for pallet pass.

use crate::{self as pallet_pass, Config, FirstItemIsFree, HoldReason, PassAuthenticate};
pub use authenticators::*;
use codec::{Decode, Encode, MaxEncodedLen};
use fc_traits_authn::{composite_authenticators, util::AuthorityFromPalletId, Challenger};
use frame_support::traits::fungible::HoldConsideration;
use frame_support::traits::{Consideration, Footprint, LinearStoragePrice};
use frame_support::weights::FixedFee;
use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstU64, EitherOf, EqualPrivilegeOnly, OnInitialize},
    weights::Weight,
    DebugNoBound, EqNoBound, PalletId,
};
use frame_system::mocking::MockUncheckedExtrinsic;
use frame_system::{EnsureRoot, EnsureRootWithSuccess, EnsureSigned};
use scale_info::TypeInfo;
use sp_core::{blake2_256, H256};
use sp_io::TestExternalities;
use sp_runtime::{
    traits::{IdentifyAccount, IdentityLookup, Verify},
    DispatchError, MultiSignature,
};

mod authenticators;

pub type TxExtensions = (
    PassAuthenticate<Test>,
    pallet_transaction_payment::ChargeTransactionPayment<Test>,
);
pub type CheckedExtrinsic =
    sp_runtime::generic::CheckedExtrinsic<AccountId, RuntimeCall, TxExtensions>;
pub type Block = sp_runtime::generic::Block<
    sp_runtime::generic::Header<u64, sp_runtime::traits::BlakeTwo256>,
    MockUncheckedExtrinsic<Test, (), TxExtensions>,
>;

pub type AccountPublic = <MultiSignature as Verify>::Signer;
pub type AccountId = <AccountPublic as IdentifyAccount>::AccountId;
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
    #[runtime::pallet_index(1)]
    pub type Scheduler = pallet_scheduler;
    #[runtime::pallet_index(2)]
    pub type TransactionPayment = pallet_transaction_payment;
    #[runtime::pallet_index(10)]
    pub type Balances = pallet_balances;
    #[runtime::pallet_index(11)]
    pub type Pass = pallet_pass;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = AccountId;
    type Lookup = IdentityLookup<AccountId>;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
}

parameter_types! {
    pub ExistentialDeposit: Balance = 1000;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
    type ExistentialDeposit = ExistentialDeposit;
}

#[derive_impl(pallet_transaction_payment::config_preludes::TestDefaultConfig)]
impl pallet_transaction_payment::Config for Test {
    type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
    type WeightToFee = FixedFee<1, Balance>;
    type LengthToFee = FixedFee<0, Balance>;
}

parameter_types! {
    pub MaxScheduledPerBlock: u32 = u32::MAX;
    pub MaximumWeight: Weight = Weight::MAX;
}

impl pallet_scheduler::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = MaximumWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type OriginPrivilegeCmp = EqualPrivilegeOnly;
    type MaxScheduledPerBlock = MaxScheduledPerBlock;
    type WeightInfo = ();
    type Preimages = ();
    type BlockNumberProvider = System;
}

parameter_types! {
    pub const RootAccount: AccountId = AccountId::new([0u8; 32]);
    pub PassPalletId: PalletId = PalletId(*b"py/pass_");
    pub HoldAccountRegistration: RuntimeHoldReason = HoldReason::AccountRegistration.into();
    pub HoldAccountDevices: RuntimeHoldReason = HoldReason::AccountDevices.into();
    pub HoldSessionKeys: RuntimeHoldReason = HoldReason::SessionKeys.into();
}

composite_authenticators! {
    pub Pass<AuthorityFromPalletId<PassPalletId>> {
        authenticator_a::Authenticator,
        AuthenticatorB::<LastThreeBlocksChallenger>,
    };
}

#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen, Debug, Eq, PartialEq)]
pub struct RootDoesNotPayConsideration<C>(Option<C>);

impl<C> Consideration<AccountId, Footprint> for RootDoesNotPayConsideration<C>
where
    C: Consideration<AccountId, Footprint>,
{
    fn new(who: &AccountId, new: Footprint) -> Result<Self, DispatchError> {
        if who == &RootAccount::get() {
            Ok(Self(None))
        } else {
            Ok(Self(Some(C::new(who, new)?)))
        }
    }

    fn update(self, who: &AccountId, new: Footprint) -> Result<Self, DispatchError> {
        if let Some(c) = self.0 {
            Ok(Self(Some(c.update(who, new)?)))
        } else {
            Ok(self)
        }
    }

    fn drop(self, who: &AccountId) -> Result<(), DispatchError> {
        FirstItemIsFree::<C>(self.0).drop(who)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn ensure_successful(who: &AccountId, new: Footprint) {
        C::ensure_successful(who, new)
    }
}

pub type RegistrationStoragePrice = LinearStoragePrice<ExistentialDeposit, ConstU64<1>, Balance>;
pub type ItemStoragePrice = LinearStoragePrice<ConstU64<100>, ConstU64<1>, Balance>;

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = ();
    type RegisterOrigin = EitherOf<
        // Root does not pay
        EnsureRootWithSuccess<Self::AccountId, RootAccount>,
        // Anyone else pays
        EnsureSigned<Self::AccountId>,
    >;
    type AddressGenerator = ();
    type Balances = Balances;
    type Authenticator = PassAuthenticator;
    type Scheduler = Scheduler;
    type BlockNumberProvider = System;
    type RegistrarConsideration = RootDoesNotPayConsideration<
        HoldConsideration<AccountId, Balances, HoldAccountRegistration, RegistrationStoragePrice>,
    >;
    type DeviceConsideration = FirstItemIsFree<
        HoldConsideration<AccountId, Balances, HoldAccountDevices, ItemStoragePrice>,
    >;
    type SessionKeyConsideration =
        FirstItemIsFree<HoldConsideration<AccountId, Balances, HoldSessionKeys, ItemStoragePrice>>;
    type PalletId = PassPalletId;
    type MaxDevicesPerAccount = ConstU64<2>;
    type MaxSessionsPerAccount = ConstU64<2>;
    type MaxSessionDuration = ConstU64<10>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = benchmarks::BenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks {
    use super::*;
    use core::cell::Cell;
    use pallet_pass::{CredentialOf, DeviceAttestationOf};
    use sp_core::U256;
    use sp_runtime::traits::TrailingZeroInput;

    thread_local! {
        pub static LAST_ID: Cell<U256>  = const { Cell::new(U256::zero()) };
    }

    pub struct BenchmarkHelper;

    impl BenchmarkHelper {
        fn next_device() -> DeviceId {
            LAST_ID.with(|id| {
                let device_id: DeviceId =
                    Decode::decode(&mut TrailingZeroInput::new(&id.get().encode()))
                        .expect("infinite size, decodes to expected byte array; qed");
                id.set(id.get().saturating_add(U256::one()));
                device_id
            })
        }
    }

    impl pallet_pass::BenchmarkHelper<Test> for BenchmarkHelper {
        fn device_attestation(xtc: &impl ExtrinsicContext) -> DeviceAttestationOf<Test, ()> {
            PassDeviceAttestation::AuthenticatorB(authenticator_b::DeviceAttestation {
                device_id: Self::next_device(),
                context: System::block_number(),
                challenge: LastThreeBlocksChallenger::generate(&System::block_number(), xtc),
            })
        }

        fn credential(
            user_id: HashedUserId,
            device_id: DeviceId,
            xtc: &impl ExtrinsicContext,
        ) -> CredentialOf<Test, ()> {
            PassCredential::AuthenticatorB(
                authenticator_b::Credential::new(
                    user_id,
                    System::block_number(),
                    0,
                    LastThreeBlocksChallenger::generate(&System::block_number(), xtc),
                )
                .sign(&device_id),
            )
        }
    }
}

pub fn new_test_ext() -> TestExternalities {
    let mut ext = TestExternalities::new(Default::default());
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}

pub fn run_to(n: u64) {
    while System::block_number() < n {
        next_block();
    }
}

pub fn next_block() {
    System::reset_events();
    System::set_block_number(System::block_number() + 1);
    log::info!("Starting block {:?}", System::block_number());
    Scheduler::on_initialize(System::block_number());
}
