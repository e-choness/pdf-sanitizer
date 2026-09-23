# Changelog

All notable changes to PDF Sanitizer are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-08-09

### Added
- Initial release of PDF Sanitizer desktop application
- Modern Svelte UI with drag-and-drop file management
- Rust backend with concurrent PDF processing (1-8 configurable threads)
- Seven configurable sanitization options:
  - Remove PDF metadata (default ON)
  - Remove JavaScript/scripts (default ON)
  - Remove embedded files/attachments (default ON)
  - Strip external links/URLs (default OFF)
  - Font subsetting (default OFF)
  - Image compression (default OFF)
- Per-file progress tracking with visual progress bars
- Individual file controls (stop processing, remove from list)
- Bulk operations (select all, remove all, stop all)
- Settings persistence to local JSON
- Configurable backup folder for original PDFs
- Windows build (.exe) via GitHub Actions (ubuntu-latest + cargo-xwin cross-compilation)
- Docker development environment
- GitHub Actions automated build pipeline
- Comprehensive documentation

### Technical Details
- Built with Tauri 2.x, Rust 2021, Svelte 4, Vite 5.4
- Native file/folder dialogs via tauri-plugin-dialog (frontend-driven)
- Zero runtime dependencies (single executable)
- All processing runs locally (no network communication)
- Configurable concurrent processing for performance tuning

---

## Unreleased (Development)

### Planned for 1.1.0
- [ ] Full PDF re-rendering to images and back (more robust sanitization)
- [ ] Progress reporting per page (not just per file)
- [ ] Folder drag-drop support
- [ ] File type icons
- [ ] Sanitization presets (Quick, Standard, Maximum)
- [ ] Processing history/logs

### Planned for 2.0.0
- [ ] Dark mode support
- [ ] Custom output folder per file
- [ ] Pause/Resume processing
- [ ] Multi-language support
- [ ] Plugin system for custom filters

---

## Version Guidelines

### Patch Version (1.0.X)
- Bug fixes
- Minor UI improvements
- Documentation updates
- Performance optimizations

### Minor Version (1.X.0)
- New features (backward compatible)
- New sanitization options
- UI enhancements
- New functionality

### Major Version (X.0.0)
- Breaking changes
- Complete rewrites
- Major architecture changes
- Dropping support for platforms

---

## How to Create a Release

1. Update this file with changes under "Unreleased" section
2. Move changes to versioned section (e.g., [1.1.0])
3. Add release date
4. Update version in `src-tauri/tauri.conf.json`
5. Commit and push: `git push origin main`
6. Create tag: `git tag v1.1.0`
7. Push tag: `git push origin v1.1.0`
8. GitHub Actions automatically builds all platforms

---

## Release History

| Version | Date | Windows | Status |
|---------|------|---------|--------|
| 1.0.0 | 2026-08-09 | ✅ | Released |

---

## Support

For issues with a specific version, please include the version number when reporting.

Find releases at: https://github.com/YOUR_USERNAME/PDFSanitizer/releases
