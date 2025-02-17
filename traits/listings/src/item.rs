use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResult;

pub struct Item<Id, Name, AccountId, Price> {
    id: Id,
    name: Name,
    owner: AccountId,
    price: Option<Price>,
}

type IdItemOf<T, Name, AccountId> = (
    (
        <T as Inspect<AccountId>>::InventoryId,
        <T as Inspect<AccountId>>::Id,
    ),
    Item<<T as Inspect<AccountId>>::Id, Name, AccountId, <T as Inspect<AccountId>>::Price>,
);

pub use {Inspect as InspectItem, Mutate as MutateItem};

/// Methods for fetching information about a regular item from an inventory.
pub trait Inspect<AccountId> {
    type InventoryId;
    type Id;
    type Price;

    /// Returns an iterable list of the items published in an inventory.
    fn items(
        inventory_id: Self::InventoryId,
    ) -> impl Iterator<Item = IdItemOf<Self, impl AsRef<[u8]>, AccountId>>;

    /// Returns an iterable list of the items owned by an account.
    fn owned(owner: AccountId)
        -> impl Iterator<Item = IdItemOf<Self, impl AsRef<[u8]>, AccountId>>;

    /// Returns the displayable name for an item.
    fn item(
        inventory_id: Self::InventoryId,
        id: Self::Id,
    ) -> Item<Self::Id, impl AsRef<[u8]>, AccountId, Self::Price>;

    fn attribute<T: Decode>(
        inventory_id: &Self::InventoryId,
        id: &Self::Id,
        key: &impl AsRef<[u8]>,
    ) -> T;
}

pub trait Mutate<AccountId>: Inspect<AccountId> {
    /// Publish a new item in an active inventory.
    fn publish(
        inventory_id: Self::InventoryId,
        id: Self::Id,
        name: impl AsRef<u8>,
        maybe_price: Option<Self::Price>,
    ) -> DispatchResult;

    /// Marks an existing item as whether it can be purchased.
    fn mark_for_sale(
        inventory_id: Self::InventoryId,
        id: Self::Id,
        for_sale: bool,
    ) -> DispatchResult;

    /// Marks an existing item as whether it can be resold.
    fn mark_for_resale(
        inventory_id: Self::InventoryId,
        id: Self::Id,
        for_resale: bool,
    ) -> DispatchResult;

    /// Sets the price fo an existing item
    fn set_price(
        inventory_id: Self::InventoryId,
        id: Self::Id,
        price: Self::Price,
    ) -> DispatchResult;

    fn set_attribute<T: Encode>(
        inventory_id: &Self::InventoryId,
        id: &Self::Id,
        key: &impl AsRef<[u8]>,
        value: T,
    ) -> DispatchResult;
}

pub mod subscriptions {
    use super::*;
    use codec::{Decode, Encode};
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

    ///
    pub trait Inspect<AccountId>: InspectItem<AccountId> {
        type Moment;

        /// Retrieves the [SubscriptionConditions] on an item, if any.
        fn subscription_conditions(
            inventory_id: Self::InventoryId,
            id: Self::Id,
        ) -> Option<SubscriptionConditions<Self::Price, Self::Moment>>;

        /// Retrieves the [Subscription] state for an item, if it has an active subscription.
        fn subscription(
            inventory_id: Self::InventoryId,
            id: Self::Id,
        ) -> Option<Subscription<Self::Price, Self::Moment>>;

        /// If a subscription termination has been disputed, retrieves the [TerminationDispute]
        /// information of such subscription item.
        fn dispute(
            inventory_id: Self::InventoryId,
            id: Self::Id,
        ) -> Option<SubscriptionTermination<AccountId, Self::Price, Self::Moment, impl AsRef<[u8]>>>;
    }

    /// Methods to modify the state of a subscription item.
    pub trait Mutate<AccountId>: Inspect<AccountId> {
        /// Publish a new subscription item in an active inventory.
        fn publish(
            inventory_id: Self::InventoryId,
            id: Self::Id,
            name: impl AsRef<u8>,
            conditions: SubscriptionConditions<Self::Price, Self::Moment>,
        ) -> DispatchResult;

        /// Set the [SubscriptionConditions] on an existing subscription item.
        fn set_conditions(
            inventory_id: Self::InventoryId,
            id: Self::Id,
            conditions: SubscriptionConditions<Self::Price, Self::Moment>,
        ) -> DispatchResult;

        /// Activates a [Subscription], given an item that contains some [SubscriptionConditions].
        fn activate(inventory_id: Self::InventoryId, id: Self::Id) -> DispatchResult;

        /// Cancels a [Subscription].
        fn cancel(inventory_id: Self::InventoryId, id: Self::Id) -> DispatchResult;

        /// Terminates a [Subscription].
        ///
        /// The effects of terminating a subscription
        fn terminate(inventory_id: Self::InventoryId, id: Self::Id) -> DispatchResult;

        fn dispute_termination(
            inventory_id: Self::InventoryId,
            id: Self::Id,
            dispute_reason: impl AsRef<[u8]>,
        ) -> DispatchResult;

        fn resolve_termination_dispute(
            inventory_id: Self::InventoryId,
            id: Self::Id,
            dispute_reason: impl AsRef<[u8]>,
        ) -> DispatchResult;
    }
}
