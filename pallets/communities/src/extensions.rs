extern crate alloc;

use crate::{
    origin::RawOrigin as CommunityOrigin, verifier::MembershipInputs, Config, MerkleRoot, SubRoots,
    UsedNullifiers,
};
use fc_traits_proof_verifier::ProofVerifier;
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_contrib_traits::memberships::GenericRank;
use frame_support::{
    pallet_prelude::{TransactionValidityError, Weight},
    CloneNoBound, DebugNoBound, DefaultNoBound, EqNoBound, PartialEqNoBound,
};
use frame_system::pallet_prelude::RuntimeCallFor;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{
    traits::{
        DispatchInfoOf, DispatchOriginOf, Implication, PostDispatchInfoOf, TransactionExtension,
        ValidateResult,
    },
    transaction_validity::{InvalidTransaction, TransactionSource, ValidTransaction},
    DispatchResult,
};

/// Transaction extension that validates anonymous membership proofs.
///
/// When `Some(params)`, it verifies a merkle proof against the community's
/// membership root, checks the nullifier hasn't been used, and replaces
/// the origin with an anonymous community origin.
///
/// When `None`, it acts as a passthrough.
#[derive(
    DefaultNoBound,
    Encode,
    Decode,
    DecodeWithMemTracking,
    CloneNoBound,
    EqNoBound,
    PartialEqNoBound,
    DebugNoBound,
    TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct AnonymousMembership<T: Config>(pub Option<MembershipProofParams<T>>);

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    CloneNoBound,
    EqNoBound,
    PartialEqNoBound,
    DebugNoBound,
    TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub struct MembershipProofParams<T: Config> {
    pub community_id: T::CommunityId,
    /// The proof in whatever format the verifier expects
    pub proof: <T::MembershipVerifier as ProofVerifier>::Proof,
    /// Rank exposed as public input (for vote weight)
    pub rank: GenericRank,
    /// Nullifier: hash(identity_secret, action_scope) -- prevents double-action
    pub nullifier: H256,
    /// What this proof is for (e.g. hash of poll_index for voting)
    pub action_scope: H256,
    /// Optional sub-track to verify against SubRoots instead of MerkleRoot
    pub sub_track: Option<u16>,
}

impl<T: Config> TransactionExtension<RuntimeCallFor<T>> for AnonymousMembership<T>
where
    T::RuntimeOrigin: From<CommunityOrigin<T>>,
    T::CommunityId: Send + Sync,
    <T::Hasher as sp_runtime::traits::Hash>::Output: Send + Sync,
    <T::MembershipVerifier as ProofVerifier>::Proof: Send + Sync,
{
    const IDENTIFIER: &'static str = "AnonymousMembership";
    type Implicit = ();
    /// (community_id, nullifier) if authenticated
    type Val = Option<(T::CommunityId, H256)>;
    /// (community_id, action_scope, nullifier) for post-dispatch cleanup
    type Pre = Option<(T::CommunityId, H256, H256)>;

    fn weight(&self, _call: &RuntimeCallFor<T>) -> Weight {
        // Merkle proof verification weight -- depends on proof length.
        // For MVP, use a placeholder.
        Weight::from_parts(50_000_000, 0)
    }

    fn validate(
        &self,
        origin: DispatchOriginOf<RuntimeCallFor<T>>,
        _call: &RuntimeCallFor<T>,
        _info: &DispatchInfoOf<RuntimeCallFor<T>>,
        _len: usize,
        _self_implicit: Self::Implicit,
        _inherited_implication: &impl Implication,
        _source: TransactionSource,
    ) -> ValidateResult<Self::Val, RuntimeCallFor<T>> {
        if let Some(params) = &self.0 {
            // Get the root to verify against
            let root = if let Some(sub_track) = params.sub_track {
                SubRoots::<T>::get(&params.community_id, sub_track)
            } else {
                MerkleRoot::<T>::get(&params.community_id)
            }
            .ok_or(TransactionValidityError::from(InvalidTransaction::Custom(
                1,
            )))?; // no root set

            // Verify proof via the configured verifier
            let public_inputs = MembershipInputs { root };
            T::MembershipVerifier::verify(&(), &params.proof, &public_inputs)
                .map_err(|_| TransactionValidityError::from(InvalidTransaction::BadSigner))?;

            // Check nullifier hasn't been used
            if UsedNullifiers::<T>::contains_key((
                &params.community_id,
                &params.action_scope,
                &params.nullifier,
            )) {
                return Err(InvalidTransaction::Stale.into());
            }

            // Create anonymous community origin
            let mut community_origin = CommunityOrigin::new(params.community_id);
            community_origin.with_subset(crate::origin::Subset::AnonymousMember {
                rank: params.rank,
                nullifier: params.nullifier,
            });
            let new_origin: T::RuntimeOrigin = community_origin.into();

            Ok((
                ValidTransaction::default(),
                Some((params.community_id, params.nullifier)),
                new_origin,
            ))
        } else {
            // No proof provided, pass origin through unchanged
            Ok((ValidTransaction::default(), None, origin))
        }
    }

    fn prepare(
        self,
        val: Self::Val,
        _origin: &DispatchOriginOf<RuntimeCallFor<T>>,
        _call: &RuntimeCallFor<T>,
        _info: &DispatchInfoOf<RuntimeCallFor<T>>,
        _len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        Ok(val.map(|(cid, nullifier)| {
            let action_scope = self.0.as_ref().map(|p| p.action_scope).unwrap_or_default();
            (cid, action_scope, nullifier)
        }))
    }

    fn post_dispatch_details(
        pre: Self::Pre,
        _info: &DispatchInfoOf<RuntimeCallFor<T>>,
        _post_info: &PostDispatchInfoOf<RuntimeCallFor<T>>,
        _len: usize,
        _result: &DispatchResult,
    ) -> Result<Weight, TransactionValidityError> {
        // Store nullifier AFTER dispatch (whether success or failure) to prevent replay
        if let Some((community_id, action_scope, nullifier)) = pre {
            UsedNullifiers::<T>::insert((&community_id, &action_scope, &nullifier), ());
        }
        Ok(Weight::zero())
    }
}
