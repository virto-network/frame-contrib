//! The pool meter: usage windows, the paying group, admission (the read-only check) and the
//! charge (`DEC-3`, `DEC-7`, `REQ-PL-*`, `REQ-PC-*`).

use super::*;

/// What admission found: the member, its paying group, the membership, the contract (its offer
/// and anchor), the usage window and the estimate (`REQ-PL-8`). Post-dispatch charges the pool
/// against it, whatever dispatch changed (`INV-13`), and only while the group's contract is still
/// the one it names.
///
/// It is opaque outside this crate: nothing else can make or read one.
#[derive(CloneNoBound, PartialEqNoBound, EqNoBound, DebugNoBound)]
pub struct Ticket<T: Config> {
    pub(crate) who: T::AccountId,
    pub(crate) group: GroupOf<T>,
    pub(crate) membership: MembershipOf<T>,
    pub(crate) offer: OfferIdOf<T>,
    pub(crate) anchor: MomentOf<T>,
    pub(crate) window_start: MomentOf<T>,
    pub(crate) estimated: Weight,
}

impl<T: Config> Pallet<T> {
    /// The allowance of `contract` in the usage window starting at `window`: an amended
    /// allowance from the first window at or after its effective boundary, the contract's
    /// otherwise (`REQ-CT-8`, `INV-20`).
    ///
    /// An amendment is in one place only: in force, in `next_allowance`; pending for a custom
    /// contract, in its `pending` slot; pending for a standard contract, in its offer's
    /// [`OfferAmendment`], while the contract's `amendment_seq` is behind it (`DEC-3`, `DEC-34`).
    pub(crate) fn allowance_at(contract: &ContractOf<T>, window: MomentOf<T>) -> Weight {
        if let Some(next) = &contract.next_allowance {
            if window >= next.from_window {
                return next.allowance;
            }
        }
        match (&contract.pending, &contract.kind) {
            (
                Some(PendingChange::Amend {
                    allowance,
                    from_window,
                    ..
                }),
                _,
            ) if window >= *from_window => *allowance,
            (_, OfferKind::Standard) => OfferAmendment::<T>::get(contract.offer)
                .filter(|amendment| amendment.seq > contract.amendment_seq)
                .and_then(|amendment| {
                    let boundary = subs::effective_boundary(
                        contract.anchor,
                        amendment.terms.billing_period,
                        amendment.enacted_at,
                        AMENDMENT_NOTICE_PERIODS,
                    )?;
                    let from_window =
                        first_window_from(contract.anchor, contract.usage_period, boundary);
                    (window >= from_window).then_some(amendment.terms.allowance)
                })
                .unwrap_or(contract.allowance),
            _ => contract.allowance,
        }
    }

    /// The paying group of `who`, and how it was found (`REQ-PC-2`, `REQ-PC-3`, `DEC-7`).
    ///
    /// The named group if `who` still holds a valid membership of it (a stale name is ignored,
    /// never deleted, `REQ-PC-4`); otherwise the only group of the memberships `who` holds, read
    /// lazily and at most `MaxMembershipScan` of them; otherwise none. Read only.
    pub fn resolve_paying_group(who: &T::AccountId) -> PayingGroupResolution<GroupOf<T>> {
        if let Some(group) = PayingGroup::<T>::get(who).and_then(|choice| choice.group) {
            if Self::holds_valid_membership(who, &group) {
                return PayingGroupResolution::Named(group);
            }
        }

        let bound = T::MaxMembershipScan::get();
        let mut found = None;
        let scan =
            <T::Memberships as fc_traits_memberships::Inspect<T::AccountId>>::user_memberships(
                who, None,
            )
            .take(bound.saturating_add(1) as usize);
        for (read, (group, _)) in scan.enumerate() {
            if read as u32 >= bound {
                return PayingGroupResolution::TooManyMemberships;
            }
            match found {
                None => found = Some(group),
                Some(only) if only == group => {}
                Some(_) => return PayingGroupResolution::SeveralGroups,
            }
        }
        found.map_or(
            PayingGroupResolution::NoMembership,
            PayingGroupResolution::Only,
        )
    }

    /// Whether `who` holds a valid membership of `group`: one of its memberships, `who` not being
    /// its group account (`REQ-PC-1`). One lazy read.
    pub(crate) fn holds_valid_membership(who: &T::AccountId, group: &GroupOf<T>) -> bool {
        <T::Memberships as fc_traits_memberships::Inspect<T::AccountId>>::user_memberships(
            who,
            Some(*group),
        )
        .next()
        .is_some()
    }

