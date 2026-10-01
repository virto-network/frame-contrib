#![cfg_attr(not(feature = "std"), no_std)]

//! # Gas Transaction Payment
//!
//! This pallet exposes a mechanism to allow transaction payment using a prepaid fees mechanism: a
//! signed transaction is paid with _gas_ (weight) from a tank the signer can use, and only when no
//! tank covers it does the runtime's ordinary fee extension charge a fee.
//!
//! This is the integration guide for the pallet and for
//! [`fc-traits-gas-tank`](frame_contrib_traits::gas_tank), the traits its tanks implement.
//!
//! ## Overview
//!
//! [`ChargeTransactionPayment<T, S>`](ChargeTransactionPayment) is a transaction extension that wraps
//! a fee extension `S` (for example `pallet_transaction_payment::ChargeTransactionPayment`). For each
//! signed transaction it asks [`Config::GasTank`], a
//! [`GasBurner`], whether a tank covers the transaction.
//! If one does, the tank pays and `S` never runs; otherwise `S` validates, prepares and charges as it
//! would alone. Unsigned and root origins always go to `S`.
//!
//! The wrapper is invisible to clients: its identifier, implicit data, type information, metadata
//! and encoding are `S`'s. Swapping `S` for `ChargeTransactionPayment<T, S>` changes neither the
//! runtime metadata nor any transaction's bytes.
//!
//! ## The burner contract
//!
//! A transaction goes through three phases, and the extension calls the burner once in each:
//!
//! | Phase | Extension | Burner |
//! |---|---|---|
//! | Validation (in the pool and in the block) | Computes the estimate and asks the tank. A tank means `Val = None` (the tank's path). Otherwise `S` validates and its value is carried as `Some(v)` | [`check_available_gas`](frame_contrib_traits::gas_tank::GasBurner::check_available_gas) |
//! | Preparation | `Some(v)`: `S` prepares with `v`, and the tank is never asked again. `None`: the tank must still cover the transaction, or it is rejected with [`InvalidTransaction::Custom`]`(`[`PATH_MISMATCH`]`)` | [`prepare_gas`](frame_contrib_traits::gas_tank::GasBurner::prepare_gas) |
//! | Post-dispatch | On the tank's path, charges the actual weight and deposits [`Event::GasBurned`]. If the transaction pays no fee, charges nothing and cancels instead | [`burn_gas`](frame_contrib_traits::gas_tank::GasBurner::burn_gas), or [`cancel_gas`](frame_contrib_traits::gas_tank::GasBurner::cancel_gas) when no fee is paid |
//!
//! A burner must keep to this:
//!
//! - **The check is read-only and deterministic.** It runs in the transaction pool, against the
//!   pool's view of state, and again in the block. It must not write anything, not even a note or a
//!   cache, and must give the same answer for the same arguments in the same state.
//! - **Preparation agrees with the check, or says it doesn't.** `prepare_gas` repeats the check and
//!   may write what the burn needs. Returning `None` makes the transaction invalid; it is never moved
//!   to a fee path that was not validated. The default `prepare_gas` is the check.
//! - **The burn is infallible.** It is called after dispatch, with what `prepare_gas` returned and
//!   the actual weight, which is never more than the estimate the tank admitted.
//! - **Whatever preparation notes, the burn or the cancel drops.** A note must not outlive its
//!   transaction, and the burn must find the tank whatever the call did during dispatch: a burn that
//!   searches the account again can miss it. The default `cancel_gas` does nothing.
//!
//! ## Placement in `TransactionExtensions`
//!
//! Put the extension where the fee extension would be, wrapping it:
//!
//! - **After `CheckWeight`.** The tank is charged the weight `CheckWeight` books, so let
//!   `CheckWeight` admit the transaction to the block first.
//! - **Inside `SkipCheckIfFeeless`**, if the runtime uses it, exactly as the fee extension would be:
//!   a feeless call then skips both the tank and the fee.
//! - **Around the fee extension**, which it replaces in the tuple.
//!
//! Signed origins are offered to the tanks; unsigned and root origins go to `S`. An origin that is
//! not a [`frame_system::RawOrigin`] (for example, one an earlier authentication extension produced)
//! is rejected with [`InvalidTransaction::BadSigner`], unchanged from 2.3.x.
//!
//! The charge sees the refunds of the extensions whose `post_dispatch` runs before it. A wrapper
//! that reclaims weight after the whole tuple (for example cumulus' `StorageWeightReclaim<T, (…)>`)
//! lowers the proof size the block books below what the tank was charged. The tank is never
//! charged more than the estimate it admitted, but it then pays for proof size the block no longer
//! books.
//!
//! ```rust,ignore
//! pub type TxExtension = (
//!     frame_system::CheckNonZeroSender<Runtime>,
//!     frame_system::CheckSpecVersion<Runtime>,
//!     frame_system::CheckTxVersion<Runtime>,
//!     frame_system::CheckGenesis<Runtime>,
//!     frame_system::CheckEra<Runtime>,
//!     frame_system::CheckNonce<Runtime>,
//!     frame_system::CheckWeight<Runtime>,
//!     pallet_skip_feeless_payment::SkipCheckIfFeeless<
//!         Runtime,
//!         fc_pallet_gas_transaction_payment::ChargeTransactionPayment<
//!             Runtime,
//!             pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
//!         >,
//!     >,
//!     frame_system::WeightReclaim<Runtime>,
//! );
//! ```
//!
//! ## Declared weight and benchmarks
//!
//! The extension declares
//! [`WeightInfo::charge_transaction_payment`]` + S::weight(call)`, so its weight covers the worst of
//! both paths: the tank's (check, preparation and burn) and the fee path (the check, then `S`).
//!
//! The `charge_transaction_payment` benchmark runs the tank's whole path: validation, preparation
//! and post-dispatch. The runtime's `BenchmarkHelper::setup_account` decides how hard the tank is to
//! find, so it must set up the worst case. For
//! [`NonFungibleGasTank`](frame_contrib_traits::gas_tank::NonFungibleGasTank), that is an account
//! with as many items as the tank's scan bound, and the tank on the item scanned last; its burn
//! reads the paying-item note and that one tank, whatever the account holds, and a transaction that
//! pays no fee only cancels the note, which costs less. The fee extension's own weight comes from its
//! own benchmark and is not measured here.
//!
//! ## What is metered
//!
//! A tank is charged the weight the block books for the transaction, counted exactly as
//! [`CheckWeight`](frame_system::CheckWeight) counts it
//! ([`calculate_consumed_extrinsic_weight`](frame_system::calculate_consumed_extrinsic_weight)):
//!
//! - **Estimate** (what the tank must cover to be chosen): the declared call weight, plus every
//!   transaction extension's declared weight, plus the base extrinsic weight of the call's dispatch
//!   class, with the transaction's encoded length added to proof size.
//! - **Actual** (what the tank is charged): the call and extension weights left after dispatch and
//!   refunds (`calc_actual_weight`), plus the same base and length, capped at the estimate, in each
//!   component.
//! - **Nothing**, when the transaction pays no fee (the call is declared `Pays::No`, or reports it
//!   after dispatch).
//!
//! On the tank's path the extension reports no unspent weight of its own, so everything it charged
//! the tank stays booked for the block.
//!
//! ## `NonFungibleGasTank`
//!
//! [`NonFungibleGasTank`](frame_contrib_traits::gas_tank::NonFungibleGasTank) keeps a periodic
//! tank on a non-fungible item (for example, a membership). Its usage window is computed from the
//! stored start and the period, never reset by a write; its check reads at most `MaxScan` of the
//! account's items; `prepare_gas` writes a paying-item note, which item pays, under a per-account key
//! in unhashed storage (`twox_128(b"NonFungibleGasTank") ++ twox_128(b"PayingItem") ++
//! blake2_128_concat(who.encode())`), and the burn takes it, rolls that item's stored window forward
//! and adds the usage, with no scan. A transaction that pays no fee cancels the note instead. The
//! note never outlives the transaction.
//!
//! **The tank that admitted a transaction pays for it**, whatever the call does to the signer's items
//! during dispatch: neither an item that now sorts before the tank nor moving the tank's item to
//! another account escapes the charge. Only an item whose tank is gone by the burn charges nothing.
//!
//! ## Pitfalls
//!
//! The defects found in kreivo#505, and one more found since, and how each is avoided.
//!
//! 1. **Writes in validation.** A write in a check (a window reset, a note) happens in the pool and
//!    again in the block, and was how kreivo#505's members were locked out. The check reads only;
//!    the note is written in preparation.
//! 2. **`expect` in preparation.** Preparation used to re-ask the tank and, when it said no, prepare
//!    the fee extension with a value validation never produced, which panicked. It now rejects with
//!    [`PATH_MISMATCH`].
//! 3. **A window reset into the future.** `since = now + period` started the next window a period
//!    late, and `now - since` then underflowed. The window start is now computed, and never written
//!    into the future. A tank the old reset left with a future start keeps that window, and its
//!    usage, until a whole period after it (`since + period`): one stretched window, and never more
//!    allowance.
//! 4. **The call weight reported as unspent.** Returning the burnt gas from `post_dispatch_details`
//!    refunded the call's whole weight from the block. Only the extension's own unspent weight is
//!    reported, and it is zero.
//! 5. **Admission on `call_weight`, charge on `actual_weight`.** The two measured different things:
//!    the charge included every extension's weight, the admission did not. Both are now the metered
//!    weight above.
//! 6. **An undeclared inner weight.** The extension declared only its own benchmark, so the fee
//!    extension's work went unbooked. It now declares both.
//! 7. **An unbounded scan.** Walking every item an account owns, in validation, made every signed
//!    transaction's cost depend on the signer's holdings. The scan stops at `MaxScan`.
//! 8. **A burn that searches again.** A burn that looked for its note among the first `MaxScan`
//!    items missed it once the call gave the signer an item that sorts earlier, and then charged
//!    nothing and left the note behind. The burn reads the paying-item note directly, and the note is
//!    cancelled when no fee is paid.
//!
//! ## Upgrading from 2.3.x
//!
//! - **Tanks drain by the full metered weight.** A tank used to be charged the call's weight; it is
//!   now charged the metered weight above: the base extrinsic weight, every extension's declared
//!   weight, and the transaction's length as proof size. A tank whose capacity per period has a
//!   small or zero proof-size component may stop admitting anything. Size both components.
//! - **The scan is bounded.** With the default `MaxScan` of 4
//!   ([`DefaultMaxScan`](frame_contrib_traits::gas_tank::DefaultMaxScan)), a member whose tank sits
//!   past the fourth item it owns takes the fee path. Set `MaxScan` explicitly if members hold more.
//! - **The old selection attribute is left behind.** The `mbmshp_pays_gas` item attributes the old
//!   validation wrote are never read or cleared again. A runtime may clear them in a one-off
//!   migration: clear that attribute on each item that has it.
//! - **The declared weight grows.** Every signed transaction now declares the fee extension's
//!   weight on top of this extension's.
//! - **Direct callers of `NonFungibleGasTank`** must call `prepare_gas` before `burn_gas`, and
//!   `cancel_gas` when no fee is paid: a burn without a matching preparation charges nothing.

