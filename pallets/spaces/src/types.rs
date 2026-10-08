//! The types a Space is made of.

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

/// The identity of the program a Space runs: the 32-byte commitment the proof verifier recognises.
///
/// For VOS's STARK verifier this is the program's preprocessed-trace Merkle root, which VOS calls
/// the program's identity. The pallet never interprets it; it hands it to
/// [`Config::Verifier`](crate::Config::Verifier).
pub type ProgramId = [u8; 32];

/// A state root: what a Space's program commits its state to.
pub type Root = [u8; 32];

/// The position of an anchor in a Space's sequence. The registration's root stands at `0`; the
/// first anchor is `1`. Numbers are never reused, across epochs too.
pub type AnchorNumber = u64;

/// A Space's epochs: the registration opens epoch `0`, and every re-founding or reset opens the
/// next one.
pub type Epoch = u32;

/// What the pallet keeps about a Space.
#[derive(
    Encode, Decode, DecodeWithMemTracking, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Debug,
)]
pub struct SpaceInfo<AccountId> {
    /// Who may re-found the Space while nothing is bound.
    pub owner: AccountId,
    /// The program every anchor of this Space must be a proof of.
    pub program: ProgramId,
    /// How many binds the Space holds. While it is above zero, only the reset origin can move
    /// the head off its anchored chain.
    pub binds: u32,
}

/// Where a Space's anchoring stands.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
)]
pub struct Head<BlockNumber> {
    /// The current epoch.
    pub epoch: Epoch,
    /// The number of the last anchor, or the epoch's base if the epoch has none yet.
    pub number: AnchorNumber,
    /// The state the next transition must start from.
    pub root: Root,
    /// The block at which the head last changed.
    pub at: BlockNumber,
}

/// A stored anchor. Never removed or overwritten.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
)]
pub struct AnchorRecord<BlockNumber> {
    /// The state root the proof moved the Space to.
    pub root: Root,
    /// The epoch the anchor belongs to.
    pub epoch: Epoch,
    /// The block in which it was stored.
    pub at: BlockNumber,
}

/// Why an epoch was opened.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
)]
pub enum EpochCause {
    /// The Space was registered.
    Registered,
    /// The owner re-founded the Space on a new genesis while nothing was bound.
    Refounded,
    /// The reset origin set the Space's head, as `Paras::set_current_head` does for a parachain.
    Reset,
}

/// How an epoch began.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
)]
pub struct EpochInfo<BlockNumber> {
    /// The root the epoch starts from: a genesis, or the root the reset origin set.
    pub root: Root,
    /// The anchor number the epoch starts after. Its first anchor is `base + 1`.
    pub base: AnchorNumber,
    /// The block in which it was opened.
    pub at: BlockNumber,
    /// Why.
    pub cause: EpochCause,
}

/// A bind: a commitment, made by another pallet, that something of value now depends on the
/// Space's anchored state (for example, assets held for it whose accounting lives in that state).
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
)]
pub struct BindRecord<BlockNumber> {
    /// The epoch in which it was made.
    pub epoch: Epoch,
    /// The last anchor at the time it was made.
    pub after_anchor: AnchorNumber,
    /// The block in which it was made.
    pub at: BlockNumber,
}

/// The statement an anchor proof must commit to, as the program's return bytes.
///
/// The pallet builds it, so a proof is bound to this chain, this Space, this epoch, this anchor
/// number and both roots; a proof copied from the pool cannot be replayed elsewhere, and a proof
/// made against a head that has since moved (by an anchor, a re-founding or a reset) no longer
/// verifies. The program's public input stays opaque to the chain.
#[derive(Encode, Decode, TypeInfo, Clone, PartialEq, Eq, Debug)]
pub struct AnchorStatement<SpaceId> {
    /// [`ANCHOR_DOMAIN`].
    pub domain: [u8; 16],
    /// [`ANCHOR_STATEMENT_VERSION`].
    pub version: u8,
    /// The chain's genesis hash.
    pub chain: [u8; 32],
    /// The Space.
    pub space: SpaceId,
    /// The head's epoch.
    pub epoch: Epoch,
    /// The anchor's number: the head's number plus one.
    pub number: AnchorNumber,
    /// The head's root: where the transition starts.
    pub prev_root: Root,
    /// The anchored root: where the transition ends.
    pub root: Root,
}

/// The domain tag that leads every [`AnchorStatement`].
pub const ANCHOR_DOMAIN: [u8; 16] = *b"fc-spaces/anchor";

/// The version of [`AnchorStatement`]'s encoding.
pub const ANCHOR_STATEMENT_VERSION: u8 = 1;
