# Architecture

PDF Sanitizer has three layers:

```text
┌─────────────────────────────┐   invoke()    ┌──────────────────────────┐
│ Svelte 5 UI  (src/)         │ ────────────▶ │ Tauri shell (src-tauri/) │
│ file list, settings, stores │ ◀──────────── │ commands, batch pipeline │
└─────────────────────────────┘    events     └────────────┬─────────────┘
                                                           │ sanitize()
                                                           ▼ verify_pdf()
                                              ┌──────────────────────────┐
                                              │ pdfsan-core (Rust crate) │
                                              │ lopdf-based sanitizer    │
                                              └──────────────────────────┘
```

- **`pdfsan-core`** does all PDF work and has no Tauri dependency, so it can
  be tested and reused on its own.
- **The Tauri shell** owns settings, concurrency, cancellation and the file
  moves around each sanitize call.
- **The UI** only holds view state; file dialogs are opened from the frontend
  with `@tauri-apps/plugin-dialog`.

## Commands (UI → Rust)

| Command | Purpose |
|---|---|
| `load_settings` / `save_settings` | Read and persist `SanitizationSettings` |
| `stat_files` | Sizes of newly added files |
| `process_files` | Start a batch |
| `cancel_file` / `cancel_all` | Cancel one file or the whole batch |

## Events (Rust → UI)

| Event | Payload |
|---|---|
| `file_progress` | `id`, `stage`, `pct` (and `pages` while verifying) |
| `file_complete` | `id`, `input_size`, `output_size`, `backup_path`, `report` |
| `file_error` | `id`, `error` (a user-facing message) |
| `file_cancelled` | `id` |
| `batch_complete` | Totals: succeeded, failed, cancelled, bytes saved |
| `batch_error` | `error` — the batch could not start (e.g. unwritable backup folder) |

## Processing a file

`pipeline::run_batch` limits parallelism with a Tokio semaphore
(`max_concurrent`, 1–8). Each file gets a child of the batch's cancellation
token, so **Stop all** cancels every file and **stop** cancels one.

For each file:

1. **Pre-flight** (once per batch): the backup folder must exist or be
   creatable, and be writable.
2. **`sanitize()`** — on a blocking thread:
   1. check the `%PDF-` header and load with lopdf (files that open with an
      empty password are decrypted here);
   2. run the enabled passes: metadata, scripts, embedded files, external
      links;
   3. optional font subsetting (`fonts.rs`) and image recompression;
   4. prune unreferenced objects, compress, and save to a temporary file
      `.<name>.sanitizing-<n>.tmp` beside the original.
3. **`verify_pdf()`** — reload the temporary file and check the catalog, page
   tree, page count, content-stream decoding and (if enabled) that no scripts
   remain. On failure the temp file is deleted and the original is untouched.
4. **Swap** — move the original into the backup folder (rename, or copy +
   fsync + delete across volumes), then rename the temp file onto the original
   path. If that rename fails, the original is restored.

Cancellation is checked between passes and between images, so a stop takes
effect quickly without leaving partial output behind.

## Error handling

`pdfsan_core::SanitizeError` holds every user-facing failure; its `Display`
text is what the UI shows. See [Troubleshooting](../guide/troubleshooting)
for the messages.

## Settings persistence

`settings.rs` stores `SanitizationSettings` as JSON in the OS config
directory (`dirs::config_dir()/pdf-sanitizer/settings.json`). Unknown or
missing fields fall back to defaults; an unparsable file is renamed to
`.bak`.
