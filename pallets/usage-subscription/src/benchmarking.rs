//! Benchmarks for `fc-pallet-usage-subscription`: every call, and the payment step's two paths
//! (PLAN §7).

use super::*;
use crate::Pallet as UsageSubscription;
use alloc::vec;
use frame_benchmarking::v2::*;
use frame_support::dispatch::{DispatchClass, DispatchInfo, Pays, PostDispatchInfo};
use frame_system::RawOrigin;
use sp_runtime::traits::{
    AsSystemOriginSigner, AsTransactionAuthorizedOrigin, DispatchTransaction, Dispatchable,
    Saturating, TransactionExtension,
};

/// Sets up groups, members and prices for the benchmarks.
pub trait BenchmarkHelper<T: Config> {
    /// A usable group, whose account can pay many billing periods at [`Self::price`].
    fn group() -> GroupOf<T>;

    /// The administrative origin of `group`.
    fn group_origin(group: &GroupOf<T>) -> T::RuntimeOrigin;

    /// Issues a new membership of `group`, and assigns it to `who`.
    fn add_member(group: &GroupOf<T>, who: &T::AccountId) -> MembershipOf<T>;

    /// A valid, non-zero price for an offer.
    fn price() -> PriceOf<T>;

    /// A group other than [`Self::group`], of which the benchmarks' members hold no membership:
    /// the stale paying-group name of the payment step's worst case.
    fn stale_group() -> GroupOf<T>;

    /// How many subscriptions started at one tick fill every due-queue bucket a new one probes,
    /// so the next one goes to its bucket's overflow: for `fc-pallet-listings`,
    /// `MAX_QUEUE_PROBES · MaxDuePerBucket`.
    fn due_queue_capacity() -> u32;
}

/// The allowance of the offers in the benchmarks: far more than one transaction needs.
fn allowance() -> Weight {
    Weight::from_parts(1_000_000_000_000_000, 1_000_000_000)
}

fn terms<T: Config>(term: Option<u32>, min_commitment: Option<u32>) -> TermsOf<T> {
    let billing_period = T::MinBillingPeriod::get().saturating_mul(30u32.into());
    Terms {
        allowance: allowance(),
        usage_period: T::MinUsagePeriod::get(),
        price: T::BenchmarkHelper::price(),
        billing_period,
        term,
        min_commitment,
        grace: Zero::zero(),
    }
}

fn origin_for<T: Config>(kind: &OfferKindOf<T>) -> Result<T::RuntimeOrigin, BenchmarkError> {
    match kind {
        OfferKind::Custom(_) => T::CustomOfferOrigin::try_successful_origin(),
        _ => T::StandardOfferOrigin::try_successful_origin(),
    }
    .map_err(|_| BenchmarkError::Weightless)
}

fn publish<T: Config>(
    kind: OfferKindOf<T>,
    terms: TermsOf<T>,
) -> Result<OfferIdOf<T>, BenchmarkError> {
    let offer = NextOfferId::<T>::get()
        .or_else(OfferIdOf::<T>::initial_value)
        .ok_or(BenchmarkError::Stop("no offer id"))?;
    UsageSubscription::<T>::publish_offer(origin_for::<T>(&kind)?, kind, terms)?;
    Ok(offer)
}

fn subscribe_group<T: Config>(
    group: &GroupOf<T>,
    offer: OfferIdOf<T>,
    converts_into: Option<OfferIdOf<T>>,
) -> Result<(), BenchmarkError> {
    UsageSubscription::<T>::subscribe(
        T::BenchmarkHelper::group_origin(group),
        offer,
        converts_into,
    )?;
    Ok(())
}

fn assert_has_event<T: Config>(event: Event<T>) {
    frame_system::Pallet::<T>::assert_has_event(<T as frame_system::Config>::RuntimeEvent::from(
        event,
    ));
}

