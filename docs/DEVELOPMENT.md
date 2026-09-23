# Development Guide

## Project Structure Overview

This is a Tauri + Rust + Svelte desktop application for PDF sanitization.

```
PDFSanitizer/
├── src/                        # Frontend (Svelte + JavaScript)
│   ├── App.svelte             # Root component
│   ├── main.js                # Entry point
│   ├── App.css                # Global styles
│   └── components/
│       ├── FileList.svelte    # File management UI
│       ├── FileRow.svelte     # Individual file row with controls
│       └── Settings.svelte    # Settings panel
├── src-tauri/                 # Backend (Rust)
│   ├── src/
│   │   ├── main.rs            # Tauri app setup, IPC commands
│   │   ├── lib.rs             # Module exports
│   │   ├── pdf_sanitizer.rs   # PDF processing logic
│   │   └── settings.rs        # Settings persistence
│   ├── Cargo.toml             # Rust dependencies
│   ├── tauri.conf.json        # Tauri configuration
│   └── build.rs               # Build script
├── index.html                 # HTML template
├── package.json               # Frontend dependencies
├── vite.config.js             # Vite bundler config
├── svelte.config.js           # Svelte compiler config
├── Dockerfile                 # Docker build config
├── docker-compose.yml         # Docker Compose setup
└── setup.sh                   # Setup script for Docker
```

## Frontend Architecture

### Components

**FileList.svelte** - Main container for file management
- Drag-drop zone with visual feedback
- Select All / Unselect All checkbox
- Per-file controls (stop, remove)
- Bulk operations (remove all, stop all)
- Start Converting button

**FileRow.svelte** - Individual file display
- File icon and name
- Semi-transparent progress bar overlay
- Original size → Output size display
- Stop (while processing) or Remove (when done) button

**Settings.svelte** - Configuration panel
- Sanitization toggles (7 options)
- Output folder selector
- Concurrent processing slider (1-8)

### Communication Flow

Frontend → Backend (Tauri IPC Commands):
- `load_settings` - Load persisted settings
- `save_settings` - Persist settings to disk
- `process_files` - Start processing files with concurrency control

File/folder dialogs are handled entirely on the frontend via `@tauri-apps/plugin-dialog` — no Rust command needed.

Backend → Frontend (Tauri Events):
- `file_progress` - Progress update for a file
- `file_complete` - File processing complete with output size
- `file_error` - Processing error for a file

## Backend Architecture

### Main Entry Point (main.rs)

- Manages `AppState` with settings and task counter
- Implements Tauri commands that invoke Rust functions
- Handles async processing with Tokio
- Uses Semaphore for concurrency limiting

### PDF Sanitizer (pdf_sanitizer.rs)

- `sanitize_pdf()` - Main sanitization pipeline
- `remove_metadata()` - Strips document metadata
- `remove_scripts()` - Removes JavaScript and actions
- `remove_embedded_files()` - Removes attachments
- `strip_external_links()` - Removes URL references

### Settings (settings.rs)

- Loads/saves settings from user config directory
- Uses JSON serialization with Serde
- Persists user preferences between sessions

## Building

### With Docker (Cross-compile Windows .exe)

```bash
# Build and write pdf-sanitizer.exe to the current directory
# (uses cargo-xwin to cross-compile the Windows .exe from Linux)
docker build --target export --output . .

# Or build an image and extract the .exe from it
docker build -t pdf-sanitizer-builder .
docker create --name extract pdf-sanitizer-builder
docker cp extract:/pdf-sanitizer.exe ./pdf-sanitizer.exe
docker rm extract
```

For quicker local test builds, skip link-time optimization with the
`fast-release` profile (the binary is somewhat larger and slower; releases
always use `release`):

```bash
docker build --build-arg PROFILE=fast-release --target export --output . .
```

Rebuilds are incremental: crate downloads, the MSVC CRT/Windows SDK fetched by
cargo-xwin, the pnpm store and `src-tauri/target` are kept in BuildKit cache
mounts, so only what changed is re-downloaded or recompiled. Run
`docker builder prune` to clear them.

### Local Setup (Requires Rust + Node.js)

