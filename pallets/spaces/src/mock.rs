//! Test environment for the Spaces pallet.

use crate::{self as pallet_spaces, Config, MockVerifier, ProgramId};
use frame_support::{derive_impl, traits::ConstU32};
use frame_system::{EnsureRoot, EnsureSigned};
use sp_io::TestExternalities;

pub type Block = frame_system::mocking::MockBlock<Test>;
pub type AccountId = u64;

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Spaces: pallet_spaces,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

pub const PROGRAM: ProgramId = [7u8; 32];
pub const OTHER_PROGRAM: ProgramId = [8u8; 32];

#[cfg(feature = "runtime-benchmarks")]
pub struct BenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl crate::BenchmarkHelper<u32> for BenchmarkHelper {
    fn space(i: u32) -> u32 {
        i
    }
    fn program() -> ProgramId {
        PROGRAM
    }
    fn proof(program: &ProgramId, public: &[u8], output: &[u8]) -> Vec<u8> {
        MockVerifier::prove(program, public, output)
    }
}

impl Config for Test {
    type SpaceId = u32;
    type BindKey = u32;
    type RegisterOrigin = EnsureSigned<AccountId>;
    type ResetOrigin = EnsureRoot<AccountId>;
    type Verifier = MockVerifier;
    type MaxProofLen = ConstU32<1024>;
    type MaxPublicLen = ConstU32<256>;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = BenchmarkHelper;
}

pub fn new_test_ext() -> TestExternalities {
    let mut ext = TestExternalities::new(Default::default());
    ext.execute_with(|| {
        System::set_block_number(1);
    });
    ext
}
