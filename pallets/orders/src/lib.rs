#![cfg_attr(not(feature = "std"), no_std)]

//! # Orders Pallet
//!
//! A component, part of the `Marketplace` subsystem, that handles orders of items existing in a
//! `Listings` component, which can be paid off using the `Payments` component.

use frame_contrib_traits::{
    listings::{item::Item, item::ItemPrice, InspectItem, MutateItem},
    payments::{OnPaymentStatusChanged, PaymentInspect, PaymentMutate},
};
use frame_support::pallet_prelude::*;
use frame_support::traits::Incrementable;
use frame_system::pallet_prelude::*;

// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::*;

pub use pallet::*;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type InventoryIdOf<T, I = ()> =
    <<T as Config<I>>::Listings as InspectItem<AccountIdOf<T>>>::InventoryId;
type ItemIdOf<T, I = ()> = <<T as Config<I>>::Listings as InspectItem<AccountIdOf<T>>>::Id;
type PaymentIdOf<T, I = ()> = <<T as Config<I>>::Payments as PaymentInspect<AccountIdOf<T>>>::Id;
type PaymentAssetIdOf<T, I = ()> =
    <<T as Config<I>>::Payments as PaymentInspect<AccountIdOf<T>>>::AssetId;
type PaymentBalanceOf<T, I = ()> =
    <<T as Config<I>>::Payments as PaymentInspect<AccountIdOf<T>>>::Balance;
type OrderDetailsOf<T, I = ()> = OrderDetails<
    AccountIdOf<T>,
    InventoryIdOf<T, I>,
    ItemIdOf<T, I>,
    PaymentIdOf<T, I>,
    <T as Config<I>>::MaxItemLen,
>;

#[derive(Clone, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(MaxItemLen))]
pub struct OrderDetails<AccountId, InventoryId, ItemId, PaymentId, MaxItemLen: Get<u32>> {
    status: OrderStatus,
    items: BoundedVec<OrderItem<AccountId, InventoryId, ItemId, PaymentId>, MaxItemLen>,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub enum OrderStatus {
    /// The list of items is editable. A buyer can add or remove items, and they'll be locked once
    /// added to the list, meaning there are some restrictions for the item (i.e. cannot transfer or
    /// resell the item).
    Cart,
    /// The order is now ready to be paid. The list of items is no longer editable.
    Checkout,
    /// The order is cancelled. The list of items is now empty, and items are unlocked, ready to be
    /// acquired by another buyer.
    Cancelled,
    /// The order is paid. One or more items in the order haven't been fully processed (i.e. not yet
    /// transferred to the beneficiary, or the funds haven't been released by the seller).
    ///
    /// In this state, items are owned by the buyer (or beneficiaries if set), and are still locked,
    /// meaning the funds need to be released by the seller, or some time needs to be elapsed,
    /// before the items can be unlocked.
    InProgress,
    /// In this state, every item has been processed, meaning every item should be unlocked.
    Delivered,
}

#[derive(Clone, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct OrderItem<AccountId, InventoryId, ItemId, PaymentId> {
    id: ItemId,
    inventory_id: InventoryId,
    seller: AccountId,
    beneficiary: Option<AccountId>,
    payment_id: Option<PaymentId>,
    delivery: Option<()>,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::traits::Incrementable;

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        // Primitives: Some overarching types that come from the system (or the system depends on).

        /// The overarching event type.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The weight information for this pallet.
        type WeightInfo: WeightInfo;

        // Origins: Types that manage authorization rules to allow or deny some caller origins to
        // execute a method.

        /// The origin to create an order. Returns an `AccountId` representing the origin,
        /// and the maximum amount of carts such origin is allowed to have simultaneously.
        ///
        /// While the maximum amount of carts can be greater than [`MaxCartLen`][Self::MaxCartLen],
        /// this limit will be enforced at all times.
        type CreateOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = (Self::AccountId, u32)>;
        /// The origin set items into an order. Returns an `AccountId` representing the origin,
        /// and the maximum amount of items such origin is allowed to have on each cart.
        ///
        /// While the maximum amount of items can be greater than [`MaxItemLen`][Self::MaxItemLen],
        /// this limit will be enforced at all times.
        type SetItemsOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = (Self::AccountId, u32)>;

        // Types: A set of parameter types that the pallet uses to handle information.

        /// A parameter type to uniquely identify an order in the `Listings` component.
        type OrderId: Parameter + Member + MaxEncodedLen + Default + Incrementable;

        // Dependencies: The external components this pallet depends on.

