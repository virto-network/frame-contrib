//! VOS's STARK verifier as a [`ProofVerifier`] (feature `vos-verifier`).
//!
//! VOS (<https://codeberg.org/Virto/VOS>) proves the execution of PVM programs with a Circle STARK
//! over M31, built on StarkWare's Stwo, and ships a `no_std` verifier,
//! `vos-pvm-proof-verifier`. One verifier serves every program: a program is identified by its
//! preprocessed-trace commitment, which is a Space's [`ProgramId`].
//!
//! A proof is the postcard encoding of VOS's `Proof`. The check, cheapest first:
//!
//! 1. decode it, with no trailing bytes ([`VerifyError::Malformed`]);
//! 2. its public input/output hash must be [`vos_io_hash`] of the public input and the pallet's
//!    output ([`VerifyError::NotBound`]);
//! 3. its preprocessed-trace commitment must be the Space's program, compared before the verifier's
//!    own preflight so a proof of another program is refused for the price of decoding it
//!    ([`VerifyError::WrongProgram`]);
//! 4. `verify_standalone_with_options` with the pinned PCS policy and log-size cap
//!    ([`VerifyError::Invalid`]).
//!
//! **Building for a runtime.** The wasm builder targets `wasm32v1-none`, for which the verifier's
//! dependency graph needs two adjustments, both at the runtime's workspace root: Stwo from a fork
//! that makes `dashmap` (std-only, prover-only) optional, and `getrandom` 0.2 with its `custom`
//! feature (registered below with a source that always fails: verification draws no randomness).
//! See `DESIGN.md`.

use crate::{vos_io_hash, ProgramId, ProofVerifier, VerifyError};
use core::marker::PhantomData;
use frame_support::{traits::Get, weights::Weight};
use vos_pvm_proof_verifier::{verify_standalone_with_options, CommitmentHash, PcsPolicy, Proof};

// getrandom 0.2 (stwo -> starknet-crypto -> starknet-ff) builds for `wasm32v1-none` only with a
// registered source. Verification never draws randomness, so the source always fails.
#[cfg(all(target_arch = "wasm32", not(feature = "std")))]
mod no_entropy {
    fn fail(_: &mut [u8]) -> Result<(), getrandom::Error> {
        Err(getrandom::Error::UNSUPPORTED)
    }
    getrandom::register_custom_getrandom!(fail);
}

/// The PCS shape every accepted proof must have exactly.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VosPolicy {
    /// Blowup 16, 19 queries, 20-bit proof of work: the smaller proofs (about 0.5 MB).
    Standard,
    /// Blowup 4, 38 queries, 20-bit proof of work: faster to prove, larger proofs.
    Mobile,
}

impl VosPolicy {
    fn pcs(self) -> PcsPolicy {
        match self {
            VosPolicy::Standard => PcsPolicy::STANDARD,
            VosPolicy::Mobile => PcsPolicy::MOBILE,
        }
    }
}

/// Picoseconds of `ref_time` charged per proof byte.
///
/// TODO: a placeholder from an early measurement of this verifier inside a Substrate runtime's wasm
/// executor (`frame-omni-bencher`, not reference hardware): an upper envelope through the origin
/// over STANDARD, MOBILE and canonical-shape proofs. Benchmark on reference hardware, per pinned
/// policy, before use.
pub const REF_TIME_PER_PROOF_BYTE: u64 = 55_000;

/// VOS's STARK verifier, with the PCS policy `P` and the log-size cap `L` pinned by the runtime.
pub struct VosVerifier<P, L>(PhantomData<(P, L)>);

impl<P: Get<VosPolicy>, L: Get<u8>> ProofVerifier for VosVerifier<P, L> {
    fn verify(
        program: &ProgramId,
        proof: &[u8],
        public: &[u8],
        output: &[u8],
    ) -> Result<(), VerifyError> {
        let (proof, rest) =
            postcard::take_from_bytes::<Proof>(proof).map_err(|_| VerifyError::Malformed)?;
        if !rest.is_empty() {
            return Err(VerifyError::Malformed);
        }
        if proof.public_io_hash() != vos_io_hash(public, output) {
            return Err(VerifyError::NotBound);
        }
        let commitment = CommitmentHash::from(&program[..]);
        if proof.stark_proof.commitments.first() != Some(&commitment) {
            return Err(VerifyError::WrongProgram);
        }
        verify_standalone_with_options(proof, commitment, L::get().into(), &P::get().pcs())
            .map_err(|_| VerifyError::Invalid)
    }

    /// The proof's bytes are not declared as proof size: `CheckWeight` already counts the
    /// extrinsic's length as PoV, and declaring them again would halve how many proofs fit a block.
    fn weight(proof_len: u32) -> Weight {
        Weight::from_parts(REF_TIME_PER_PROOF_BYTE, 0).saturating_mul(proof_len.into())
    }
}

#[cfg(test)]
mod tests {
    //! A real proof through the pallet, in a runtime whose verifier is VOS's.
    //!
    //! `fixtures/anchor1.*` is a STANDARD proof made by `fixtures/gen` with VOS's prover. Its
    //! program commits to one [`AnchorStatement`](crate::AnchorStatement): Space `10`, epoch `0`,
    //! anchor `1`, from the zero root to `[1; 32]`, on a chain whose genesis hash is zero (as in a
    //! test externality). See `fixtures/README.md`.
    use super::*;
    use crate::{self as pallet_spaces, Error, Heads, Pallet as Spaces};
    use codec::Encode;
    use frame_support::{
        assert_noop, assert_ok, derive_impl, parameter_types,
        traits::{ConstU32, ConstU8},
        BoundedVec,
    };
    use frame_system::{EnsureRoot, EnsureSigned};