use frame::{deps::sp_runtime::traits::TransactionExtension, prelude::*};
use frame_contrib_traits::gas_tank::GasBurner;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod mock_tank;
#[cfg(test)]
mod tests;

mod extensions;
mod weights;

pub use extensions::*;
pub use pallet::*;
pub use weights::*;

#[frame::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        // Primitives: Some overarching types that come from the system (or the system depends on).
        /// The Weight info
        type WeightInfo: WeightInfo;

        // Dependencies: The external components this pallet depends on.
        /// A type that handles gas tanks
        type GasTank: GasBurner<AccountId = Self::AccountId, Gas = Weight>;

        // Benchmarking: Types to handle benchmarks.
        /// A helper to prepare benchmarking tests
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: BenchmarkHelper<Self>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        GasBurned {
            who: T::AccountId,
            remaining: Weight,
        },
    }
}

/// Sets up the `charge_transaction_payment` benchmark in a runtime.
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<T: Config> {
    /// The fee extension the runtime wraps.
    type Ext: TransactionExtension<T::RuntimeCall>;

    /// An instance of the extension, ready to be used.
    fn ext() -> ChargeTransactionPayment<T, Self::Ext>;

    /// Prepares an account with enough gas to execute
    ///
    /// Set up the worst case for the runtime's tank: the benchmark measures how hard the tank is to
    /// find, check and burn from the account this leaves.
    fn setup_account(who: &T::AccountId, gas: Weight) -> DispatchResult;
}
