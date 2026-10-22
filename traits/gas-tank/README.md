# Gas tanks

Traits for gas tanks: prepaid allowances of weight that pay for transactions instead of fees.

- `GasBurner` is what a payment step asks: `check_available_gas` in validation, `prepare_gas` in preparation, and
  `burn_gas` after dispatch (or `cancel_gas`, when the transaction pays no fee).
- `GasFueler` refills a tank, and `MakeTank` creates one.
- `NonFungibleGasTank` implements all three on non-fungible items (for example, memberships).

The payment step that uses a `GasBurner` is `fc-pallet-gas-transaction-payment`'s `ChargeTransactionPayment`, and its
crate documentation (and README) is the integration guide for both crates: placement, declared weight, benchmarks,
what is metered, and the pitfalls to avoid.

## The burner contract

- **The check is read-only and deterministic.** It runs in transaction validation, in the pool and in the block. It
  writes nothing, and gives the same answer for the same arguments in the same state.
- **Preparation agrees with the check, or says it doesn't.** `prepare_gas` repeats the check and may write what the
  burn needs to find the tank. `None` makes the payment step reject the transaction. The provided default is the check
  itself, so a burner that needs no note only implements the two required methods.
- **The burn is infallible.** It receives what `prepare_gas` returned and the weight used, which the payment step caps
  at the estimate the check admitted.
- **A transaction that pays no fee is cancelled, not burnt.** `cancel_gas` drops whatever `prepare_gas` noted and
  charges nothing. The provided default does nothing.

## `NonFungibleGasTank`

A tank is a `WeightTank` (stored window start, usage, period and capacity per period) kept in the item's
`membership_gas` system attribute.

- **Windows are computed, never reset by a write.** The window that contains `now` starts at
  `since + ((now - since) / period) * period`. A window ends exactly when `now - since >= period`, and usage counts only
  inside the current window. A stored start in the future (written by the reset this crate used to do) keeps its
  window, and its usage, until a whole period after it (`since + period`): a tank the old reset left behind gets one
  stretched window, and never more allowance.
- **The check reads at most `MaxScan` items** of the account (default `DefaultMaxScan`), and chooses the first selected
  one whose tank covers the estimate in both weight components.
- **The note is written in preparation.** `prepare_gas` writes the paying-item note, which `(collection, item)` pays
  and what `prepare_gas` returned, under a per-account key in unhashed storage:
  `twox_128(b"NonFungibleGasTank") ++ twox_128(b"PayingItem") ++ blake2_128_concat(who.encode())`. `burn_gas` takes
  the note, rolls that item's stored window forward and adds the usage, with no scan, so whatever the call does to the
  account's items (a purchase, a swap, a new membership) cannot hide the tank from it. `cancel_gas` takes the note and
  charges nothing. The note never outlives the transaction. A burn without a matching preparation charges nothing.
- All block-number and weight arithmetic saturates or is checked.

**The tank that admitted a transaction pays for it.** A call that moves the noted item to another account during
dispatch does not escape the charge: the item's tank is charged, in its new owner's hands. Only an item whose tank is
gone by the burn charges nothing.
