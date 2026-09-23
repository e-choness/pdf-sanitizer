# Testing

## Automated checks

Run these before opening a pull request — CI runs the same set
(`.github/workflows/test.yml`) on every push and pull request to `main` and
`develop`.

```bash
# Frontend
pnpm test          # Vitest unit tests (src/lib/store.test.js)
pnpm check         # svelte-check: types and accessibility

# Rust (from src-tauri/)
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test -p pdfsan-core
cargo check --workspace
```

The `pdfsan-core` tests build small PDFs in memory and check each pass,
verification and error case, so they need no fixture files.

## Adding a core test

Tests live in `src-tauri/crates/pdfsan-core/src/tests.rs`. Build a
`lopdf::Document`, save it to a temporary path, run `sanitize()` with the
settings under test, then load the output and assert on what is (or is not)
left. When fixing a bug, add a test that fails without the fix.

## Manual checklist

Before a release, run the built `.exe` and check:

- [ ] Drag-and-drop and **Add files…** both add PDFs
- [ ] A batch with several files completes; the summary shows the bytes saved
- [ ] Sanitized files are at the original paths; originals are in the backup
      folder, with `(2)` suffixes on name clashes
- [ ] **stop** on one row and **Stop all** cancel without leaving `.tmp` files
- [ ] **Retry failed** works after fixing the cause (e.g. closing the file
      in another program)
- [ ] A password-protected PDF fails with the *Encrypted PDF* message and is
      left untouched
- [ ] Settings persist after restarting the app
- [ ] Light and dark system themes both render correctly