/// Moves the chain clock to `tick`.
fn set_clock<T: Config>(tick: MomentOf<T>) {
    T::BlockNumberProvider::set_block_number(tick);
}

fn amend_origin<T: Config>() -> Result<T::RuntimeOrigin, BenchmarkError> {
    T::AmendOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)
}

/// The listings subscription behind `group`'s contract to `offer`.
fn subscription_of<T: Config>(
    group: &GroupOf<T>,
    offer: &OfferIdOf<T>,
) -> Result<subs::SubscriptionOf<T::Subscriptions, T::AccountId>, BenchmarkError> {
    <T::Subscriptions as subs::Inspect<T::AccountId>>::subscription(
        &T::OfferInventory::get(),
        offer,
        &T::GroupAccount::convert(*group),
    )
    .ok_or(BenchmarkError::Stop("no subscription"))
}

/// Gives `group` a contract to a new standard offer, *Suspended* and past its grace end: the
/// offer's amendment raises its price beyond what the group's account holds, so the charge at
/// the contract's effective boundary fails. The next `subscribe` settles (ends) it first.
fn suspended_past_grace<T: Config>(group: &GroupOf<T>) -> Result<(), BenchmarkError> {
    let inventory = T::OfferInventory::get();
    let account = T::GroupAccount::convert(*group);
    let mut old_terms = terms::<T>(None, None);
    old_terms.grace = old_terms.billing_period / 10u32.into();
    let offer = publish::<T>(OfferKind::Standard, old_terms.clone())?;
    subscribe_group::<T>(group, offer, None)?;

    let mut unaffordable = old_terms;
    unaffordable.price.amount = unaffordable
        .price
        .amount
        .saturating_mul(1_000_000_000u32.into());
    UsageSubscription::<T>::amend_offer(amend_origin::<T>()?, offer, unaffordable)?;
    let boundary = <T::Subscriptions as subs::Inspect<T::AccountId>>::pending_amendment(
        &inventory,
        &offer,
        &account,
        UsageSubscription::<T>::now(),
    )
    .map(|pending| pending.effective_at)
    .ok_or(BenchmarkError::Stop("no amendment pending"))?;
    // Every charge before the boundary is paid.
    loop {
        let due = subscription_of::<T>(group, &offer)?.paid_through;
        set_clock::<T>(due);
        <T::Subscriptions as subs::Mutate<T::AccountId>>::charge_due(&inventory, &offer, &account)?;
        if due >= boundary {
            break;
        }
    }
    if !matches!(
        subscription_of::<T>(group, &offer)?.state,
        subs::SubscriptionState::Suspended { .. }
    ) {
        return Err(BenchmarkError::Stop("the contract was not suspended"));
    }
    let grace_end =
        <T::Subscriptions as subs::Inspect<T::AccountId>>::grace_end(&inventory, &offer, &account)
            .ok_or(BenchmarkError::Stop("no grace end"))?;
    set_clock::<T>(grace_end);
    Ok(())
}

/// Fills every due-queue bucket a subscription to `offer` started now would probe, so the next
/// one goes to its bucket's overflow: as many subscriptions to `offer` of other accounts as
/// [`BenchmarkHelper::due_queue_capacity`] says. `offer` must be free, so they need no funds.
fn fill_due_queue<T: Config>(offer: &OfferIdOf<T>) -> Result<(), BenchmarkError> {
    let inventory = T::OfferInventory::get();
    for i in 0..T::BenchmarkHelper::due_queue_capacity() {
        let who: T::AccountId = account("filler", i, 0);
        <T::Subscriptions as subs::Mutate<T::AccountId>>::subscribe(&inventory, offer, &who)?;
    }
    Ok(())
}

fn assert_last_event<T: Config>(event: Event<T>) {
    frame_system::Pallet::<T>::assert_last_event(<T as frame_system::Config>::RuntimeEvent::from(
        event,
    ));
}

