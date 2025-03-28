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

pub struct NonFungiblesMemberships<T>(PhantomData<T>);

impl<T, AccountId> Inspect<AccountId> for NonFungiblesMemberships<T>
where
    T: nonfungibles::Inspect<AccountId> + nonfungibles::InspectEnumerable<AccountId>,
    T::OwnedInCollectionIterator: 'static,
    T::OwnedIterator: 'static,
    T::CollectionId: 'static,
{
    type Group = T::CollectionId;
    type Membership = T::ItemId;

    fn user_memberships(
        who: &AccountId,
        maybe_group: Option<Self::Group>,
    ) -> Box<dyn Iterator<Item = (Self::Group, Self::Membership)>> {
        if let Some(group) = maybe_group {
            Box::new(T::owned_in_collection(&group, who).map(move |m| (group.clone(), m)))
        } else {
            Box::new(T::owned(who))
        }
    }

    fn check_membership(who: &AccountId, m: &Self::Membership) -> Option<Self::Group> {
        Self::user_memberships(who, None).find_map(|(g, membership)| membership.eq(m).then_some(g))
    }

    fn members_total(group: &Self::Group) -> u32 {
        T::typed_system_attribute(group, None, &ATTR_MEMBER_TOTAL).unwrap_or(0u32)
    }
}

impl<T, AccountId, ItemConfig> Manager<AccountId, ItemConfig> for NonFungiblesMemberships<T>
where
    T: nonfungibles::Mutate<AccountId, ItemConfig>
        + nonfungibles::Inspect<AccountId>
        + nonfungibles::Transfer<AccountId>
        + nonfungibles::InspectEnumerable<AccountId>,
    T::OwnedInCollectionIterator: 'static,
    T::OwnedIterator: 'static,
    T::CollectionId: Parameter + Zero + 'static,
    AccountId: From<[u8; 32]>,
    ItemConfig: Default,
{
    fn assign(
        group: &Self::Group,
        m: &Self::Membership,
        who: &AccountId,
    ) -> Result<(), DispatchError> {
        let mgr_group = Self::Group::zero();
        T::transfer(&mgr_group, m, &ASSIGNED_MEMBERSHIPS_ACCOUNT.into())?;
        T::mint_into(group, m, who, &ItemConfig::default(), true)?;
        // membership shouldn't have a rank but just in case we reset it to 0
        T::set_typed_attribute(group, m, &ATTR_MEMBER_RANK, &GenericRank::from(0))?;
        let count = Self::members_total(group);
        T::set_typed_collection_attribute(group, &ATTR_MEMBER_TOTAL, &(count + 1))
    }

    fn release(group: &Self::Group, m: &Self::Membership) -> Result<(), DispatchError> {
        Self::set_rank(group, m, 0)?;
        T::burn(group, m, None)?;
        let count = Self::members_total(group);
        T::set_typed_collection_attribute(group, &ATTR_MEMBER_TOTAL, &(count - 1))?;

        let mgr_group = Self::Group::zero();
        let group_owner = T::collection_owner(group)
            .expect("the group existed when burning the membership, the group has an owner; qed");
        T::transfer(&mgr_group, m, &group_owner)
    }
}

impl<T, AccountId, ItemConfig> Rank<AccountId, ItemConfig> for NonFungiblesMemberships<T>
where
    T: nonfungibles::Mutate<AccountId, ItemConfig>
        + nonfungibles::Inspect<AccountId>
        + nonfungibles::InspectEnumerable<AccountId>,
    T::OwnedInCollectionIterator: 'static,
    T::OwnedIterator: 'static,
    T::CollectionId: 'static,
    ItemConfig: Default,
{
    fn rank_of(group: &Self::Group, m: &Self::Membership) -> Option<GenericRank> {
        T::typed_system_attribute(group, Some(m), &ATTR_MEMBER_RANK)
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
        T::set_typed_attribute(group, m, &ATTR_MEMBER_RANK, &new)?;
        T::set_typed_collection_attribute(group, &ATTR_MEMBER_RANK_TOTAL, &new_total)
    }

    fn ranks_total(group: &Self::Group) -> u32 {
        T::typed_system_attribute(group, None, &ATTR_MEMBER_RANK_TOTAL).unwrap_or(0u32)
    }
}
