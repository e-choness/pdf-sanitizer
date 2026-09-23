# Changelog

All notable changes to PDF Sanitizer are documented here. The format is based
on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- GitHub release notes are generated from this changelog, followed by the
  list of merged pull requests. `pnpm release:prepare <version>` moves the
  unreleased entries under the new version and bumps the version everywhere.

## [1.0.1] - 2026-09-23

### Fixed

- PDFs that open without a password but are encrypted with AES (owner
  password / permission restrictions only) were rejected as *"Encrypted PDF"*.
  They are now decrypted and sanitized. Upgraded `lopdf` to 0.36.

### Changed

- License changed from MIT to the
  [PolyForm Noncommercial License 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0).
  Earlier releases remain available under MIT.
- Docker and CI builds cache dependencies, the MSVC SDK and compiled crates,
  so rebuilds only redo what changed.
- New `fast-release` build profile for quicker local test builds.
- Documentation moved to a GitHub Pages site.

## [1.0.0] - 2026-08-09

### Added

- Desktop app built with Tauri 2, Rust and Svelte, running fully offline.
- Drag-and-drop and file-picker input, batch processing with 1–8 concurrent
  files, per-file progress, stop, retry and remove.
- Sanitization options: remove metadata, remove scripts and actions, remove
  embedded files, strip external links, font subsetting and image
  recompression.
- Verification of each output before the original is replaced; originals are
  moved to a configurable backup folder.
- Settings persisted to the OS config directory.
- Windows `.exe` built via GitHub Actions (cargo-xwin cross-compilation) and a
  Docker build environment.

[Unreleased]: https://github.com/e-choness/pdf-sanitizer/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/e-choness/pdf-sanitizer/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/e-choness/pdf-sanitizer/releases/tag/v1.0.0