        /// The `Listings` component of a `Marketplace` system.
        type Listings: InspectItem<
                Self::AccountId,
                Asset = PaymentAssetIdOf<Self, I>,
                Balance = PaymentBalanceOf<Self, I>,
            > + MutateItem<Self::AccountId>;
        /// The `Payments` component of a `Marketplace` system.
        type Payments: PaymentInspect<Self::AccountId> + PaymentMutate<Self::AccountId>;

        // Parameters: A set of constant parameters to configure limits.

        /// Determines the maximum amount of carts (regardless of origin restrictions) an account
        /// can have.
        #[pallet::constant]
        type MaxCartLen: Get<u32>;
        /// Determines the maximum amount of items (regardless of origin restrictions) a single
        /// order can have.
        #[pallet::constant]
        type MaxItemLen: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// An order cart has been created.
        CartCreated {
            owner: T::AccountId,
            order_id: T::OrderId,
        },
        /// An order has been checked out, and is ready for payment.
        OrderCheckout { order_id: T::OrderId },
        /// An order has been cancelled, and items released.
        OrderCancelled { order_id: T::OrderId },
        /// An order has been paid, and is in progress.
        OrderInProgress { order_id: T::OrderId },
        /// An item in an order has been delivered, and is now fully usable.
        ItemDelivered {
            order_id: T::OrderId,
            inventory_id: InventoryIdOf<T, I>,
            id: ItemIdOf<T, I>,
        },
        /// An order has been fully delivered.
        OrderDelivered { order_id: T::OrderId },
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// It's not possible to add a new cart because the maximum amount of carts for either the
        /// account or the system overall has been exceeded.
        MaxCartsExceeded,
        /// It's not possible to add a new item to the order because the maximum amount of items for
        /// either the account or the system overall has been exceeded.
        MaxItemsExceeded,
        /// The specified order is not found.
        OrderNotFound,
        /// The order is in an invalid state where it cannot be cancelled.
        InvalidState,
        /// The specified item is not found in the listings.
        ItemNotFound,
        /// The specified item is not for sale.
        ItemNotForSale,
        /// The specified item is already locked.
        ItemAlreadyLocked,
        /// The specified item is already unlocked, and out of control from this pallet.
        ItemAlreadyUnlocked,
        /// The upper bound of the [`T::OrderId`] has been reached, and it's not possible to
        /// generate the [`NextOrderid`]
        CannotIncrementOrderId,
        /// The origin does not have permissions to mutate this order.
        NoPermission,
    }

    #[pallet::storage]
    pub type Cart<T: Config<I>, I: 'static = ()> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<T::OrderId, T::MaxCartLen>,
        ValueQuery,
    >;

    #[pallet::storage]
    pub type Order<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, T::OrderId, (T::AccountId, OrderDetailsOf<T, I>)>;

    #[pallet::storage]
    pub type NextOrderId<T: Config<I>, I: 'static = ()> = StorageValue<_, T::OrderId, ValueQuery>;

    #[pallet::storage]
    pub type Payment<T: Config<I>, I: 'static = ()> = StorageMap<
        _,
        Blake2_128Concat,
        PaymentIdOf<T, I>,
        (T::OrderId, InventoryIdOf<T, I>, ItemIdOf<T, I>),
    >;

    #[pallet::call(weight(<T as Config<I>>::WeightInfo))]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Creates a new cart.
        #[pallet::call_index(0)]
        pub fn create_cart(
            origin: OriginFor<T>,
            maybe_items: Option<Vec<(InventoryIdOf<T, I>, ItemIdOf<T, I>, Option<T::AccountId>)>>,
        ) -> DispatchResult {
            let (owner, max_carts) = T::CreateOrigin::ensure_origin(origin.clone())?;

            Cart::<T, I>::try_mutate(owner.clone(), |carts| {
                ensure!(
                    (carts.len() as u32) < max_carts,
                    Error::<T, I>::MaxCartsExceeded
                );

                let order_id = Self::generate_order_id()?;

                Order::<T, I>::insert(
                    order_id.clone(),
                    (
                        owner.clone(),
                        OrderDetails {
                            status: OrderStatus::Cart,
                            items: BoundedVec::new(),
                        },
                    ),
                );

                carts
                    .try_push(order_id.clone())
                    .map_err(|_| Error::<T, I>::MaxCartsExceeded)?;

                Self::deposit_event(Event::CartCreated {
                    owner,
                    order_id: order_id.clone(),
                });

                Self::set_order_items(
                    origin,
                    &order_id,
                    maybe_items.unwrap_or_default().into_iter(),
                )
            })
        }

        #[pallet::call_index(1)]
        pub fn set_cart_items(
            origin: OriginFor<T>,
            order_id: T::OrderId,
            items: Vec<(InventoryIdOf<T, I>, ItemIdOf<T, I>, Option<T::AccountId>)>,
        ) -> DispatchResult {
            Self::set_order_items(origin, &order_id, items.into_iter())
        }

        #[pallet::call_index(2)]
        pub fn checkout(origin: OriginFor<T>, order_id: T::OrderId) -> DispatchResult {
            let ref who = ensure_signed(origin)?;

            Order::<T, I>::try_mutate(order_id.clone(), |order| -> DispatchResult {
                let Some((owner, details)) = order else {
                    return Err(Error::<T, I>::OrderNotFound.into());
                };
                ensure!(who == owner, Error::<T, I>::NoPermission);

                for item in details.items.iter_mut() {
                    Self::try_lock_item(&item.inventory_id, &item.id)?;
                }

                details.status = OrderStatus::Checkout;
                Self::remove_cart(who, &order_id)?;

                // TODO: Schedule cancellation (and items' release) task

                Self::deposit_event(Event::<T, I>::OrderCheckout { order_id });

                Ok(())
            })
        }

        #[pallet::call_index(3)]
        pub fn cancel(origin: OriginFor<T>, order_id: T::OrderId) -> DispatchResult {
            let ref maybe_who = ensure_signed_or_root(origin)?;
            let (ref owner, details) =
                Order::<T, I>::get(&order_id).ok_or(Error::<T, I>::OrderNotFound)?;

            if let Some(who) = maybe_who {
                ensure!(who == owner, Error::<T, I>::NoPermission);
            }
            ensure!(
                matches!(details.status, OrderStatus::Cart | OrderStatus::Checkout),
                Error::<T, I>::InvalidState,
            );

            // TODO: Remove scheduled cancellation task.

            Self::deposit_event(Event::<T, I>::OrderCancelled { order_id });

            Ok(())
        }

        #[pallet::call_index(4)]
        pub fn pay(origin: OriginFor<T>, order_id: T::OrderId) -> DispatchResult {
            let ref who = ensure_signed(origin)?;

            Order::<T, I>::try_mutate(order_id.clone(), |order| -> DispatchResult {
                let Some((owner, details)) = order else {
                    return Err(Error::<T, I>::OrderNotFound.into());
                };

                for order_item in details.items.iter_mut() {
                    let beneficiary = order_item.beneficiary.clone().unwrap_or(owner.clone());

                    let item = T::Listings::item(&order_item.inventory_id, &order_item.id)
                        .ok_or(Error::<T, I>::ItemNotFound)?;
                    let Some(ItemPrice { asset, amount }) = item.price else {
                        return Err(Error::<T, I>::ItemNotForSale.into());
                    };

                    let payment_id = T::Payments::create(
                        &who.clone(),
                        asset,
                        amount,
                        &beneficiary,
                        Some(order_item.clone()),
                    )?;

                    order_item.payment_id = Some(payment_id.clone());

                    Payment::<T, I>::insert(
                        payment_id,
                        (
                            order_id.clone(),
                            order_item.inventory_id.clone(),
                            order_item.id.clone(),
                        ),
                    );
                }

                details.status = OrderStatus::InProgress;

                // TODO: Remove scheduled cancellation task.

                Self::deposit_event(Event::<T, I>::OrderInProgress { order_id });

                Ok(())
            })
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    fn generate_order_id() -> Result<T::OrderId, DispatchError> {
        NextOrderId::<T, I>::try_mutate(|order_id| {
            let current_order_id = order_id.clone();
            *order_id = order_id
                .increment()
                .ok_or(Error::<T, I>::CannotIncrementOrderId)?;

            Ok(current_order_id)
        })
    }

    fn try_lock_item(inventory_id: &InventoryIdOf<T, I>, id: &ItemIdOf<T, I>) -> DispatchResult {
        let item = T::Listings::item(inventory_id, id).ok_or(Error::<T, I>::ItemNotFound)?;
        ensure!(item.price.is_some(), Error::<T, I>::ItemNotForSale,);
        ensure!(
            T::Listings::transferable(inventory_id, id),
            Error::<T, I>::ItemAlreadyLocked
        );
        ensure!(
            T::Listings::can_resell(inventory_id, id),
            Error::<T, I>::ItemAlreadyLocked
        );

        T::Listings::mark_can_transfer(inventory_id, id, false)?;
        T::Listings::disable_resell(inventory_id, id)?;
        Ok(())
    }

    fn try_unlock_item(inventory_id: &InventoryIdOf<T, I>, id: &ItemIdOf<T, I>) -> DispatchResult {
        ensure!(
            !T::Listings::transferable(inventory_id, id),
            Error::<T, I>::ItemAlreadyUnlocked
        );
        ensure!(
            !T::Listings::can_resell(inventory_id, id),
            Error::<T, I>::ItemAlreadyUnlocked
        );

        T::Listings::mark_can_transfer(inventory_id, id, true)?;
        T::Listings::enable_resell(inventory_id, id)?;
        Ok(())
    }

    /// Infallibly removes a cart from the user's list of carts if it exists there.
    fn remove_cart(who: &AccountIdOf<T>, order_id: &T::OrderId) -> DispatchResult {
        Cart::<T, I>::try_mutate(who, |carts| {
            if let Some(i) = carts
                .iter()
                .enumerate()
                .find_map(|(i, id)| (id == order_id).then(|| i))
            {
                carts.swap_remove(i);
            }
            Ok(())
        })
    }

    #[allow(dead_code)]
    fn transfer_item(
        inventory_id: &InventoryIdOf<T, I>,
        id: &ItemIdOf<T, I>,
        beneficiary: &T::AccountId,
    ) -> DispatchResult {
        ensure!(
            !T::Listings::transferable(inventory_id, id),
            Error::<T, I>::ItemAlreadyUnlocked
        );

        T::Listings::transfer(inventory_id, id, beneficiary)
    }

    fn set_order_items(
        origin: OriginFor<T>,
        order_id: &T::OrderId,
        items: impl Iterator<Item = (InventoryIdOf<T, I>, ItemIdOf<T, I>, Option<T::AccountId>)>,
    ) -> Result<(), DispatchError> {
        let (ref who, max_items) = T::SetItemsOrigin::ensure_origin(origin)?;

        let items = items
            .map(|(inventory_id, id, beneficiary)| {
                let Item { owner, .. } =
                    T::Listings::item(&inventory_id, &id).ok_or(Error::<T, I>::ItemNotFound)?;

                Ok(OrderItem {
                    inventory_id,
                    id,
                    seller: owner,
                    beneficiary,
                    payment_id: None,
                    delivery: None,
                })
            })
            .collect::<Result<Vec<_>, DispatchError>>()?;

        ensure!(
            items.len() <= max_items as usize,
            Error::<T, I>::MaxItemsExceeded
        );

        let items = BoundedVec::try_from(items).map_err(|_| Error::<T, I>::MaxItemsExceeded)?;

        Order::<T, I>::try_mutate(order_id, |order_items| {
            let Some((owner, details)) = order_items else {
                return Err(Error::<T, I>::OrderNotFound.into());
            };

            ensure!(who == owner, Error::<T, I>::NoPermission);

            details.items = items;

            Ok(())
        })
    }
}

