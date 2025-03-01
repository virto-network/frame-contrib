use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResult;
use scale_info::TypeInfo;

#[derive(Encode, Decode, PartialEq, Clone, Debug, TypeInfo)]
pub struct Item<AccountId, Price> {
    pub name: Vec<u8>,
    pub owner: AccountId,
    pub price: Option<Price>,
}

pub type IdItemOf<T, AccountId> = (
    (
        <T as Inspect<AccountId>>::InventoryId,
        <T as Inspect<AccountId>>::Id,
    ),
    Item<AccountId, <T as Inspect<AccountId>>::Price>,
);

pub use {Inspect as InspectItem, Mutate as MutateItem};

/// Methods for fetching information about a regular item from an inventory.
pub trait Inspect<AccountId> {
    type InventoryId;
    type Id;
    type Price;

    /// Returns an iterable list of the items published in an inventory.
    fn items(inventory_id: &Self::InventoryId) -> impl Iterator<Item = IdItemOf<Self, AccountId>>;

    /// Returns an iterable list of the items owned by an account.
    fn owned(owner: &AccountId) -> impl Iterator<Item = IdItemOf<Self, AccountId>>;

    /// Returns the displayable name for an item.
    fn item(
        inventory_id: &Self::InventoryId,
        id: &Self::Id,
    ) -> Option<Item<AccountId, Self::Price>>;

    /// Returns an attribute associated to the item.
    fn attribute<K: Encode, V: Decode>(
        inventory_id: &Self::InventoryId,
        id: &Self::Id,
        key: &K,
    ) -> Option<V>;

    /// Returns whether an item can be transferred.
    fn transferable(inventory_id: &Self::InventoryId, id: &Self::Id) -> bool;

    /// Returns whether an item is available for resale.
    fn can_resell(inventory_id: &Self::InventoryId, id: &Self::Id) -> bool;
}

pub trait Mutate<AccountId>: Inspect<AccountId> {
    /// Publish a new item in an active inventory.
    fn publish(
        inventory_id: &Self::InventoryId,
        id: &Self::Id,
        name: Vec<u8>,
        maybe_price: Option<Self::Price>,
    ) -> DispatchResult;

    /// Marks an existing item as whether it cannot be resold.
    fn mark_not_for_resale(
        inventory_id: &Self::InventoryId,
        id: &Self::Id,
        not_for_resale: bool,
    ) -> DispatchResult;

    /// Marks an existing item as non-transferable
    fn mark_can_transfer(
        inventory_id: &Self::InventoryId,
        id: &Self::Id,
        can_tranfer: bool,
    ) -> DispatchResult;

    /// Sets the price on an existing item.
    fn set_price(
        inventory_id: &Self::InventoryId,
        id: &Self::Id,
        price: Self::Price,
    ) -> DispatchResult;

    /// Sets an arbitrary attribute on an existing item.
    fn set_attribute<K: Encode, V: Encode>(
        inventory_id: &Self::InventoryId,
        id: &Self::Id,
        key: &K,
        value: V,
    ) -> DispatchResult;

    /// Clears an arbitrary attribute on an existing item.
    fn clear_attribute<K: Encode>(
        inventory_id: &Self::InventoryId,
        id: &Self::Id,
        key: &K,
    ) -> DispatchResult;
}

pub mod subscriptions {
    use super::*;
    use frame_support::pallet_prelude::DispatchError;

    pub struct SubscriptionConditions<Price, Moment> {
        pub price: Price,
        pub period: Moment,
    }

    pub struct Subscription<Price, Moment> {
        pub price: Price,
        pub next_renewal: Moment,
    }

    #[derive(Encode, Decode)]
    pub struct SubscriptionTermination<AccountId, Price, Moment, Reason> {
        pub when: Moment,
        pub reason: Reason,
        pub subscription: Subscription<Price, Moment>,
        pub maybe_dispute: Option<TerminationDispute<AccountId, Reason>>,
    }

    /// A submitted dispute over a termination.
    #[derive(Encode, Decode)]
    pub struct TerminationDispute<AccountId, Reason> {
        pub reason: Reason,
        pub state: DisputeState<AccountId, Reason>,
    }

