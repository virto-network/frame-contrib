#![cfg_attr(not(feature = "std"), no_std)]
//! # Memberships
//!
//! Traits to manage the memberships of groups, and an implementation over a
//! [`nonfungibles_v2`](frame_support::traits::tokens::nonfungibles_v2) collection system (such as
//! `pallet-nfts`):
//!
//! - [`GroupCollectionMemberships`]: every group's memberships live in the group's own collection.
//!   The group account holds the group's **stock** (its unassigned memberships), members hold the
//!   assigned ones, and a manager account owns every collection. Every membership item is locked
//!   against transfers and burns, so only the memberships manager moves it, and a member hands a
//!   membership on only as its group's [`TransferPolicy`] allows ([`Transfer`]). A group retires a
//!   membership by moving it from its stock to a retirement holder ([`Issue::retire`]), which,
//!   like the group account, is never a member, and burns it from there ([`Issue::burn`]). It
//!   indexes each membership's group when it is issued, so [`Inspect::check_membership`] takes
//!   two reads, whatever else the account holds.
//!
//!   [`Attributes::set_membership_attribute`] writes only the keys the runtime allows (the
//!   `AttributeKeys` parameter; none by default). A release and a burn clear those keys, and the
//!   membership's rank, so a membership leaves its holder with nothing its manager wrote for
//!   them. A transfer keeps them: they travel with the membership.
extern crate alloc;

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use core::{
    num::NonZeroU8,
    ops::{Add, Sub},
};
use frame_support::{dispatch::DispatchResult, sp_runtime::DispatchError, Parameter};

mod group_collection;
pub use group_collection::GroupCollectionMemberships;

mod hooks;
mod impls;

pub use hooks::*;
pub use impls::WithHooks;

/// The attribute that counts a group's assigned memberships, as a `u32`. A collection attribute.
///
/// Every attribute key below is written by the memberships manager in the `Pallet` namespace of
/// the collection system, SCALE-encoded (so a `&[u8]` key is length-prefixed), and read through
/// `Inspect::system_attribute`.
pub const ATTR_MEMBER_TOTAL: &[u8] = b"membership_member_total";
/// The attribute that holds an assigned membership's rank, as a [`GenericRank`]. An item attribute.
pub const ATTR_MEMBER_RANK: &[u8] = b"membership_member_rank";
/// The attribute that sums the ranks of a group's assigned memberships, as a `u32`. A collection
/// attribute.
pub const ATTR_MEMBER_RANK_TOTAL: &[u8] = b"membership_rank_total";
/// The attribute that holds a group's [`TransferPolicy`]. A collection attribute; when it is
/// absent, the policy is [`TransferPolicy::default`].
pub const ATTR_TRANSFER_POLICY: &[u8] = b"membership_transfer_policy";

/// Access data associated to a unique membership
pub trait Inspect<AccountId> {
    type Group: Parameter;
    type Membership: Parameter;

    /// Retrieve all memberships belonging to member optionally filtering by group
    fn user_memberships(
        who: &AccountId,
        maybe_group: Option<Self::Group>,
    ) -> Box<dyn Iterator<Item = (Self::Group, Self::Membership)>>;

    /// Check membership is owned by the given account
    fn is_member_of(group: &Self::Group, who: &AccountId) -> bool {
        Self::user_memberships(who, Some(group.clone()))
            .next()
            .is_some()
    }

    /// The memberships `who` holds, in every group, reading at most `max_reads` items: the items
    /// it returns and the items it skips (such as a group account's own stock) count alike.
    ///
    /// Each membership found is yielded as `Ok`. Once all `max_reads` reads are spent, the
    /// iterator yields one `Err(ReadBoundReached)` and stops, without reading on to find out
    /// whether anything was left. So a caller that must tell "at most `n` items" from "more than
    /// `n`" asks for `n + 1` reads and takes the error to mean "more".
    ///
    /// The default counts one read per membership [`Inspect::user_memberships`] yields for every
    /// group, so it bounds the reads of an implementation that skips nothing. An implementation
    /// that skips items overrides it.
    fn user_memberships_bounded(
        who: &AccountId,
        max_reads: u32,
    ) -> BoundedMemberships<Self::Group, Self::Membership>
    where
        Self::Group: 'static,
        Self::Membership: 'static,
    {
        bounded_reads(Self::user_memberships(who, None).map(Some), max_reads)
    }

