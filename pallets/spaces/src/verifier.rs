//! The seam between the pallet and a proof system.
//!
//! The pallet never decodes a proof. It hands the configured [`ProofVerifier`] the program the
//! Space currently runs ([`ProgramId`]), the submitted proof, the program's public input (opaque
//! bytes only the program decodes) and the output the pallet expects the program to have returned
//! (an encoded [`AnchorStatement`](crate::AnchorStatement)). A backend accepts the proof only if it
//! is a valid proof of an execution of that program that binds exactly that input and that output.
//!
//! Backends for real proof systems live with those proof systems and implement this trait; this
//! crate ships only [`MockVerifier`], for tests and benchmarks.

use crate::ProgramId;
use codec::{Decode, Encode};
use frame_support::weights::Weight;
use scale_info::TypeInfo;

/// Why a backend refused a proof.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Encode, Decode, TypeInfo)]
pub enum VerifyError {
    /// The bytes are not a proof this backend can decode.
    Malformed,
    /// The proof does not bind this public input and this output.
    NotBound,
    /// The proof is of another program.
    WrongProgram,
    /// The proof does not verify.
    Invalid,
}

/// A proof system the pallet can check anchors with.
pub trait ProofVerifier {
    /// Check that `proof` proves an execution of `program` whose public input is `public` and
    /// whose output is `output`.
    fn verify(
        program: &ProgramId,
        proof: &[u8],
        public: &[u8],
        output: &[u8],
    ) -> Result<(), VerifyError>;

    /// The weight of [`verify`](Self::verify) for a proof of `proof_len` bytes, at its worst.
    ///
    /// It belongs to the backend, not to the pallet's benchmarks, so that a runtime that swaps
    /// the backend swaps its cost with it.
    fn weight(proof_len: u32) -> Weight;
}

/// A stand-in proof system, **for tests and benchmarks only**: it proves nothing.
///
/// A "proof" is `program ‖ binding ‖ seal`: `binding` is BLAKE2b-256 of the SCALE encoding of
/// `(public, output)`, and `seal` is BLAKE2b-256 of the first 64 bytes. It refuses the way a real
/// backend does: bytes of the wrong length are [`Malformed`](VerifyError::Malformed), a broken
/// seal is [`Invalid`](VerifyError::Invalid), another program is
/// [`WrongProgram`](VerifyError::WrongProgram), and another input or output is
/// [`NotBound`](VerifyError::NotBound).
#[cfg(any(test, feature = "runtime-benchmarks"))]
pub struct MockVerifier;

#[cfg(any(test, feature = "runtime-benchmarks"))]
impl MockVerifier {
    /// The length of a mock proof.
    pub const PROOF_LEN: usize = 96;

    fn binding(public: &[u8], output: &[u8]) -> [u8; 32] {
        sp_io::hashing::blake2_256(&(public, output).encode())
    }

    /// A proof `verify` accepts for `program`, `public` and `output`.
    pub fn prove(program: &ProgramId, public: &[u8], output: &[u8]) -> alloc::vec::Vec<u8> {
        let mut p = alloc::vec::Vec::with_capacity(Self::PROOF_LEN);
        p.extend_from_slice(program);
        p.extend_from_slice(&Self::binding(public, output));
        let seal = sp_io::hashing::blake2_256(&p);
        p.extend_from_slice(&seal);
        p
    }
}

#[cfg(any(test, feature = "runtime-benchmarks"))]
impl ProofVerifier for MockVerifier {
    fn verify(
        program: &ProgramId,
        proof: &[u8],
        public: &[u8],
        output: &[u8],
    ) -> Result<(), VerifyError> {
        if proof.len() != Self::PROOF_LEN {
            return Err(VerifyError::Malformed);
        }
        if sp_io::hashing::blake2_256(&proof[..64]) != proof[64..] {
            return Err(VerifyError::Invalid);
        }
        if &proof[..32] != program {
            return Err(VerifyError::WrongProgram);
        }
        if proof[32..64] != Self::binding(public, output) {
            return Err(VerifyError::NotBound);
        }
        Ok(())
    }

    fn weight(_proof_len: u32) -> Weight {
        Weight::from_parts(1_000_000, 0)
    }
}
