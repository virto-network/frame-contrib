//! Test environment for the Spaces pallet.

use crate::{self as pallet_spaces, Config, MockVerifier, ProgramId};
use frame_support::{derive_impl, parameter_types, traits::ConstU32, PalletId};
use frame_system::{EnsureRoot, EnsureSigned};
use sp_io::TestExternalities;
use sp_runtime::BuildStorage;

pub type Block = frame_system::mocking::MockBlock<Test>;
/// Wide enough for a sub-account to keep the whole Space id: `modl` ‖ pallet id ‖ `u32`.
pub type AccountId = u128;
pub type Balance = u64;

/// A pallet that lets a Space act as itself, for the tests.
#[frame_support::pallet(dev_mode)]
pub mod space_user {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        type SpaceOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// `space` called.
        CalledBy { space: u32 },
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Only a Space may call this.
        #[pallet::call_index(0)]
        pub fn as_space(origin: OriginFor<T>) -> DispatchResult {
            let space = T::SpaceOrigin::ensure_origin(origin)?;
            Self::deposit_event(Event::CalledBy { space });
            Ok(())
        }
    }
}

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Spaces: pallet_spaces,
        SpaceUser: space_user,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = AccountId;
    type Lookup = sp_runtime::traits::IdentityLookup<AccountId>;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

impl space_user::Config for Test {
    type SpaceOrigin = crate::EnsureSpace<Test>;
}

pub const PROGRAM: ProgramId = [7u8; 32];
pub const OTHER_PROGRAM: ProgramId = [8u8; 32];

parameter_types! {
    pub const SpacesPalletId: PalletId = PalletId(*b"fc/space");
}

/// The authority origin of an account.
pub fn signed(who: AccountId) -> OriginCaller {
    OriginCaller::system(frame_system::RawOrigin::Signed(who))
}

#[cfg(feature = "runtime-benchmarks")]
pub struct BenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl crate::BenchmarkHelper for BenchmarkHelper {
    fn program() -> ProgramId {
        PROGRAM
    }
    fn proof(program: &ProgramId, public: &[u8], output: &[u8]) -> Vec<u8> {
        MockVerifier::prove(program, public, output)
    }
}

impl Config for Test {
    type PalletId = SpacesPalletId;
    type SpaceId = u32;
    type BindKey = u32;
    type CreateOrigin = EnsureSigned<AccountId>;
    type ResetOrigin = EnsureRoot<AccountId>;
    type Verifier = MockVerifier;
    type MaxProofLen = ConstU32<1024>;
    type MaxPublicLen = ConstU32<256>;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = BenchmarkHelper;
}

/// Account `1` holds 1,000.
pub fn new_test_ext() -> TestExternalities {
    let storage = RuntimeGenesisConfig {
        balances: pallet_balances::GenesisConfig {
            balances: vec![(1, 1_000)],
            ..Default::default()
        },
        ..Default::default()
    }
    .build_storage()
    .expect("genesis builds");
    let mut ext = TestExternalities::new(storage);
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}