/// A member, its group, and a transaction of it: the call, its dispatch info and length.
type Member<T> = (
    <T as frame_system::Config>::AccountId,
    GroupOf<T>,
    <T as frame_system::Config>::RuntimeCall,
    DispatchInfo,
    usize,
);

/// The payment step's worst case (PLAN §7): a member whose named paying group is stale (so
/// admission reads the name, looks for a membership of it, and then scans) and who holds
/// `MaxMembershipScan` memberships of one usable group, an *Active* standard contract whose offer
/// has an amendment (so admission reads it), and a pool at its limit minus the estimate (or, when
/// `exhausted`, at its limit).
fn setup_member<T>(exhausted: bool) -> Result<Member<T>, BenchmarkError>
where
    T: Config + Send + Sync,
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    <T::RuntimeCall as Dispatchable>::RuntimeOrigin: AsSystemOriginSigner<T::AccountId>,
{
    let group = T::BenchmarkHelper::group();
    let offer = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
    subscribe_group::<T>(&group, offer, None)?;
    UsageSubscription::<T>::amend_offer(
        T::AmendOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?,
        offer,
        terms::<T>(None, None),
    )?;

    let who: T::AccountId = account("member", 0, 0);
    for _ in 0..T::MaxMembershipScan::get() {
        T::BenchmarkHelper::add_member(&group, &who);
    }
    // A name the member no longer holds a membership of: read, looked up, and ignored.
    let stale = T::BenchmarkHelper::stale_group();
    if stale == group || UsageSubscription::<T>::holds_valid_membership(&who, &stale) {
        return Err(BenchmarkError::Stop("the stale group is held"));
    }
    PayingGroup::<T>::insert(
        &who,
        PayingGroupChoice {
            group: Some(stale),
            window: Zero::zero(),
            changes: 0,
        },
    );

    let call: T::RuntimeCall = frame_system::Call::<T>::remark { remark: vec![] }.into();
    let ext = ChargeUsageSubscription::<T, ()>::new(());
    let info = DispatchInfo {
        call_weight: Weight::from_parts(100, 0),
        extension_weight: ext.weight(&call),
        class: DispatchClass::Normal,
        pays_fee: Pays::Yes,
    };
    let len = 10;
    let estimate = ChargeUsageSubscription::<T, ()>::estimate(&info, len);

    let now = UsageSubscription::<T>::now();
    Contracts::<T>::try_mutate(group, |maybe_contract| {
        let contract = maybe_contract.as_mut().ok_or(())?;
        contract.window_start = contract.window_at(now).ok_or(())?;
        contract.used = if exhausted {
            allowance()
        } else {
            allowance().saturating_sub(estimate)
        };
        Ok::<_, ()>(())
    })
    .map_err(|_| BenchmarkError::Stop("no contract"))?;

    Ok((who, group, call, info, len))
}

#[benchmarks(where
    T: Config + Send + Sync,
    T::RuntimeOrigin: AsTransactionAuthorizedOrigin,
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    <T::RuntimeCall as Dispatchable>::RuntimeOrigin: AsSystemOriginSigner<T::AccountId>,
)]
mod benchmarks {
    use super::*;

    /// A custom offer, the first: the inventory is created, and the item made exclusive.
    #[benchmark]
    fn publish_offer() -> Result<(), BenchmarkError> {
        let group = T::BenchmarkHelper::group();
        let kind = OfferKind::Custom(group);
        let origin = origin_for::<T>(&kind)?;
        let terms = terms::<T>(Some(12), Some(6));

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, kind, terms);

