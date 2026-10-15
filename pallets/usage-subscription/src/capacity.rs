//! The sales capacity (`REQ-CP-1`–`REQ-CP-6`, `DEC-39`–`DEC-44`): the committed load, kept where
//! a contract is written, the check of every sale against the binding capacity, the capacity and
//! its scheduled change, and the alert. Every step is O(1).
//!
//! What a contract counts is its `load`, plus, for a standard contract that has not applied its
//! offer's last amendment yet, that amendment's `load_increase`: `amend_offer` adds the increase
//! for every contract it covers at once, and each contract moves it into its own `load` when it
//! applies the amendment. So the committed load is always the sum of what the counting contracts
//! count, with no loop over them.

use super::*;

impl<T: Config> Pallet<T> {
    /// `allowance` per `usage_period`, normalized to [`Config::CapacityPeriod`] (`DEC-39`).
    pub(crate) fn normalized(allowance: Weight, usage_period: MomentOf<T>) -> Weight {
        normalized_allowance(allowance, usage_period, T::CapacityPeriod::get())
    }

    /// What `contract` counts in the committed load: its own load, and the increase its offer's
    /// last amendment brought, while it has not applied it. Nothing once it defaulted.
    pub(crate) fn committed_by(contract: &ContractOf<T>) -> Weight {
        let Some(load) = contract.load else {
            return Weight::zero();
        };
        let unapplied_increase = match contract.kind {
            OfferKind::Standard => OfferAmendment::<T>::get(contract.offer)
                .filter(|amendment| amendment.seq > contract.amendment_seq)
                .map_or(Weight::zero(), |amendment| amendment.load_increase),
            OfferKind::Trial | OfferKind::Custom(_) => Weight::zero(),
        };
        load.saturating_add(unapplied_increase)
    }

    /// Refuses with [`Error::CapacityExceeded`] a sale that releases `released` from the
    /// committed load and adds `added` to it, unless the load after it fits the binding capacity
    /// in both components (`REQ-CP-3`, `DEC-41`). A change that raises no component is no sale,
    /// and with no capacity every sale fits.
    ///
    /// A sale whose addition would overflow the load in either component is refused too, with or
    /// without a capacity: the load never saturates, so each release subtracts exactly what its
    /// contract added, and the load stays the sum of what the counting contracts count (`INV-14`).
    pub(crate) fn ensure_fits(released: Weight, added: Weight) -> DispatchResult {
        let load = CommittedLoad::<T>::get();
        let after = load
            .saturating_sub(released)
            .checked_add(&added)
            .ok_or(Error::<T>::CapacityExceeded)?;
        if !after.any_gt(load) {
            return Ok(());
        }
        let mut state = Capacity::<T>::get();
        state.apply_due(Self::now());
        if let Some(binding) = state.binding() {
            ensure!(after.all_lte(binding), Error::<T>::CapacityExceeded);
        }
        Ok(())
    }

    /// Releases `released` from the committed load and adds `added` to it, then applies a due
    /// scheduled capacity and checks the alert (`DEC-42`, `DEC-44`).
    ///
    /// Neither step saturates: what is released is what a contract counts, which is part of the
    /// load, and what is added was checked by [`Self::ensure_fits`], or is a contract's own share
    /// moving into its `load` (an amended allowance in force is never above what it counted).
    pub(crate) fn update_load(released: Weight, added: Weight) {
        if released == added {
            return;
        }
        let load = CommittedLoad::<T>::mutate(|load| {
            *load = load.saturating_sub(released).saturating_add(added);
            *load
        });
        let mut state = Capacity::<T>::get();
        let applied = state.apply_due(Self::now());
        Self::store_capacity(state, load, applied);
    }

    /// Sets what `contract` counts in the committed load to `load`, moving the committed load by
    /// the difference. A contract that no longer counts is left alone.
    pub(crate) fn set_contract_load(contract: &mut ContractOf<T>, load: Weight) {
        if let Some(current) = contract.load {
            Self::update_load(current, load);
            contract.load = Some(load);
        }
    }

