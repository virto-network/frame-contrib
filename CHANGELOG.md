# Changelog

All notable changes to this project are documented in this file. Every crate in the
workspace is released together under the same version.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html),
with one extra rule: every new Polkadot SDK line is a major release.

## [Unreleased]

## [2.3.1](https://github.com/virto-network/frame-contrib/releases/tag/v2.3.1)

A security fix. Upgrade from any 2.x release; nothing else changes.

### Fixed

- *(fc-pallet-communities)* `set_decision_method` let the admin of **any** community change the
  decision method of **any other** community: it checked that the origin was some community's
  admin but discarded which one, then wrote the `community_id` it was given. Such an admin could,
  for example, stop another community's governance by switching it to a method under which none of
  its members' votes carries weight. The call now requires the community resolved from
  `AdminOrigin` to be `community_id`, and fails with `BadOrigin` otherwise, before any side effect
  (#114). Present since the pallet moved into this repository; every release up to 2.3.0 is
  affected.

The call's signature, index, encoding, events, storage and weight are unchanged, so runtimes only
need to take the new version; no `transaction_version` bump.

## [2.3.0](https://github.com/virto-network/frame-contrib/releases/tag/v2.3.0)

The first release with **measured weights**: every pallet's `SubstrateWeight` now comes from a
benchmark run on dedicated hardware, not from placeholders. Authenticators' verification weights
can also be bound by the runtime. Encodings and metadata are unchanged: no call, event, error,
storage or transaction extension changes, so no `transaction_version` bump. Runtimes adapt their
configuration, as listed under "Changed".

### Added

- *(fc-traits-authn)* `AuthenticatorWeightInfo`, with `verify_device(c, a)` and `verify_user(c, a)`
  (`c`: client data length, `a`: authenticator data length). `()` implements it as zero.
- *(fc-traits-authn)* A `WeightInfo` associated type on `Authenticator` and `UserAuthenticator`,
  and `verification_weight(&attestation)` / `verification_weight(&credential)` on those traits,
  which default to asking `WeightInfo` with the payload's actual size. A runtime binds an
  authenticator's weights like a pallet's (#100).
- *(fc-traits-authn)* `weight_components(&self) -> (u32, u32)` on `DeviceChallengeResponse` and
  `UserChallengeResponse`, the `(c, a)` lengths `verification_weight` charges with. The default is
  the whole encoded size for both, an upper bound; authenticators return their real lengths (#111).
- A kitchensink runtime (`fc-kitchensink-runtime`, not published) that runs every pallet's
  benchmarks, checked on each pull request (#95).

### Changed

- **Weights for every pallet are measured**, on a Hetzner CCX43 (AMD EPYC-Milan, 16 dedicated
  vCPUs) with `--steps 50 --repeat 20` through the kitchensink runtime (#109). Notable changes in
  `fc-pallet-pass`: `register` was undercharged (50 µs → 83.5 µs) and `add_session_key`'s proof
  size grows to about 220 KB; most other calls get cheaper.
- *(fc-traits-authn)* `verification_weight` moves from `DeviceChallengeResponse` /
  `UserChallengeResponse` to the authenticator traits. Implementors of `Authenticator` or
  `UserAuthenticator` must set `type WeightInfo` (use `()` to keep reporting zero). The `Auth`,
  `Dev`, `Dummy` and `DummyDev` aliases take an optional weight parameter that defaults to `()`,
  and `composite_authenticator!` dispatches to each member's weights.
- *(fc-pallet-pass)* `register`, `add_device` and `PassAuthenticate` read the verification weight
  through the authenticator.

### Fixed

- Clippy warnings in benchmark code (useless conversions) and an unused import (#105).

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
