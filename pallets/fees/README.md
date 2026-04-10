# Fees Pallet (`fc-pallet-fees`)

Custom fees pallet that lets protocol governance and communities configure fees
on asset transfers using `pallet-assets`.

- **Protocol fees**: global fees set by `AdminOrigin`, applied to all transfers
- **Community fees**: per-community fees set by `CommunityOrigin`, applied when
  the sender belongs to a community
- **Fee types**: `Fixed`, `Percentage` (Permill), `PercentageClamped` (with
  min/max bounds)
- **`WithFees<T>` adapter**: wraps `pallet-assets` via the `fungibles` traits
  to charge fees transparently on `transfer` calls from other pallets
- **`ChargeFees<T>` transaction extension**: charges fees on direct
  `pallet-assets` extrinsics, with automatic refund on dispatch failure

## Recommended runtime setup: wallet-less & fee-less transactions

This pallet is designed to work with
[`fc-pallet-pass`](../pass) and
[`fc-pallet-gas-transaction-payment`](../gas-transaction-payment)
to enable a transaction pipeline where users need **no wallet** (no keypair)
and pay **no upfront gas fees**:

```
General transaction (no outer signature)
  │
  ├─ CheckSpecVersion
  ├─ CheckTxVersion
  ├─ CheckGenesis
  ├─ CheckMortality
  ├─ CheckNonce
  ├─ CheckWeight
  │
  ├─ PassAuthenticate          ← authenticates via passkey/WebAuthn credential
  │                              sets origin to the user's pass account
  │
  ├─ ChargeTransactionPayment  ← gas-tank wrapper, covers weight costs
  │  (gas-tank)                  from the community's pre-paid gas tank
  │
  └─ ChargeFees                ← charges protocol/community fees on
                                 asset transfers (in the transferred asset)
```

### How it works

1. **No wallet needed.** Transactions are submitted as
   [`General` extrinsics](https://docs.rs/sp-runtime/latest/sp_runtime/generic/struct.UncheckedExtrinsic.html)
   which carry no outer cryptographic signature. Authentication is handled
   entirely by `PassAuthenticate` using device credentials (e.g. passkeys).

2. **No gas fees for users.** Weight-based fees are covered by a pre-paid gas
   tank associated with the user's community membership. The
   `ChargeTransactionPayment` wrapper from `fc-pallet-gas-transaction-payment`
   checks the tank before falling back to standard fee payment.

3. **Transfer fees in the transferred asset.** When a user makes an asset
   transfer, `ChargeFees` deducts the configured protocol and community fees
   directly from the transferred asset — no native token balance required.

### Runtime configuration

```rust
// Transaction extension pipeline
type TxExtension = (
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckMortality<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    fc_pallet_pass::PassAuthenticate<Runtime>,
    fc_pallet_gas_transaction_payment::ChargeTransactionPayment<
        Runtime,
        pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
    >,
    fc_pallet_fees::ChargeFees<Runtime>,
);

// Use General extrinsics (no outer signature required)
type UncheckedExtrinsic = generic::UncheckedExtrinsic<
    sp_runtime::MultiAddress<AccountId, ()>,
    RuntimeCall,
    sp_runtime::MultiSignature,
    TxExtension,
>;
```

### Fees pallet config

```rust
impl fc_pallet_fees::Config for Runtime {
    type CommunityId = u64;
    type MaxFeeNameLen = ConstU32<64>;
    type MaxProtocolFees = ConstU32<10>;
    type MaxCommunityFees = ConstU32<10>;
    type AdminOrigin = EnsureRoot<AccountId>;
    type CommunityOrigin = pallet_communities::EnsureCommunity<Runtime>;
    type CommunityDetector = pallet_communities::Pallet<Runtime>;
}
```

The pallet requires `pallet_assets::Config` as a supertrait — no separate
`Assets` type is needed since it operates directly on `pallet-assets`.

### Adapter for other pallets

If other pallets in your runtime perform asset transfers (e.g. a marketplace
or DEX), use `WithFees<Runtime>` as their fungibles implementation instead of
`pallet_assets::Pallet<Runtime>` to automatically charge fees on those
transfers too:

```rust
impl pallet_marketplace::Config for Runtime {
    type Assets = fc_pallet_fees::WithFees<Runtime>;
    // ...
}
```
