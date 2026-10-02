//! The contract registry's helpers: origins, kind rules, eligibility, the one-contract rule, and
//! the mapping of listings' refusals to this pallet's errors.

use super::*;

impl<T: Config> Pallet<T> {
    /// The current tick of the chain clock.
    pub(crate) fn now() -> MomentOf<T> {
        T::BlockNumberProvider::current_block_number()
    }

    /// The origin that publishes (and withdraws) offers of `kind` (`REQ-OF-1`).
    pub(crate) fn ensure_offer_origin(
        origin: OriginFor<T>,
        kind: &OfferKindOf<T>,
    ) -> DispatchResult {
        match kind {
            OfferKind::Custom(_) => T::CustomOfferOrigin::ensure_origin(origin).map(|_| ()),
            OfferKind::Standard | OfferKind::Trial => {
                T::StandardOfferOrigin::ensure_origin(origin).map(|_| ())
            }
        }
        .map_err(Into::into)
    }

    /// Whether `terms` keep the rules of an offer of `kind` (SPEC §5.1). Listings checks the rest
    /// when it takes the conditions: a billing period longer than its lead, and the price's asset
    /// and minimum.
    pub(crate) fn validate_terms(kind: &OfferKindOf<T>, terms: &TermsOf<T>) -> DispatchResult {
        let allowance = terms.allowance;
        let shaped = allowance.ref_time() > 0
            && allowance.proof_size() > 0
            && terms.usage_period >= T::MinUsagePeriod::get()
            && terms.billing_period >= T::MinBillingPeriod::get()
            && terms.conditions().is_well_formed();
        let priced = !terms.price.amount.is_zero();
        let kind_rules = match kind {
            OfferKind::Standard => terms.term.is_none() && terms.min_commitment.is_none() && priced,
            OfferKind::Trial => {
                terms
                    .term
                    .is_some_and(|term| (1..=T::MaxTrialPeriods::get()).contains(&term))
                    && terms.min_commitment.is_none()
            }
            OfferKind::Custom(_) => priced,
        };
        ensure!(shaped && kind_rules, Error::<T>::InvalidTerms);
        Ok(())
    }

    /// Whether `group` may accept an offer of `kind`: a custom offer only by its group.
    pub(crate) fn is_eligible(kind: &OfferKindOf<T>, group: &GroupOf<T>) -> bool {
        match kind {
            OfferKind::Custom(only) => only == group,
            OfferKind::Standard | OfferKind::Trial => true,
        }
    }

    /// Whether a trial of `group` may name `offer` as its conversion: an open offer, not a trial,
    /// that the group is eligible for (`REQ-CT-13`).
    pub(crate) fn is_valid_conversion(offer: &OfferIdOf<T>, group: &GroupOf<T>) -> bool {
        Offers::<T>::get(offer).is_some_and(|record| {
            record.status == OfferStatus::Open
                && record.kind != OfferKind::Trial
                && Self::is_eligible(&record.kind, group)
        })
    }

    /// Whether the contract holds an amended allowance that is not in force yet at `now`: its first
    /// usage window has not begun.
    pub(crate) fn allowance_pending(contract: &ContractOf<T>, now: MomentOf<T>) -> bool {
        contract.next_allowance.as_ref().is_some_and(|next| {
            contract
                .window_at(now)
                .is_none_or(|window| window < next.from_window)
        })
    }

    /// Moves an amended allowance whose first window has come into the contract's allowance.
    pub(crate) fn fold_next_allowance(contract: &mut ContractOf<T>, now: MomentOf<T>) {
        let Some(window) = contract.window_at(now) else {
            return;
        };
        if let Some(next) = contract.next_allowance.take() {
            if window >= next.from_window {
                contract.allowance = next.allowance;
            } else {
                contract.next_allowance = Some(next);
            }
        }
    }

    /// Refuses a group that has a contract in any state (`REQ-CT-1`, `INV-8`), except a
    /// contract whose end is already due and needs no charge, whether or not the due queue has
    /// reached it: a *Suspended* one past its grace end outside its commitment, or a *Defaulted*
    /// one past its commitment end. It is settled (ended, *lapsed*) first (`REQ-CT-12`).
    pub(crate) fn ensure_no_contract(
        group: &GroupOf<T>,
        account: &T::AccountId,
        now: MomentOf<T>,
    ) -> DispatchResult {
        if let Some(contract) = Contracts::<T>::get(group) {
            let inventory = T::OfferInventory::get();
            // Settling is transactional and `Ok` when nothing is due. A refusal changes nothing,
            // and the checks below decide.
            let _ = <T::Subscriptions as subs::Mutate<T::AccountId>>::settle(
                &inventory,
                &contract.offer,
                account,
            );
            ensure!(
                !<T::Subscriptions as subs::Inspect<T::AccountId>>::blocks_new_subscription(
                    &inventory,
                    &contract.offer,
                    account,
                    now,
                ),
                Error::<T>::AlreadyContracted
            );
            // Settling ends it, through `on_ended`. Anything left blocks.
            ensure!(
                !Contracts::<T>::contains_key(group),
                Error::<T>::AlreadyContracted
            );
        }
        // Another group with the same account would share its subscriptions.
        ensure!(
            GroupOfAccount::<T>::get(account).is_none(),
            Error::<T>::AlreadyContracted
        );
        Ok(())
    }

