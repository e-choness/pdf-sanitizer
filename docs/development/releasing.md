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

Build workflows cache Rust dependencies, the pnpm store, the cargo-xwin binary
and the MSVC CRT/Windows SDK between runs.

## Versioning

Versions follow [Semantic Versioning](https://semver.org/):

- **Patch** `1.0.x` — bug fixes
- **Minor** `1.x.0` — new, backward-compatible features
- **Major** `x.0.0` — breaking changes

Pre-releases add a suffix: `v1.1.0-alpha.1`, `v1.1.0-beta.1`, `v1.1.0-rc.1`.

## Stable release

1. Make sure `main` is green in CI.
2. Update the version in all three places:
   - `src-tauri/tauri.conf.json` → `"version"`
   - `src-tauri/Cargo.toml` → `[package] version`
   - `package.json` → `"version"`
3. Move the *Unreleased* entries in [the changelog](../changelog) under the new
   version with today's date.
4. Commit, tag and push:

   ```bash
   git commit -am "release: v1.1.0"
   git tag v1.1.0
   git push origin main v1.1.0
   ```

5. `release.yml` builds the `.exe` and publishes the release. Check the
   [Actions tab](https://github.com/e-choness/pdf-sanitizer/actions), then
   edit the release notes on GitHub if needed.

## Beta / pre-release

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
