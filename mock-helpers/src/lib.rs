#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "pallet-assets")]
mod assets;
#[cfg(feature = "pallet-balances")]
mod balances;
#[cfg(feature = "fc-pallet-listings")]
mod listings;

#[cfg(feature = "pallet-assets")]
pub use assets::*;
#[cfg(feature = "pallet-balances")]
pub use balances::*;
#[cfg(feature = "fc-pallet-listings")]
pub use listings::*;

pub trait ExtHelper {
    fn as_storage(&self) -> impl sp_runtime::BuildStorage;
}
