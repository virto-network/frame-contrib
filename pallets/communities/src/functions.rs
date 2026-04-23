use super::*;

use frame_contrib_traits::memberships::GenericRank;
use frame_support::{
    fail,
    traits::{
        fungible::{InspectFreeze, Mutate, MutateFreeze},
        fungibles::{InspectFreeze as _, MutateFreeze as _},
        tokens::Fortitude::Polite,
        Polling,
    },
};
use sp_runtime::traits::{AccountIdConversion, Dispatchable};
use sp_runtime::Saturating;

impl<T: Config> Pallet<T> {
    #[inline]
    pub fn community_account(community_id: &T::CommunityId) -> AccountIdOf<T> {
        T::PalletId::get().into_sub_account_truncating(community_id)
    }

    pub fn community_exists(community_id: &T::CommunityId) -> bool {
        Info::<T>::contains_key(community_id)
    }

    pub fn is_member(community_id: &T::CommunityId, who: &AccountIdOf<T>) -> bool {
        Members::<T>::get(community_id, who)
            .map(|m| m.status == MemberStatus::Active)
            .unwrap_or(false)
    }

    pub fn member_rank(community_id: &T::CommunityId, who: &AccountIdOf<T>) -> GenericRank {
        Members::<T>::get(community_id, who)
            .map(|m| m.rank)
            .unwrap_or_default()
    }

    /// Whether `who` can act as a manager of the community: Admin and Manager roles,
    /// active status only. Used to gate member-management extrinsics when invoked by
    /// a signed origin that's also a community member.
    pub fn is_member_manager(community_id: &T::CommunityId, who: &AccountIdOf<T>) -> bool {
        match Members::<T>::get(community_id, who) {
            Some(rec) => {
                rec.status == MemberStatus::Active
                    && matches!(rec.role, Role::Admin | Role::Manager)
            }
            None => false,
        }
    }

    /// Resolve the community id for a member-mgmt action, and enforce that if the
    /// caller is a *signed* account (rather than a community/root/governance origin),
    /// that account holds the Admin or Manager role in the target community.
    ///
    /// This is the authorization layer that makes `Role` meaningful. Without it,
    /// anyone who satisfies `MemberMgmtOrigin` (often `EnsureCommunity`, which accepts
    /// the community origin or accounts registered via `CommunityIdFor`) could take
    /// member-management actions regardless of role.
    pub(crate) fn ensure_member_mgmt(
        origin: OriginFor<T>,
    ) -> Result<CommunityIdOf<T>, DispatchError> {
        // Pull out the signed caller (if any) before handing the origin to the
        // configured guard — the guard consumes it.
        let maybe_signer = origin
            .as_system_ref()
            .and_then(|s| match s {
                frame_system::RawOrigin::Signed(who) => Some(who.clone()),
                _ => None,
            });
        let community_id = T::MemberMgmtOrigin::ensure_origin(origin)?;
        if let Some(signer) = maybe_signer {
            ensure!(
                Self::is_member_manager(&community_id, &signer),
                Error::<T>::NotAuthorized,
            );
        }
        Ok(community_id)
    }

    pub fn force_state(community_id: &CommunityIdOf<T>, state: CommunityState) {
        Info::<T>::mutate(community_id, |c| c.as_mut().map(|c| c.state = state));
    }

    /// Stores an initial info about the community
    /// Sets the caller as the community admin, the initial community state
    /// to its default value(awaiting)
    pub fn register(
        admin: &PalletsOriginOf<T>,
        community_id: &CommunityIdOf<T>,
        maybe_deposit: Option<(NativeBalanceOf<T>, AccountIdOf<T>, AccountIdOf<T>)>,
    ) -> DispatchResult {
        ensure!(
            !Self::community_exists(community_id),
            Error::<T>::CommunityAlreadyExists
        );
        ensure!(
            !CommunityIdFor::<T>::contains_key(admin),
            Error::<T>::AlreadyAdmin
        );

        if let Some((deposit, depositor, depositee)) = maybe_deposit {
            T::Balances::transfer(
                &depositor,
                &depositee,
                deposit,
                frame_support::traits::tokens::Preservation::Preserve,
            )?;
        }

        CommunityIdFor::<T>::insert(admin, community_id);
        Info::<T>::insert(
            community_id,
            CommunityInfo {
                state: CommunityState::default(),
                privacy: PrivacyLevel::default(),
                capacity: T::MaxMembers::get(),
            },
        );
        frame_system::Pallet::<T>::inc_providers(&Self::community_account(community_id));

        Ok(())
    }

