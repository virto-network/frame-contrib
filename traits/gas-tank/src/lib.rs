#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::Parameter;

#[cfg(test)]
mod tests;

mod impl_nonfungibles;

pub trait GasTank: GasBurner + GasFueler {}

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
    type AccountId: Parameter;
    type Gas: Parameter;

    /// Refills as much `gas` as possible returning what the updated amount of gas in the tank.
    ///
    /// This method is expected not to fail.
    fn refuel_gas(who: &Self::AccountId, gas: &Self::Gas) -> Self::Gas;
}
