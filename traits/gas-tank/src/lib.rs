#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::Parameter;
use sp_runtime::DispatchResult;

#[cfg(test)]
mod tests;

mod impl_nonfungibles;

pub trait GasTank: GasBurner + GasFueler {}

pub use impl_nonfungibles::{DefaultMaxScan, NonFungibleGasTank, SelectNonFungibleItem};

/// Handles burning _"gas"_ from a tank to be spendable in transactions
///
/// A payment step calls the three methods in the three phases of one transaction: [`check_available_gas`]
/// in validation, [`prepare_gas`] in preparation, and [`burn_gas`] after dispatch, passing it what
/// [`prepare_gas`] returned. When the transaction turns out to pay no fee, it calls [`cancel_gas`]
/// instead of [`burn_gas`].
///
/// [`check_available_gas`]: GasBurner::check_available_gas
/// [`prepare_gas`]: GasBurner::prepare_gas
/// [`burn_gas`]: GasBurner::burn_gas
/// [`cancel_gas`]: GasBurner::cancel_gas
pub trait GasBurner {
    type AccountId: Parameter;
    type Gas: Parameter;

    /// Check if account has a minimum of `gas` to consume.
    /// Returns the gas that would be left after burning the requested amount or `None` if there's not enough left.
    /// When `gas` is not provided it simply returns the available gas.
    ///
    /// This runs in transaction validation, including in the transaction pool, so it **must not write
    /// any state**, and it must return the same answer for the same arguments in the same state.
    fn check_available_gas(who: &Self::AccountId, estimated: &Self::Gas) -> Option<Self::Gas>;

    /// Check again, in transaction preparation, that `who` has `estimated` gas to consume, and make
    /// whatever note [`burn_gas`](GasBurner::burn_gas) needs to find the tank after dispatch.
    ///
    /// Returns what [`check_available_gas`](GasBurner::check_available_gas) would return. `None` means
    /// the tank no longer covers the transaction: the payment step then rejects it, because validation
    /// chose the tank. Unlike the check, this method may write.
    ///
    /// The default calls [`check_available_gas`](GasBurner::check_available_gas) and writes nothing.
    fn prepare_gas(who: &Self::AccountId, estimated: &Self::Gas) -> Option<Self::Gas> {
        Self::check_available_gas(who, estimated)
    }

    /// Spend as much `gas` as possible returning what is left in the tank.
    ///
    /// `expected` is what [`prepare_gas`](GasBurner::prepare_gas) returned for this transaction.
    ///
    /// This method is expected not to fail.
    fn burn_gas(who: &Self::AccountId, expected: &Self::Gas, used: &Self::Gas) -> Self::Gas;

    /// Drop whatever note [`prepare_gas`](GasBurner::prepare_gas) made for a transaction that, after
    /// dispatch, pays no fee, so the note does not outlive the transaction. Charges nothing.
    ///
    /// A payment step calls this after dispatch instead of [`burn_gas`](GasBurner::burn_gas), with
    /// what [`prepare_gas`](GasBurner::prepare_gas) returned.
    ///
    /// The default does nothing, which is right for a burner whose `prepare_gas` writes nothing.
    fn cancel_gas(_who: &Self::AccountId, _expected: &Self::Gas) {}
}

/// Handles fueling _"gas"_ on a tank to spend in future transactions
pub trait GasFueler {
    type TankId: Parameter;
    type Gas: Parameter;

    /// Refills as much `gas` as possible returning what the updated amount of gas in the tank.
    ///
    /// This method is expected not to fail.
    fn refuel_gas(id: &Self::TankId, gas: &Self::Gas) -> Self::Gas;
}

pub trait MakeTank {
    type TankId: Parameter;
    type Gas: Parameter;
    type BlockNumber;

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