        assert_eq!(Offers::<T>::iter_keys().count(), 1);
        Ok(())
    }

    #[benchmark]
    fn withdraw_offer() -> Result<(), BenchmarkError> {
        let offer = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
        let origin = origin_for::<T>(&OfferKind::Standard)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, offer);

        assert_last_event::<T>(Event::OfferWithdrawn { offer });
        Ok(())
    }

    /// The worst case: the group's old contract, *Suspended* past its grace end, is settled
    /// (ended, *lapsed*) first; the new contract's due-queue entry finds every bucket it probes
    /// full, and goes to its bucket's overflow; and the offer is a trial that names its
    /// conversion, so the replacement is scheduled too.
    #[benchmark]
    fn subscribe() -> Result<(), BenchmarkError> {
        let group = T::BenchmarkHelper::group();
        suspended_past_grace::<T>(&group)?;
        let target = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
        let offer = publish::<T>(OfferKind::Trial, terms::<T>(Some(1), None))?;
        // A free trial of the same shape: its subscriptions fall in the same buckets.
        let mut free = terms::<T>(Some(1), None);
        free.price.amount = Zero::zero();
        let filler = publish::<T>(OfferKind::Trial, free)?;
        fill_due_queue::<T>(&filler)?;
        let origin = T::BenchmarkHelper::group_origin(&group);

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, offer, Some(target));

        assert_has_event::<T>(Event::ContractEnded {
            group,
            reason: EndReason::Lapsed,
        });
        assert_last_event::<T>(Event::ConversionScheduled {
            group,
            offer: target,
        });
        Ok(())
    }

    /// The heaviest cancellation: at its due tick, before the due queue reaches it, a contract
    /// outside any commitment with a switch pending ends at once. Listings drops the switch and
    /// ends the subscription, running both hooks. (A *Suspended* one outside its commitment takes
    /// the same path, but can only have a replacement pending as a trial's conversion that waits
    /// for its term limit, suspended by a charge it could pay at first and not later.)
    #[benchmark]
    fn cancel() -> Result<(), BenchmarkError> {
        let group = T::BenchmarkHelper::group();
        let offer = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
        let target = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
        subscribe_group::<T>(&group, offer, None)?;
        UsageSubscription::<T>::switch_offer(T::BenchmarkHelper::group_origin(&group), target)?;
        set_clock::<T>(subscription_of::<T>(&group, &offer)?.paid_through);
        let origin = T::BenchmarkHelper::group_origin(&group);

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin);

        assert!(!Contracts::<T>::contains_key(group));
        assert_has_event::<T>(Event::SwitchDropped {
            group,
            offer: target,
            reason: subs::ReplacementDropReason::SubscriptionCancelled,
        });
        assert_has_event::<T>(Event::ContractEnded {
            group,
            reason: EndReason::Cancelled,
        });
        Ok(())
    }

    #[benchmark]
    fn switch_offer() -> Result<(), BenchmarkError> {
        let group = T::BenchmarkHelper::group();
        let offer = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
        let target = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
        subscribe_group::<T>(&group, offer, None)?;
        let origin = T::BenchmarkHelper::group_origin(&group);

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, target);

        assert_last_event::<T>(Event::SwitchScheduled {
            group,
            offer: target,
        });
        Ok(())
    }

    /// A contract with a switch pending: the larger record, ended with it.
    #[benchmark]
    fn terminate_contract() -> Result<(), BenchmarkError> {
        let group = T::BenchmarkHelper::group();
        let offer = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
        let target = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
        subscribe_group::<T>(&group, offer, None)?;
        UsageSubscription::<T>::switch_offer(T::BenchmarkHelper::group_origin(&group), target)?;
        let origin =
            T::TerminateOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, group);

        assert!(!Contracts::<T>::contains_key(group));
        Ok(())
    }

    /// A member whose earlier choice is in the current rate window.
    #[benchmark]
    fn set_paying_group() -> Result<(), BenchmarkError> {
        let group = T::BenchmarkHelper::group();
        let who: T::AccountId = whitelisted_caller();
        T::BenchmarkHelper::add_member(&group, &who);
        UsageSubscription::<T>::set_paying_group(
            RawOrigin::Signed(who.clone()).into(),
            Some(group),
        )?;

        #[extrinsic_call]
        _(RawOrigin::Signed(who.clone()), Some(group));

        assert_last_event::<T>(Event::PayingGroupSet {
            who,
            group: Some(group),
        });
        Ok(())
    }

    /// A custom contract with a switch pending, which the amendment drops.
    #[benchmark]
    fn amend_contract() -> Result<(), BenchmarkError> {
        let group = T::BenchmarkHelper::group();
        let offer = publish::<T>(OfferKind::Custom(group), terms::<T>(None, Some(6)))?;
        let target = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
        subscribe_group::<T>(&group, offer, None)?;
        UsageSubscription::<T>::switch_offer(T::BenchmarkHelper::group_origin(&group), target)?;
        let origin =
            T::AmendOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, group, terms::<T>(None, Some(3)));

        assert!(matches!(
            Contracts::<T>::get(group).and_then(|contract| contract.pending),
            Some(PendingChange::Amend { .. })
        ));
        Ok(())
    }

    #[benchmark]
    fn cancel_switch() -> Result<(), BenchmarkError> {
        let group = T::BenchmarkHelper::group();
        let offer = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
        let target = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
        subscribe_group::<T>(&group, offer, None)?;
        UsageSubscription::<T>::switch_offer(T::BenchmarkHelper::group_origin(&group), target)?;
        let origin = T::BenchmarkHelper::group_origin(&group);

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin);

        assert_eq!(
            Contracts::<T>::get(group).and_then(|contract| contract.pending),
            None
        );
        Ok(())
    }

    #[benchmark]
    fn amend_offer() -> Result<(), BenchmarkError> {
        let offer = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
        let origin =
            T::AmendOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, offer, terms::<T>(None, None));

        assert!(OfferAmendment::<T>::contains_key(offer));
        Ok(())
    }

    /// The pool path end to end: the check in validation, again in preparation, and the charge
    /// after dispatch, at the worst case of `setup_member`.
    #[benchmark]
    fn pool_path() -> Result<(), BenchmarkError> {
        let (who, group, call, info, len) = setup_member::<T>(false)?;
        let ext = ChargeUsageSubscription::<T, ()>::new(());
        let post_info = PostDispatchInfo {
            actual_weight: None,
            pays_fee: Pays::Yes,
        };

        let result;
        #[block]
        {
            result = ext.test_run(
                RawOrigin::Signed(who.clone()).into(),
                &call,
                &info,
                len,
                0,
                |_| Ok(post_info),
            );
        }

        result
            .map_err(|_| BenchmarkError::Stop("the transaction was invalid"))?
            .map_err(|_| BenchmarkError::Stop("the transaction failed"))?;
        // The pool paid, and is now at its limit.
        assert_eq!(
            Contracts::<T>::get(group).map(|contract| contract.used),
            Some(allowance())
        );
        Ok(())
    }

    /// The check of the fee path: it reads everything, and fails at its last condition. It
    /// measures `Pallet::check` alone: the extension adds the estimate and wraps the fee path's
    /// value, which costs next to nothing beside it.
    #[benchmark]
    fn fee_path_check() -> Result<(), BenchmarkError> {
        let (who, _, _, info, len) = setup_member::<T>(true)?;
        let estimate = ChargeUsageSubscription::<T, ()>::estimate(&info, len);

        let result;
        #[block]
        {
            result = UsageSubscription::<T>::check(&who, estimate);
        }

        assert_eq!(result.err(), Some(FeePathReason::AllowanceExceeded));
        Ok(())
    }

    /// The heaviest hook path, and one half of `max_hook_weight`: a trial's conversion into a
    /// custom offer, allowed and then taking effect. It reads the group's account entry, the
    /// contract, the new offer, whether the group is usable, its eligibility and the new offer's
    /// item amendment, and writes the contract, the account entry and the accepted custom offer.
    #[benchmark]
    fn hook_replaced() -> Result<(), BenchmarkError> {
        let group = T::BenchmarkHelper::group();
        let custom = publish::<T>(OfferKind::Custom(group), terms::<T>(None, Some(6)))?;
        let trial = publish::<T>(OfferKind::Trial, terms::<T>(Some(1), None))?;
        subscribe_group::<T>(&group, trial, Some(custom))?;
        let inventory = T::OfferInventory::get();
        let who = T::GroupAccount::convert(group);
        let anchor = subscription_of::<T>(&group, &trial)?.paid_through;

        let allowed;
        #[block]
        {
            allowed =
                <UsageSubscription<T> as OnSubscriptionChanged<_, _, _, _>>::allow_replacement(
                    &inventory, &trial, &who, &custom,
                );
            <UsageSubscription<T> as OnSubscriptionChanged<_, _, _, _>>::on_replaced(
                &inventory, &trial, &who, &custom, anchor,
            );
        }

        allowed?;
        assert_eq!(
            Contracts::<T>::get(group).map(|contract| contract.offer),
            Some(custom)
        );
        assert_last_event::<T>(Event::OfferWithdrawn { offer: custom });
        Ok(())
    }

    /// The other half of `max_hook_weight`: an offer's amendment coming into force for a standard
    /// contract, which can come before a replacement for the same subscription. It reads the
    /// offer's amendment besides the contract, and writes the contract.
    #[benchmark]
    fn hook_amendment_in_force() -> Result<(), BenchmarkError> {
        let group = T::BenchmarkHelper::group();
        let offer = publish::<T>(OfferKind::Standard, terms::<T>(None, None))?;
        subscribe_group::<T>(&group, offer, None)?;
        let mut amended = terms::<T>(None, None);
        amended.allowance = allowance().saturating_mul(2);
        UsageSubscription::<T>::amend_offer(amend_origin::<T>()?, offer, amended)?;
        let inventory = T::OfferInventory::get();
        let who = T::GroupAccount::convert(group);
        let effective_at = <T::Subscriptions as subs::Inspect<T::AccountId>>::pending_amendment(
            &inventory,
            &offer,
            &who,
            UsageSubscription::<T>::now(),
        )
        .map(|pending| pending.effective_at)
        .ok_or(BenchmarkError::Stop("no amendment pending"))?;
        set_clock::<T>(effective_at);

        #[block]
        {
            <UsageSubscription<T> as OnSubscriptionChanged<_, _, _, _>>::on_amendment_in_force(
                &inventory,
                &offer,
                &who,
                effective_at,
            );
        }

        assert_eq!(
            Contracts::<T>::get(group).map(|contract| contract.amendment_seq),
            Some(1)
        );
        assert_last_event::<T>(Event::ContractAmendmentInForce {
            group,
            effective_at,
        });
        Ok(())
    }

    impl_benchmark_test_suite!(
        UsageSubscription,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{new_test_ext, Test};

    // NFR-1
    #[test]
    fn payment_step_benchmarks_read_a_stale_name_then_a_full_scan() {
        for exhausted in [false, true] {
            new_test_ext().execute_with(|| {
                let (who, group, ..) = setup_member::<Test>(exhausted).expect("set up; qed");
                let stale = <Test as Config>::BenchmarkHelper::stale_group();

                // The name is read and looked up, then ignored.
                assert_eq!(
                    PayingGroup::<Test>::get(&who).and_then(|choice| choice.group),
                    Some(stale)
                );
                assert!(!UsageSubscription::<Test>::holds_valid_membership(
                    &who, &stale
                ));
                // The scan reads every membership the bound allows, and finds the one group.
                assert_eq!(
                    <<Test as Config>::Memberships as fc_traits_memberships::Inspect<_>>::user_memberships(
                        &who, None
                    )
                    .count(),
                    <Test as Config>::MaxMembershipScan::get() as usize
                );
                assert_eq!(
                    UsageSubscription::<Test>::resolve_paying_group(&who),
                    PayingGroupResolution::Only(group)
                );
            });
        }
    }
}