impl<T: Config<I>, I: 'static> OnPaymentStatusChanged<PaymentIdOf<T, I>, PaymentBalanceOf<T, I>>
    for Pallet<T, I>
{
    fn on_payment_released(
        payment_id: &PaymentIdOf<T, I>,
        _fees: PaymentBalanceOf<T, I>,
        _resulting_amount: PaymentBalanceOf<T, I>,
    ) {
        // This should be infallible.
        let _: Result<_, DispatchError> =
            Payment::<T, I>::try_mutate(payment_id, |maybe_order_item| {
                if let Some((order_id, inventory_id, id)) = maybe_order_item {
                    return Order::<T, I>::try_mutate(order_id.clone(), |maybe_order| {
                        let Some((_, details)) = maybe_order else {
                            return Err(Error::<T, I>::OrderNotFound.into());
                        };

                        let item = details
                            .items
                            .iter_mut()
                            .find(|order_item| {
                                &order_item.inventory_id == inventory_id && &order_item.id == id
                            })
                            .ok_or(Error::<T, I>::ItemNotFound)?;

                        // Unlocking an item should be infallible.
                        let _ = Self::try_unlock_item(inventory_id, id);
                        item.delivery = Some(());

                        // Clear the payment
                        Payment::<T, I>::remove(payment_id);

                        let delivered_items = details
                            .items
                            .iter()
                            .filter(|order_item| order_item.delivery.is_some())
                            .count();

                        Self::deposit_event(Event::<T, I>::ItemDelivered {
                            order_id: order_id.clone(),
                            inventory_id: inventory_id.clone(),
                            id: id.clone(),
                        });

                        if delivered_items == details.items.len() {
                            details.status = OrderStatus::Delivered;

                            Self::deposit_event(Event::<T, I>::OrderDelivered {
                                order_id: order_id.clone(),
                            })
                        }

                        Ok(())
                    });
                }

                Ok(())
            });
    }
}