    /// Check if an account owns the given membership and return the group it belongs to
    fn check_membership(who: &AccountId, m: &Self::Membership) -> Option<Self::Group>;

    /// Whether `who` holds the membership `m` of `group`, as a member.
    ///
    /// By default, this scans the memberships `who` holds in `group`. Implementations that can
    /// read a membership's holder directly should override it.
    fn holds(group: &Self::Group, who: &AccountId, m: &Self::Membership) -> bool {
        Self::user_memberships(who, Some(group.clone())).any(|(_, membership)| &membership == m)
    }

    /// How many members exist in a group
    fn members_total(group: &Self::Group) -> u32;
}

/// The read bound of [`Inspect::user_memberships_bounded`] was reached: there may be more
/// memberships than those yielded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadBoundReached;

/// What [`Inspect::user_memberships_bounded`] returns: each membership found, as `(group,
/// membership)`, then [`ReadBoundReached`] if the read bound was reached.
pub type BoundedMemberships<Group, Membership> =
    Box<dyn Iterator<Item = Result<(Group, Membership), ReadBoundReached>>>;

/// Yields the `Some` items of `reads`, counting one read for every item of `reads` (a `None` is a
/// read that found nothing to yield), and, once `max_reads` reads are spent, one
/// `Err(ReadBoundReached)`. It never reads more than `max_reads` items of `reads`.
pub(crate) fn bounded_reads<T: 'static>(
    mut reads: impl Iterator<Item = Option<T>> + 'static,
    max_reads: u32,
) -> Box<dyn Iterator<Item = Result<T, ReadBoundReached>>> {
    let mut left = max_reads;
    let mut done = false;
    Box::new(core::iter::from_fn(move || loop {
        if done {
            return None;
        }
        if left == 0 {
            done = true;
            return Some(Err(ReadBoundReached));
        }
        left -= 1;
        match reads.next() {
            Some(Some(item)) => return Some(Ok(item)),
            Some(None) => continue,
            None => {
                done = true;
                return None;
            }
        }
    }))
}

/// Access lists of memberships.
pub trait InspectEnumerable<AccountId>: Inspect<AccountId> {
    /// Returns an optional iterator of the available memberships owned by a group (i.e. memberships
    /// which haven't been activated), or `None` if the group doesn't exist.
    fn group_available_memberships(
        group: &Self::Group,
    ) -> Box<dyn Iterator<Item = Self::Membership>>;

    /// Returns an iterator of the memberships owned by the given account optionally filtering
    /// by group
    fn memberships_of(
        who: &AccountId,
        maybe_group: Option<Self::Group>,
    ) -> Box<dyn Iterator<Item = (Self::Group, Self::Membership)>>;
}

pub trait Attributes<AccountId>: Inspect<AccountId> {
    /// Retrieves an attribute associated to the membership, if any
    fn membership_attribute<K: Encode, V: Parameter>(
        g: &Self::Group,
        m: &Self::Membership,
        key: &K,
    ) -> Option<V>;

    /// Sets some value for an attribute on a membership, if the membership exists
    ///
    /// An implementation may limit the keys it accepts. [`GroupCollectionMemberships`] accepts
    /// only the keys its runtime allows, and clears them when the membership is released or
    /// burnt, so no attribute set here outlives its holder's membership.
    fn set_membership_attribute<K: Encode, V: Encode>(
        g: &Self::Group,
        m: &Self::Membership,
        key: &K,
        value: &V,
    ) -> Result<(), DispatchError>;

