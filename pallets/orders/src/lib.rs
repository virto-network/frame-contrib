#![cfg_attr(not(feature = "std"), no_std)]

//! # Orders Pallet
//!
//! A component, part of the `Marketplace` subsystem, that handles orders of items existing in a
//! `Listings` component, which can be paid off using the `Payments` component.

use frame_contrib_traits::{
	listings::{item::Item, InspectItem, MutateItem},
	payments::{OnPaymentStatusChanged, PaymentInspect, PaymentMutate},
};
use frame_support::pallet_prelude::*;
use frame_support::traits::Incrementable;
use frame_system::pallet_prelude::*;

// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;
//
// #[cfg(test)]
// mod mock;
// #[cfg(test)]
// mod tests;

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
type OrderDetailsOf<T, I = ()> =
OrderDetails<AccountIdOf<T>, InventoryIdOf<T, I>, ItemIdOf<T, I>, <T as Config<I>>::MaxItemLen>;
type OrderItemOf<T, I = ()> = OrderItem<AccountIdOf<T>, InventoryIdOf<T, I>, ItemIdOf<T, I>>;

#[derive(Clone, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct OrderDetails<AccountId, InventoryId, ItemId, MaxItemLen: Get<u32>> {
	status: OrderStatus,
	items: BoundedVec<OrderItem<AccountId, InventoryId, ItemId>, MaxItemLen>,
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
pub struct OrderItem<AccountId, InventoryId, ItemId> {
	seller: AccountId,
	beneficiary: Option<AccountId>,
	inventory_id: InventoryId,
	id: ItemId,
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
		type CreateOrigin: EnsureOrigin<Self::RuntimeOrigin, Success=(Self::AccountId, u32)>;
		/// The origin set items into an order. Returns an `AccountId` representing the origin,
		/// and the maximum amount of items such origin is allowed to have on each cart.
		///
		/// While the maximum amount of items can be greater than [`MaxItemLen`][Self::MaxItemLen],
		/// this limit will be enforced at all times.
		type SetItemsOrigin: EnsureOrigin<Self::RuntimeOrigin, Success=(Self::AccountId, u32)>;

		// Types: A set of parameter types that the pallet uses to handle information.

		/// A parameter type to uniquely identify an order in the `Listings` component.
		type OrderId: Parameter + Member + MaxEncodedLen + Default + Incrementable;

		// Dependencies: The external components this pallet depends on.

		/// The `Listings` component of a `Marketplace` system.
		type Listings: InspectItem<Self::AccountId> + MutateItem<Self::AccountId>;
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
		type MaxItemLen: Parameter + Get<u32>;
	}

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		CartCreated {
			owner: T::AccountId,
			order_id: T::OrderId,
		},
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// It's not possible to add a new cart because the maximum amount of carts for either the
		/// account or the system overall has been exceeded.
		MaxCartsExceeded,
		/// It's not possible to add a new item to the order because the maximum amount of items for
		/// either the account or the system overall has been exceeded.
		MaxItemsExceeded,
		/// An specified item is not found in the listings.
		ItemNotFound,
		/// The upper bound of the [`T::OrderId`] has been reached, and it's not possible to
		/// generate the [`NextOrderid`]
		CannotIncrementOrderId,
		/// The origin does not have permissions to mutate this order.
		NoPermission,
	}

	#[pallet::storage]
	pub type CartCount<T: Config<I>, I: 'static = ()> =
	StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

	#[pallet::storage]
	pub type Cart<T: Config<I>, I: 'static = ()> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<T::OrderId, T::MaxCartLen>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type NextOrderId<T: Config<I>, I: 'static = ()> = StorageValue<_, T::OrderId, ValueQuery>;

	#[pallet::storage]
	pub type Order<T: Config<I>, I: 'static = ()> =
	StorageMap<_, Blake2_128Concat, T::OrderId, (T::AccountId, OrderDetailsOf<T, I>)>;

	#[pallet::call(weight(<T as Config<I>>::WeightInfo))]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Creates a new cart.
		#[pallet::call_index(0)]
		pub fn create_cart(
			origin: OriginFor<T>,
			maybe_items: Option<Vec<(InventoryIdOf<T, I>, ItemIdOf<T, I>, Option<T::AccountId>)>>,
		) -> DispatchResult {
			let (owner, max_carts) = T::CreateOrigin::ensure_origin(origin)?;

			Cart::<T, I>::try_mutate(owner.clone(), |carts| {
				ensure!(
                    carts.len() as u32 <= max_carts,
                    Error::<T, I>::MaxCartsExceeded
                );

				let order_id = Self::generate_order_id()?;

				carts
					.try_push(order_id.clone())
					.map_err(|_| Error::<T, I>::MaxCartsExceeded)?;

				Self::set_order_items(
					&order_id,
					&owner,
					maybe_items.unwrap_or_default().into_iter(),
				)?;

				Self::deposit_event(Event::CartCreated { owner, order_id });

				Ok(())
			})
		}
	}
}

impl<T: Config<I>, I: 'static> OnPaymentStatusChanged<PaymentIdOf<T, I>, PaymentBalanceOf<T, I>>
for Pallet<T, I>
{
	fn on_payment_released(
		_id: &PaymentIdOf<T, I>,
		_fees: PaymentBalanceOf<T, I>,
		_resulting_amount: PaymentBalanceOf<T, I>,
	) {
		todo!()
	}
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	fn generate_order_id() -> Result<T::OrderId, DispatchError> {
		let order_id = NextOrderId::<T, I>::get();
		let next_order_id = order_id
			.increment()
			.ok_or(Error::<T, I>::CannotIncrementOrderId)?;
		NextOrderId::<T, I>::put(next_order_id);

		Ok(order_id)
	}

	fn set_order_items(
		order_id: &T::OrderId,
		order_owner: &T::AccountId,
		items: impl Iterator<Item=(InventoryIdOf<T, I>, ItemIdOf<T, I>, Option<T::AccountId>)>,
	) -> Result<(), DispatchError> {
		let items = items
			.map(|(inventory_id, id, beneficiary)| {
				let Item { owner, .. } =
					T::Listings::item(&inventory_id, &id).ok_or(Error::<T, I>::ItemNotFound)?;

				Ok(OrderItem {
					seller: owner,
					beneficiary,
					inventory_id,
					id,
				})
			})
			.collect::<Result<Vec<_>, DispatchError>>()?;

		let items = BoundedVec::try_from(items).map_err(|_| Error::<T, I>::MaxItemsExceeded)?;

		Order::<T, I>::try_mutate(order_id, |order_items| {
			let Some((owner, order)) = order_items else {
				*order_items = Some((
					order_owner.clone(),
					OrderDetails {
						status: OrderStatus::Cart,
						items,
					},
				));
				return Ok(());
			};

			ensure!(order_owner == owner, Error::<T, I>::NoPermission);

			order.items = items;
			Ok(())
		})
	}
}
