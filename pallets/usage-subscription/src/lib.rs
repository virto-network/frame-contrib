#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../README.md")]

extern crate alloc;

use fc_traits_listings::{
    item::subscriptions::{self as subs, OnSubscriptionChanged},
    InspectInventory, InventoryLifecycle, MutateItem,
};
use frame_support::{
    pallet_prelude::*,
    traits::{Contains, Incrementable},
};
use frame_system::pallet_prelude::*;
use sp_runtime::{
    traits::{BlockNumberProvider, Convert, Zero},
    ArithmeticError,
};

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod admission;
pub mod extension;
mod functions;
mod hooks;
mod types;
mod views;
pub mod weights;

pub use admission::Ticket;
#[cfg(feature = "runtime-benchmarks")]
pub use benchmarking::BenchmarkHelper;
pub use extension::{ChargeUsageSubscription, Path, PATH_MISMATCH};
pub use pallet::*;
pub use types::*;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        // Primitives.

        /// The weights of the calls, and of the payment step.
        type WeightInfo: WeightInfo;

        // Group bindings (`REQ-GR-*`).

        /// The memberships of groups: which memberships an account holds, optionally in one group
        /// (`CTR-MEM-1`, `CTR-MEM-2`), and each group's transfer policy, read only. A group's own
        /// account must never count as its member (`REQ-MI-13`).
        type Memberships: fc_traits_memberships::Inspect<
                Self::AccountId,
                Group: MaxEncodedLen + Copy,
                Membership: MaxEncodedLen,
            > + fc_traits_memberships::Transfer<Self::AccountId>;
        /// The account of each group: it pays the group's charges. Naming it must need no storage
        /// read (`REQ-GR-2`), and different groups must have different accounts.
        type GroupAccount: Convert<GroupOf<Self>, Self::AccountId>;
        /// The groups that may subscribe, and whose pools may admit a transaction, in at most one
        /// storage read (`REQ-GR-3`). The collective's own group, if any, must not be one
        /// (`REQ-GR-4`).
        type UsableGroup: Contains<GroupOf<Self>>;

        // Billing.

        /// Recurring subscriptions (`CTR-SUB`): the money and time half of every contract. The
        /// inventory and item traits let the pallet create the collective's inventory and publish
        /// an item per offer (`DEC-8`), so their merchant and inventory ids are the subscription
        /// items'. Item ids must be incrementable: offers take ascending ids.
        ///
        /// Its subscription hooks must notify this pallet ([`OnSubscriptionChanged`] is
        /// implemented for [`Pallet`]), and only this pallet may subscribe to, publish in, or
        /// administer [`Config::OfferInventory`]. In particular nothing else may publish an item
        /// there: offers take ascending item ids from [`NextOfferId`], so an item published by
        /// anything else at the next offer's id would make every later `publish_offer` fail.
        type Subscriptions: subs::Mutate<Self::AccountId, ItemId: Incrementable>
            + MutateItem<Self::AccountId>
            + InventoryLifecycle<
                Self::AccountId,
                MerchantId = MerchantIdOf<Self>,
                InventoryId = InventoryIdOf<Self>,
            >;
        /// The collective's inventory, where every offer is an item. It is created on the first
        /// offer, owned by [`Config::Payee`].
        #[pallet::constant]
        type OfferInventory: Get<(MerchantIdOf<Self>, InventoryIdOf<Self>)>;
        /// The configured payee: the owner of the collective's inventory, so the recipient of
        /// every charge (`DEC-8`).
        type Payee: Get<Self::AccountId>;

        // Origins.

        /// The collective's administrative origin for standard and trial offers (`REQ-OF-1`).
        type StandardOfferOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// The outcome of a collective referendum, for custom offers (`REQ-OF-1`).
        type CustomOfferOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// The collective's origin to terminate a contract (`REQ-CT-6`).
        type TerminateOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// The only origin that amends a custom contract, or a standard offer and its contracts
        /// (`REQ-OF-8`, `DEC-28`). Distinct from the origins above: the outcome of a collective
        /// referendum whose shortest path to enactment is at least the deployment's minimum
        /// notice.
        type AmendOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// A group's administrative origin, resolving to the group.
        type GroupOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = GroupOf<Self>>;

        // The clock.

        /// The chain clock (`CTR-CLK-1`). It must be the clock of [`Config::Subscriptions`], so
        /// billing periods and usage windows are counted in the same ticks.
        type BlockNumberProvider: BlockNumberProvider<BlockNumber = MomentOf<Self>>;

        // Parameters.

        /// The shortest usage period an offer may have, in ticks. Non-zero.
        #[pallet::constant]
        type MinUsagePeriod: Get<MomentOf<Self>>;
        /// The shortest billing period an offer may have, in ticks. Non-zero, and longer than the
        /// lead of [`Config::Subscriptions`].
        #[pallet::constant]
        type MinBillingPeriod: Get<MomentOf<Self>>;
        /// The most billing periods a trial may run. Non-zero.
        #[pallet::constant]
        type MaxTrialPeriods: Get<u32>;
        /// The most memberships admission reads to find a member's only group, when it named
        /// none (`REQ-PC-3`, `NFR-1`). Every signed transaction pays for reading them.
        #[pallet::constant]
        type MaxMembershipScan: Get<u32>;
        /// The most free changes of paying group a member makes per rate window (`REQ-PC-5`).
        #[pallet::constant]
        type MaxPayingGroupChanges: Get<u32>;
        /// The rate window of paying-group changes, in ticks, counted from tick 0: the
        /// deployment's minimum usage period (`REQ-PC-5`). Non-zero.
        #[pallet::constant]
        type PayingGroupChangeWindow: Get<MomentOf<Self>>;
        /// The most offers one page of the `open_offers` view function examines (0008-A1).
        /// Non-zero.
        #[pallet::constant]
        type MaxOffersPerPage: Get<u32>;

        // Benchmarking.

        /// Sets up a usable group, its members and a price for the benchmarks.
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: BenchmarkHelper<Self>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The offers, by id. Each is an item of [`Config::OfferInventory`].
    #[pallet::storage]
    pub type Offers<T: Config> = StorageMap<_, Blake2_128Concat, OfferIdOf<T>, OfferOf<T>>;

    /// The id the next offer takes.
    #[pallet::storage]
    pub type NextOfferId<T: Config> = StorageValue<_, OfferIdOf<T>>;

    /// The contract of each group, in any state but *Ended* (`INV-8`).
    #[pallet::storage]
    pub type Contracts<T: Config> = StorageMap<_, Blake2_128Concat, GroupOf<T>, ContractOf<T>>;

    /// The group whose account each contract's subscription is keyed by: the inverse of
    /// [`Config::GroupAccount`], for the subscription hooks. Kept while the contract exists.
    #[pallet::storage]
    pub type GroupOfAccount<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, GroupOf<T>>;

    /// Each member's named paying group, and its changes in the current rate window (`DEC-7`,
    /// `DEC-21`). A name that becomes invalid is ignored by admission, never deleted by it.
    #[pallet::storage]
    pub type PayingGroup<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, PayingGroupChoice<GroupOf<T>, MomentOf<T>>>;

    /// The groups that have started a trial (`REQ-OF-7`). Never removed.
    #[pallet::storage]
    pub type TrialUsed<T: Config> = StorageMap<_, Blake2_128Concat, GroupOf<T>, ()>;

    /// The last amendment of each standard offer, applied lazily: each contract made from the
    /// offer before it takes it at its own effective boundary (`DEC-34`).
    #[pallet::storage]
    pub type OfferAmendment<T: Config> =
        StorageMap<_, Blake2_128Concat, OfferIdOf<T>, OfferAmendmentOf<T>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The collective published an offer.
        OfferPublished {
            offer: OfferIdOf<T>,
            kind: OfferKindOf<T>,
        },
        /// An offer was withdrawn, or a custom offer was accepted: no new contract may be made
        /// from it.
        OfferWithdrawn { offer: OfferIdOf<T> },
        /// A contract started, with its billing period 0 charged.
        ContractStarted {
            group: GroupOf<T>,
            offer: OfferIdOf<T>,
        },
        /// A billing period of a contract was charged.
        ContractCharged {
            group: GroupOf<T>,
            period: u32,
            paid_through: MomentOf<T>,
        },
        /// A charge failed at or after its due tick: the pool is unusable until it is paid, or
        /// `grace_end` passes.
        ContractSuspended {
            group: GroupOf<T>,
            grace_end: MomentOf<T>,
        },
        /// A suspended contract was paid within grace.
        ContractRestored { group: GroupOf<T> },
        /// A contract lapsed within its minimum commitment, and blocks the group until `until`.
        ContractDefaulted {
            group: GroupOf<T>,
            until: MomentOf<T>,
        },
        /// The group cancelled its contract. It ends at a later due tick.
        CancellationRequested { group: GroupOf<T> },
        /// A contract ended, and its record was removed.
        ContractEnded {
            group: GroupOf<T>,
            reason: EndReason,
        },
        /// The group scheduled a switch to `offer`.
        SwitchScheduled {
            group: GroupOf<T>,
            offer: OfferIdOf<T>,
        },
        /// The group cancelled its pending switch to `offer`.
        SwitchCancelled {
            group: GroupOf<T>,
            offer: OfferIdOf<T>,
        },
        /// A pending switch to `offer` did not take effect, for `reason`.
        SwitchDropped {
            group: GroupOf<T>,
            offer: OfferIdOf<T>,
            reason: subs::ReplacementDropReason,
        },
        /// The group's trial converts into `offer` at its end, if its first charge succeeds then.
        ConversionScheduled {
            group: GroupOf<T>,
            offer: OfferIdOf<T>,
        },
        /// The group cancelled its trial's conversion into `offer`.
        ConversionCancelled {
            group: GroupOf<T>,
            offer: OfferIdOf<T>,
        },
        /// The trial's conversion into `offer` did not take effect, for `reason`.
        ConversionDropped {
            group: GroupOf<T>,
            offer: OfferIdOf<T>,
            reason: subs::ReplacementDropReason,
        },
        /// A standard offer was amended: at once for new contracts, and for each contract made
        /// from it at its own effective boundary, every one before `last_boundary`.
        OfferAmended {
            offer: OfferIdOf<T>,
            enacted_at: MomentOf<T>,
            last_boundary: MomentOf<T>,
        },
        /// An amendment of a group's custom contract was enacted. It applies from
        /// `effective_at`, and until then the group may cancel with its commitment waived.
        ContractAmended {
            group: GroupOf<T>,
            effective_at: MomentOf<T>,
        },
        /// An amendment came into force for a group's contract at `effective_at`: the new price
        /// from there, the new allowance from the first usage window starting at or after it.
        ContractAmendmentInForce {
            group: GroupOf<T>,
            effective_at: MomentOf<T>,
        },
        /// A member named (or, with `None`, cleared) its paying group.
        PayingGroupSet {
            who: T::AccountId,
            group: Option<GroupOf<T>>,
        },
        /// A pool-path transaction of `who` was charged `weight` to its group's pool, which has
        /// `remaining` left in the current usage window. The only event of a pool-path
        /// transaction (`CTR-EVT-2`).
        UsageCharged {
            group: GroupOf<T>,
            who: T::AccountId,
            weight: Weight,
            remaining: Weight,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The terms break the rules of the offer's kind (SPEC §5.1), or of an amendment.
        InvalidTerms,
        /// There is no such offer.
        UnknownOffer,
        /// The offer is withdrawn, or is a custom offer already accepted.
        OfferWithdrawn,
        /// The offer is a custom offer for another group.
        NotEligible,
        /// The group already has a contract, in some state.
        AlreadyContracted,
        /// Billing period 0 could not be charged.
        ChargeFailed,
        /// The group has no contract in a state the call accepts.
        NoContract,
        /// A switch or conversion is already pending.
        SwitchPending,
        /// An amendment is pending, so no switch, and no second amendment, may be made.
        ChangePending,
        /// There is no pending switch or conversion to cancel.
        NoPendingSwitch,
        /// A trial's named conversion is not an open, non-trial offer the group is eligible for.
        InvalidConversion,
        /// The group is not usable.
        GroupUnusable,
        /// The group has already started a trial.
        TrialUsed,
        /// The account holds no valid membership of the named group.
        NotAMember,
    }

    #[pallet::call(weight(<T as Config>::WeightInfo))]
    impl<T: Config> Pallet<T> {
        /// Publishes an offer of `kind` with `terms`.
        ///
        /// A standard or trial offer takes [`Config::StandardOfferOrigin`]; a custom one
        /// [`Config::CustomOfferOrigin`]. The terms must keep the rules of the kind (SPEC §5.1):
        /// otherwise [`Error::InvalidTerms`]. The offer becomes an item of
        /// [`Config::OfferInventory`] with the money terms as its subscription conditions; a
        /// custom offer is made subscribable by its group's account only, once.
        #[pallet::call_index(0)]
        pub fn publish_offer(
            origin: OriginFor<T>,
            kind: OfferKindOf<T>,
            terms: TermsOf<T>,
        ) -> DispatchResult {
            Self::ensure_offer_origin(origin, &kind)?;
            Self::validate_terms(&kind, &terms)?;

            let inventory = T::OfferInventory::get();
            if !<T::Subscriptions as InspectInventory>::exists(&inventory) {
                <T::Subscriptions as InventoryLifecycle<T::AccountId>>::create(
                    inventory,
                    &T::Payee::get(),
                )?;
            }

            let offer = NextOfferId::<T>::get()
                .or_else(OfferIdOf::<T>::initial_value)
                .ok_or(ArithmeticError::Overflow)?;
            let next = offer.increment().ok_or(ArithmeticError::Overflow)?;

            <T::Subscriptions as MutateItem<T::AccountId>>::publish(
                &inventory,
                &offer,
                alloc::vec::Vec::new(),
                None,
            )?;
            <T::Subscriptions as subs::Mutate<T::AccountId>>::set_conditions(
                &inventory,
                &offer,
                terms.conditions(),
            )
            .map_err(|e| Self::listings_error(e, Error::<T>::InvalidTerms))?;
            if let OfferKind::Custom(group) = kind {
                <T::Subscriptions as subs::Mutate<T::AccountId>>::set_exclusive(
                    &inventory,
                    &offer,
                    &T::GroupAccount::convert(group),
                )?;
            }
            // The collective amends with one billing period of notice, and may terminate.
            <T::Subscriptions as subs::Mutate<T::AccountId>>::set_policy(
                &inventory,
                &offer,
                offer_policy(),
            )?;

            Offers::<T>::insert(
                offer,
                Offer {
                    kind,
                    allowance: terms.allowance,
                    usage_period: terms.usage_period,
                    status: OfferStatus::Open,
                },
            );
            NextOfferId::<T>::put(next);

            Self::deposit_event(Event::<T>::OfferPublished { offer, kind });
            Ok(())
        }

        /// Withdraws an open offer: no new contract may be made from it. Contracts made from it
        /// are not touched (`REQ-OF-3`).
        ///
        /// It takes the origin that publishes the offer's kind.
        #[pallet::call_index(1)]
        pub fn withdraw_offer(origin: OriginFor<T>, offer: OfferIdOf<T>) -> DispatchResult {
            let mut record = Offers::<T>::get(offer).ok_or(Error::<T>::UnknownOffer)?;
            Self::ensure_offer_origin(origin, &record.kind)?;
            ensure!(
                record.status == OfferStatus::Open,
                Error::<T>::OfferWithdrawn
            );

            // Listings refuses new subscriptions and switches into it too.
            <T::Subscriptions as subs::Mutate<T::AccountId>>::withdraw_conditions(
                &T::OfferInventory::get(),
                &offer,
            )?;
            record.status = OfferStatus::Withdrawn;
            Offers::<T>::insert(offer, record);
            Self::deposit_event(Event::<T>::OfferWithdrawn { offer });
            Ok(())
        }

        /// Subscribes the origin's group to `offer`: billing period 0 is charged from the group's
        /// account to the payee, and the contract is *Active*, anchored now (`REQ-BL-1`).
        ///
        /// `converts_into` names the offer a trial converts into at its end (`REQ-CT-13`).
        ///
        /// Refused, with nothing changed, if the group is not usable, the offer is unknown,
        /// withdrawn or not for the group, the group has a contract (a *Defaulted* one past its
        /// commitment end excepted, `REQ-CT-12`), or period 0 cannot be charged.
        #[pallet::call_index(2)]
        pub fn subscribe(
            origin: OriginFor<T>,
            offer: OfferIdOf<T>,
            converts_into: Option<OfferIdOf<T>>,
        ) -> DispatchResult {
            let group = T::GroupOrigin::ensure_origin(origin)?;
            ensure!(T::UsableGroup::contains(&group), Error::<T>::GroupUnusable);
            let target = Offers::<T>::get(offer).ok_or(Error::<T>::UnknownOffer)?;
            ensure!(
                target.status == OfferStatus::Open,
                Error::<T>::OfferWithdrawn
            );
            ensure!(
                Self::is_eligible(&target.kind, &group),
                Error::<T>::NotEligible
            );

            let account = T::GroupAccount::convert(group);
            let now = Self::now();
            Self::ensure_no_contract(&group, &account, now)?;
            let trial = target.kind == OfferKind::Trial;
            ensure!(
                !trial || !TrialUsed::<T>::contains_key(group),
                Error::<T>::TrialUsed
            );
            if let Some(conversion) = converts_into {
                ensure!(
                    trial && Self::is_valid_conversion(&conversion, &group),
                    Error::<T>::InvalidConversion
                );
            }

            Self::start_contract(group, &account, offer, &target, now)?;

            if trial {
                TrialUsed::<T>::insert(group, ());
            }
            if let Some(conversion) = converts_into {
                <T::Subscriptions as subs::Mutate<T::AccountId>>::schedule_replacement_at_term_end(
                    &T::OfferInventory::get(),
                    &offer,
                    &account,
                    &conversion,
                )
                .map_err(|_| Error::<T>::InvalidConversion)?;
                Contracts::<T>::mutate(group, |maybe_contract| {
                    if let Some(contract) = maybe_contract {
                        contract.pending = Some(PendingChange::Convert(conversion));
                    }
                });
                Self::deposit_event(Event::<T>::ConversionScheduled {
                    group,
                    offer: conversion,
                });
            }
            Ok(())
        }

        /// Cancels the origin's group's contract: at the later of its `paid through` and its
        /// commitment end (`REQ-CT-5`, `REQ-CT-10`). A *Suspended* contract outside its commitment
        /// ends at once. A pending switch is dropped.
        #[pallet::call_index(3)]
        pub fn cancel(origin: OriginFor<T>) -> DispatchResult {
            let group = T::GroupOrigin::ensure_origin(origin)?;
            let contract = Contracts::<T>::get(group).ok_or(Error::<T>::NoContract)?;
            let account = T::GroupAccount::convert(group);

            <T::Subscriptions as subs::Mutate<T::AccountId>>::cancel(
                &T::OfferInventory::get(),
                &contract.offer,
                &account,
            )
            .map_err(|e| Self::listings_error(e, Error::<T>::NoContract))?;

            if Contracts::<T>::contains_key(group) {
                Self::deposit_event(Event::<T>::CancellationRequested { group });
            }
            Ok(())
        }

        /// Schedules a switch of the origin's group's *Active* contract to `offer`, at the first
        /// billing boundary at or after both its `paid through` and its commitment end, if the new
        /// contract's first charge succeeds then (`REQ-CT-7`).
        #[pallet::call_index(4)]
        pub fn switch_offer(origin: OriginFor<T>, offer: OfferIdOf<T>) -> DispatchResult {
            let group = T::GroupOrigin::ensure_origin(origin)?;
            let mut contract = Contracts::<T>::get(group).ok_or(Error::<T>::NoContract)?;
            let account = T::GroupAccount::convert(group);
            let inventory = T::OfferInventory::get();
            let now = Self::now();

            let subscription = <T::Subscriptions as subs::Inspect<T::AccountId>>::subscription(
                &inventory,
                &contract.offer,
                &account,
            )
            .ok_or(Error::<T>::NoContract)?;
            ensure!(
                matches!(subscription.state, subs::SubscriptionState::Active)
                    && subscription.cancel_requested.is_none(),
                Error::<T>::NoContract
            );

            let target = Offers::<T>::get(offer).ok_or(Error::<T>::UnknownOffer)?;
            ensure!(
                target.status == OfferStatus::Open,
                Error::<T>::OfferWithdrawn
            );
            ensure!(
                Self::is_eligible(&target.kind, &group),
                Error::<T>::NotEligible
            );
            ensure!(offer != contract.offer, Error::<T>::AlreadyContracted);
            ensure!(
                target.kind != OfferKind::Trial || !TrialUsed::<T>::contains_key(group),
                Error::<T>::TrialUsed
            );

            match contract.pending {
                Some(PendingChange::Switch(_) | PendingChange::Convert(_)) => {
                    return Err(Error::<T>::SwitchPending.into())
                }
                Some(PendingChange::Amend { .. }) => return Err(Error::<T>::ChangePending.into()),
                None => {}
            }
            ensure!(
                <T::Subscriptions as subs::Inspect<T::AccountId>>::pending_amendment(
                    &inventory,
                    &contract.offer,
                    &account,
                    now,
                )
                .is_none(),
                Error::<T>::ChangePending
            );

            // A trial does not renew: a switch scheduled during it takes effect at its end
            // (`REQ-CT-9`).
            if contract.kind == OfferKind::Trial {
                <T::Subscriptions as subs::Mutate<T::AccountId>>::schedule_replacement_at_term_end(
                    &inventory,
                    &contract.offer,
                    &account,
                    &offer,
                )
            } else {
                <T::Subscriptions as subs::Mutate<T::AccountId>>::schedule_replacement(
                    &inventory,
                    &contract.offer,
                    &account,
                    &offer,
                )
            }
            .map_err(|e| Self::listings_error(e, Error::<T>::NotEligible))?;

            contract.pending = Some(PendingChange::Switch(offer));
            Contracts::<T>::insert(group, contract);
            Self::deposit_event(Event::<T>::SwitchScheduled { group, offer });
            Ok(())
        }

        /// Terminates a group's contract at once, whatever its state (`REQ-CT-6`). Nothing is
        /// refunded, and the group's membership deposits are not touched.
        #[pallet::call_index(5)]
        pub fn terminate_contract(origin: OriginFor<T>, group: GroupOf<T>) -> DispatchResult {
            T::TerminateOrigin::ensure_origin(origin)?;
            let contract = Contracts::<T>::get(group).ok_or(Error::<T>::NoContract)?;
            let account = T::GroupAccount::convert(group);
            let inventory = T::OfferInventory::get();

            if let Err(e) = <T::Subscriptions as subs::Mutate<T::AccountId>>::terminate(
                &inventory,
                &contract.offer,
                &account,
            ) {
                // A contract whose subscription is gone is only a stale record: end it here.
                if <T::Subscriptions as subs::Inspect<T::AccountId>>::subscription(
                    &inventory,
                    &contract.offer,
                    &account,
                )
                .is_some()
                {
                    return Err(Self::listings_error(e, Error::<T>::NoContract));
                }
                Self::end_contract(&group, &account, EndReason::Terminated);
            }
            Ok(())
        }

        /// Names the signer's paying group, or clears it with `None` (`REQ-PC-2`, `REQ-PC-4`).
        ///
        /// Naming a group the signer holds no valid membership of is refused with
        /// [`Error::NotAMember`]. The call is feeless when the signer holds a valid membership of
        /// the group it names (or, clearing, of the group it had named) and has made fewer than
        /// [`Config::MaxPayingGroupChanges`] changes in the current rate window
        /// (`REQ-PC-5`, `DEC-21`); otherwise it is an ordinary transaction.
        #[pallet::call_index(6)]
        #[pallet::feeless_if(|origin: &OriginFor<T>, group: &Option<GroupOf<T>>| -> bool {
            Pallet::<T>::is_free_naming(origin, group)
        })]
        pub fn set_paying_group(origin: OriginFor<T>, group: Option<GroupOf<T>>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            if let Some(group) = group {
                ensure!(
                    Self::holds_valid_membership(&who, &group),
                    Error::<T>::NotAMember
                );
            }

            let window = Self::rate_window(Self::now());
            PayingGroup::<T>::mutate(&who, |choice| {
                let changes = match choice {
                    Some(choice) if choice.window == window => choice.changes.saturating_add(1),
                    _ => 1,
                };
                *choice = Some(PayingGroupChoice {
                    group,
                    window,
                    changes,
                });
            });

            Self::deposit_event(Event::<T>::PayingGroupSet { who, group });
            Ok(())
        }

        /// Amends a group's *Active* or *Suspended* custom contract, without the group's
        /// acceptance (`REQ-CT-8`). Only [`Config::AmendOrigin`] may.
        ///
        /// The terms must keep the rules of a custom offer and the contract's usage and billing
        /// periods. They take effect at the contract's effective boundary: the first billing
        /// boundary at least one billing period from now (`REQ-CT-14`), for the price, and the
        /// first usage window starting at or after it, for the allowance. Until then the group may
        /// cancel with its commitment waived (`REQ-CT-15`). A pending switch is dropped.
        #[pallet::call_index(7)]
        pub fn amend_contract(
            origin: OriginFor<T>,
            group: GroupOf<T>,
            terms: TermsOf<T>,
        ) -> DispatchResult {
            T::AmendOrigin::ensure_origin(origin)?;
            let contract = Contracts::<T>::get(group).ok_or(Error::<T>::NoContract)?;
            let account = T::GroupAccount::convert(group);
            let inventory = T::OfferInventory::get();
            let now = Self::now();

            let subscription = <T::Subscriptions as subs::Inspect<T::AccountId>>::subscription(
                &inventory,
                &contract.offer,
                &account,
            )
            .ok_or(Error::<T>::NoContract)?;
            ensure!(
                !matches!(
                    subscription.state,
                    subs::SubscriptionState::Defaulted { .. }
                ),
                Error::<T>::NoContract
            );
            // Only a custom contract is amended on its own.
            ensure!(
                matches!(contract.kind, OfferKind::Custom(_)),
                Error::<T>::InvalidTerms
            );
            // At most one allowance outside the current one (`DEC-3`): the last amendment's must
            // be in force first.
            ensure!(
                !Self::allowance_pending(&contract, now),
                Error::<T>::ChangePending
            );
            Self::validate_terms(&contract.kind, &terms)?;
            ensure!(
                terms.usage_period == contract.usage_period
                    && terms.billing_period == subscription.conditions.period,
                Error::<T>::InvalidTerms
            );
            ensure!(
                !matches!(contract.pending, Some(PendingChange::Amend { .. }))
                    && <T::Subscriptions as subs::Inspect<T::AccountId>>::pending_amendment(
                        &inventory,
                        &contract.offer,
                        &account,
                        now,
                    )
                    .is_none(),
                Error::<T>::ChangePending
            );

            // Listings drops a pending switch, and reports the enactment.
            let effective_at = <T::Subscriptions as subs::Mutate<T::AccountId>>::amend(
                &inventory,
                &contract.offer,
                &account,
                terms.conditions(),
            )
            .map_err(|e| Self::listings_error(e, Error::<T>::InvalidTerms))?;

            Contracts::<T>::mutate(group, |maybe_contract| {
                if let Some(contract) = maybe_contract {
                    contract.pending = Some(PendingChange::Amend {
                        allowance: terms.allowance,
                        effective_at,
                        from_window: first_window_from(
                            contract.anchor,
                            contract.usage_period,
                            effective_at,
                        ),
                    });
                }
            });
            Ok(())
        }

        /// Cancels the origin's group's pending switch (or conversion) before it takes effect.
        #[pallet::call_index(8)]
        pub fn cancel_switch(origin: OriginFor<T>) -> DispatchResult {
            let group = T::GroupOrigin::ensure_origin(origin)?;
            let contract = Contracts::<T>::get(group).ok_or(Error::<T>::NoPendingSwitch)?;
            ensure!(
                matches!(
                    contract.pending,
                    Some(PendingChange::Switch(_) | PendingChange::Convert(_))
                ),
                Error::<T>::NoPendingSwitch
            );

            <T::Subscriptions as subs::Mutate<T::AccountId>>::cancel_replacement(
                &T::OfferInventory::get(),
                &contract.offer,
                &T::GroupAccount::convert(group),
            )
            .map_err(|e| Self::listings_error(e, Error::<T>::NoPendingSwitch))
        }

        /// Amends an open standard offer, without any group's acceptance (`DEC-34`). Only
        /// [`Config::AmendOrigin`] may.
        ///
        /// The terms must keep the rules of a standard offer and the offer's usage and billing
        /// periods. Contracts made from the offer from now on take them at once. Every *Active* or
        /// *Suspended* contract made from it before takes them at its own effective boundary,
        /// with the free exit until then (`REQ-CT-8`, `REQ-CT-14`, `REQ-CT-15`): lazily, with no
        /// loop over contracts. A second amendment waits (`ChangePending`) until every contract
        /// made from the offer before the first has applied it, or ended, and until the
        /// allowance it brought is in force for each of them (`DEC-3`, `DEC-34`).
        #[pallet::call_index(9)]
        pub fn amend_offer(
            origin: OriginFor<T>,
            offer: OfferIdOf<T>,
            terms: TermsOf<T>,
        ) -> DispatchResult {
            T::AmendOrigin::ensure_origin(origin)?;
            let mut record = Offers::<T>::get(offer).ok_or(Error::<T>::UnknownOffer)?;
            ensure!(
                record.status == OfferStatus::Open,
                Error::<T>::OfferWithdrawn
            );
            // Only a standard offer is amended with its contracts.
            ensure!(record.kind == OfferKind::Standard, Error::<T>::InvalidTerms);
            Self::validate_terms(&record.kind, &terms)?;
            let inventory = T::OfferInventory::get();
            let conditions =
                <T::Subscriptions as subs::Inspect<T::AccountId>>::subscription_conditions(
                    &inventory, &offer,
                )
                .ok_or(Error::<T>::UnknownOffer)?;
            ensure!(
                terms.usage_period == record.usage_period
                    && terms.billing_period == conditions.period,
                Error::<T>::InvalidTerms
            );

            // At most one allowance outside the current one per contract (`DEC-3`): every
            // contract's allowance from the previous amendment is in force from the first usage
            // window at or after its effective boundary, so before that boundary's bound plus a
            // usage period.
            if let Some(previous) = OfferAmendment::<T>::get(offer) {
                use sp_runtime::traits::Saturating;
                let notice_and_one =
                    MomentOf::<T>::from(AMENDMENT_NOTICE_PERIODS.saturating_add(1));
                let in_force_by = previous
                    .enacted_at
                    .saturating_add(conditions.period.saturating_mul(notice_and_one))
                    .saturating_add(record.usage_period);
                ensure!(Self::now() >= in_force_by, Error::<T>::ChangePending);
            }

            // Listings refuses it too while any contract has not applied the previous one.
            let amendment = <T::Subscriptions as subs::Mutate<T::AccountId>>::amend_item(
                &inventory,
                &offer,
                terms.conditions(),
            )
            .map_err(|e| Self::listings_error(e, Error::<T>::InvalidTerms))?;

            record.allowance = terms.allowance;
            Offers::<T>::insert(offer, record);
            OfferAmendment::<T>::insert(
                offer,
                OfferAmendmentRecord {
                    seq: amendment.seq,
                    terms,
                    enacted_at: amendment.enacted_at,
                },
            );
            Self::deposit_event(Event::<T>::OfferAmended {
                offer,
                enacted_at: amendment.enacted_at,
                last_boundary: amendment.last_boundary(AMENDMENT_NOTICE_PERIODS),
            });
            Ok(())
        }
    }

    /// The queries of usage subscriptions (`CTR-QRY-1`, SPEC §8.3). Each answers from one state,
    /// and writes nothing (`REQ-OB-1`).
    #[pallet::view_functions]
    impl<T: Config> Pallet<T> {
        /// An offer: its kind, terms, status, and any amendment pending for its contracts.
        pub fn offer(offer: OfferIdOf<T>) -> Option<OfferInfoOf<T>> {
            Self::offer_info(&offer)
        }

        /// The open offers after `start_after` (from the first when `None`), in ascending id
        /// order. A page examines at most `min(limit, MaxOffersPerPage)` offers, skipping withdrawn
        /// ones; pass its `next` as `start_after` for the following page (`None`: no more). A zero
        /// limit examines nothing and gives `start_after` back as `next`.
        pub fn open_offers(start_after: Option<OfferIdOf<T>>, limit: u32) -> OffersPageOf<T> {
            Self::offers_page(start_after, limit)
        }

        /// A group's contract: state, kind, terms, anchor, `paid through`, next due, grace end,
        /// commitment end, periods charged, and any pending cancellation, switch, conversion or
        /// amendment (with its effective boundary, and whether the free exit is open).
        ///
        /// Between an amendment's effective boundary and the charge that brings it into force,
        /// the terms may show the amended allowance (once its usage window has begun) with the
        /// pre-amendment price.
        pub fn contract(group: GroupOf<T>) -> Option<ContractInfoOf<T>> {
            Self::contract_info(&group)
        }

        /// A group's pool: the allowance, the current window's start and end, its usage and
        /// remainder, and whether it is usable now, with the reason when it is not.
        pub fn pool(group: GroupOf<T>) -> Option<PoolInfo<MomentOf<T>>> {
            Self::pool_info(&group)
        }

        /// An account's resolved paying group, or none, with the reason (`REQ-PC-2`).
        pub fn paying_group(account: T::AccountId) -> PayingGroupResolution<GroupOf<T>> {
            Self::resolve_paying_group(&account)
        }

        /// Whether a group has started a trial (`REQ-OF-7`).
        pub fn trial_used(group: GroupOf<T>) -> bool {
            TrialUsed::<T>::contains_key(group)
        }

        /// A group's transfer policy, as its memberships manager keeps it (`REQ-MI-15`).
        pub fn transfer_policy(group: GroupOf<T>) -> fc_traits_memberships::TransferPolicy {
            <T::Memberships as fc_traits_memberships::Transfer<T::AccountId>>::transfer_policy(
                &group,
            )
        }

        /// Whether a transaction of `account` with metered weight `estimate` would take the pool
        /// path now, or the first condition of admission that fails (`REQ-PL-7`).
        pub fn would_waive(account: T::AccountId, estimate: Weight) -> Waiver<GroupOf<T>> {
            Self::waiver(&account, estimate)
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn integrity_test() {
            assert!(
                !T::MinUsagePeriod::get().is_zero(),
                "`MinUsagePeriod` must be non-zero"
            );
            assert!(
                !T::MinBillingPeriod::get().is_zero(),
                "`MinBillingPeriod` must be non-zero"
            );
            assert!(
                T::MaxTrialPeriods::get() > 0,
                "`MaxTrialPeriods` must be non-zero"
            );
            assert!(
                T::PayingGroupChangeWindow::get() == T::MinUsagePeriod::get(),
                "`PayingGroupChangeWindow` must be `MinUsagePeriod`"
            );
            assert!(
                !T::PayingGroupChangeWindow::get().is_zero(),
                "`PayingGroupChangeWindow` must be non-zero"
            );
            assert!(
                T::MaxOffersPerPage::get() > 0,
                "`MaxOffersPerPage` must be non-zero"
            );
        }
    }
}