    /// Admission (`REQ-PL-7`): a ticket if a transaction of `who` with metered weight `estimate`
    /// takes the pool path now, or the first condition that fails. Each step returns early.
    ///
    /// It writes nothing (`INV-1`), and gives the same answer for the same arguments in the same
    /// state (`CTR-FEE-1`). It reads a bounded number of items (`NFR-1`): the named group and a
    /// membership of it, at most `MaxMembershipScan + 1` memberships, a membership of the paying
    /// group, the usable-group check, the contract, its subscription and, for a standard
    /// contract, its offer's amendment.
    pub(crate) fn check(who: &T::AccountId, estimate: Weight) -> Result<Ticket<T>, FeePathReason> {
        let group = match Self::resolve_paying_group(who) {
            PayingGroupResolution::Named(group) | PayingGroupResolution::Only(group) => group,
            _ => return Err(FeePathReason::NoPayingGroup),
        };
        let (_, membership) =
            <T::Memberships as fc_traits_memberships::Inspect<T::AccountId>>::user_memberships(
                who,
                Some(group),
            )
            .next()
            .ok_or(FeePathReason::NotAMember)?;
        ensure!(
            T::UsableGroup::contains(&group),
            FeePathReason::GroupUnusable
        );
        let contract = Contracts::<T>::get(group).ok_or(FeePathReason::NoContract)?;

        let now = Self::now();
        let subscription = <T::Subscriptions as subs::Inspect<T::AccountId>>::subscription(
            &T::OfferInventory::get(),
            &contract.offer,
            &T::GroupAccount::convert(group),
        )
        .ok_or(FeePathReason::NoContract)?;
        // Usable only while *Active* and paid, from the clock (`REQ-CT-4`).
        ensure!(subscription.is_paid(now), FeePathReason::NotPaid);

        let window_start = contract
            .window_at(now)
            .ok_or(FeePathReason::AllowanceExceeded)?;
        let allowance = Self::allowance_at(&contract, window_start);
        let used = contract.used_in(window_start);
        // Both components, or not at all (`REQ-PL-12`).
        let fits = used
            .checked_add(&estimate)
            .is_some_and(|total| total.all_lte(allowance));
        ensure!(fits, FeePathReason::AllowanceExceeded);

        Ok(Ticket {
            who: who.clone(),
            group,
            membership,
            offer: contract.offer,
            anchor: contract.anchor,
            window_start,
            estimated: estimate,
        })
    }

    /// Charges the pool of the ticket's contract `min(actual, estimate)` in the ticket's window
    /// (`CTR-FEE-2`, `INV-3`, `INV-13`), in one write. It never fails.
    ///
    /// Returns the weight charged and the pool's remainder in that window, or `None` if there is
    /// nothing to charge: the contract's record is gone (a termination during dispatch), it is no
    /// longer the contract the ticket names (another offer or anchor), or it already records a
    /// later window.
    ///
    /// Only the first can happen while the ticket's paid window holds. The other two are
    /// defensive: a replacement or a no-lead tick needs `now >= paid_through`, and `now` cannot
    /// advance inside one dispatch.
    pub(crate) fn charge(ticket: &Ticket<T>, actual: Weight) -> Option<(Weight, Weight)> {
        let weight = actual.min(ticket.estimated);
        Contracts::<T>::mutate(ticket.group, |maybe_contract| {
            let contract = maybe_contract
                .as_mut()
                .filter(|c| c.offer == ticket.offer && c.anchor == ticket.anchor)?;
            if contract.window_start == ticket.window_start {
                contract.used = contract.used.saturating_add(weight);
            } else if contract.window_start < ticket.window_start {
                // A new window: its usage starts from zero (`REQ-PL-2`).
                contract.window_start = ticket.window_start;
                contract.used = weight;
            } else {
                return None;
            }
            let allowance = Self::allowance_at(contract, ticket.window_start);
            Some((weight, allowance.saturating_sub(contract.used)))
        })
    }

    /// The paying-group rate window of `now`, counted from tick 0 (`REQ-PC-5`).
    pub(crate) fn rate_window(now: MomentOf<T>) -> MomentOf<T> {
        now.checked_div(&T::PayingGroupChangeWindow::get())
            .unwrap_or_else(Zero::zero)
    }

    /// Whether naming (or clearing) a paying group is free: the signer holds a valid membership
    /// of the group it names (or, clearing, of the group it had named), and has made fewer than
    /// `MaxPayingGroupChanges` changes in the current rate window (`REQ-PC-5`, `DEC-21`).
    pub(crate) fn is_free_naming(origin: &OriginFor<T>, group: &Option<GroupOf<T>>) -> bool {
        let Ok(who) = ensure_signed(origin.clone()) else {
            return false;
        };
        let choice = PayingGroup::<T>::get(&who);
        let named = group.or_else(|| choice.as_ref().and_then(|choice| choice.group));
        let Some(named) = named else {
            return false;
        };
        let window = Self::rate_window(Self::now());
        let changes = choice
            .filter(|choice| choice.window == window)
            .map_or(0, |choice| choice.changes);
        changes < T::MaxPayingGroupChanges::get() && Self::holds_valid_membership(&who, &named)
    }
}
