#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::Parameter;
use sp_runtime::DispatchError;

/// Error from proof verification
#[derive(Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// The proof is invalid
    InvalidProof,
    /// The program/verification key is not registered
    UnknownProgram,
    /// Public inputs are malformed
    InvalidInputs,
}

impl From<VerifyError> for DispatchError {
    fn from(e: VerifyError) -> Self {
        match e {
            VerifyError::InvalidProof => DispatchError::Other("InvalidProof"),
            VerifyError::UnknownProgram => DispatchError::Other("UnknownProgram"),
            VerifyError::InvalidInputs => DispatchError::Other("InvalidInputs"),
        }
    }
}

/// General-purpose proof verification trait.
///
/// Implementations can range from simple merkle inclusion proofs to
/// full ZK proof verification (e.g. stwo STARKs via a PVM zkVM).
pub trait ProofVerifier {
    /// The proof blob
    type Proof: Parameter;
    /// Public inputs/outputs visible to the verifier
    type PublicInputs: Parameter;
    /// Identifies the verification program (e.g. verification key, circuit ID).
    /// Use `()` for verifiers that don't need program selection.
    type ProgramId: Parameter;

    /// Verify a proof for the given program and public inputs.
    fn verify(
        program: &Self::ProgramId,
        proof: &Self::Proof,
        public_inputs: &Self::PublicInputs,
    ) -> Result<(), VerifyError>;
}
