//! The contract registry follows its contracts' subscriptions: [`OnSubscriptionChanged`] for
//! [`Pallet`].
//!
//! Each hook acts only on a subscription of [`Config::OfferInventory`] whose subscriber is the
//! account of a group with a contract to that item; every other subscription is left alone.

use super::*;
use subs::{EndReason as SubscriptionEnd, ReplacementDropReason};

impl<T: Config>
    OnSubscriptionChanged<
        (MerchantIdOf<T>, InventoryIdOf<T>),
        OfferIdOf<T>,
        T::AccountId,
        MomentOf<T>,
    > for Pallet<T>
{
    /// The heaviest hooks one subscription can trigger together, in one call or one processed
    /// entry of the due queue: an offer's amendment coming into force, then a switch or
    /// conversion allowed and taking effect. Every other combination does less: a refused or
    /// failed replacement followed by a suspension, a default and an end reads only what these
    /// read, and writes or removes only the contract and the account entry, which `on_replaced`
    /// rewrites too.
    fn max_hook_weight() -> Weight {
        T::WeightInfo::hook_amendment_in_force().saturating_add(T::WeightInfo::hook_replaced())
    }

    /// The contract recorded by `subscribe` started: billing period 0 is paid.
    fn on_started(
        inventory: &(MerchantIdOf<T>, InventoryIdOf<T>),
        item: &OfferIdOf<T>,
        who: &T::AccountId,
    ) {
        if let Some((group, _)) = Self::contract_of(inventory, item, who) {
            Self::deposit_event(Event::<T>::ContractStarted {
                group,
                offer: *item,
            });
        }
    }

    fn on_charged(
        inventory: &(MerchantIdOf<T>, InventoryIdOf<T>),
        item: &OfferIdOf<T>,
        who: &T::AccountId,
        period: u32,
        paid_through: MomentOf<T>,
    ) {
        if let Some((group, _)) = Self::contract_of(inventory, item, who) {
            Self::deposit_event(Event::<T>::ContractCharged {
                group,
                period,
                paid_through,
            });
        }
    }

    fn on_suspended(
        inventory: &(MerchantIdOf<T>, InventoryIdOf<T>),
        item: &OfferIdOf<T>,
        who: &T::AccountId,
        grace_end: MomentOf<T>,
    ) {
        if let Some((group, _)) = Self::contract_of(inventory, item, who) {
            Self::deposit_event(Event::<T>::ContractSuspended { group, grace_end });
        }
    }

    fn on_restored(
        inventory: &(MerchantIdOf<T>, InventoryIdOf<T>),
        item: &OfferIdOf<T>,
        who: &T::AccountId,
    ) {
        if let Some((group, _)) = Self::contract_of(inventory, item, who) {
            Self::deposit_event(Event::<T>::ContractRestored { group });
        }
    }

    /// The contract is kept, blocking the group, until its commitment end (`REQ-CT-11`).
    fn on_defaulted(
        inventory: &(MerchantIdOf<T>, InventoryIdOf<T>),
        item: &OfferIdOf<T>,
        who: &T::AccountId,
        until: MomentOf<T>,
    ) {
        if let Some((group, _)) = Self::contract_of(inventory, item, who) {
            Self::deposit_event(Event::<T>::ContractDefaulted { group, until });
        }
    }

    /// The contract's record is removed in the same block, with the reason (`REQ-CT-12`).
    fn on_ended(
        inventory: &(MerchantIdOf<T>, InventoryIdOf<T>),
        item: &OfferIdOf<T>,
        who: &T::AccountId,
        reason: SubscriptionEnd,
    ) {
        if let Some((group, _)) = Self::contract_of(inventory, item, who) {
            let reason = match reason {
                SubscriptionEnd::Completed => EndReason::Completed,
                SubscriptionEnd::Cancelled => EndReason::Cancelled,
                SubscriptionEnd::Lapsed => EndReason::Lapsed,
                SubscriptionEnd::Replaced => EndReason::Switched,
                SubscriptionEnd::Terminated => EndReason::Terminated,
            };
            Self::end_contract(&group, who, reason);
        }
    }

    /// An amendment of a custom contract was enacted (`REQ-CT-8`). The pallet records it when
    /// the amendment returns.
    fn on_amendment_enacted(
        inventory: &(MerchantIdOf<T>, InventoryIdOf<T>),
        item: &OfferIdOf<T>,
        who: &T::AccountId,
        effective_at: MomentOf<T>,
    ) {
        if let Some((group, _)) = Self::contract_of(inventory, item, who) {
            Self::deposit_event(Event::<T>::ContractAmended {
                group,
                effective_at,
            });
        }
    }

    /// An amendment came into force at the contract's effective boundary: the amended allowance
    /// applies from the first usage window starting at or after it (`REQ-CT-8`, `INV-20`). For a
    /// custom contract it comes from its `pending` slot; for a standard one, from its offer's
    /// amendment, which it has now applied.
    fn on_amendment_in_force(
        inventory: &(MerchantIdOf<T>, InventoryIdOf<T>),
        item: &OfferIdOf<T>,
        who: &T::AccountId,
        effective_at: MomentOf<T>,
    ) {
        let Some((group, mut contract)) = Self::contract_of(inventory, item, who) else {
            return;
        };
        let now = Self::now();
        Self::fold_next_allowance(&mut contract, now);

        let amended = match contract.pending {
            Some(PendingChange::Amend {
                allowance,
                from_window,
                ..
            }) => {
                contract.pending = None;
                Some((allowance, from_window))
            }
            _ => OfferAmendment::<T>::get(contract.offer)
                .filter(|amendment| amendment.seq > contract.amendment_seq)
                .map(|amendment| {
                    contract.amendment_seq = amendment.seq;
                    (
                        amendment.terms.allowance,
                        first_window_from(contract.anchor, contract.usage_period, effective_at),
                    )
                }),
        };
        if let Some((allowance, from_window)) = amended {
            contract.next_allowance = Some(NextAllowance {
                allowance,
                from_window,
            });
            Self::fold_next_allowance(&mut contract, now);
        }
        Contracts::<T>::insert(group, contract);
        Self::deposit_event(Event::<T>::ContractAmendmentInForce {
            group,
            effective_at,
        });
    }

    /// A switch or conversion took effect: the old contract ended (*switched* or *converted*),
    /// and a contract to `new_item` started at `new_anchor` with its period 0 charged, copying the
    /// new offer's terms as they stand now (`REQ-CT-7`, `REQ-CT-13`, `INV-9`).
    fn on_replaced(
        inventory: &(MerchantIdOf<T>, InventoryIdOf<T>),
        item: &OfferIdOf<T>,
        who: &T::AccountId,
        new_item: &OfferIdOf<T>,
        new_anchor: MomentOf<T>,
    ) {
        let Some((group, contract)) = Self::contract_of(inventory, item, who) else {
            return;
        };
        let reason = match contract.pending {
            Some(PendingChange::Convert(_)) => EndReason::Converted,
            _ => EndReason::Switched,
        };
        Self::end_contract(&group, who, reason);

        // `allow_replacement` only lets a known offer take effect.
        let Some(record) = Offers::<T>::get(new_item) else {
            return;
        };
        let seq =
            <T::Subscriptions as subs::Inspect<T::AccountId>>::item_amendment(inventory, new_item)
                .map_or(0, |amendment| amendment.seq);
        Contracts::<T>::insert(group, Contract::start(*new_item, &record, new_anchor, seq));
        GroupOfAccount::<T>::insert(who, group);
        Self::deposit_event(Event::<T>::ContractStarted {
            group,
            offer: *new_item,
        });
        match record.kind {
            OfferKind::Custom(_) => Self::mark_accepted(new_item),
            OfferKind::Trial => TrialUsed::<T>::insert(group, ()),
            OfferKind::Standard => {}
        }
    }

    /// A switch or conversion takes effect only into an open offer the group is still eligible
    /// for, while the group is still usable (`REQ-CT-13`). The refusal is the reason it is
    /// dropped.
    fn allow_replacement(
        inventory: &(MerchantIdOf<T>, InventoryIdOf<T>),
        item: &OfferIdOf<T>,
        who: &T::AccountId,
        new_item: &OfferIdOf<T>,
    ) -> Result<(), DispatchError> {
        let Some((group, _)) = Self::contract_of(inventory, item, who) else {
            return Ok(());
        };
        let record = Offers::<T>::get(new_item).ok_or(Error::<T>::UnknownOffer)?;
        ensure!(
            record.status == OfferStatus::Open,
            Error::<T>::OfferWithdrawn
        );
        ensure!(T::UsableGroup::contains(&group), Error::<T>::GroupUnusable);
        ensure!(
            Self::is_eligible(&record.kind, &group),
            Error::<T>::NotEligible
        );
        ensure!(
            record.kind != OfferKind::Trial || !TrialUsed::<T>::contains_key(group),
            Error::<T>::TrialUsed
        );
        Ok(())
    }

    /// The pending switch or conversion is cleared: cancelled by the group, or dropped with the
    /// reason (`AC-B8.2`, `AC-B8.3`).
    fn on_replacement_dropped(
        inventory: &(MerchantIdOf<T>, InventoryIdOf<T>),
        item: &OfferIdOf<T>,
        who: &T::AccountId,
        new_item: &OfferIdOf<T>,
        reason: ReplacementDropReason,
    ) {
        let Some((group, mut contract)) = Self::contract_of(inventory, item, who) else {
            return;
        };
        let conversion = matches!(contract.pending, Some(PendingChange::Convert(_)));
        if matches!(
            contract.pending,
            Some(PendingChange::Switch(_) | PendingChange::Convert(_))
        ) {
            contract.pending = None;
            Contracts::<T>::insert(group, contract);
        }
        let offer = *new_item;
        Self::deposit_event(match (reason, conversion) {
            (ReplacementDropReason::ReplacementCancelled, false) => {
                Event::<T>::SwitchCancelled { group, offer }
            }
            (ReplacementDropReason::ReplacementCancelled, true) => {
                Event::<T>::ConversionCancelled { group, offer }
            }
            (reason, false) => Event::<T>::SwitchDropped {
                group,
                offer,
                reason,
            },
            (reason, true) => Event::<T>::ConversionDropped {
                group,
                offer,
                reason,
            },
        });
    }
}