    pub(crate) fn try_vote_by_key(
        community_id: &CommunityIdOf<T>,
        decision_method: &DecisionMethodFor<T>,
        vote_multiplier: u32,
        voter_key: &<T::Hasher as sp_runtime::traits::Hash>::Output,
        poll_index: PollIndexOf<T>,
        vote: &VoteOf<T>,
    ) -> DispatchResult {
        T::Polls::try_access_poll(poll_index, |poll_status| {
            let (tally, class) = poll_status.ensure_ongoing().ok_or(Error::<T>::NotOngoing)?;
            ensure!(community_id == &class, Error::<T>::InvalidTrack);

            let say = *match (vote, decision_method) {
                (
                    Vote::AssetBalance(say, asset, amount),
                    DecisionMethod::CommunityAsset(a, min),
                ) if asset == a => {
                    ensure!(amount >= min, Error::<T>::VoteBelowMinimum);
                    say
                }
                (Vote::NativeBalance(say, ..), DecisionMethod::NativeToken)
                | (Vote::Standard(say), DecisionMethod::Membership | DecisionMethod::Rank) => say,
                _ => fail!(Error::<T>::InvalidVoteType),
            };

            let vote_weight = VoteWeight::from(vote);
            let multiplied = vote_multiplier.saturating_mul(vote_weight);
            tally.add_vote(say, multiplied, vote_weight);

            CommunityVotes::<T>::insert(poll_index, voter_key, (vote, multiplied));
            Ok(())
        })
    }

    pub(crate) fn try_remove_vote_by_key(
        community_id: &CommunityIdOf<T>,
        voter_key: &<T::Hasher as sp_runtime::traits::Hash>::Output,
        poll_index: PollIndexOf<T>,
    ) -> DispatchResult {
        T::Polls::try_access_poll(poll_index, |poll_status| {
            let (tally, class) = poll_status.ensure_ongoing().ok_or(Error::<T>::NotOngoing)?;
            ensure!(community_id == &class, Error::<T>::InvalidTrack);

            let (vote, multiplied) = CommunityVotes::<T>::get(poll_index, voter_key)
                .ok_or(Error::<T>::NoVoteCasted)?;

            let vote_weight = VoteWeight::from(&vote);
            tally.remove_vote(vote.say(), multiplied, vote_weight);

            CommunityVotes::<T>::remove(poll_index, voter_key);
            Ok(())
        })
    }

    pub(crate) fn update_locks(
        who: &AccountIdOf<T>,
        poll_index: PollIndexOf<T>,
        vote: &VoteOf<T>,
        update_type: LockUpdateType,
    ) -> DispatchResult {
        use sp_runtime::traits::Zero;

        let reason = FreezeReason::VoteCasted.into();

        match vote.clone() {
            Vote::AssetBalance(..) | Vote::NativeBalance(..) => match update_type {
                LockUpdateType::Add => {
                    CommunityVoteLocks::<T>::insert(who, poll_index, vote.clone())
                }
                LockUpdateType::Remove => CommunityVoteLocks::<T>::remove(who, poll_index),
            },
            _ => (),
        }

        match (update_type, vote) {
            (LockUpdateType::Add, Vote::AssetBalance(_, asset_id, amount)) => {
                let amount =
                    T::AssetsFreezer::balance_frozen(asset_id.clone(), &reason, who).max(*amount);
                T::AssetsFreezer::set_frozen(asset_id.clone(), &reason, who, amount, Polite)?;
            }
            (LockUpdateType::Add, Vote::NativeBalance(_, amount)) => {
                let amount = T::Balances::balance_frozen(&reason, who).max(*amount);
                T::Balances::set_frozen(&reason, who, amount, Polite)?;
            }
            (LockUpdateType::Remove, Vote::AssetBalance(_, asset_id, _)) => {
                let mut amount_to_freeze: AssetBalanceOf<T> = Zero::zero();

                for locked_vote in CommunityVoteLocks::<T>::iter_prefix_values(who) {
                    if let Vote::AssetBalance(_, ref id, amount) = locked_vote {
                        if id == asset_id {
                            amount_to_freeze = amount_to_freeze.max(amount)
                        }
                    }
                }

                T::AssetsFreezer::set_frozen(
                    asset_id.clone(),
                    &reason,
                    who,
                    amount_to_freeze,
                    Polite,
                )?;
            }
            (LockUpdateType::Remove, Vote::NativeBalance(_, _)) => {
                let mut amount_to_freeze: NativeBalanceOf<T> = Zero::zero();

                for locked_vote in CommunityVoteLocks::<T>::iter_prefix_values(who) {
                    if let Vote::NativeBalance(_, amount) = locked_vote {
                        amount_to_freeze = amount_to_freeze.max(amount)
                    }
                }

                T::Balances::set_frozen(
                    &FreezeReason::VoteCasted.into(),
                    who,
                    amount_to_freeze,
                    Polite,
                )?;
            }
            _ => (),
        }

        Ok(())
    }

    pub(crate) fn do_dispatch_as_community_account(
        community_id: &CommunityIdOf<T>,
        call: RuntimeCallFor<T>,
    ) -> DispatchResult {
        let community_account = Self::community_account(community_id);
        let signer = frame_system::RawOrigin::Signed(community_account);
        call.dispatch(signer.into())
            .map(|_| ())
            .map_err(|e| e.error)
    }