    /// The state of a dispute.
    #[derive(Default)]
    pub enum DisputeState<AccountId, Reason> {
        #[default]
        /// A dispute process for the termination has been submitted.
        Submitted,
        /// The dispute has been assigned to a judge. A judge can be any actor in the system in
        /// charge of reviewing a dispute and issuing a resolution (accepting or rejecting it).
        Assigned(AccountId),
        /// The dispute is being reviewed. This process can take any amount of time (or even be
        /// skipped at all if the judge resolves immediately upon being assigned with the dispute).
        InReview(AccountId),
        /// The dispute is resolved in favour to the submitter by a judge. A reason must be given.
        Accepted(AccountId, Reason),
        /// The dispute is resolved in favour to the terminator by a judge. A reason must be given.
        Rejected(AccountId, Reason),
    }

    impl<AccountId: Clone, Reason> TerminationDispute<AccountId, Reason> {
        /// Initializes a new dispute.
        pub fn new(reason: Reason) -> Self {
            Self {
                reason,
                state: Default::default(),
            }
        }

        /// Assigns a dispute to a judge.
        pub fn assign(&mut self, judge: AccountId) -> DispatchResult {
            match self.state {
                DisputeState::Submitted => {
                    self.state = DisputeState::Assigned(judge);
                    Ok(())
                }
                _ => Err(DispatchError::Other("Invalid state")),
            }
        }

        /// Bumps the dispute towards an [DisputeState::InReview] state.
        pub fn start_review(&mut self) -> DispatchResult {
            match &self.state {
                DisputeState::Assigned(judge) => {
                    self.state = DisputeState::InReview(judge.clone());
                    Ok(())
                }
                _ => Err(DispatchError::Other("Invalid state")),
            }
        }

        pub fn approve(&mut self, reason: Reason) -> DispatchResult {
            match &self.state {
                DisputeState::Assigned(judge) | DisputeState::InReview(judge) => {
                    self.state = DisputeState::Accepted(judge.clone(), reason);
                    Ok(())
                }
                _ => Err(DispatchError::Other("Invalid state")),
            }
        }

        pub fn reject(&mut self, reason: Reason) -> DispatchResult {
            match &self.state {
                DisputeState::Assigned(judge) | DisputeState::InReview(judge) => {
                    self.state = DisputeState::Rejected(judge.clone(), reason);
                    Ok(())
                }
                _ => Err(DispatchError::Other("Invalid state")),
            }
        }
    }

    pub use {Inspect as InspectSubscription, Mutate as MutateSubscription};

    pub trait Inspect<AccountId>: InspectItem<AccountId> {
        type Moment;

        /// Retrieves the [SubscriptionConditions] on an item, if any.
        fn subscription_conditions(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
        ) -> Option<SubscriptionConditions<Self::Price, Self::Moment>>;

        /// Retrieves the [Subscription] state for an item, if it has an active subscription.
        fn subscription(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
        ) -> Option<Subscription<Self::Price, Self::Moment>>;

        /// If a subscription termination has been disputed, retrieves the [TerminationDispute]
        /// information of such subscription item.
        fn dispute<Reason: AsRef<[u8]>>(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
        ) -> Option<SubscriptionTermination<AccountId, Self::Price, Self::Moment, Reason>>;
    }

    /// Methods to modify the state of a subscription item.
    pub trait Mutate<AccountId>: Inspect<AccountId> {
        /// Publish a new subscription item in an active inventory.
        fn publish<Reason: AsRef<[u8]>>(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            name: Reason,
            conditions: SubscriptionConditions<Self::Price, Self::Moment>,
        ) -> DispatchResult;

        /// Set the [SubscriptionConditions] on an existing subscription item.
        fn set_conditions(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            conditions: SubscriptionConditions<Self::Price, Self::Moment>,
        ) -> DispatchResult;

        /// Activates a [Subscription], given an item that contains some [SubscriptionConditions].
        fn activate(inventory_id: &Self::InventoryId, id: &Self::Id) -> DispatchResult;

        /// Cancels a [Subscription].
        fn cancel(inventory_id: &Self::InventoryId, id: &Self::Id) -> DispatchResult;

        /// Terminates a [Subscription].
        ///
        /// The effects of terminating a subscription
        fn terminate(inventory_id: &Self::InventoryId, id: &Self::Id) -> DispatchResult;

        fn dispute_termination<Reason: AsRef<[u8]>>(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            dispute_reason: Reason,
        ) -> DispatchResult;

        fn resolve_termination_dispute<Reason: AsRef<[u8]>>(
            inventory_id: &Self::InventoryId,
            id: &Self::Id,
            dispute_reason: Reason,
        ) -> DispatchResult;
    }
}
