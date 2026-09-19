# Changelog

All notable changes to this project are documented in this file. Every crate in the
workspace is released together under the same version. See
[CONTRIBUTING.md](./CONTRIBUTING.md#changelog) for how this file is maintained.

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

The changes below are relative to [`polkadot-stable2512`].

### ⚠ Breaking changes

- Update to polkadot-sdk `stable2603` ([#73](https://github.com/virto-network/frame-contrib/pull/73)).
  SDK crates move from `stable2512` (`frame-support` 45) to `stable2603`
  (`frame-support` 46).
- *(fc-pallet-pass)* Per-device call filters ([#71](https://github.com/virto-network/frame-contrib/pull/71)).
  This change alters the pallet's metadata, so clients must regenerate their bindings.
  - **Calls:** `add_device` and `add_session_key` take a new trailing
    `filter: DeviceFilter` argument, which changes their call encoding.
  - **Config:** new `SpendMatcher`, `CallMatcher`, `MaxFilteredCalls` and
    `MaxFilteredAssets`. `ScaleCallMatcher` is provided as a `CallMatcher`.
  - **Storage:**
    - New `DeviceFilters`. Devices without an entry are treated as `Admin`.
    - New `AuthenticatedDevice`.
    - The value of `SessionKeys` changes from `(AccountId, BlockNumber)` to
      `(AccountId, BlockNumber, DeviceFilter)`. **No migration is included**, so
      existing `SessionKeys` entries will not decode after the upgrade. Clear them
      or migrate them in the runtime.
  - **Errors:** new `PermissionEscalation`, `CallNotAllowed` and
    `NotAuthenticatedByDevice`. They are appended at the end, so existing error
    indices are unchanged.
- *(fc-pallet-referenda-tracks)* Tracks are organised into groups and sub-tracks
  ([#68](https://github.com/virto-network/frame-contrib/pull/68)).
  - **Config:**
    - `AdminOrigin` and `UpdateOrigin` are replaced by `CreateOrigin`,
      `GroupManagerCreateOrigin`, `GroupManagerOrigin` and `RemoveGroupOrigin`.
    - `TrackId` must implement `SplitId`.
  - **Calls:**
    - `insert` and `update` are replaced by `new_group_with_track` and
      `add_sub_track`.
    - `remove` now takes only the track id.
    - New `remove_group`, `set_decision_deposit`, `set_periods`, `set_curves` and
      `set_max_deciding`.
  - **Storage:** v0 → v1. Check your on-chain layout first:
    - Runtimes on the v0 layout (`Tracks` keyed by a flat `TrackId`) should add
      `fc_pallet_referenda_tracks::migration::MigrateV0ToV1` to their migrations.
      Each old track `n` becomes group `n`, sub-track `0`.
    - Runtimes whose data already matches v1 only need the storage version set
      to `1`. Kreivo does this with `SetCommunityTracksStorageVersion`.
- *(fc-traits-tracks)* The `fc-traits-tracks` crate and the
  `frame_contrib_traits::tracks` re-export are removed
  ([#68](https://github.com/virto-network/frame-contrib/pull/68)).
- *(fc-traits-authn)* `frame-support` is now optional, behind a new `runtime` feature
  that is on by default ([#72](https://github.com/virto-network/frame-contrib/pull/72)).
  Crates that depend on `fc-traits-authn` with `default-features = false`
  (e.g. pass-authenticators) must enable `runtime` to keep `Challenger`,
  `Authenticator`, `UserAuthenticator` and the `util` module.
- The `mock-helpers` package is renamed `fc-mock-helpers`. Keep using it under the
  `mock-helpers` dependency key with `package = "fc-mock-helpers"`.

### Added

- *(fc-pallet-fees)* New `fc-pallet-fees`: configurable protocol and community fee
  layers applied on asset transfers
  ([#70](https://github.com/virto-network/frame-contrib/pull/70)).

### Other

- Lockstep versioning, automated Polkadot SDK upgrades and crates.io releases
  ([#77](https://github.com/virto-network/frame-contrib/pull/77)).

[`polkadot-stable2509`]: https://github.com/virto-network/frame-contrib/tree/polkadot-stable2509
[`polkadot-stable2512`]: https://github.com/virto-network/frame-contrib/tree/polkadot-stable2512
