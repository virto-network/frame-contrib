# Account Profile Pallet

The **Account Profile** pallet enables each account to associate itself with a
single profile content item published via
[`fc-pallet-content`](../fc-pallet-content).

## ✨ Overview

Each on-chain account may point to exactly one content item as their public
profile. The referenced item is verified at the time of registration to ensure
it is owned by the sender, has not been retracted, and is still revisionable
(updatable). This mirrors the semantics of the original
[AcuityAccountProfile](https://github.com/acuity-network/acuity-contracts/blob/master/src/acuity-account-profile/AcuityAccountProfile.sol)
Solidity contract.

## ⚙️ Configuration

| Type / Constant  | Description                                                               |
|------------------|---------------------------------------------------------------------------|
| `RuntimeEvent`   | The aggregated event type.                                                |
| `ContentStore`   | Implementation of `ItemInspect` – typically `fc_pallet_content::Pallet`. |
| `WeightInfo`     | Weight cost functions.                                                    |

### Runtime wiring example

```rust
impl fc_pallet_account_profile::Config for Runtime {
    type ContentStore = fc_pallet_content::Pallet<Runtime>;
    type WeightInfo = fc_pallet_account_profile::weights::SubstrateWeight<Runtime>;
}
```

## 🧠 Storage

| Item             | Description                                    |
|------------------|------------------------------------------------|
| `AccountProfile` | Maps `AccountId → ItemId` (profile item ID).  |

## 📦 Dispatchable Functions

### `set_profile`

Sets the profile item for the sender.

```rust
fn set_profile(origin: OriginFor<T>, item_id: ItemId) -> DispatchResult
```

**Validations (in order):**

1. `ItemNotFound` – the item does not exist in `fc-pallet-content`.
2. `NotItemOwner` – the item is not owned by the sender.
3. `ItemRetracted` – the item has been retracted.
4. `ItemNotRevisionable` – the item is no longer revisionable (updatable).

## 📡 Events

| Event        | Fields              | Description                         |
|--------------|---------------------|-------------------------------------|
| `ProfileSet` | `account`, `item_id`| Emitted when a profile is updated.  |

## ❌ Errors

| Error                 | Description                                          |
|-----------------------|------------------------------------------------------|
| `ItemNotFound`        | No item with the given ID exists.                    |
| `NotItemOwner`        | Sender does not own the item.                        |
| `ItemRetracted`       | The item has been retracted.                         |
| `ItemNotRevisionable` | The item is not revisionable (cannot be updated).    |
| `NoProfile`           | The account has no profile set (helper / future use).|

## 🧪 Testing and Benchmarking

Run the unit tests:

```bash
cargo test -p fc-pallet-account-profile
```

Run the benchmarks (requires a runtime that includes both pallets):

```bash
frame-omni-bencher v1 benchmark pallet \
  --runtime target/release/wbuild/<runtime>/<runtime>.wasm \
  --pallets pallet_account_profile \
  --extrinsic "" \
  --steps 50 --repeat 20 \
  --output src/weights.rs
```
