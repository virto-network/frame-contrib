#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::{DispatchResult, MaybeSerializeDeserialize, Parameter};

pub mod inventory;
pub mod item;
pub mod test_utils;

pub trait ListingsIdentifier: Parameter + MaxEncodedLen + Copy + MaybeSerializeDeserialize {}

impl<T> ListingsIdentifier for T where
    T: Parameter + MaxEncodedLen + Copy + MaybeSerializeDeserialize
{
}

pub use inventory::{
    InspectInventory, InventoryInspectEnumerable, InventoryLifecycle, MutateInventory,
};
pub use item::{
    subscriptions::{InspectSubscription, MutateSubscription},
    InspectItem, ItemInspectEnumerable, MutateItem,
};
