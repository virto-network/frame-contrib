#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::Parameter;
use sp_runtime::traits::BlockNumber;
use sp_runtime::DispatchResult;

#[cfg(test)]
mod tests;

mod impl_nonfungibles;

pub trait GasTank: GasBurner + GasFueler {}

pub use impl_nonfungibles::{NonFungibleGasTank, SelectNonFungibleItem};

/// Handles burning _"gas"_ from a tank to be spendable in transactions
pub trait GasBurner {
    type AccountId: Parameter;
    type Gas: Parameter;

    /// Check if account has a minimum of `gas` to consume.
    /// Returns the gas that would be left after burning the requested amount or `None` if there's not enough left.
    /// When `gas` is not provided it simply returns the available gas.
    fn check_available_gas(who: &Self::AccountId, estimated: &Self::Gas) -> Option<Self::Gas>;

    /// Spend as much `gas` as possible returning what is left in the tank.
    ///
    /// This method is expected not to fail.
    fn burn_gas(who: &Self::AccountId, expected: &Self::Gas, used: &Self::Gas) -> Self::Gas;
}

/// Handles fueling _"gas"_ on a tank to spend in future transactions
pub trait GasFueler {
    type TankId: Parameter;
    type Gas: Parameter;
    #[cfg(feature = "runtime-benchmarks")]
    type AccountId;

    /// Refills as much `gas` as possible returning what the updated amount of gas in the tank.
    ///
    /// This method is expected not to fail.
    fn refuel_gas(id: &Self::TankId, gas: &Self::Gas) -> Self::Gas;

    /// Refills as much `gas` as possible returning what the updated amount of gas in the tank,
    /// indicating an account instead of a tank.
    ///
    /// This method is expected not to fail.
    #[cfg(feature = "runtime-benchmarks")]
    fn refuel_gas_to_account(who: &Self::AccountId, gas: &Self::Gas) -> Self::Gas;
}

pub trait MakeTank {
    type TankId: Parameter;
    type Gas: Parameter;
    type BlockNumber: BlockNumber;

    /// Creates a new tank, allowing to specify a max gas `capacity` and a `periodicity` after
    /// which the tank gets renewed.
    ///
    /// Returns `Some(())` if the creation was successful, or `None` otherwise.
    fn make_tank(
        id: &Self::TankId,
        capacity: Option<Self::Gas>,
        periodicity: Option<Self::BlockNumber>,
    ) -> DispatchResult;
}
