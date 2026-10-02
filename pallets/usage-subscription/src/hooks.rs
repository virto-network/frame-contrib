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

    /// A switch took effect: the old contract ended (*switched*), and a contract to `new_item`
    /// started at `new_anchor` with its period 0 charged, copying the new offer's terms as they
    /// stand now (`REQ-CT-7`, `INV-9`).
    fn on_replaced(
        inventory: &(MerchantIdOf<T>, InventoryIdOf<T>),
        item: &OfferIdOf<T>,
        who: &T::AccountId,
        new_item: &OfferIdOf<T>,
        new_anchor: MomentOf<T>,
    ) {
        let Some((group, _)) = Self::contract_of(inventory, item, who) else {
            return;
        };
        Self::end_contract(&group, who, EndReason::Switched);

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
        if let OfferKind::Custom(_) = record.kind {
            Self::mark_accepted(new_item);
        }
    }

    /// A switch takes effect only into an open offer the group is still eligible for, while the
    /// group is still usable.
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
        Ok(())
    }

    /// The pending switch is cleared: cancelled by the group, or dropped with the reason.
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
        if matches!(contract.pending, Some(PendingChange::Switch(_))) {
            contract.pending = None;
            Contracts::<T>::insert(group, contract);
        }
        let offer = *new_item;
        Self::deposit_event(match reason {
            ReplacementDropReason::ReplacementCancelled => {
                Event::<T>::SwitchCancelled { group, offer }
            }
            reason => Event::<T>::SwitchDropped {
                group,
                offer,
                reason,
            },
        });
    }
}
