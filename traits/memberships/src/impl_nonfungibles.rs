use crate::*;
use frame_support::{sp_runtime::DispatchError, traits::tokens::nonfungibles_v2 as nonfungibles};

const ATTR_MEMBER_COUNT: &[u8] = b"membership_member_count";
const ATTR_MEMBER_RANK: &[u8] = b"membership_member_rank";

impl<T, AccountId> Inspect<AccountId> for T
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

    fn member_count(group: &Self::Group) -> u32 {
        T::typed_system_attribute(group, None, &ATTR_MEMBER_COUNT).unwrap_or(0u32)
    }
}

impl<T, AccountId> Manager<AccountId> for T
where
    T: nonfungibles::Mutate<AccountId>
        + nonfungibles::Inspect<AccountId>
        + nonfungibles::InspectEnumerable<AccountId>,
    T::OwnedInCollectionIterator: 'static,
    T::OwnedIterator: 'static,
    T::CollectionId: Parameter + Zero + 'static,
    T::ItemConfig: Default,
{
    fn assign(
        group: &Self::Group,
        m: &Self::Membership,
        who: &AccountId,
    ) -> Result<(), DispatchError> {
        let mgr_group = Self::Group::zero();
        T::burn(&mgr_group, m, None)?;
        let count = Self::member_count(group);
        T::mint_into(group, m, who, &T::ItemConfig::default(), true)?;
        T::set_typed_collection_attribute(group, &ATTR_MEMBER_COUNT, &(count + 1))
    }

    fn release(group: &Self::Group, m: &Self::Membership) -> Result<(), DispatchError> {
        T::burn(&group, m, None)?;

        let count = Self::member_count(group);
        T::set_typed_collection_attribute(group, &ATTR_MEMBER_COUNT, &(count - 1))
    }
}

impl<T, AccountId> Rank<AccountId> for T
where
    T: nonfungibles::Mutate<AccountId>
        + nonfungibles::Inspect<AccountId>
        + nonfungibles::InspectEnumerable<AccountId>,
    T::OwnedInCollectionIterator: 'static,
    T::OwnedIterator: 'static,
    T::CollectionId: 'static,
{
    fn rank_of(group: &Self::Group, m: &Self::Membership) -> GenericRank {
        T::typed_system_attribute(group, Some(m), &ATTR_MEMBER_RANK).unwrap_or_default()
    }

    fn set_rank(
        group: &Self::Group,
        m: &Self::Membership,
        rank: impl Into<GenericRank>,
    ) -> Result<(), DispatchError> {
        T::set_typed_attribute(group, m, &ATTR_MEMBER_RANK, &rank.into())
    }
}
