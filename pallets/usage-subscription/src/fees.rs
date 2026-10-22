//! The adapter for `fc-pallet-fees`: an account's community is its paying group.

use crate::{Config, GroupOf, Pallet};
use core::marker::PhantomData;
use fc_pallet_fees::AccountCommunity;

/// Resolves an account's community for `fc-pallet-fees` as its **paying group** (`REQ-PC-2`): the
/// group the account named and still belongs to, or else the only group it belongs to. An account
/// with no membership, or with several and none named, has no community.
///
/// It is the resolution the [`Pallet::paying_group`] view and the payment step make, with the
/// same bounded reads (`REQ-PC-3`). A runtime binds it in the fees pallet when that pallet's
/// `CommunityId` is the group type:
///
/// ```ignore
/// type CommunityDetector = fc_pallet_usage_subscription::PayingGroupCommunity<Runtime>;
/// ```
///
/// Available with the `fees` feature.
pub struct PayingGroupCommunity<T>(PhantomData<T>);

impl<T: Config> AccountCommunity<T::AccountId, GroupOf<T>> for PayingGroupCommunity<T> {
    fn community_of(who: &T::AccountId) -> Option<GroupOf<T>> {
        Pallet::<T>::resolve_paying_group(who).group()
    }
}
