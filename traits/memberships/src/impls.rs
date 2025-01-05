use super::*;
use alloc::boxed::Box;
use core::marker::PhantomData;
use frame_support::traits::Get;

/// Extends a structure that already implements [`Manager`], and [`Rank`] to support
/// hooks that are triggered after changes in memberships or ranks happen.
pub struct WithHooks<T, OnMembershipAssigned = NoOp, OnMembershipReleased = NoOp, OnRankSet = NoOp>(
    PhantomData<(T, OnMembershipAssigned, OnMembershipReleased, OnRankSet)>,
);

impl<T, MA, MR, RS, AccountId> Inspect<AccountId> for WithHooks<T, MA, MR, RS>
where
    T: Inspect<AccountId>,
{
    type Group = T::Group;
    type Membership = T::Membership;

    fn user_memberships(
        who: &AccountId,
        maybe_group: Option<Self::Group>,
    ) -> Box<dyn Iterator<Item = (Self::Group, Self::Membership)>> {
        T::user_memberships(who, maybe_group)
    }

    fn is_member_of(group: &Self::Group, who: &AccountId) -> bool {
        T::is_member_of(group, who)
    }

    fn check_membership(who: &AccountId, m: &Self::Membership) -> Option<Self::Group> {
        T::check_membership(who, m)
    }

    fn members_total(group: &Self::Group) -> u32 {
        T::members_total(group)
    }
}

impl<T, MA, MR, RS, AccountId, ItemConfig> Manager<AccountId, ItemConfig>
    for WithHooks<T, MA, MR, RS>
where
    AccountId: Clone,
    T: Manager<AccountId, ItemConfig>,
    MA: Get<Box<dyn OnMembershipAssigned<AccountId, T::Group, T::Membership>>>,
    MR: Get<Box<dyn OnMembershipReleased<T::Group, T::Membership>>>,
{
    fn assign(
        group: &Self::Group,
        m: &Self::Membership,
        who: &AccountId,
    ) -> Result<(), DispatchError> {
        T::assign(group, m, who)?;
        MA::get().on_membership_assigned(who.clone(), group.clone(), m.clone())?;
        Ok(())
    }

    fn release(group: &Self::Group, m: &Self::Membership) -> Result<(), DispatchError> {
        T::release(group, m)?;
        MR::get().on_membership_released(group.clone(), m.clone())?;
        Ok(())
    }
}

impl<T, MA, MR, RS, R, AccountId, ItemConfig> Rank<AccountId, ItemConfig, R>
    for WithHooks<T, MA, MR, RS>
where
    AccountId: Clone,
    R: Ord + Clone,
    T: Rank<AccountId, ItemConfig, R>,
    RS: Get<Box<dyn OnRankSet<T::Group, T::Membership, R>>>,
{
    fn rank_of(group: &Self::Group, m: &Self::Membership) -> Option<R> {
        T::rank_of(group, m)
    }

    fn set_rank(
        group: &Self::Group,
        m: &Self::Membership,
        rank: impl Into<R>,
    ) -> Result<(), DispatchError> {
        let rank = rank.into();
        T::set_rank(group, m, rank.clone())?;
        RS::get().on_rank_set(group.clone(), m.clone(), rank)?;
        Ok(())
    }

    fn ranks_total(group: &Self::Group) -> u32 {
        T::ranks_total(group)
    }
}
