# PDF Sanitizer

A desktop application for stripping potentially malicious content from PDF files. Built with Tauri v2, Rust, and Svelte 5. Runs entirely offline — no data leaves your machine.

## Features

- **Drag & drop** or file-picker to add PDFs
- **Batch processing** with configurable concurrency (1–8 files at once)
- **Per-file cancellation** — stop individual files mid-processing
- **Original backup** — originals are moved to a folder you choose; sanitized files replace them in-place
- **Sanitization options** (all toggleable):
  - Remove metadata (author, dates, software info)
  - Remove JavaScript and PDF actions
  - Remove embedded files and attachments
  - Strip external links
  - Font subsetting (keep only used glyphs)
  - Image recompression (Low / Medium / High JPEG quality)
- **Settings persistence** — saved to your OS config directory automatically

## How to Use

1. Launch `pdf-sanitizer.exe`
2. Drag PDF files onto the window, or click **Add files**
3. In the **Settings** panel, choose a **Backup folder** (required before processing)
4. Toggle sanitization options as needed
5. Click **Sanitize** — progress is shown per file
6. When done, sanitized PDFs are at the original paths; originals are in the backup folder

To stop a running batch, click **Stop all**. Individual files can be cancelled with the stop button on each row. Failed files can be retried.

## Download

Pre-built Windows binaries are available on the [Releases](../../releases) page.

## Development

### Local setup

**Prerequisites:**
- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 24+
- [pnpm](https://pnpm.io/) 9+
- Windows: [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with C++ workload

```bash
# Install frontend dependencies
pnpm install

# Start dev server + Tauri window
pnpm tauri dev
```

### Docker setup

**Prerequisites:** Docker with Linux containers.

```bash
# Run tests / checks inside Docker (no local Rust/Node needed)
docker compose run --rm pdf-sanitizer pnpm test
docker compose run --rm pdf-sanitizer pnpm check
docker compose run --rm pdf-sanitizer sh -c "cd src-tauri && cargo test -p pdfsan-core"
```

### Run tests

```bash
# Frontend (Vitest)
pnpm test

# Svelte type + a11y check
pnpm check

# Rust core library
cd src-tauri && cargo test -p pdfsan-core

# Rust lint
cd src-tauri && cargo clippy --workspace -- -D warnings
```

### Build release binary

**Local** (Windows, requires MSVC toolchain):
```bash
cd src-tauri && cargo build --release --features custom-protocol
# Output: src-tauri/target/release/pdf-sanitizer.exe
```

**Docker** (cross-compiles a Windows `.exe` from any OS):
```bash
# Build the image — this compiles the .exe inside the container
docker build -t pdf-sanitizer-builder .

# Extract the .exe to the current directory
docker create --name extract pdf-sanitizer-builder
docker cp extract:/pdf-sanitizer.exe ./pdf-sanitizer.exe
docker rm extract
```

The Docker build uses [`cargo-xwin`](https://github.com/rust-cross/cargo-xwin) to cross-compile for `x86_64-pc-windows-msvc` without a Windows host.

## CI / CD

Three GitHub Actions workflows live in `.github/workflows/`:

| Workflow | Trigger | What it does |
|---|---|---|
| `test.yml` | Push/PR to `main` or `develop` | Frontend tests, svelte-check, Rust fmt/clippy/tests |
| `release.yml` | Push a `v*` tag (or manual) | Builds Windows `.exe`, creates a GitHub Release |
| `beta.yml` | Manual (`workflow_dispatch`) | Builds Windows `.exe`, creates a pre-release with a custom version tag |

### Publishing a release

```bash
# Tag and push — release.yml fires automatically
git tag v1.0.0
git push origin v1.0.0
```

The workflow cross-compiles a Windows `.exe` from Linux using [`cargo-xwin`](https://github.com/rust-cross/cargo-xwin). No Windows runner is needed.

### Publishing a beta

Go to **Actions → Build Beta Release → Run workflow** and enter a version string like `v1.1.0-beta.1`. The binary is published as a pre-release.

## Project Structure

```
.
├── src/                        # Frontend (Svelte 5)
│   ├── App.svelte              # Root component, Tauri event wiring
│   ├── App.css                 # CSS design tokens (light/dark themes)
│   ├── main.js                 # Entry point
│   ├── lib/
│   │   ├── store.js            # Svelte stores + helper functions
│   │   └── store.test.js       # Vitest unit tests
│   └── components/
│       ├── FileList.svelte     # File list, toolbar, footer
│       ├── FileRow.svelte      # Per-file row with status pill + progress
│       └── Settings.svelte     # Settings panel with toggles + stepper
├── src-tauri/                  # Backend (Rust / Tauri v2)
│   ├── src/
│   │   ├── main.rs             # Tauri commands, AppState
│   │   ├── pipeline.rs         # Batch runner, concurrency, cancellation
│   │   └── settings.rs         # Settings load/save (OS config dir)
│   ├── crates/
│   │   └── pdfsan-core/        # Pure Rust sanitization library (no Tauri dep)
│   │       └── src/lib.rs      # SanitizationSettings, sanitize(), 9 unit tests
│   ├── capabilities/
│   │   └── default.json        # Tauri v2 capability grants
│   └── tauri.conf.json         # App metadata, window config
├── .github/workflows/          # CI/CD pipelines
├── index.html                  # HTML entry point
├── vite.config.js              # Vite + Vitest config
├── package.json                # Frontend deps (Svelte 5, Vite 8, Vitest 5)
└── pnpm-workspace.yaml         # pnpm workspace
```

## Stack

| Layer | Technology |
|---|---|
| UI framework | Svelte 5 |
| Build tool | Vite 8 |
| Desktop shell | Tauri v2 |
| Language | Rust 2021 |
| Async runtime | Tokio |
| Frontend tests | Vitest 5 |
| Package manager | pnpm 9 |
| Cross-compilation | cargo-xwin |

## License

Licensed under the [PolyForm Noncommercial License 1.0.0](LICENSE).
Free for personal, research, educational and other noncommercial use;
commercial use requires a separate license from the author.

Copyright (c) 2026 Beili (Echo) Yin