    /// Records the contract of `group` to `offer`, anchored at `now`, then subscribes its account
    /// through listings, which charges billing period 0 and calls `on_started`. A failed charge
    /// fails the call, and nothing is kept.
    pub(crate) fn start_contract(
        group: GroupOf<T>,
        account: &T::AccountId,
        offer: OfferIdOf<T>,
        record: &OfferOf<T>,
        now: MomentOf<T>,
    ) -> DispatchResult {
        let inventory = T::OfferInventory::get();
        let seq =
            <T::Subscriptions as subs::Inspect<T::AccountId>>::item_amendment(&inventory, &offer)
                .map_or(0, |amendment| amendment.seq);
        Contracts::<T>::insert(group, Contract::start(offer, record, now, seq));
        GroupOfAccount::<T>::insert(account, group);

        <T::Subscriptions as subs::Mutate<T::AccountId>>::subscribe(&inventory, &offer, account)
            .map_err(|e| Self::listings_error(e, Error::<T>::ChargeFailed))?;

        if let OfferKind::Custom(_) = record.kind {
            Self::mark_accepted(&offer);
        }
        Ok(())
    }

    /// A custom offer, once accepted, reads as withdrawn (`REQ-OF-4`, `AC-A2.2`).
    pub(crate) fn mark_accepted(offer: &OfferIdOf<T>) {
        Offers::<T>::mutate(offer, |maybe_offer| {
            if let Some(record) = maybe_offer {
                record.status = OfferStatus::Withdrawn;
            }
        });
        Self::deposit_event(Event::<T>::OfferWithdrawn { offer: *offer });
    }

    /// Removes the contract of `group` and its account's entry, and says why (`REQ-CT-12`).
    pub(crate) fn end_contract(group: &GroupOf<T>, account: &T::AccountId, reason: EndReason) {
        Contracts::<T>::remove(group);
        GroupOfAccount::<T>::remove(account);
        Self::deposit_event(Event::<T>::ContractEnded {
            group: *group,
            reason,
        });
    }

    /// The group and contract a subscription of the collective's inventory belongs to, if it is a
    /// contract's: the subscriber is a group's account, and the item is its contract's offer.
    pub(crate) fn contract_of(
        inventory: &(MerchantIdOf<T>, InventoryIdOf<T>),
        item: &OfferIdOf<T>,
        who: &T::AccountId,
    ) -> Option<(GroupOf<T>, ContractOf<T>)> {
        if *inventory != T::OfferInventory::get() {
            return None;
        }
        let group = GroupOfAccount::<T>::get(who)?;
        let contract = Contracts::<T>::get(group)?;
        (contract.offer == *item).then_some((group, contract))
    }

    /// Maps a refusal of [`Config::Subscriptions`] to this pallet's error of the same meaning
    /// (`CTR-CALL-2`), or to `fallback`. The subscriptions system says which refusal it is
    /// ([`subs::Inspect::subscription_error`]).
    pub(crate) fn listings_error(error: DispatchError, fallback: Error<T>) -> DispatchError {
        use subs::SubscriptionError as E;
        match <T::Subscriptions as subs::Inspect<T::AccountId>>::subscription_error(&error) {
            Some(E::InvalidConditions) => Error::<T>::InvalidTerms,
            Some(E::NotEligible) => Error::<T>::NotEligible,
            Some(E::NotSubscribable) => Error::<T>::OfferWithdrawn,
            Some(E::AlreadySubscribed) => Error::<T>::AlreadyContracted,
            Some(E::NoSubscription | E::CancelPending) => Error::<T>::NoContract,
            Some(E::ChargeFailed) => Error::<T>::ChargeFailed,
            Some(E::ReplacementPending) => Error::<T>::SwitchPending,
            Some(E::ChangePending) => Error::<T>::ChangePending,
            Some(E::NoPendingReplacement) => Error::<T>::NoPendingSwitch,
            _ => fallback,
        }
        .into()
    }
}
