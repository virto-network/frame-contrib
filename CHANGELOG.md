# Changelog

All notable changes to this project are documented in this file. Every crate in the
workspace is released together under the same version.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html),
with one extra rule: every new Polkadot SDK line is a major release.

## [Unreleased]

## [2.2.0](https://github.com/virto-network/frame-contrib/releases/tag/v2.2.0)

`fc-pallet-pass` now charges the weight of what it actually does, and authenticators own their
weights and benchmark inputs (#57, #76). Encodings and metadata are unchanged: no call, event,
error, storage or transaction extension changes, so no `transaction_version` bump. Runtimes adapt
their configuration and weights, as listed under "Changed".

### Added

- *(fc-traits-authn)* `verification_weight(&self)` on device attestations and user credentials,
  so an authenticator can report what verifying its input costs (e.g. a signature check, or
  parsing that grows with the input size). It defaults to zero.
- *(fc-traits-authn)* Benchmark helpers, behind the new `runtime-benchmarks` feature:
  `AuthenticatorBenchmarkHelper`, `ChallengerBenchmarkHelper`,
  `DeviceAttestationBenchmarkHelper<Cx>` and `CredentialBenchmarkHelper<Cx>`. An authenticator
  produces valid device attestations and credentials for benchmarks. `composite_authenticator!`
  derives the helper by delegating to the **first** authenticator listed.
- *(fc-pallet-pass)* The `authenticate_none` weight, for transactions that don't authenticate.

### Changed

- *(fc-pallet-pass)* `PassAuthenticate` charges `authenticate_none` when there is no credential,
  and `authenticate` only when there is one. Before, every transaction paid the full
  `authenticate` weight. It refunds weight it doesn't use.
- *(fc-pallet-pass)* The `authenticate` benchmark measures the whole extension (validate, prepare
  and post-dispatch), not only validation, and `add_session_key` is benchmarked at its worst case.
- *(fc-pallet-pass)* `register`, `add_device` and `PassAuthenticate` add the authenticator's
  `verification_weight`.
- *(fc-pallet-pass)* `Config::BenchmarkHelper` is removed: benchmarks get their inputs from the
  authenticator (`AuthenticatorBenchmarkHelper`).
- *(fc-pallet-pass)* The default `SubstrateWeight` values are labelled placeholders that count
  database accesses, pending a benchmark run on reference hardware. Runtimes should use their own
  benchmarked weights.

**Runtime integration:**

- Add `fn authenticate_none() -> Weight` to your `fc_pallet_pass` weights, and re-run the pallet's
  benchmarks (`authenticate` changed too).
- Remove `type BenchmarkHelper` from your `pallet_pass` config, and implement
  `ChallengerBenchmarkHelper` for your challenger. Enable `fc-traits-authn/runtime-benchmarks`
  (through `frame-contrib-traits`) in your `runtime-benchmarks` feature.
- Use authenticators that implement `verification_weight` and the benchmark helpers (for
  `composite_authenticator!`, at least the first one listed).

## [2.1.0](https://github.com/virto-network/frame-contrib/releases/tag/v2.1.0)

### Added

- *(fc-pallet-pass)* `FirstItemsAreFree<N, C>`: a consideration where the first `N` items are
  free. It is stored as `Option<C>` for every `N`, the same encoding as `FirstItemIsFree`, so
  switching a runtime to `N = 2` needs no storage migration. `FirstItemIsFree<C>` is now an
  alias for `FirstItemsAreFree<ConstU32<1>, C>` and behaves exactly as before.

## [2.0.0](https://github.com/virto-network/frame-contrib/releases/tag/v2.0.0)

This is the first versioned release and the first to be published to crates.io.
Before it, the crates were consumed from git without tags, and their versions
(`0.1.0` / `1.0.0`) never changed. The states that downstream runtimes shipped are
tagged after the fact: [`polkadot-stable2509`] (Kreivo `0.16.9`) and
[`polkadot-stable2512`] (Kreivo `0.17.0-pre.1`).

This release is [`polkadot-stable2512`] moved to polkadot-sdk `stable2606`. **Pallet
calls, storage, events, errors and `Config` are unchanged**, so a runtime already on
`polkadot-stable2512` only needs the SDK upgrade. It needs no frame-contrib
migrations.

### ⚠ Breaking changes

- Update to polkadot-sdk `stable2606-2`. SDK crates move from `stable2512`
  (`frame-support` 45) to `stable2606` (`frame-support` 48).
- The `mock-helpers` package is renamed `fc-mock-helpers`. Keep using it under the
  `mock-helpers` dependency key with `package = "fc-mock-helpers"`.
- All crates now share one version and are released together. Depend on the same
  version of every `fc-*` crate.

### Changed

- *(fc-pallet-pass)* Hash with `sp_io::hashing::blake2_256`, because `sp-core` no
  longer re-exports it. `sp-io` is now a regular dependency. Hashes are unchanged.
- *(fc-pallet-listings)* Types derive `Debug` instead of the removed `RuntimeDebug`.

[`polkadot-stable2509`]: https://github.com/virto-network/frame-contrib/tree/polkadot-stable2509
[`polkadot-stable2512`]: https://github.com/virto-network/frame-contrib/tree/polkadot-stable2512