    /// `contract` no longer counts: it defaulted (`DEC-39`).
    pub(crate) fn release_load(contract: &mut ContractOf<T>) {
        Self::update_load(Self::committed_by(contract), Weight::zero());
        Self::uncount_contract(contract);
        contract.load = None;
    }

    /// A contract to `offer` starts counting toward the committed load: one more in
    /// [`ContractsOnOffer`].
    pub(crate) fn count_contract(offer: &OfferIdOf<T>) {
        ContractsOnOffer::<T>::mutate(offer, |count| *count = count.saturating_add(1));
    }

    /// `contract` stops counting toward the committed load (it ends, defaults or is replaced):
    /// one fewer in [`ContractsOnOffer`] for its offer. A contract that no longer counted changes
    /// nothing.
    pub(crate) fn uncount_contract(contract: &ContractOf<T>) {
        if contract.load.is_some() {
            ContractsOnOffer::<T>::mutate_exists(contract.offer, |count| {
                *count = count
                    .and_then(|count| count.checked_sub(1))
                    .filter(|count| *count > 0);
            });
        }
    }

    /// Declares `capacity` now, or schedules it from `from` on (`REQ-CP-2`, `REQ-CP-4`).
    pub(crate) fn do_set_capacity(
        capacity: Option<Weight>,
        from: Option<MomentOf<T>>,
    ) -> DispatchResult {
        let load = CommittedLoad::<T>::get();
        if let Some(capacity) = capacity {
            ensure!(load.all_lte(capacity), Error::<T>::CapacityBelowLoad);
        }
        let now = Self::now();
        let mut state = Capacity::<T>::get();
        // A change already due took effect before this one.
        state.apply_due(now);
        match from {
            Some(from) if from > now => {
                state.scheduled = Some(ScheduledCapacity { capacity, from });
                Self::deposit_event(Event::<T>::CapacityScheduled { capacity, from });
            }
            _ => {
                // A schedule from a moment already reached replaces the pending one, and applies
                // at once; a declaration for now leaves the pending one in place.
                if from.is_some() {
                    state.scheduled = None;
                }
                state.current = capacity;
                Self::deposit_event(Event::<T>::CapacitySet { capacity });
            }
        }
        Self::store_capacity(state, load, true);
        Ok(())
    }

    /// The committed load, the capacity now and the scheduled change, from one state
    /// (`REQ-CP-6`). A due change reads as applied.
    pub(crate) fn capacity_info() -> CapacityInfoOf<T> {
        let mut state = Capacity::<T>::get();
        state.apply_due(Self::now());
        CapacityInfo {
            load: CommittedLoad::<T>::get(),
            capacity: state.current,
            scheduled: state.scheduled,
        }
    }

    /// Checks the alert against `load`, and stores `state` if it, or the alert, changed: the
    /// event is emitted when the load is at or above the mark of the binding capacity (the one
    /// sales are checked against) in some component and was not at the last check (`REQ-CP-5`,
    /// `DEC-44`).
    fn store_capacity(mut state: CapacityStateOf<T>, load: Weight, changed: bool) {
        let binding = state.binding();
        let above = binding.is_some_and(|capacity| Self::at_or_above_mark(load, capacity));
        if above && !state.above_threshold {
            if let Some(capacity) = binding {
                Self::deposit_event(Event::<T>::CapacityThresholdCrossed { load, capacity });
            }
        }
        if changed || above != state.above_threshold {
            state.above_threshold = above;
            Capacity::<T>::put(state);
        }
    }

    /// Whether `load` is at or above [`Config::CapacityAlertThreshold`] of `capacity` in some
    /// component.
    fn at_or_above_mark(load: Weight, capacity: Weight) -> bool {
        let threshold = T::CapacityAlertThreshold::get();
        load.ref_time() >= threshold.mul_ceil(capacity.ref_time())
            || load.proof_size() >= threshold.mul_ceil(capacity.proof_size())
    }
}