    const PROOF: &[u8] = include_bytes!("../fixtures/anchor1.proof");
    const COMMITMENT: &[u8; 32] = include_bytes!("../fixtures/anchor1.commitment");
    const PUBLIC: &[u8] = include_bytes!("../fixtures/anchor1.public");
    const OUTPUT: &[u8] = include_bytes!("../fixtures/anchor1.output");

    const SPACE: u32 = 10;
    const GENESIS: [u8; 32] = [0; 32];
    const ROOT: [u8; 32] = [1; 32];

    type Block = frame_system::mocking::MockBlock<VosTest>;

    frame_support::construct_runtime!(
        pub enum VosTest {
            System: frame_system,
            SpacesVos: pallet_spaces,
        }
    );

    #[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
    impl frame_system::Config for VosTest {
        type Block = Block;
    }

    parameter_types! {
        pub const Standard: VosPolicy = VosPolicy::Standard;
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub struct NoHelper;
    #[cfg(feature = "runtime-benchmarks")]
    impl crate::BenchmarkHelper<u32> for NoHelper {
        fn space(i: u32) -> u32 {
            i
        }
        fn program() -> ProgramId {
            *COMMITMENT
        }
        fn proof(_: &ProgramId, _: &[u8], _: &[u8]) -> alloc::vec::Vec<u8> {
            PROOF.to_vec()
        }
    }

    impl crate::Config for VosTest {
        type SpaceId = u32;
        type BindKey = u32;
        type RegisterOrigin = EnsureSigned<u64>;
        type ResetOrigin = EnsureRoot<u64>;
        type Verifier = VosVerifier<Standard, ConstU8<18>>;
        type MaxProofLen = ConstU32<{ 1536 * 1024 }>;
        type MaxPublicLen = ConstU32<{ 16 * 1024 }>;
        type WeightInfo = ();
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper = NoHelper;
    }

    fn ext() -> sp_io::TestExternalities {
        let mut ext = sp_io::TestExternalities::new(Default::default());
        ext.execute_with(|| {
            System::set_block_number(1);
            assert_ok!(Spaces::<VosTest>::register(
                RuntimeOrigin::signed(1),
                SPACE,
                *COMMITMENT,
                GENESIS
            ));
        });
        ext
    }

    fn anchor(
        number: u64,
        root: [u8; 32],
        proof: &[u8],
        public: &[u8],
    ) -> frame_support::dispatch::DispatchResultWithPostInfo {
        Spaces::<VosTest>::anchor(
            RuntimeOrigin::signed(2),
            SPACE,
            number,
            root,
            BoundedVec::truncate_from(proof.to_vec()),
            BoundedVec::truncate_from(public.to_vec()),
        )
    }

    #[test]
    fn the_fixture_commits_to_the_pallets_statement() {
        ext().execute_with(|| {
            let head = Heads::<VosTest>::get(SPACE).unwrap();
            assert_eq!(
                Spaces::<VosTest>::anchor_statement(SPACE, &head, ROOT).encode(),
                OUTPUT
            );
        });
    }

    #[test]
    fn a_real_proof_anchors() {
        ext().execute_with(|| {
            assert_ok!(anchor(1, ROOT, PROOF, PUBLIC));
            let head = Heads::<VosTest>::get(SPACE).unwrap();
            assert_eq!((head.number, head.root), (1, ROOT));
        });
    }

    #[test]
    fn a_real_proof_is_bound_to_its_statement() {
        ext().execute_with(|| {
            // Another root.
            assert_noop!(
                anchor(1, [2; 32], PROOF, PUBLIC),
                Error::<VosTest>::NotBound
            );
            // Another public input.
            assert_noop!(anchor(1, ROOT, PROOF, b"other"), Error::<VosTest>::NotBound);
            // The head moved: a reset to the same root opens a new epoch.
            assert_ok!(Spaces::<VosTest>::set_current_head(
                RuntimeOrigin::root(),
                SPACE,
                GENESIS
            ));
            assert_noop!(anchor(1, ROOT, PROOF, PUBLIC), Error::<VosTest>::NotBound);
        });
    }

    #[test]
    fn a_real_proof_of_another_program_is_refused() {
        ext().execute_with(|| {
            assert_ok!(Spaces::<VosTest>::register(
                RuntimeOrigin::signed(1),
                SPACE + 1,
                [9; 32],
                GENESIS
            ));
            // Bound to Space 10's statement, so it fails the binding first; check the program
            // comparison on the verifier directly.
            assert_eq!(
                <VosVerifier<Standard, ConstU8<18>> as ProofVerifier>::verify(
                    &[9; 32], PROOF, PUBLIC, OUTPUT
                ),
                Err(VerifyError::WrongProgram)
            );
        });
    }

    #[test]
    fn a_tampered_real_proof_is_refused() {
        ext().execute_with(|| {
            let mut tampered = PROOF.to_vec();
            let mid = tampered.len() / 2;
            tampered[mid] ^= 1;
            assert!(anchor(1, ROOT, &tampered, PUBLIC).is_err());
            assert_eq!(Heads::<VosTest>::get(SPACE).unwrap().number, 0);
        });
    }

    #[test]
    fn undecodable_bytes_are_malformed() {
        ext().execute_with(|| {
            assert_noop!(
                anchor(1, ROOT, &[1, 2, 3], PUBLIC),
                Error::<VosTest>::MalformedProof
            );
        });
    }
}
