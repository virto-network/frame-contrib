# Contributing to FRAME Contrib

Thanks for helping out! This document covers how changes get from a branch into a
published release. Everything in it follows from three facts:

1. Every PR is **squash-merged**, and the squash commit is the **PR title**. The
   PR description is not kept in the commit.
2. All crates are released **in lockstep** under one version. The version bump for
   the next release is computed from the PR titles on `main`.
3. `CHANGELOG.md` is generated from those same titles.

So **the PR title is the release note.** Most of this guide is about getting it
right.

## Development

CI runs the following, and a PR needs all of them to pass:

```sh
cargo fmt --all -- --check
cargo clippy --release --locked --all-features --workspace
cargo test --release --locked --all-features --workspace
```

You need `protobuf-compiler`, and the `wasm32v1-none` target with `rust-src` for the
tests. Set `SKIP_WASM_BUILD=1` for faster `check`/`clippy` runs.

## PR titles

Titles must follow [Conventional Commits](https://www.conventionalcommits.org).
`lint-pr.yml` enforces this and won't let you merge otherwise:

```
<type>(<scope>)<!>: <description>
```

- **type** is one of `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`,
  `chore`, `style` or `revert`.
- **scope** is the crate name, e.g. `fc-pallet-pass` or `fc-traits-authn`. Use
  `deps` for dependency bumps. Leave it out for workspace-wide changes.
- **description** is imperative and lowercase, and reads well as a changelog entry
  for someone who uses the crate.
- **`!`** marks a breaking change. See below.

| Title | Next release | Changelog section |
| --- | --- | --- |
| `feat(fc-pallet-pass)!: per-device call filters` | **major** | ⚠ Breaking changes |
| `feat(fc-pallet-fees): add community fee layer` | minor | Added |
| `fix(fc-pallet-listings): off-by-one in pagination` | patch | Fixed |
| `perf(...)`, `refactor(...)`, `docs(...)`, `chore(...)` | patch | Performance / Changed / Documentation / Other |
| `test(...)`, `ci(...)`, `style(...)` | patch | *(not listed)* |

A bump only happens when the commit touches a crate's files. `ci:` changes, or
root-level `docs:`, never cause a release on their own.

### What counts as breaking (`!`)

Use `!` whenever a runtime or client that depends on this crate would need to
change something when it upgrades. For FRAME code, that is much more than the Rust
API:

- **A new Polkadot SDK line** (`stable2603` → `stable2606`). The SDK-upgrade bot
  does this for you.
- **`Config`:** adding, removing, renaming, or tightening the bounds of an
  associated type.
- **Calls:** changing a signature, a call index, or the argument order, or removing
  a call. Anything that changes call encoding breaks clients, even when the Rust
  code still compiles.
- **Storage:** changing a key or value type, the hasher, a prefix or a name.
  Include a migration and bump `STORAGE_VERSION`. If there's no migration, say so
  explicitly.
- **Events and errors:** removing or reordering variants, or changing their fields.
  Appending a new variant at the end is not breaking.
- **Public Rust API:** removing or renaming items, changing trait signatures,
  removing a crate or a re-export.
- **Cargo features:** moving code behind a feature, or changing defaults (e.g. #72,
  which moved most of `fc-traits-authn` behind `runtime`).

`cargo-semver-checks` runs on every release and **raises** the bump if it finds a
Rust API break that the title didn't declare. It **cannot** see storage, call
encoding or metadata changes, and those are the ones that break a live chain. For
example, #71 changed `SessionKeys` and two call signatures but was titled `feat:`,
and nothing caught it. The `!` is on you and your reviewer.

### Describe the migration in the PR

The PR description doesn't reach `main`, but the changelog links every entry to its
PR, so the description is where the details go. For a `!` PR, include a **Migration**
section that says what a runtime integrator has to do: new `Config` items and
sensible values, the migration to add to `Migrations`, and any storage that has to be
cleared.

## Changelog

`CHANGELOG.md` has one section per release and is written by the release workflow,
so you **don't edit it in feature PRs**.

1. On every push to `main`, [release-plz](https://release-plz.dev) opens or updates a
   `chore: release vX.Y.Z` PR with the next version.
2. The same workflow generates that version's section with
   [git-cliff](https://git-cliff.org) (`cliff.toml`). The section covers every PR
   title merged since the last `vX.Y.Z` tag, with breaking changes listed first and
   each entry linked to its PR.
3. **Before merging the release PR, a maintainer curates the section.** Add the
   migration notes from the `!` PRs under their entries, reword unclear titles, and
   merge related entries. Do this right before merging: any new push to `main`
   rebuilds the release PR and discards manual edits.
4. Merging publishes the crates and creates the GitHub release. The release notes
   are the curated section, copied as is.

To preview the next section locally:

```sh
git cliff --unreleased --tag vX.Y.Z --strip all
```

## Adding a crate

Every crate is released together with the rest, so a new crate needs to be wired
into the workspace:

- In its `Cargo.toml`, set `version.workspace = true` (plus `authors`, `edition`,
  `license` and `repository` from the workspace) and a `description`.
- Add it to `[workspace.dependencies]` in the root `Cargo.toml` with both `path` and
  `version`.
- Add a `[[package]]` entry with `version_group = "frame-contrib"` to
  `release-plz.toml`. Without it, the crate won't move in lockstep, and CI fails
  until you add it.
- Set `publish = false` if it should not go to crates.io, and add it to
  `release-plz.toml` with `release = false`.
- Use **path-only** dev-dependencies (no `version`) on `fc-mock-helpers` or any
  other workspace crate that depends back on yours, e.g.
  `mock-helpers = { package = "fc-mock-helpers", path = "../../mock-helpers" }`.
  `cargo publish` strips path-only dev-dependencies, which breaks the cycle.

## Polkadot SDK upgrades

You normally don't do these by hand. `sdk-upgrade.yml` opens a PR every time a new
`polkadot-stableYYMM[-N]` is released. If that PR is a **draft**, the build broke and
it needs someone to port the API changes. Push the fixes onto the PR's branch, mark
it ready, and **keep the `!`** in the title. See [RELEASING.md](./RELEASING.md) for
the full process, including how to run it manually.
