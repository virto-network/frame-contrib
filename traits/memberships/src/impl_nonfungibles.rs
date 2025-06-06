use crate::*;
use alloc::boxed::Box;
use core::marker::PhantomData;
use frame_support::{
    pallet_prelude::DispatchError,
    sp_runtime::{str_array, traits::Zero},
    traits::tokens::nonfungibles_v2 as nonfungibles,
};

const ATTR_MEMBER_TOTAL: &[u8] = b"membership_member_total";
const ATTR_MEMBER_RANK: &[u8] = b"membership_member_rank";
const ATTR_MEMBER_RANK_TOTAL: &[u8] = b"membership_rank_total";

pub const ASSIGNED_MEMBERSHIPS_ACCOUNT: [u8; 32] = str_array("memberships/assigned_memberships");

pub struct NonFungiblesMemberships<NF, IC>(PhantomData<(NF, IC)>);

impl<NF, IC, AccountId> Inspect<AccountId> for NonFungiblesMemberships<NF, IC>
where
    NF: nonfungibles::Inspect<AccountId> + nonfungibles::InspectEnumerable<AccountId>,
    NF::OwnedInCollectionIterator: 'static,
    NF::OwnedIterator: 'static,
    NF::CollectionId: 'static,
{
    type Group = NF::CollectionId;
    type Membership = NF::ItemId;

    fn user_memberships(
        who: &AccountId,
        maybe_group: Option<Self::Group>,
    ) -> Box<dyn Iterator<Item = (Self::Group, Self::Membership)>> {
        if let Some(group) = maybe_group {
            Box::new(NF::owned_in_collection(&group, who).map(move |m| (group.clone(), m)))
        } else {
            Box::new(NF::owned(who))
        }
    }

    fn check_membership(who: &AccountId, m: &Self::Membership) -> Option<Self::Group> {
        Self::user_memberships(who, None).find_map(|(g, membership)| membership.eq(m).then_some(g))
    }

    fn members_total(group: &Self::Group) -> u32 {
        NF::typed_system_attribute(group, None, &ATTR_MEMBER_TOTAL).unwrap_or(0u32)
    }
}

impl<NF, IC, AccountId> InspectEnumerable<AccountId> for NonFungiblesMemberships<NF, IC>
where
    NF: nonfungibles::Inspect<AccountId> + nonfungibles::InspectEnumerable<AccountId>,
    NF::OwnedInCollectionIterator: 'static,
    NF::OwnedIterator: 'static,
    NF::CollectionId: Parameter + Zero + 'static,
    NF::ItemId: Parameter + 'static,
{
    fn group_available_memberships(
        group: &Self::Group,
    ) -> Box<dyn Iterator<Item = Self::Membership>> {
        let mgr_group = Self::Group::zero();
        if let Some(group_owner) = &NF::collection_owner(group) {
            Box::new(NF::owned_in_collection(&mgr_group, group_owner))
        } else {
            Box::new(core::iter::empty::<Self::Membership>())
        }
    }

    fn memberships_of(
        who: &AccountId,
        maybe_group: Option<Self::Group>,
    ) -> Box<dyn Iterator<Item = (Self::Group, Self::Membership)>> {
        let iter = NF::owned(who);

        if let Some(group) = maybe_group {
            Box::new(iter.filter(move |(g, _)| *g == group.clone()))
        } else {
            Box::new(iter)
        }
    }
}

impl<NF, ItemConfig, AccountId> Attributes<AccountId> for NonFungiblesMemberships<NF, ItemConfig>
where
    NF: nonfungibles::Inspect<AccountId>
        + nonfungibles::InspectEnumerable<AccountId>
        + nonfungibles::Mutate<AccountId, ItemConfig>,
    NF::OwnedInCollectionIterator: 'static,
    NF::OwnedIterator: 'static,
    NF::CollectionId: Parameter + Zero + 'static,
    NF::ItemId: Parameter + 'static,
{
    fn membership_attribute<K: Encode, V: Decode>(
        g: &Self::Group,
        m: &Self::Membership,
        key: &K,
    ) -> Option<V> {
        NF::typed_attribute(g, m, key)
    }

    fn set_membership_attribute<K: Encode, V: Encode>(
        g: &Self::Group,
        m: &Self::Membership,
        key: &K,
        value: &V,
    ) -> Result<(), DispatchError> {
        NF::set_typed_attribute(g, m, key, value)
    }

    fn clear_membership_attribute<K: Encode>(
        g: &Self::Group,
        m: &Self::Membership,
        key: &K,
    ) -> Result<(), DispatchError> {
        NF::clear_typed_attribute(g, m, key)
    }
}

