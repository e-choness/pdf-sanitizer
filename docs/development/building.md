# Building

The app ships as a single Windows `.exe` with the frontend embedded. It can
be built natively on Windows, or cross-compiled from Linux, macOS or Windows
with Docker.

::: warning Always enable `custom-protocol` for release builds
The `custom-protocol` feature embeds `dist/` into the binary. Without it,
Tauri builds in dev mode and the window points at `http://localhost:5173`,
showing a blank page.
:::

## Docker (any OS)

Cross-compiles for `x86_64-pc-windows-msvc` with
[cargo-xwin](https://github.com/rust-cross/cargo-xwin); no Windows machine or
Visual Studio is needed.

```bash
# Build and write pdf-sanitizer.exe to the current directory
docker build --target export --output . .
```

For quicker local test builds, use the `fast-release` profile. It skips
link-time optimization, so the binary is a little larger and slower, but an
incremental rebuild takes seconds instead of about a minute:

```bash
docker build --build-arg PROFILE=fast-release --target export --output . .
```

::: details Extracting from an image instead
```bash
docker build -t pdf-sanitizer-builder .
docker create --name extract pdf-sanitizer-builder
docker cp extract:/pdf-sanitizer.exe ./pdf-sanitizer.exe
docker rm extract
```
:::

### Build caching

Rebuilds only redo what changed:

- The toolchain (system packages, Node, pnpm, cargo-xwin) is its own stage and
  is only rebuilt when that part of the Dockerfile changes.
- `pnpm install` runs from `package.json` and `pnpm-lock.yaml` alone, so
  source edits don't reinstall packages.
- Downloaded crates, the MSVC CRT and Windows SDK fetched by cargo-xwin, the
  pnpm store and `src-tauri/target` are kept in BuildKit cache mounts.

Clear the caches with `docker builder prune`.

## Native Windows build

Requires the [local setup](./setup) prerequisites.

```bash
pnpm install
pnpm build
cd src-tauri
cargo build --release --features custom-protocol
# → src-tauri/target/release/pdf-sanitizer.exe
```

Use `--profile fast-release` instead of `--release` for a quicker build; the
output is then in `target/fast-release/`.

## Build profiles

| Profile | LTO | Codegen units | Use for |
|---|---|---|---|
| `release` | Full | 1 | Published releases — smallest, fastest binary |
| `fast-release` | Off | 16, incremental | Local testing of optimized builds |

Both use `panic = "abort"`. The profiles are defined in
`src-tauri/Cargo.toml`.
