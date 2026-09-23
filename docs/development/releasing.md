# Releasing

Releases are built by GitHub Actions on `ubuntu-latest`, cross-compiling the
Windows `.exe` with cargo-xwin. No Windows runner is needed.

## Workflows

| Workflow | Trigger | Result |
|---|---|---|
| `test.yml` | Push or PR to `main` / `develop` | Frontend tests, svelte-check, Rust fmt, clippy, tests |
| `release.yml` | Push a `v*` tag (or run manually) | Builds the `.exe` and publishes a GitHub Release |
| `beta.yml` | Run manually with a version | Builds the `.exe` and publishes a **pre-release** |
| `docs.yml` | Push to `main` touching `docs/` (or run manually) | Builds and deploys this site to GitHub Pages |
| `release-notes.yml` | Run manually with a tag | Rewrites an existing release's notes from the changelog |

Build workflows cache Rust dependencies, the pnpm store, the cargo-xwin binary
and the MSVC CRT/Windows SDK between runs.

## Versioning

Versions follow [Semantic Versioning](https://semver.org/):

- **Patch** `1.0.x` — bug fixes
- **Minor** `1.x.0` — new, backward-compatible features
- **Major** `x.0.0` — breaking changes

Pre-releases add a suffix: `v1.1.0-alpha.1`, `v1.1.0-beta.1`, `v1.1.0-rc.1`.

## Release notes come from the changelog

[`docs/changelog.md`](../changelog) is the single source of release notes.
As you work, add each user-visible change under **`## [Unreleased]`**, grouped
by *Added*, *Changed*, *Fixed* or *Removed*
([Keep a Changelog](https://keepachangelog.com/en/1.1.0/)). Merging a PR is a
good time to add its entry.

When a release is published, its GitHub release body is:

1. that version's changelog section, then
2. GitHub's generated list of merged pull requests and a *Full Changelog*
   compare link.

## Stable release

1. Make sure `main` is green in CI and *Unreleased* describes the changes.
2. Prepare the release:

   ```bash
   pnpm release:prepare 1.1.0
   ```

   This moves the *Unreleased* entries under `## [1.1.0] - <today>`, updates
   the compare links at the bottom of the changelog, and sets the version in
   `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
   and `package.json`. It refuses to run if *Unreleased* is empty.

3. Review the diff, then commit, tag and push:

   ```bash
   git commit -am "release: v1.1.0"
   git tag v1.1.0
   git push origin main v1.1.0
   ```

4. `release.yml` checks that the tag matches the app version, builds the
   `.exe` and publishes the release with the notes above. Follow it in the
   [Actions tab](https://github.com/e-choness/pdf-sanitizer/actions).

::: tip Fixing notes after publishing
Edit the version's section in the changelog on `main`, then run
**Actions → Update Release Notes** with the tag. The release body is rebuilt
from the changelog without rebuilding the app:

```bash
gh workflow run release-notes.yml -f tag=v1.1.0
```
:::

## Beta / pre-release

Pre-releases use the current *Unreleased* section as their notes, under a
"pre-release for testing" warning. Don't run `release:prepare` for them.

1. Push the code you want to test.
2. Open **Actions → Build Beta Release → Run workflow**, choose the branch and
   enter a version such as `v1.1.0-beta.1`.

   Or with the GitHub CLI:

   ```bash
   gh workflow run beta.yml -f beta_version=v1.1.0-beta.1
   ```

3. The build is published as a pre-release, so it is not shown as the
   *latest* release.

Iterate with `-beta.2`, `-rc.1`, …, then publish the stable version as above.

## Publishing the docs

The site is deployed by `docs.yml`. One-time setup: in the repository's
**Settings → Pages**, set **Source** to **GitHub Actions**.

Preview locally with:

```bash
pnpm docs:dev      # live reload at http://localhost:5173/pdf-sanitizer/
pnpm docs:build    # production build into docs/.vitepress/dist
```
