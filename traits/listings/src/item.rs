use super::*;

use alloc::vec::Vec;
use codec::DecodeWithMemTracking;
use scale_info::TypeInfo;

#[derive(Encode, Decode, PartialEq, Clone, Debug, TypeInfo)]
pub struct Item<AccountId, Asset, Balance> {
    pub name: Vec<u8>,
    pub owner: AccountId,
    pub price: Option<ItemPrice<Asset, Balance>>,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, PartialEq, Clone, Debug, TypeInfo, MaxEncodedLen,
)]
pub struct ItemPrice<Asset, Balance> {
    pub asset: Asset,
    pub amount: Balance,
}

pub type InventoryIdOf<T, AccountId> = (
    <T as Inspect<AccountId>>::MerchantId,
    <T as Inspect<AccountId>>::InventoryId,
);
pub type ItemOf<T, AccountId> =
    Item<AccountId, <T as Inspect<AccountId>>::Asset, <T as Inspect<AccountId>>::Balance>;

pub use {
    Inspect as InspectItem, InspectEnumerable as ItemInspectEnumerable, Mutate as MutateItem,
};

/// Methods for fetching information about a regular item from an inventory.
pub trait Inspect<AccountId> {
    /// A listings merchant.
    type MerchantId: ListingsIdentifier;
    /// A type to uniquely identify an inventory from the same merchant.
    type InventoryId: ListingsIdentifier;
    /// A type to uniquely identify each item within an inventory.
    type ItemId: ListingsIdentifier;
    /// The type to represent an asset class used to set the price of an item.
    type Asset: Parameter + MaxEncodedLen;
    /// The type to represent the amount in the price of an item.
    type Balance: frame_support::traits::tokens::Balance;