    /// Check if community has enough budget for the given cost.
    /// Resets session if expired. Returns remaining capacity after deduction.
    pub fn check_budget(community_id: &CommunityIdOf<T>, cost: u64) -> Result<u64, Error<T>> {
        let mut budget = Budget::<T>::get(community_id).ok_or(Error::<T>::BudgetExhausted)?;
        let now = T::BlockNumberProvider::current_block_number();

        // Reset session if expired
        if now >= budget.session_start.saturating_add(budget.session_length) {
            budget.used = 0;
            budget.session_start = now;
        }

        let remaining = budget.capacity.saturating_sub(budget.used);
        if remaining < cost {
            return Err(Error::<T>::BudgetExhausted);
        }
        Ok(remaining.saturating_sub(cost))
    }

    /// Burn gas from community budget. Resets session if expired.
    pub fn burn_budget(community_id: &CommunityIdOf<T>, cost: u64) {
        Budget::<T>::mutate(community_id, |maybe_budget| {
            if let Some(budget) = maybe_budget {
                let now = T::BlockNumberProvider::current_block_number();
                if now >= budget.session_start.saturating_add(budget.session_length) {
                    budget.used = 0;
                    budget.session_start = now;
                }
                budget.used = budget.used.saturating_add(cost);
            }
        });
    }

    /// Refund gas back to community budget.
    pub fn refund_budget(community_id: &CommunityIdOf<T>, amount: u64) {
        Budget::<T>::mutate(community_id, |maybe_budget| {
            if let Some(budget) = maybe_budget {
                budget.used = budget.used.saturating_sub(amount);
            }
        });
    }

    /// Keep the stored merkle root consistent after a membership change.
    ///
    /// Behaviour by privacy level:
    /// - **Public**: rebuild the root from on-chain members (authoritative source).
    /// - **Hybrid**: clear the root. Hybrid communities mix on-chain and off-chain
    ///   members, so an on-chain change alone cannot produce a valid tree — the admin
    ///   must republish via `update_membership_root`. Clearing the root is the safe
    ///   default: in-flight anonymous proofs would otherwise remain valid against an
    ///   already-stale tree (e.g. a suspended member could continue to anonymously
    ///   vote until the admin bothered to update).
    /// - **Private**: clear the root for the same reason as Hybrid. Since all
    ///   membership is off-chain, we can't rebuild, but we can *refuse* anonymous
    ///   actions until the admin republishes.
    ///
    /// Clearing `MerkleRoot` causes the extension to reject proofs with
    /// `NO_MEMBERSHIP_ROOT`, which is the desired fail-closed behaviour.
    pub fn recompute_merkle_root(community_id: &CommunityIdOf<T>) {
        match Info::<T>::get(community_id).map(|i| i.privacy) {
            Some(PrivacyLevel::Public) => {
                let mut leaves: alloc::vec::Vec<<T::Hasher as sp_runtime::traits::Hash>::Output> =
                    Members::<T>::iter_prefix(community_id)
                        .filter(|(_, record)| record.status == MemberStatus::Active)
                        .map(|(who, record)| {
                            T::Hasher::hash_of(&(who, community_id, record.rank, record.nonce))
                        })
                        .collect();
                leaves.sort();
                if leaves.is_empty() {
                    MerkleRoot::<T>::remove(community_id);
                } else {
                    let root = binary_merkle_tree::merkle_root::<T::Hasher, _>(leaves);
                    MerkleRoot::<T>::insert(community_id, root);
                }
            }
            Some(PrivacyLevel::Private) | Some(PrivacyLevel::Hybrid) => {
                // Fail-closed: an on-chain suspend/remove may have invalidated the tree.
                // The admin must republish with `update_membership_root`.
                MerkleRoot::<T>::remove(community_id);
            }
            None => {}
        }
    }
}

impl<T: Config> Tally<T> {
    pub(self) fn add_vote(
        &mut self,
        say_aye: bool,
        multiplied_weight: VoteWeight,
        weight: VoteWeight,
    ) {
        if say_aye {
            self.ayes = self.ayes.saturating_add(multiplied_weight);
            self.bare_ayes = self.bare_ayes.saturating_add(weight);
        } else {
            self.nays = self.nays.saturating_add(multiplied_weight);
        }
    }

    pub(self) fn remove_vote(
        &mut self,
        say_aye: bool,
        multiplied_weight: VoteWeight,
        weight: VoteWeight,
    ) {
        if say_aye {
            self.ayes = self.ayes.saturating_sub(multiplied_weight);
            self.bare_ayes = self.bare_ayes.saturating_sub(weight);
        } else {
            self.nays = self.nays.saturating_sub(multiplied_weight);
        }
    }
}
