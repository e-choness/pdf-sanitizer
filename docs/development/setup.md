# Local setup

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 24+
- [pnpm](https://pnpm.io/) 9 — `npm install -g pnpm@9`, or `corepack enable`
- **Windows:** [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
  with the *Desktop development with C++* workload, and WebView2 (preinstalled
  on Windows 11)
- **Linux:** the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/#linux)
  (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, …)

## Run the app

```bash
git clone https://github.com/e-choness/pdf-sanitizer.git
cd pdf-sanitizer
pnpm install
pnpm tauri dev
```

`pnpm tauri dev` starts the Vite dev server on port 5173 and opens the app
window. Svelte changes hot-reload; Rust changes rebuild and restart the app.

## Using Docker instead

The `toolchain` image has Rust, Node, pnpm and cargo-xwin installed, so you
can run checks without installing anything locally:

```bash
docker compose run --rm pdf-sanitizer pnpm install
docker compose run --rm pdf-sanitizer pnpm test
docker compose run --rm pdf-sanitizer sh -c "cd src-tauri && cargo test -p pdfsan-core"
```

`node_modules`, the Cargo registry, the cargo-xwin SDK download and
`src-tauri/target` live in named volumes, so they persist between runs.

## Project layout

```text
.
├── src/                          # Frontend (Svelte 5)
│   ├── App.svelte                # Root component, Tauri event wiring
│   ├── app.css                   # Design tokens, light/dark theme
│   ├── components/               # FileList, FileRow, Settings
│   └── lib/store.js              # Stores and helpers (+ store.test.js)
├── src-tauri/                    # Desktop shell (Rust, Tauri v2)
│   ├── src/main.rs               # Tauri commands and app state
│   ├── src/pipeline.rs           # Batch runner: concurrency, cancel, backup, swap
│   ├── src/settings.rs           # Settings load/save
│   ├── crates/pdfsan-core/       # Sanitization library, no Tauri dependency
│   │   └── src/                  # sanitizer.rs, fonts.rs, error.rs, tests.rs
│   ├── capabilities/default.json # Tauri permission grants
│   └── tauri.conf.json           # App metadata and window config
├── docs/                         # This documentation site (VitePress)
├── .github/workflows/            # CI, release and docs pipelines
├── Dockerfile                    # Toolchain image + Windows cross-build
└── docker-compose.yml            # Dev container
```

## Useful commands

| Command | What it does |
|---|---|
| `pnpm tauri dev` | Run the app with hot reload |
| `pnpm build` | Build the frontend into `dist/` |
| `pnpm test` | Frontend unit tests (Vitest) |
| `pnpm check` | Svelte type and accessibility checks |
| `pnpm docs:dev` | Serve this documentation site locally |
| `cargo test -p pdfsan-core` | Core library tests (run in `src-tauri/`) |
| `cargo clippy --workspace -- -D warnings` | Rust lints (run in `src-tauri/`) |
| `cargo fmt` | Format Rust code (run in `src-tauri/`) |