    /// Returns the displayable name for an item.
    fn item(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> Option<Item<AccountId, Self::Asset, Self::Balance>>;

    /// Returns the creator of an item, if it exists.
    fn creator(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> Option<AccountId>;

    /// Returns an attribute associated to the item.
    fn attribute<K: Encode, V: Decode>(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        key: &K,
    ) -> Option<V>;

    /// Returns whether an item can be transferred.
    fn transferable(inventory_id: &InventoryIdOf<Self, AccountId>, id: &Self::ItemId) -> bool;

    /// Returns whether an item is available for resale.
    fn can_resell(inventory_id: &InventoryIdOf<Self, AccountId>, id: &Self::ItemId) -> bool;
}

/// Methods to fetching lists of items.
pub trait InspectEnumerable<AccountId>: Inspect<AccountId> {
    /// Returns an iterable list of the items published in an inventory.
    fn items(
        inventory_id: &InventoryIdOf<Self, AccountId>,
    ) -> impl Iterator<Item = (Self::ItemId, ItemOf<Self, AccountId>)>;

    /// Returns an iterable list of the items owned by an account.
    fn owned(
        owner: &AccountId,
    ) -> impl Iterator<
        Item = (
            impl Into<InventoryIdOf<Self, AccountId>>,
            Self::ItemId,
            ItemOf<Self, AccountId>,
        ),
    >;
}

pub trait Mutate<AccountId>: Inspect<AccountId> {
    /// Publish a new item in an active inventory.
    fn publish(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        name: Vec<u8>,
        maybe_price: Option<ItemPrice<Self::Asset, Self::Balance>>,
    ) -> DispatchResult;

    /// Enables an existing item to be resold.
    fn enable_resell(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult;

    /// Disables an existing item to be resold.
    fn disable_resell(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult;

    /// Marks an existing item as transferable
    fn enable_transfer(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult;

    /// Marks an existing item as non-transferable
    fn disable_transfer(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult;

    /// Forcefully transfers an item, even though is disabled for transfer.
    fn transfer(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        beneficiary: &AccountId,
    ) -> DispatchResult;

    /// Transfers an item, marking the beneficiary as the item creator.
    fn creator_transfer(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        beneficiary: &AccountId,
    ) -> DispatchResult;

    /// Sets the price on an existing item.
    fn set_price(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        price: ItemPrice<Self::Asset, Self::Balance>,
    ) -> DispatchResult;

    /// Clears the price on an existing item.
    fn clear_price(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
    ) -> DispatchResult;

    /// Sets an arbitrary attribute on an existing item.
    fn set_attribute<K: Encode, V: Encode>(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
        key: &K,
        value: V,
    ) -> DispatchResult;

    /// Clears an arbitrary attribute on an existing item.
    fn clear_attribute<K: Encode>(
        inventory_id: &InventoryIdOf<Self, AccountId>,
        id: &Self::ItemId,
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

    type MomentOf<T, AccountId> = <T as Inspect<AccountId>>::Moment;
    type ItemPriceOf<T, AccountId> =
        ItemPrice<<T as InspectItem<AccountId>>::Asset, <T as InspectItem<AccountId>>::Balance>;
    type SubscriptionConditionsOf<T, AccountId> =
        SubscriptionConditions<ItemPriceOf<T, AccountId>, MomentOf<T, AccountId>>;
    type SubscriptionOf<T, AccountId> =
        Subscription<ItemPriceOf<T, AccountId>, MomentOf<T, AccountId>>;
    type SubscriptionTerminationOf<T, AccountId, Reason> = SubscriptionTermination<
        AccountId,
        ItemPriceOf<T, AccountId>,
        MomentOf<T, AccountId>,
        Reason,
    >;

    pub trait Inspect<AccountId>: InspectItem<AccountId> {
        type Moment;

        /// Retrieves the [SubscriptionConditions] on an item, if any.
        fn subscription_conditions(
            inventory_id: &InventoryIdOf<Self, AccountId>,
            id: &Self::ItemId,
        ) -> Option<SubscriptionConditionsOf<Self, AccountId>>;

        /// Retrieves the [Subscription] state for an item, if it has an active subscription.
        fn subscription(
            inventory_id: &InventoryIdOf<Self, AccountId>,
            id: &Self::ItemId,
        ) -> Option<SubscriptionOf<Self, AccountId>>;

        /// If a subscription termination has been disputed, retrieves the [TerminationDispute]
        /// information of such subscription item.
        fn dispute<Reason: Encode>(
            inventory_id: &InventoryIdOf<Self, AccountId>,
            id: &Self::ItemId,
        ) -> Option<SubscriptionTerminationOf<Self, AccountId, Reason>>;
    }

    /// Methods to modify the state of a subscription item.
    pub trait Mutate<AccountId>: Inspect<AccountId> {
        /// Publish a new subscription item in an active inventory.
        fn publish<Reason: Encode>(
            inventory_id: &InventoryIdOf<Self, AccountId>,
            id: &Self::ItemId,
            name: Reason,
            conditions: SubscriptionConditions<ItemPrice<Self::Asset, Self::Balance>, Self::Moment>,
        ) -> DispatchResult;

        /// Set the [SubscriptionConditions] on an existing subscription item.
        fn set_conditions(
            inventory_id: &InventoryIdOf<Self, AccountId>,
            id: &Self::ItemId,
            conditions: SubscriptionConditions<ItemPrice<Self::Asset, Self::Balance>, Self::Moment>,
        ) -> DispatchResult;

        /// Activates a [Subscription], given an item that contains some [SubscriptionConditions].
        fn activate(
            inventory_id: &InventoryIdOf<Self, AccountId>,
            id: &Self::ItemId,
        ) -> DispatchResult;

        /// Cancels a [Subscription].
        fn cancel(
            inventory_id: &InventoryIdOf<Self, AccountId>,
            id: &Self::ItemId,
        ) -> DispatchResult;

        /// Terminates a [Subscription].
        ///
        /// The effects of terminating a subscription
        fn terminate(
            inventory_id: &InventoryIdOf<Self, AccountId>,
            id: &Self::ItemId,
        ) -> DispatchResult;

        fn dispute_termination<Reason: Encode>(
            inventory_id: &InventoryIdOf<Self, AccountId>,
            id: &Self::ItemId,
            dispute_reason: Reason,
        ) -> DispatchResult;

        fn resolve_termination_dispute<Reason: Encode>(
            inventory_id: &InventoryIdOf<Self, AccountId>,
            id: &Self::ItemId,
            dispute_reason: Reason,
        ) -> DispatchResult;
    }
}