    /// Clears some value for an attribute on a membership, if any
    fn clear_membership_attribute<K: Encode>(
        g: &Self::Group,
        m: &Self::Membership,
        key: &K,
    ) -> Result<(), DispatchError>;
}

pub trait Manager<AccountId>: Inspect<AccountId> {
    /// Transfers ownership of an unclaimed membership in the manager group to an account in the given group and activates it.
    fn assign(
        group: &Self::Group,
        m: &Self::Membership,
        who: &AccountId,
    ) -> Result<(), DispatchError>;

    /// Releases the ownership of a claimed membership in a given group.
    fn release(group: &Self::Group, m: &Self::Membership) -> Result<(), DispatchError>;
}

/// Moves a membership between holders, as its group's policy allows.
pub trait Transfer<AccountId>: Manager<AccountId> {
    /// The transfer policy of `group`. A group that never set one has
    /// [`TransferPolicy::default`]: no transfers, and the rank reset.
    fn transfer_policy(group: &Self::Group) -> TransferPolicy;

    /// Sets the transfer policy of `group`. Whoever calls this is responsible for checking that
    /// the caller may administer `group`.
    fn set_transfer_policy(
        group: &Self::Group,
        policy: TransferPolicy,
    ) -> Result<(), DispatchError>;

    /// Moves the membership `m` of `group` from its current holder, a member, to `to`, as the
    /// group's transfer policy allows. Whoever calls this is responsible for checking that the
    /// caller is the holder.
    ///
    /// Refuses with [`Error::NotAMember`] if `m` is not held by a member of `group`,
    /// [`Error::TransferDisabled`] if the policy allows no transfers, and [`Error::NotSameGroup`]
    /// if the policy does not allow `to` to receive it, or `to` is the group's own account.
    fn transfer(
        group: &Self::Group,
        m: &Self::Membership,
        to: &AccountId,
    ) -> Result<(), DispatchError>;
}

/// Who may receive a membership a member transfers.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Decode,
    DecodeWithMemTracking,
    Encode,
    MaxEncodedLen,
    scale_info::TypeInfo,
)]
pub enum Receivers {
    /// Nobody: memberships cannot be transferred.
    #[default]
    Disabled,
    /// Only an account that already holds a valid membership of the group.
    ToExistingMembers,
    /// Any account other than the group's own account.
    ToAnyAccount,
}

/// What happens to a membership's rank when it is transferred.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Decode,
    DecodeWithMemTracking,
    Encode,
    MaxEncodedLen,
    scale_info::TypeInfo,
)]
pub enum RankOnTransfer {
    /// The rank is reset to zero.
    #[default]
    Reset,
    /// The rank travels with the membership.
    Keep,
}

/// A group's rule for transfers of its memberships.
///
/// The default, which every group has until it sets one, allows no transfers and resets the
/// rank.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Decode,
    DecodeWithMemTracking,
    Encode,
    MaxEncodedLen,
    scale_info::TypeInfo,
)]
pub struct TransferPolicy {
    /// Who may receive a membership.
    pub receivers: Receivers,
    /// Whether the rank travels with a membership.
    pub rank: RankOnTransfer,
}

