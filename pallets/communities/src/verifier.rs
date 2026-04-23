//! Default merkle proof verifier for membership proofs.

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use fc_traits_proof_verifier::{ProofVerifier, VerifyError};
use scale_info::TypeInfo;
use sp_runtime::traits::Hash;

/// Merkle inclusion proof
#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking, PartialEq, Eq, TypeInfo)]
pub struct MerkleProof<H: Hash> {
    /// The leaf hash
    pub leaf: H::Output,
    /// Proof siblings
    pub siblings: Vec<H::Output>,
    /// Leaf index in the tree
    pub leaf_index: u32,
    /// Total number of leaves
    pub leaf_count: u32,
}

/// Public inputs for membership proof verification
#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct MembershipInputs<H: Hash> {
    /// The merkle root to verify against
    pub root: H::Output,
}

/// Simple merkle inclusion proof verifier (no ZK).
/// Uses binary-merkle-tree for verification.
pub struct MerkleVerifier<H>(core::marker::PhantomData<H>);

impl<H: Hash + scale_info::TypeInfo + 'static> ProofVerifier for MerkleVerifier<H>
where
    H::Output: Ord + Default,
{
    type Proof = MerkleProof<H>;
    type PublicInputs = MembershipInputs<H>;
    type ProgramId = (); // no program selection needed

    fn verify(
        _program: &Self::ProgramId,
        proof: &Self::Proof,
        public_inputs: &Self::PublicInputs,
    ) -> Result<(), VerifyError> {
        // `H::Output: Copy` for all substrate hashers we care about, so the iterator-based
        // form avoids the otherwise-needless clone of the siblings vec.
        let valid = binary_merkle_tree::verify_proof::<H, _, _>(
            &public_inputs.root,
            proof.siblings.iter().copied(),
            proof.leaf_count,
            proof.leaf_index,
            &proof.leaf,
        );
        if valid {
            Ok(())
        } else {
            Err(VerifyError::InvalidProof)
        }
    }
}
