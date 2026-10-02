//! The answers of the view functions (`CTR-QRY-1`, SPEC §8.3). Each reads one state and writes
//! nothing (`REQ-OB-1`).

use super::*;
use alloc::vec::Vec;
use sp_runtime::traits::Saturating;
use subs::Inspect as _;

#[cfg(test)]
thread_local! {
    /// How many times pages read `Offers`: the tests' measure of a page's cost. The test
    /// externalities count no reads, and a storage proof grows with the trie, not with the reads.
    pub(crate) static OFFER_KEYS_READ: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
}

impl<T: Config> Pallet<T> {
    /// The offer `offer`: its kind, its terms for new contracts, its status, and its last
    /// amendment while some contract may still have it pending.
    pub(crate) fn offer_info(offer: &OfferIdOf<T>) -> Option<OfferInfoOf<T>> {
        let record = Offers::<T>::get(offer)?;
        let conditions =
            T::Subscriptions::subscription_conditions(&T::OfferInventory::get(), offer)?;
        let now = Self::now();
        let amendment = OfferAmendment::<T>::get(offer).filter(|amendment| {
            let last_boundary = amendment.enacted_at.saturating_add(
                amendment
                    .terms
                    .billing_period
                    .saturating_mul(MomentOf::<T>::from(2u32)),
            );
            now < last_boundary
        });
        Some(OfferInfo {
            kind: record.kind,
            terms: Terms {
                allowance: record.allowance,
                usage_period: record.usage_period,
                price: conditions.price,
                billing_period: conditions.period,
                term: conditions.term,
                min_commitment: conditions.min_commitment,
                grace: conditions.grace,
            },
            status: record.status,
            amendment,
        })
    }

    /// A page of open offers after `start_after` (from the first one when `None`), in ascending
    /// id order. It examines at most `min(limit, MaxOffersPerPage)` ids, skipping withdrawn
    /// offers (and an offer whose listings conditions are missing), so its cost never depends on
    /// how many offers exist (0008-A1). A zero limit examines nothing, and returns `start_after`
    /// as the cursor.
    pub(crate) fn offers_page(start_after: Option<OfferIdOf<T>>, limit: u32) -> OffersPageOf<T> {
        let limit = limit.min(T::MaxOffersPerPage::get());
        if limit == 0 {
            return OffersPage {
                offers: Vec::new(),
                next: start_after,
            };
        }
        let mut cursor = match start_after {
            Some(after) => after.increment(),
            None => OfferIdOf::<T>::initial_value(),
        };
        let mut offers = Vec::new();
        let mut last = None;
        let mut examined = 0;
        while examined < limit {
            let Some(id) = cursor else { break };
            // Offer ids are contiguous, and offers are never removed: the first missing one is
            // the end.
            if !Self::offer_exists(&id) {
                return OffersPage { offers, next: None };
            }
            if let Some(info) = Self::offer_info(&id) {
                if info.status == OfferStatus::Open {
                    offers.push((id, info));
                }
            }
            examined += 1;
            last = Some(id);
            cursor = id.increment();
        }
        let more = cursor.is_some_and(|id| Self::offer_exists(&id));
        OffersPage {
            offers,
            next: last.filter(|_| more),
        }
    }

    /// Whether `offer` exists: the one read of `Offers` a page makes per id it examines (one more
    /// for the id after its last).
    fn offer_exists(offer: &OfferIdOf<T>) -> bool {
        #[cfg(test)]
        OFFER_KEYS_READ.with(|reads| reads.set(reads.get() + 1));
        Offers::<T>::contains_key(offer)
    }