```bash
# Install Rust: https://rustup.rs/
# Install Node.js 24+: https://nodejs.org/

# Install pnpm
npm install -g pnpm

# Install dependencies
pnpm install

# Run in development mode
pnpm tauri dev

# Build release executable
pnpm tauri build
```

## Available Commands

### Frontend

```bash
pnpm install    # Install dependencies
pnpm dev        # Start Vite dev server (used by Tauri)
pnpm build      # Build frontend bundle
pnpm preview    # Preview production build
```

### Backend

```bash
cd src-tauri
cargo build              # Debug build
cargo build --release --features custom-protocol   # Optimized release build
cargo fmt               # Format code
cargo clippy            # Lint and suggestions
```

## Development Workflow

1. **Modify Frontend:**
   - Edit `.svelte` files in `src/components/`
   - Changes hot-reload automatically in dev mode

2. **Modify Backend:**
   - Edit `.rs` files in `src-tauri/src/`
   - Restart `cargo tauri dev` to see changes

3. **Add Dependencies:**
   - Frontend: `pnpm add package-name`
   - Backend: `cd src-tauri && cargo add package-name`

4. **Test Settings:**
   - Settings are stored in: `~/.config/pdf-sanitizer/settings.json` (Linux/Mac) or `%APPDATA%/pdf-sanitizer/settings.json` (Windows)

## File Processing Flow

1. **User adds files** via drag-drop or folder selection
2. **Frontend shows files** in list with "pending" status
3. **User clicks "Start Converting"**
4. **Backend processes files** with concurrency limit:
   - Reads PDF from disk
   - Applies sanitization filters based on settings
   - Moves original PDF to output folder
   - Writes sanitized PDF to original location
5. **Backend sends events** back to frontend:
   - `file_progress` every processing step
   - `file_complete` when done (with output size)
   - `file_error` if processing fails
6. **Frontend updates UI** in real-time
7. **User can remove files** from the list when done

## Testing

### Manual Testing Checklist

- [ ] Drag and drop PDF files
- [ ] Select files individually and with Select All
- [ ] Remove files (pending, processing, and done states)
- [ ] Start processing with different sanitization options
- [ ] Stop processing individual files
- [ ] Toggle settings and verify they persist
- [ ] Test concurrent processing with different thread counts
- [ ] Verify original PDFs move to backup folder
- [ ] Verify sanitized PDFs appear in original location
- [ ] Test with various PDF sizes and types

### Known Limitations

1. PDF processing is basic text manipulation - doesn't fully re-render PDFs
2. Progress reporting is per-file, not per-page
3. No drag-drop for file folders (only files)
4. Concurrent processing limited to 1-8 threads

## Performance Considerations

- **Concurrency:** Default 4 threads, adjustable 1-8
- **File Size:** Larger PDFs process slower (basic text parsing)
- **Memory:** Multiple concurrent files use more RAM
- **Disk:** Temporary double disk usage during processing

## Security Notes

- All processing happens locally - no network requests
- Settings stored in user config directory (unencrypted)
- PDFs processed in memory and written sequentially
- Original PDFs preserved in backup folder
- Consider running on isolated machine for sensitive PDFs

## Troubleshooting

### Build Issues

**"failed to find pdfium-render"**
- System might not have required dev libraries
- Use Docker build instead

**"failed to resolve: use of undeclared type"**
- Run `cargo clean` in src-tauri directory
- Rebuild with `cargo tauri build`

### Runtime Issues

**File picker / drag-and-drop not working**
- Ensure `dialog:allow-open` is in `src-tauri/capabilities/default.json`
- Ensure `tauri-plugin-dialog` is registered in `main.rs` via `.plugin(tauri_plugin_dialog::init())`

**Files not moving to backup folder**
- Verify output folder path is set in settings
- Check folder permissions
- Try with a different folder path

## Contributing

1. Follow Rust naming conventions (snake_case for functions)
2. Use Svelte's reactive syntax (`$:` for reactive statements)
3. Keep components focused and reusable
4. Add comments for complex logic
5. Test before committing

## Resources

- [Tauri Documentation](https://tauri.app/docs/)
- [Svelte Guide](https://svelte.dev/docs)
- [Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Guide](https://tokio.rs/)
