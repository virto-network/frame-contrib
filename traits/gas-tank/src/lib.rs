#![cfg_attr(not(feature = "std"), no_std)]

//! # Gas tanks
//!
//! Traits for gas tanks: prepaid allowances of weight that pay for transactions instead of fees.
//!
//! - [`GasBurner`] is what a payment step asks: [`check_available_gas`](GasBurner::check_available_gas)
//!   in validation, [`prepare_gas`](GasBurner::prepare_gas) in preparation, and
//!   [`burn_gas`](GasBurner::burn_gas) after dispatch (or [`cancel_gas`](GasBurner::cancel_gas), when
//!   the transaction pays no fee).
//! - [`GasFueler`] refills a tank, and [`MakeTank`] creates one.
//! - [`NonFungibleGasTank`] implements all three on non-fungible items (for example, memberships).
//!
//! The payment step that uses a [`GasBurner`] is `fc-pallet-gas-transaction-payment`'s
//! `ChargeTransactionPayment`, and its crate documentation is the integration guide for both crates:
//! placement, declared weight, benchmarks, what is metered, and the pitfalls to avoid.
//!
//! ## The burner contract
//!
//! - **The check is read-only and deterministic.** It runs in transaction validation, in the pool
//!   and in the block. It writes nothing, and gives the same answer for the same arguments in the
//!   same state.
//! - **Preparation agrees with the check, or says it doesn't.** `prepare_gas` repeats the check and
//!   may write what the burn needs to find the tank. `None` makes the payment step reject the
//!   transaction. The provided default is the check itself, so a burner that needs no note only
//!   implements the two required methods.
//! - **The burn is infallible.** It receives what `prepare_gas` returned and the weight used, which
//!   the payment step caps at the estimate the check admitted.
//! - **A transaction that pays no fee is cancelled, not burnt.** `cancel_gas` drops whatever
//!   `prepare_gas` noted and charges nothing. The provided default does nothing.
//!
//! ## `NonFungibleGasTank`
//!
//! A tank is a `WeightTank` (stored window start, usage, period and capacity per period) kept in the
//! item's `membership_gas` system attribute.
//!
//! - **Windows are computed, never reset by a write.** The window that contains `now` starts at
//!   `since + ((now - since) / period) * period`. A window ends exactly when `now - since >= period`,
//!   and usage counts only inside the current window. A stored start in the future (written by the
//!   reset this crate used to do) keeps its window, and its usage, until a whole period after it
//!   (`since + period`): a tank the old reset left behind gets one stretched window, and never more
//!   allowance.
//! - **The check reads at most `MaxScan` items** of the account (default [`DefaultMaxScan`]), and
//!   chooses the first selected one whose tank covers the estimate in both weight components.
//! - **The note is written in preparation.** `prepare_gas` writes the paying-item note, which
//!   `(collection, item)` pays and what `prepare_gas` returned, under a per-account key in unhashed
//!   storage: `twox_128(b"NonFungibleGasTank") ++ twox_128(b"PayingItem") ++
//!   blake2_128_concat(who.encode())`. `burn_gas` takes the note, rolls that item's stored window
//!   forward and adds the usage, with no scan, so whatever the call does to the account's items
//!   (a purchase, a swap, a new membership) cannot hide the tank from it. `cancel_gas` takes the
//!   note and charges nothing. The note never outlives the transaction. A burn without a matching
//!   preparation charges nothing.
//! - All block-number and weight arithmetic saturates or is checked.
//!
//! **The tank that admitted a transaction pays for it.** A call that moves the noted item to another
//! account during dispatch does not escape the charge: the item's tank is charged, in its new
//! owner's hands. Only an item whose tank is gone by the burn charges nothing.

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