impl<NF, ItemConfig, AccountId> Manager<AccountId> for NonFungiblesMemberships<NF, ItemConfig>
where
    NF: nonfungibles::Mutate<AccountId, ItemConfig>
        + nonfungibles::Inspect<AccountId>
        + nonfungibles::Transfer<AccountId>
        + nonfungibles::InspectEnumerable<AccountId>,
    NF::OwnedInCollectionIterator: 'static,
    NF::OwnedIterator: 'static,
    NF::CollectionId: Parameter + Zero + 'static,
    AccountId: From<[u8; 32]>,
    ItemConfig: Default,
{
    fn assign(
        group: &Self::Group,
        m: &Self::Membership,
        who: &AccountId,
    ) -> Result<(), DispatchError> {
        let mgr_group = Self::Group::zero();
        NF::transfer(&mgr_group, m, &ASSIGNED_MEMBERSHIPS_ACCOUNT.into())?;
        NF::mint_into(group, m, who, &ItemConfig::default(), true)?;
        // membership shouldn't have a rank but just in case we reset it to 0
        NF::set_typed_attribute(group, m, &ATTR_MEMBER_RANK, &GenericRank::from(0))?;
        let count = Self::members_total(group);
        NF::set_typed_collection_attribute(group, &ATTR_MEMBER_TOTAL, &(count + 1))
    }

    fn release(group: &Self::Group, m: &Self::Membership) -> Result<(), DispatchError> {
        Self::set_rank(group, m, 0)?;
        NF::burn(group, m, None)?;
        let count = Self::members_total(group);
        NF::set_typed_collection_attribute(group, &ATTR_MEMBER_TOTAL, &(count - 1))?;

        let mgr_group = Self::Group::zero();
        let group_owner = NF::collection_owner(group)
            .expect("the group existed when burning the membership, the group has an owner; qed");
        NF::transfer(&mgr_group, m, &group_owner)
    }
}

impl<NF, ItemConfig, AccountId> Rank<AccountId> for NonFungiblesMemberships<NF, ItemConfig>
where
    NF: nonfungibles::Mutate<AccountId, ItemConfig>
        + nonfungibles::Inspect<AccountId>
        + nonfungibles::InspectEnumerable<AccountId>,
    NF::OwnedInCollectionIterator: 'static,
    NF::OwnedIterator: 'static,
    NF::CollectionId: 'static,
    ItemConfig: Default,
{
    fn rank_of(group: &Self::Group, m: &Self::Membership) -> Option<GenericRank> {
        NF::typed_system_attribute(group, Some(m), &ATTR_MEMBER_RANK)
    }

    fn set_rank(
        group: &Self::Group,
        m: &Self::Membership,
        rank: impl Into<GenericRank>,
    ) -> Result<(), DispatchError> {
        let prev = Self::rank_of(group, m).ok_or(DispatchError::Other("Invalid membership"))?;
        let new = rank.into();
        let prev_total = Self::ranks_total(group);
        let new_total = if new > prev {
            prev_total + u32::from(new - prev)
        } else {
            prev_total - u32::from(prev - new)
        };
        NF::set_typed_attribute(group, m, &ATTR_MEMBER_RANK, &new)?;
        NF::set_typed_collection_attribute(group, &ATTR_MEMBER_RANK_TOTAL, &new_total)
    }

    fn ranks_total(group: &Self::Group) -> u32 {
        NF::typed_system_attribute(group, None, &ATTR_MEMBER_RANK_TOTAL).unwrap_or(0u32)
    }
}
