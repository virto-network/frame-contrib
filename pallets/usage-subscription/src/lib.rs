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

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod functions;
mod hooks;
mod types;
pub mod weights;

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
        /// administer [`Config::OfferInventory`].
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
            // Only a trial converts, and there are no trials yet.
            ensure!(converts_into.is_none(), Error::<T>::InvalidConversion);

            Self::start_contract(group, &account, offer, &target, now)
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

            <T::Subscriptions as subs::Mutate<T::AccountId>>::schedule_replacement(
                &inventory,
                &contract.offer,
                &account,
                &offer,
            )
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
        }
    }
}
