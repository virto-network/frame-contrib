# Changelog

All notable changes to this project are documented in this file. Every crate in the
workspace is released together under the same version.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html),
with one extra rule: every new Polkadot SDK line is a major release.

## [Unreleased]

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