/// Why a memberships manager refused to act.
///
/// Each variant converts into a [`DispatchError`] (`DispatchError::Other` with the variant's
/// name), and back with [`TryFrom`], so a pallet can map it to an error of its own.
///
/// `DispatchError::Other` because this crate is no pallet: it has no pallet index, so it cannot
/// make a `DispatchError::Module`, and the traits return [`DispatchError`]. Two consequences for
/// callers:
/// - `Other`'s message is not encoded, so an error that reaches a client unmapped carries no
///   name. A pallet maps every variant its calls can meet to an error of its own.
/// - The mapping recognises any `Other` with a variant's name, whoever made it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The membership is not held by a member of its group: it doesn't exist, it is in the
    /// group's stock, or the account named does not hold it.
    NotAMember,
    /// The group's transfer policy allows no transfers.
    TransferDisabled,
    /// The group's transfer policy does not allow the recipient to receive the membership, or the
    /// recipient is the group's own account.
    NotSameGroup,
    /// The membership is not in the group's stock, so it cannot be assigned.
    NotInStock,
    /// The attribute is kept by the memberships manager: it cannot be written or cleared through
    /// [`Attributes`], and no attribute write may unlock a membership item.
    ReservedAttribute,
    /// The account named is not the memberships manager's retirement holder, or the manager has
    /// none, so it cannot retire memberships.
    NotRetirementHolder,
    /// The membership is assigned to a member, so it cannot be burnt: it is released first.
    Assigned,
    /// The membership identifier is already another group's membership.
    IdInUse,
    /// The group's collection holds no such membership.
    UnknownMembership,
    /// The attribute key is not one the memberships manager lets callers write or clear.
    AttributeNotAllowed,
}

impl Error {
    /// The name of the variant, as carried by `DispatchError::Other`.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Error::NotAMember => "NotAMember",
            Error::TransferDisabled => "TransferDisabled",
            Error::NotSameGroup => "NotSameGroup",
            Error::NotInStock => "NotInStock",
            Error::ReservedAttribute => "ReservedAttribute",
            Error::NotRetirementHolder => "NotRetirementHolder",
            Error::Assigned => "Assigned",
            Error::IdInUse => "IdInUse",
            Error::UnknownMembership => "UnknownMembership",
            Error::AttributeNotAllowed => "AttributeNotAllowed",
        }
    }

    const ALL: [Error; 10] = [
        Error::NotAMember,
        Error::TransferDisabled,
        Error::NotSameGroup,
        Error::NotInStock,
        Error::ReservedAttribute,
        Error::NotRetirementHolder,
        Error::Assigned,
        Error::IdInUse,
        Error::UnknownMembership,
        Error::AttributeNotAllowed,
    ];
}

impl From<Error> for DispatchError {
    fn from(e: Error) -> Self {
        DispatchError::Other(e.as_str())
    }
}

impl TryFrom<DispatchError> for Error {
    type Error = DispatchError;

    /// Recovers the manager's error from a [`DispatchError`], or returns the error unchanged if it
    /// is not one of the manager's.
    fn try_from(e: DispatchError) -> Result<Self, Self::Error> {
        match e {
            DispatchError::Other(name) => Error::ALL
                .into_iter()
                .find(|variant| variant.as_str() == name)
                .ok_or(e),
            e => Err(e),
        }
    }
}

/// Issues memberships into a group's stock, locked, takes them out of it when the group retires
/// them, and burns them.
///
/// Issuance and retirement belong to the group's issuer (the runtime's), which also holds the
/// deposits and enforces the removal delay; this is the part of both that touches the items.
pub trait Issue<AccountId>: Inspect<AccountId> {
    /// Mints `m` into the stock of `group`, held by the group's account, locks it, and indexes
    /// its group.
    ///
    /// Refuses with [`Error::IdInUse`] if `m` is already indexed as another group's membership.
    fn issue(group: &Self::Group, m: &Self::Membership) -> DispatchResult;

    /// Indexes `m` as a membership of `group`. A migration aid: a runtime migration indexes the
    /// memberships minted before the index existed, or without [`Issue::issue`]. Idempotent.
    ///
    /// Refuses with [`Error::UnknownMembership`] if `group`'s collection holds no item `m`, and
    /// with [`Error::IdInUse`] if `m` is already indexed as another group's membership.
    fn index(group: &Self::Group, m: &Self::Membership) -> DispatchResult;

