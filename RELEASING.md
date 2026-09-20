# Releasing FRAME Contrib

This is the maintainer side of the process. For PR titles, what counts as a breaking
change, and how the changelog is written, see [CONTRIBUTING.md](./CONTRIBUTING.md).

All crates in this workspace are released **in lockstep**: they share a single
version (`[workspace.package].version`), a single `CHANGELOG.md` and a single
`vX.Y.Z` git tag. A runtime that uses several `fc-*` crates should always depend on
the same version of all of them.

| Change | Bump |
| --- | --- |
| New Polkadot SDK line (`stable2603` → `stable2606`) | **major** |
| Breaking change: `Config`, calls, storage, events/errors, public API, features | **major** |
| New feature, backwards compatible | minor |
| Fix, or SDK patch release (`stable2603` → `stable2603-6`) | patch |

The bump is computed from the PR titles on `main` (`!` → major, `feat` → minor,
anything else → patch). `cargo-semver-checks` can raise it, but it can't see storage
or call-encoding changes. See
[What counts as breaking](./CONTRIBUTING.md#what-counts-as-breaking-).

## Tags

- `vX.Y.Z`: one per release, created by release-plz. Pin to these.
- `polkadot-stableYYMM`: a **moving** tag that points to the latest release built on
  that SDK line. The release workflow moves it. Use it when you want "whatever
  frame-contrib is current for `stable2603`". The `polkadot-stable2509` and
  `polkadot-stable2512` tags were added after the fact and mark what Kreivo `0.16.9`
  and `0.17.0-pre.1` shipped.

Consumers should use the crates.io versions. A bare `git = "..."` dependency floats
with `Cargo.lock` and has already caused metadata drift once. If you have to use
git, check that `Cargo.lock` points at a tagged commit. Adding `tag = "..."` in only
one place can **duplicate crates**: Cargo treats `?tag=` as a different source from
the bare URL, and other git dependencies (e.g. pass-authenticators) use the bare URL.

## Upgrading the Polkadot SDK

`polkadot-sdk-version` is the source of truth for the SDK line this workspace
targets.

**Automated:** `.github/workflows/sdk-upgrade.yml` runs every Monday. It also runs on
demand from the Actions tab, where you can give a specific version. It picks the
latest `polkadot-stableYYMM[-N]` release, rewrites every SDK dependency with
[psvm](https://github.com/paritytech/psvm), runs `cargo check`, and opens a PR:

- A new SDK line opens `feat(deps)!: update to polkadot-sdk ...`, so it is released
  as a major.
- A patch of the current line opens `fix(deps): ...`, so it is released as a patch.
- If `cargo check` fails, the PR is opened as a **draft** with the compiler errors
  in its description. Push the API migration onto that branch, then mark it ready.

**Manually:**

```sh
cargo install psvm --locked
psvm -v stable2606 -p Cargo.toml
echo stable2606 > polkadot-sdk-version
cargo check --workspace --all-features --all-targets
```

## Cutting a release

1. Merge PRs into `main` as usual.
2. [release-plz](https://release-plz.dev) keeps a `chore: release vX.Y.Z` PR open
   with the computed version, and the workflow writes that version's
   `CHANGELOG.md` section onto it with git-cliff.
3. **Curate the changelog section right before merging.** Add migration notes under
   the breaking entries, taking them from each PR's *Migration* section. Any new
   push to `main` rebuilds the release PR and discards manual edits. If you push a
   hand-written section, the workflow leaves it alone.
4. Merge the release PR. The `Release` workflow:
   - publishes every crate to crates.io in dependency order;
   - pushes `vX.Y.Z` and creates a GitHub release whose notes are the curated
     section;
   - moves `polkadot-stableYYMM`.

### One-time setup (repository settings)

- Secret `CARGO_REGISTRY_TOKEN`: a crates.io API token with the `publish-new` and
  `publish-update` scopes.
- Variable `CRATES_IO_PUBLISH` = `true`: enables the publishing job. Until it is
  set, only the release PR is maintained.
- Secret `RELEASE_PLZ_TOKEN` (recommended): a fine-grained PAT or GitHub App token
  with `contents` and `pull-requests` write access. PRs opened with the default
  `GITHUB_TOKEN` don't trigger CI, so without this secret the release PR and the SDK
  upgrade PRs never get their checks.
- Settings → General → Pull Requests → "Default commit message" for squash merges:
  choose **Pull request title**. With "Default to commit title", a single-commit PR
  takes the commit's title, and a `!` that is only on the PR title is lost.

### First publish

None of the crates exist on crates.io yet. crates.io rate-limits **new** crates to a
small burst followed by roughly one per 10 minutes, and this workspace publishes 18.
If the first run hits `429 Too Many Requests`, re-run the workflow later; crates that
were already published are skipped. You can also ask help@crates.io to raise the
limit for the initial upload.
