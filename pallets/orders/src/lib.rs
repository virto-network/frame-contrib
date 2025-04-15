#![cfg_attr(not(feature = "std"), no_std)]

//! # Orders Pallet
//!
//! A component, part of the `Marketplace` subsystem, that handles orders of items existing in a
//! `Listings` component, which can be paid off using the `Payments` component.

#[cfg(feature = "runtime-benchmarks")]
use frame_contrib_traits::listings::InventoryLifecycle;
use frame_contrib_traits::{
    listings::{item::Item, item::ItemPrice, InspectItem, MutateItem},
    payments::{OnPaymentStatusChanged, PaymentInspect, PaymentMutate},
};
use frame_support::pallet_prelude::*;
use frame_support::traits::schedule::{DispatchTime, Priority};
use frame_support::traits::{schedule::v3::Named, Bounded, BoundedInline, Incrementable};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::{Hash, TrailingZeroInput};

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod types;
pub mod weights;

pub use pallet::*;
pub use types::*;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::dispatch::{GetDispatchInfo, PostDispatchInfo};
    use frame_support::traits::{CallerTrait, Incrementable};
    use sp_runtime::traits::Dispatchable;

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        // Primitives: Some overarching types that come from the system (or the system depends on).

        /// The overarching event type.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The caller origin, overarching type of all pallets origins.
        type PalletsOrigin: From<frame_system::RawOrigin<Self::AccountId>>
            + CallerTrait<Self::AccountId>
            + MaxEncodedLen;
        /// The overarching call type.
        type RuntimeCall: Parameter
            + Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
            + GetDispatchInfo
            + From<Call<Self, I>>;
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
        /// The origin admins an order. Returns an `AccountId` representing the origin,
        /// and the maximum amount of items such origin is allowed to have on each cart.
        ///
        /// While the maximum amount of items can be greater than [`MaxItemLen`][Self::MaxItemLen],
        /// this limit will be enforced at all times.
        type OrderAdminOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = (Self::AccountId, u32)>;
        /// The origin to complete a payment. Returns an `AccountId`, representing the account
        /// which will pay for the order.
        type PaymentOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;

        // Types: A set of parameter types that the pallet uses to handle information.

        /// A parameter type to uniquely identify an order in the `Listings` component.
        type OrderId: Parameter + Member + MaxEncodedLen + Default + Incrementable;

        // Dependencies: The external components this pallet depends on.

        /// The `Listings` component of a `Marketplace` system.
        #[cfg(not(feature = "runtime-benchmarks"))]
        type Listings: InspectItem<
                Self::AccountId,
                Asset = PaymentAssetIdOf<Self, I>,
                Balance = PaymentBalanceOf<Self, I>,
            > + MutateItem<Self::AccountId>;
        /// The `Listings` component of a `Marketplace` system.
        #[cfg(feature = "runtime-benchmarks")]
        type Listings: InventoryLifecycle<
                MerchantIdOf<Self::BenchmarkHelper, Self, I>,
                Id = InternalInventoryIdOf<Self::BenchmarkHelper, Self, I>,
                AccountId = Self::AccountId,
            > + InspectItem<
                Self::AccountId,
                Asset = PaymentAssetIdOf<Self, I>,
                Balance = PaymentBalanceOf<Self, I>,
            > + MutateItem<Self::AccountId>;
        /// The `Payments` component of a `Marketplace` system.
        type Payments: PaymentInspect<Self::AccountId> + PaymentMutate<Self::AccountId>;
        /// The `Scheduler` system.
        type Scheduler: Named<
            BlockNumberFor<Self>,
            <Self as Config<I>>::RuntimeCall,
            Self::PalletsOrigin,
        >;

        // Parameters: A set of constant parameters to configure limits.

        /// The time after which an unpaid order in `Checkout` status is automatically cancelled.
        #[pallet::constant]
        type MaxLifetimeForCheckoutOrder: Get<BlockNumberFor<Self>>;
        /// Determines the maximum amount of carts (regardless of origin restrictions) an account
        /// can have.
        #[pallet::constant]
        type MaxCartLen: Get<u32>;
        /// Determines the maximum amount of items (regardless of origin restrictions) a single
        /// order can have.
        #[pallet::constant]
        type MaxItemLen: Get<u32>;

        // Benchmarking: Types to handle benchmarks.

        #[cfg(feature = "runtime-benchmarks")]
        /// A helper trait to set up benchmark tests.
        type BenchmarkHelper: BenchmarkHelper<Self, I>;
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
        OrderCompleted { order_id: T::OrderId },
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
        /// The order is in an invalid state where it cannot be mutated.
        InvalidState,
        /// The specified item is not found in the listings.
        ItemNotFound,
        /// The specified payment is not found in the payments.
        PaymentNotFound,
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
            maybe_items: Option<Vec<CartItemParameterOf<T, I>>>,
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
            items: Vec<CartItemParameterOf<T, I>>,
        ) -> DispatchResult {
            Self::set_order_items(origin, &order_id, items.into_iter())
        }

        #[pallet::call_index(2)]
        pub fn checkout(origin: OriginFor<T>, order_id: T::OrderId) -> DispatchResult {
            let (ref who, _) = T::OrderAdminOrigin::ensure_origin(origin)?;

            Order::<T, I>::try_mutate(order_id.clone(), |order| -> DispatchResult {
                let Some((owner, details)) = order else {
                    return Err(Error::<T, I>::OrderNotFound.into());
                };
                ensure!(
                    details.status == OrderStatus::Cart,
                    Error::<T, I>::InvalidState
                );
                ensure!(who == owner, Error::<T, I>::NoPermission);

                for order_item in details.items.iter_mut() {
                    let item = T::Listings::item(&order_item.inventory_id, &order_item.id)
                        .ok_or(Error::<T, I>::ItemNotFound)?;
                    ensure!(item.price.is_some(), Error::<T, I>::ItemNotForSale);
                    Self::try_lock_item(&order_item.inventory_id, &order_item.id)?;
                }

                details.status = OrderStatus::Checkout;
                Self::remove_cart(who, &order_id)?;

                Self::schedule_cancel(&order_id)?;

                Self::deposit_event(Event::<T, I>::OrderCheckout { order_id });

                Ok(())
            })
        }

        #[pallet::call_index(3)]
        pub fn cancel(origin: OriginFor<T>, order_id: T::OrderId) -> DispatchResult {
            Order::<T, I>::try_mutate(order_id.clone(), |maybe_order| -> DispatchResult {
                let maybe_who = &T::OrderAdminOrigin::ensure_origin_or_root(origin)?;
                let Some((ref owner, details)) = maybe_order else {
                    return Err(Error::<T, I>::OrderNotFound.into());
                };

                if let Some((who, _)) = maybe_who {
                    ensure!(who == owner, Error::<T, I>::NoPermission);
                }
                ensure!(
                    matches!(details.status, OrderStatus::Cart | OrderStatus::Checkout),
                    Error::<T, I>::InvalidState,
                );

                for OrderItem {
                    inventory_id, id, ..
                } in &details.items
                {
                    Self::try_unlock_item(inventory_id, id)?;
                }

                details.status = OrderStatus::Cancelled;

                Self::try_remove_scheduled_cancel(&order_id)?;

                Self::deposit_event(Event::<T, I>::OrderCancelled { order_id });

                Ok(())
            })
        }

        #[pallet::call_index(4)]
        pub fn pay(origin: OriginFor<T>, order_id: T::OrderId) -> DispatchResult {
            let who = &T::PaymentOrigin::ensure_origin(origin)?;

            Order::<T, I>::try_mutate(order_id.clone(), |order| -> DispatchResult {
                let Some((owner, details)) = order else {
                    return Err(Error::<T, I>::OrderNotFound.into());
                };
                ensure!(
                    details.status == OrderStatus::Checkout,
                    Error::<T, I>::InvalidState
                );

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
                        &item.owner,
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

                    Self::transfer_item(&order_item.inventory_id, &order_item.id, &beneficiary)?;
                }

                details.status = OrderStatus::InProgress;

                Self::try_remove_scheduled_cancel(&order_id)?;

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
        ensure!(
            T::Listings::transferable(inventory_id, id),
            Error::<T, I>::ItemAlreadyLocked
        );

        // TODO: Ensure that I can get the address of the original seller before
        // checking whether an item is attempted to be resold.

        // ensure!(
        //     T::Listings::can_resell(inventory_id, id),
        //     Error::<T, I>::ItemAlreadyLocked
        // );

        T::Listings::disable_transfer(inventory_id, id)
    }

    fn try_unlock_item(inventory_id: &InventoryIdOf<T, I>, id: &ItemIdOf<T, I>) -> DispatchResult {
        T::Listings::enable_transfer(inventory_id, id)
    }

    /// Infallibly removes a cart from the user's list of carts if it exists there.
    fn remove_cart(who: &AccountIdOf<T>, order_id: &T::OrderId) -> DispatchResult {
        Cart::<T, I>::try_mutate(who, |carts| {
            if let Some(i) = carts
                .iter()
                .enumerate()
                .find_map(|(i, id)| (id == order_id).then_some(i))
            {
                carts.swap_remove(i);
            }
            Ok(())
        })
    }

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
        Order::<T, I>::try_mutate(order_id, |order_items| {
            let (ref who, max_items) = T::OrderAdminOrigin::ensure_origin(origin)?;
            let Some((owner, details)) = order_items else {
                return Err(Error::<T, I>::OrderNotFound.into());
            };

            ensure!(who == owner, Error::<T, I>::NoPermission);
            ensure!(
                matches!(details.status, OrderStatus::Cart),
                Error::<T, I>::InvalidState
            );

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
            details.items =
                BoundedVec::try_from(items).map_err(|_| Error::<T, I>::MaxItemsExceeded)?;

            Ok(())
        })
    }

    fn schedule_cancel(order_id: &T::OrderId) -> DispatchResult {
        let (id, call) = Self::schedule_cancel_params(order_id);
        T::Scheduler::schedule_named(
            id,
            DispatchTime::After(T::MaxLifetimeForCheckoutOrder::get()),
            None,
            Priority::default(),
            frame_system::RawOrigin::Root.into(),
            Bounded::Inline(BoundedInline::truncate_from(call.encode())),
        )?;

        Ok(())
    }

    fn try_remove_scheduled_cancel(order_id: &T::OrderId) -> DispatchResult {
        let (id, _) = Self::schedule_cancel_params(order_id);
        if T::Scheduler::next_dispatch_time(id).is_ok() {
            T::Scheduler::cancel_named(id)
        } else {
            Ok(())
        }
    }

    fn schedule_cancel_params(order_id: &T::OrderId) -> ([u8; 32], <T as Config<I>>::RuntimeCall) {
        let call = Call::<T, I>::cancel {
            order_id: order_id.clone(),
        }
        .into();
        let hash = T::Hashing::hash_of(&call).using_encoded(|bytes| {
            Decode::decode(&mut TrailingZeroInput::new(bytes))
                .expect("decode from TrailingZeroes onto [u8;32] is safe; qed")
        });

        (hash, call)
    }

    fn update_delivered_items(
        payment_id: &PaymentIdOf<T, I>,
        delivery_status: DeliveryStatus,
    ) -> DispatchResult {
        Payment::<T, I>::try_mutate(payment_id, |payment| {
            let (order_id, inventory_id, id) =
                &payment.clone().ok_or(Error::<T, I>::PaymentNotFound)?;

            Order::<T, I>::try_mutate(order_id.clone(), |maybe_order| {
                let Some((_, details)) = maybe_order else {
                    Err(Error::<T, I>::OrderNotFound)?
                };

                let item = details
                    .items
                    .iter_mut()
                    .find(|order_item| {
                        &order_item.inventory_id == inventory_id && &order_item.id == id
                    })
                    .ok_or(Error::<T, I>::ItemNotFound)?;

                if delivery_status == DeliveryStatus::Cancelled {
                    let payment =
                        T::Payments::details(payment_id).ok_or(Error::<T, I>::OrderNotFound)?;
                    Self::transfer_item(inventory_id, id, payment.beneficiary())?;
                }

                Self::try_unlock_item(inventory_id, id)?;
                item.delivery = Some(delivery_status);

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
                    details.status = OrderStatus::Completed;

                    Self::deposit_event(Event::<T, I>::OrderCompleted {
                        order_id: order_id.clone(),
                    })
                }

                // Remove the payment
                *payment = None;

                Ok(())
            })
        })
    }
}

impl<T: Config<I>, I: 'static> OnPaymentStatusChanged<PaymentIdOf<T, I>, PaymentBalanceOf<T, I>>
    for Pallet<T, I>
{
    fn on_payment_cancelled(payment_id: &PaymentIdOf<T, I>) {
        let _ = Self::update_delivered_items(payment_id, DeliveryStatus::Cancelled);
    }

    fn on_payment_released(
        payment_id: &PaymentIdOf<T, I>,
        _fees: PaymentBalanceOf<T, I>,
        _resulting_amount: PaymentBalanceOf<T, I>,
    ) {
        // This should be infallible.
        let _ = Self::update_delivered_items(payment_id, DeliveryStatus::Delivered);
    }
}