    /// The contract of `group`, with its billing state from the subscriptions system.
    pub(crate) fn contract_info(group: &GroupOf<T>) -> Option<ContractInfoOf<T>> {
        let contract = Contracts::<T>::get(group)?;
        let inventory = T::OfferInventory::get();
        let account = T::GroupAccount::convert(*group);
        let subscription = T::Subscriptions::subscription(&inventory, &contract.offer, &account)?;
        let now = Self::now();
        let allowance = contract
            .window_at(now)
            .map_or(contract.allowance, |window| {
                Self::allowance_at(&contract, window)
            });
        let conditions = &subscription.conditions;
        let pending_amendment =
            T::Subscriptions::pending_amendment(&inventory, &contract.offer, &account, now).map(
                |pending| {
                    let amended_allowance = match contract.pending {
                        Some(PendingChange::Amend { allowance, .. }) => allowance,
                        _ => OfferAmendment::<T>::get(contract.offer)
                            .map_or(contract.allowance, |amendment| amendment.terms.allowance),
                    };
                    PendingAmendmentInfo {
                        terms: Terms {
                            allowance: amended_allowance,
                            usage_period: contract.usage_period,
                            price: pending.conditions.price,
                            billing_period: pending.conditions.period,
                            term: pending.conditions.term,
                            min_commitment: pending.conditions.min_commitment,
                            grace: pending.conditions.grace,
                        },
                        effective_at: pending.effective_at,
                        free_exit: true,
                    }
                },
            );

        Some(ContractInfo {
            offer: contract.offer,
            kind: contract.kind,
            state: subscription.state.clone(),
            terms: Terms {
                allowance,
                usage_period: contract.usage_period,
                price: conditions.price.clone(),
                billing_period: conditions.period,
                term: conditions.term,
                min_commitment: conditions.min_commitment,
                grace: conditions.grace,
            },
            anchor: contract.anchor,
            paid_through: subscription.paid_through,
            next_due: T::Subscriptions::next_due(&inventory, &contract.offer, &account),
            grace_end: T::Subscriptions::grace_end(&inventory, &contract.offer, &account),
            commitment_end: conditions
                .min_commitment
                .map(|_| subscription.commitment_end()),
            periods_charged: subscription.periods_charged,
            cancel_requested: subscription.cancel_requested.clone(),
            pending_switch: contract
                .pending
                .clone()
                .filter(|pending| !matches!(pending, PendingChange::Amend { .. })),
            pending_amendment,
        })
    }

    /// The pool of `group` in the current usage window, and whether it is usable now.
    pub(crate) fn pool_info(group: &GroupOf<T>) -> Option<PoolInfo<MomentOf<T>>> {
        let contract = Contracts::<T>::get(group)?;
        let now = Self::now();
        let window_start = contract.window_at(now)?;
        let allowance = Self::allowance_at(&contract, window_start);
        let used = contract.used_in(window_start);
        let paid = T::Subscriptions::is_paid(
            &T::OfferInventory::get(),
            &contract.offer,
            &T::GroupAccount::convert(*group),
            now,
        );
        let usable = if !T::UsableGroup::contains(group) {
            Err(FeePathReason::GroupUnusable)
        } else if !paid {
            Err(FeePathReason::NotPaid)
        } else {
            Ok(())
        };
        Some(PoolInfo {
            allowance,
            window_start,
            window_end: window_start.saturating_add(contract.usage_period),
            used,
            remaining: allowance.saturating_sub(used),
            usable,
        })
    }

    /// The admission decision for `account` and `estimate`, with its reason.
    pub(crate) fn waiver(account: &T::AccountId, estimate: Weight) -> Waiver<GroupOf<T>> {
        match Self::check(account, estimate) {
            Ok(ticket) => {
                let remaining = Contracts::<T>::get(ticket.group)
                    .map(|contract| {
                        Self::allowance_at(&contract, ticket.window_start)
                            .saturating_sub(contract.used_in(ticket.window_start))
                            .saturating_sub(estimate)
                    })
                    .unwrap_or_default();
                Waiver::Pool {
                    group: ticket.group,
                    remaining,
                }
            }
            Err(reason) => Waiver::Fee(reason),
        }
    }
}
