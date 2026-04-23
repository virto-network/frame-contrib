extern crate alloc;

use crate::{
    origin::RawOrigin as CommunityOrigin, verifier::MembershipInputs, Config, MerkleRoot, SubRoots,
    UsedNullifiers,
};
use codec::{Decode, DecodeWithMemTracking, Encode};
use fc_traits_proof_verifier::ProofVerifier;
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

/// Custom transaction validity codes for this extension.
pub mod codes {
    /// The community has no membership root to verify against.
    pub const NO_MEMBERSHIP_ROOT: u8 = 100;
    /// The proof did not verify against the membership root.
    pub const INVALID_MEMBERSHIP_PROOF: u8 = 101;
    /// The nullifier has already been consumed for this action scope.
    pub const NULLIFIER_USED: u8 = 102;
}

/// Transaction extension that authenticates anonymous community members.
///
/// When `Some(params)`, it verifies a merkle inclusion proof against the community's
/// root and replaces the origin with an anonymous community origin. The nullifier
/// stored in `UsedNullifiers` is **derived deterministically** from the proof leaf
/// and the call hash, so a single leaf cannot authorize more than one action per
/// call scope. Callers cannot pick their own nullifier or rank — both are derived
/// on-chain.
///
/// When `None`, it acts as a passthrough.
///
/// ### Soundness limits of the merkle-only MVP
///
/// The verifier only proves leaf membership, not leaf *contents*. Therefore:
/// - The pallet cannot trust any claimed rank carried in the proof; anonymous votes
///   are rank-1 regardless of the leaf's actual rank.
/// - Anonymity is pseudonymity: the leaf is public, so on-chain observers can link a
///   vote to a member. Real privacy requires the ZK verifier backend.
///
/// These limits are lifted once `MembershipVerifier` is replaced with a ZK proof
/// verifier that binds private inputs (rank, nullifier seed) to the leaf.
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
    const IDENTIFIER: &'static str = "fc_pallet_communities::AnonymousMembership";
    type Implicit = ();
    /// (community_id, action_scope, nullifier) when authenticated. Used at dispatch and
    /// post-dispatch time. All three are derived server-side — never trusted from the caller.
    type Val = Option<(T::CommunityId, H256, H256)>;
    type Pre = Option<(T::CommunityId, H256, H256)>;

    fn weight(&self, _call: &RuntimeCallFor<T>) -> Weight {
        // Placeholder until benchmarks return. Scales with proof size in a real backend.
        Weight::from_parts(50_000_000, 0)
    }

    fn validate(
        &self,
        origin: DispatchOriginOf<RuntimeCallFor<T>>,
        call: &RuntimeCallFor<T>,
        _info: &DispatchInfoOf<RuntimeCallFor<T>>,
        _len: usize,
        _self_implicit: Self::Implicit,
        _inherited_implication: &impl Implication,
        _source: TransactionSource,
    ) -> ValidateResult<Self::Val, RuntimeCallFor<T>> {
        let Some(params) = &self.0 else {
            return Ok((ValidTransaction::default(), None, origin));
        };

        let root = if let Some(sub_track) = params.sub_track {
            SubRoots::<T>::get(&params.community_id, sub_track)
        } else {
            MerkleRoot::<T>::get(&params.community_id)
        }
        .ok_or(TransactionValidityError::from(InvalidTransaction::Custom(
            codes::NO_MEMBERSHIP_ROOT,
        )))?;

        let public_inputs = MembershipInputs { root };
        T::MembershipVerifier::verify(&(), &params.proof, &public_inputs).map_err(|_| {
            TransactionValidityError::from(InvalidTransaction::Custom(
                codes::INVALID_MEMBERSHIP_PROOF,
            ))
        })?;

        // Action scope is derived from the call being dispatched — the caller cannot
        // choose it, so a proof authenticated for one call cannot be re-used for another.
        let action_scope: H256 = sp_io::hashing::blake2_256(&call.encode()).into();

        // Nullifier is derived from the verified proof leaf and the action scope. Two
        // consequences: (a) the caller cannot pick fresh random nullifiers to bypass the
        // replay check, and (b) the same leaf produces the same nullifier for the same
        // call, so duplicates collide in `UsedNullifiers`.
        let nullifier_preimage = (
            b"fc-communities/null/v1",
            &params.community_id,
            action_scope.as_bytes(),
            params.sub_track,
            root.encode(),
            params.proof.encode(),
        )
            .encode();
        let nullifier: H256 = sp_io::hashing::blake2_256(&nullifier_preimage).into();

        if UsedNullifiers::<T>::contains_key((&params.community_id, &action_scope, &nullifier)) {
            return Err(TransactionValidityError::from(InvalidTransaction::Custom(
                codes::NULLIFIER_USED,
            )));
        }

        let mut community_origin = CommunityOrigin::new(params.community_id);
        community_origin.with_subset(crate::origin::Subset::AnonymousMember {
            // Rank cannot be trusted without a ZK binding — treat the anonymous vote as
            // rank-1 (membership-only) regardless of the leaf's on-chain rank.
            rank: frame_contrib_traits::memberships::GenericRank::default(),
            nullifier,
        });
        let new_origin: T::RuntimeOrigin = community_origin.into();

        Ok((
            ValidTransaction::default(),
            Some((params.community_id, action_scope, nullifier)),
            new_origin,
        ))
    }

    fn prepare(
        self,
        val: Self::Val,
        _origin: &DispatchOriginOf<RuntimeCallFor<T>>,
        _call: &RuntimeCallFor<T>,
        _info: &DispatchInfoOf<RuntimeCallFor<T>>,
        _len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        Ok(val)
    }

    fn post_dispatch_details(
        pre: Self::Pre,
        _info: &DispatchInfoOf<RuntimeCallFor<T>>,
        _post_info: &PostDispatchInfoOf<RuntimeCallFor<T>>,
        _len: usize,
        _result: &DispatchResult,
    ) -> Result<Weight, TransactionValidityError> {
        // Store the nullifier whether dispatch succeeded or failed — otherwise a caller
        // could griefing-loop by crafting failing calls that never consume the nullifier.
        if let Some((community_id, action_scope, nullifier)) = pre {
            UsedNullifiers::<T>::insert((&community_id, &action_scope, &nullifier), ());
        }
        Ok(Weight::zero())
    }
}
