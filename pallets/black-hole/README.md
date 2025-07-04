# 🕳️ Black Hole Pallet

The **Black Hole** pallet defines a sink account that receives token transfers and periodically burns its entire
balance, removing tokens from circulation. This mechanism can be useful for implementing economic sinks or deflationary
behaviors within a runtime.

## ✨ Overview

- The pallet owns an account known as the **event horizon**.
- Any user or module can transfer tokens to this account.
- Periodically (after a configurable number of blocks), the balance of the account is **burned** automatically.
- Authorized origins can dispatch extrinsic calls **as if** signed by the event horizon account.

## 🔧 Features

- **Automated burning** of all tokens in the event horizon account every `BurnPeriod` blocks.
- **Total burned balance tracking** via `BlackHoleMass`.
- **Dispatch proxying** to allow controlled usage of the event horizon identity.

## ⚙️ Configuration

This pallet requires the following types and parameters to be configured in the runtime:

| Type / Constant              | Description                                                 |
|------------------------------|-------------------------------------------------------------|
| `RuntimeEvent`               | Event type for this pallet                                  |
| `WeightInfo`                 | Weight cost functions                                       |
| `EventHorizonDispatchOrigin` | Origin authorized to proxy calls as the event horizon       |
| `Balances`                   | A `fungible::Mutate` implementation (e.g., native balances) |
| `BlockNumberProvider`        | Source of current block number                              |
| `PalletId`                   | Unique identifier for deriving the event horizon account    |
| `BurnPeriod`                 | Number of blocks between automatic burns                    |

## 🧠 Storage

| Item            | Description                                  |
|-----------------|----------------------------------------------|
| `LastBurn`      | Block number of the last burn event          |
| `BlackHoleMass` | Total amount of tokens burned by this pallet |

## 📦 Dispatchable Functions

### `dispatch_as_event_horizon`

```rust
fn dispatch_as_event_horizon(origin, call)
```

Allows an authorized origin to dispatch a call **on behalf of the event horizon account**. This enables tightly
controlled usage of the burn account in more complex workflows.

## 🔁 Runtime Hooks

- The pallet implements `on_idle`, and will attempt to burn the balance in the event horizon account whenever sufficient
  weight is available.

## 🔒 Security Notes

- The pallet does not restrict who can transfer funds to the event horizon account.
- Only the `EventHorizonDispatchOrigin` can perform calls as the event horizon, preventing abuse.

## 🧪 Testing and Benchmarking

- Includes unit tests and benchmarking support behind `runtime-benchmarks`.
