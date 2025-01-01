use super::*;
use frame_support::dispatch::DispatchResult;

/// Triggers an action when a membership has been assigned
pub trait OnMembershipAssigned<AccountId: Clone, Group: Clone, Membership: Clone> {
    fn on_membership_assigned(
        &self,
        who: AccountId,
        group: Group,
        membership: Membership,
    ) -> DispatchResult;
}

impl<A: Clone, G: Clone, M: Clone> OnMembershipAssigned<A, G, M> for () {
    fn on_membership_assigned(&self, _: A, _: G, _: M) -> DispatchResult {
        Ok(())
    }
}

impl<T, A: Clone, G: Clone, M: Clone> OnMembershipAssigned<A, G, M> for T
where
    T: Fn(A, G, M) -> DispatchResult,
{
    fn on_membership_assigned(&self, who: A, group: G, membership: M) -> DispatchResult {
        self(who, group, membership)
    }
}

/// Triggers an action when a membership has been released
pub trait OnMembershipReleased<Group: Clone, Membership: Clone> {
    fn on_membership_released(&self, group: Group, membership: Membership) -> DispatchResult;
}

impl<T, G: Clone, M: Clone> OnMembershipReleased<G, M> for T
where
    T: Fn(G, M) -> DispatchResult,
{
    fn on_membership_released(&self, group: G, membership: M) -> DispatchResult {
        self(group, membership)
    }
}

impl<G: Clone, M: Clone> OnMembershipReleased<G, M> for () {
    fn on_membership_released(&self, _: G, _: M) -> DispatchResult {
        Ok(())
    }
}

/// Triggers an action when a rank has been set for a membership
pub trait OnRankSet<Group: Clone, Membership: Clone, Rank: Clone = GenericRank> {
    fn on_rank_set(&self, group: Group, membership: Membership, rank: Rank) -> DispatchResult;
}
impl<G: Clone, M: Clone, R: Clone> OnRankSet<G, M, R> for () {
    fn on_rank_set(&self, _: G, _: M, _: R) -> DispatchResult {
        Ok(())
    }
}

impl<T, G: Clone, M: Clone, R: Clone> OnRankSet<G, M, R> for T
where
    T: Fn(G, M, R) -> DispatchResult,
{
    fn on_rank_set(&self, group: G, membership: M, rank: R) -> DispatchResult {
        self(group, membership, rank)
    }
}