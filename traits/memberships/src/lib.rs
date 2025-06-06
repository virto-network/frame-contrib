#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use core::{
    num::NonZeroU8,
    ops::{Add, Sub},
};
use frame_support::{sp_runtime::DispatchError, Parameter};

mod impl_nonfungibles;
pub use impl_nonfungibles::NonFungiblesMemberships;

mod hooks;
mod impls;

pub use hooks::*;
pub use impls::WithHooks;

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

    /// Check if an account owns the given membership and return the group it belongs to
    fn check_membership(who: &AccountId, m: &Self::Membership) -> Option<Self::Group>;

    /// How many members exist in a group
    fn members_total(group: &Self::Group) -> u32;
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
    fn membership_attribute<K: Encode, V: Decode>(
        g: &Self::Group,
        m: &Self::Membership,
        key: &K,
    ) -> Option<V>;

    /// Sets some value for an attribute on a membership, if the membership exists
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