    /// Locks `m` against transfers and burns through the collection system, if it isn't already.
    /// A migration aid, for memberships minted unlocked; [`Issue::issue`] locks what it mints.
    fn lock(group: &Self::Group, m: &Self::Membership) -> DispatchResult;

    /// Takes `m` out of the stock of `group` and gives it to `holder`, the manager's retirement
    /// holder, where it is no member's and cannot be assigned. It stays locked; the group's member
    /// count and rank sum do not change.
    ///
    /// The caller names `holder`, so the call states where the membership goes; it must be the
    /// retirement holder the manager is configured with.
    ///
    /// Refuses with [`Error::NotRetirementHolder`] if `holder` is not the manager's retirement
    /// holder, and with [`Error::NotInStock`] if `m` is not in the stock of `group`.
    fn retire(group: &Self::Group, m: &Self::Membership, holder: &AccountId) -> DispatchResult;

    /// Clears what the manager wrote on the membership (its rank, and the attributes callers may
    /// set through [`Attributes`]), unlocks it, burns it, and removes it from the index.
    ///
    /// Only a membership that is not assigned is burnt: one in the stock of `group`, or one its
    /// retirement holder holds. One held by any other account is a member's, and is refused with
    /// [`Error::Assigned`]: an assigned membership is released first. Burning does not change the
    /// group's member count.
    ///
    /// Attributes others wrote in their own namespaces of the collection system, such as a
    /// holder's or a delegate's, are not the manager's to clear: the issuer clears them when it
    /// burns, if the collection system lets them outlive the item.
    fn burn(group: &Self::Group, m: &Self::Membership) -> DispatchResult;
}

/// A membership with a rating system
pub trait Rank<AccountId, Rank = GenericRank>: Inspect<AccountId>
where
    Rank: Eq + Ord,
{
    fn rank_of(group: &Self::Group, m: &Self::Membership) -> Option<Rank>;

    fn set_rank(
        group: &Self::Group,
        m: &Self::Membership,
        rank: impl Into<Rank>,
    ) -> Result<(), DispatchError>;

    /// The sum of the ranks for all members in a group
    fn ranks_total(group: &Self::Group) -> u32;
}

/// A generic rank in the range 0 to 100
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    Decode,
    DecodeWithMemTracking,
    Encode,
    MaxEncodedLen,
    scale_info::TypeInfo,
)]
pub struct GenericRank(u8);
impl GenericRank {
    pub const MIN: Self = GenericRank(0);
    pub const MAX: Self = GenericRank(100);
    pub const ADMIN: Self = Self::MAX;

    pub fn set(self, n: u8) -> Self {
        Self(n.min(Self::MAX.0))
    }
    pub fn promote_by(self, n: NonZeroU8) -> Self {
        Self(self.0.saturating_add(n.get()).min(Self::MAX.0))
    }
    pub fn demote_by(self, n: NonZeroU8) -> Self {
        Self(self.0.saturating_sub(n.get()))
    }
}
impl From<GenericRank> for u8 {
    fn from(value: GenericRank) -> u8 {
        value.0
    }
}
impl From<GenericRank> for u16 {
    fn from(value: GenericRank) -> u16 {
        u8::from(value) as u16
    }
}
impl From<GenericRank> for u32 {
    fn from(value: GenericRank) -> u32 {
        u8::from(value) as u32
    }
}
impl From<u8> for GenericRank {
    fn from(value: u8) -> Self {
        GenericRank::default().set(value)
    }
}
impl Add for GenericRank {
    type Output = Self;
    fn add(self, r: GenericRank) -> Self::Output {
        if r.0 == 0 {
            return self;
        }
        self.promote_by(NonZeroU8::new(r.0).unwrap())
    }
}
impl Sub for GenericRank {
    type Output = Self;
    fn sub(self, r: Self) -> Self::Output {
        if r.0 == 0 {
            return self;
        }
        self.demote_by(NonZeroU8::new(r.0).unwrap())
    }
}
