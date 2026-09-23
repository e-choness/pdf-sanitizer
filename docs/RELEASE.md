# Release & Build Process

This project uses GitHub Actions to automatically build executables for Windows, macOS, and Linux.

## How It Works

### Automatic Builds via GitHub Actions

**Three workflows are configured:**

1. **release.yml** - Builds & releases executables when you push a tag (stable releases only)
2. **beta.yml** - Builds beta/RC releases manually (triggered on-demand via workflow_dispatch)
3. **test.yml** - Runs tests & linting on every push/PR

---

## Building a Release

### Step 1: Commit Your Changes

```bash
git add .
git commit -m "version x.x.x: Add feature/fix"
git push origin main
```

### Step 2: Create a Version Tag

```bash
# Tag format: v<major>.<minor>.<patch>
git tag v1.0.0
git push origin v1.0.0
```

### Step 3: GitHub Actions Builds Automatically

1. Go to **GitHub Repo** → **Actions** tab
2. Watch the **Build Release** workflow run
3. It builds for:
   - ✅ Windows (x64) → `.exe` (cross-compiled via cargo-xwin on ubuntu-latest)

### Step 4: Download Executables

**Option A: From Artifacts (During Build)**
- While workflow is running, click the workflow name
- Scroll to "Artifacts" section
- Download desired platform executable

**Option B: From Releases (After Build)**
- Go to **Releases** section on GitHub
- Find your version tag (e.g., `v1.0.0`)
- Download the executable for your platform

---

## Building a Beta Release

Beta and RC (Release Candidate) releases are for testing purposes only. They don't require a git tag and are triggered manually.

### Step 1: Prepare Beta Code

```bash
git add .
git commit -m "beta: add new experimental features"
git push origin main
```

### Step 2: Trigger Beta Build Manually

1. Go to **GitHub Repo** → **Actions** tab
2. Click **Build Beta Release** workflow
3. Click "Run workflow" button
4. Enter beta version (e.g., `v1.1.0-beta.1`, `v1.0.0-rc.1`)
5. Click "Run workflow"

### Step 3: Monitor Build

- Watch the workflow run (~15-25 minutes)
- Windows binary built via cargo-xwin on ubuntu-latest

### Step 4: Download Beta Executables

- Beta releases appear in **Releases** tab
- Marked as **Pre-release** with ⚠️ warning
- Download desired platform

### Beta Version Format

Use semantic versioning with suffixes:

```
v<MAJOR>.<MINOR>.<PATCH>-<type>.<number>
```

Examples:
- `v1.1.0-beta.1` - First beta of 1.1.0
- `v1.1.0-beta.2` - Second beta of 1.1.0
- `v1.1.0-rc.1` - Release candidate 1
- `v2.0.0-alpha.1` - Alpha release

### Beta Release Workflow

```bash
# 1. Make experimental changes
git add .
git commit -m "beta: test new PDF rendering"
git push origin main

# 2. Go to GitHub Actions → Build Beta Release
# 3. Enter: v1.1.0-beta.1
# 4. Click "Run workflow"

# 5. Wait ~35 minutes for builds

# 6. Download from Releases (marked as pre-release)

# 7. After testing, either:
#    a) Release stable: git tag v1.1.0 && git push origin v1.1.0
#    b) Another beta: Trigger beta workflow again with v1.1.0-beta.2
```

**Key Differences:**
- ✅ Beta releases: Manual, marked as pre-release, no git tag required
- ✅ Stable releases: Automatic on tag push, not marked as pre-release

---

## Release Checklist

Before creating a release tag, ensure:

- [ ] All commits pushed to main
- [ ] Test workflow passed (green checkmark in Actions)
- [ ] Version number updated in `src-tauri/tauri.conf.json`:
  ```json
  "package": {
    "version": "1.0.0"
  }
  ```
- [ ] CHANGELOG updated (optional but recommended)
- [ ] No uncommitted changes locally

---

## Version Number Format

Use semantic versioning: `v<MAJOR>.<MINOR>.<PATCH>`

Examples:
- `v1.0.0` - Initial release
- `v1.1.0` - New features
- `v1.0.1` - Bug fixes
- `v2.0.0` - Breaking changes

---

## GitHub Actions Configuration

### Build Triggers
- **release.yml** - Runs when you push a tag matching `v*`
- **test.yml** - Runs on every push to main/develop and PR

### Build Environments
- **Windows:** ubuntu-latest (cross-compiled via cargo-xwin to x86_64-pc-windows-msvc)

### Build Time
- Windows: ~15-25 minutes (includes cargo-xwin install + cross-compilation)

---

## Downloading & Installing

### Windows Users
1. Download `pdf-sanitizer.exe` from Releases
2. Run it (no installation needed!)
3. If Windows SmartScreen warns, click "More info" → "Run anyway"

---

## Troubleshooting Builds

### Workflow Failed on GitHub

**Check logs:**
1. Go to **Actions** tab
2. Click failed workflow
3. Expand job logs to see error

**Common issues:**
- Rust compilation error → Fix in code, push new commit
- pnpm lock file outdated → Run `pnpm install` locally, commit
- Missing dependencies → Update `.github/workflows/release.yml`

### Can't Find Build Artifacts

**Wait for workflow to complete:**
- Look at Actions tab (orange = running, green = done, red = failed)
- Artifacts only appear after workflow finishes

**Artifacts expire after 90 days**
- Keep official Releases page updated instead

### Build Size Too Large

Typical sizes:
- Windows: 80-120 MB
- macOS: 100-150 MB
- Linux: 70-100 MB

If larger, check for:
- Duplicate dependencies in Cargo.toml
- Large binary assets in project
- Debug symbols (should be stripped in release)

---

## Manual Build (Local)

If you want to build locally without GitHub Actions:

```bash
# Build frontend first
pnpm build

# Build for current platform
cd src-tauri && cargo build --release --features custom-protocol

# Cross-compile Windows .exe from Linux/macOS (requires cargo-xwin)
cargo install cargo-xwin --locked
rustup target add x86_64-pc-windows-msvc
cargo xwin build --release --features custom-protocol --target x86_64-pc-windows-msvc
```

Windows output location: `src-tauri/target/x86_64-pc-windows-msvc/release/pdf-sanitizer.exe`

---

## Setting Up Releases Page

After first successful build:

1. Go to **Settings** → **General**
2. Under "Releases" section, enable:
   - ✅ "Include pre-releases"
   - ✅ "Set as latest release"

---

## CI/CD Best Practices

### Automatic Testing
- test.yml runs on every push/PR
- Prevents broken code from being released
- Fix errors before pushing to main

### Semantic Versioning
- Helps users understand what changed
- Easier to track updates

### Release Notes
- Describe changes, fixes, new features
- Users can see what's new before downloading

---

## Example: Complete Release Flow

```bash
# 1. Make changes and commit
git add src/
git commit -m "feat: add dark mode support"

# 2. Update version number
# Edit: src-tauri/tauri.conf.json
# Change "version": "1.0.0" → "1.1.0"

git add src-tauri/tauri.conf.json
git commit -m "bump version to 1.1.0"

# 3. Push to GitHub
git push origin main

# 4. Wait for test.yml to pass (check Actions tab)

# 5. Create release tag
git tag v1.1.0
git push origin v1.1.0

# 6. GitHub Actions builds automatically
# Monitor at: https://github.com/username/PDFSanitizer/actions

# 7. After ~35 minutes, download from Releases page
```

---

## Environment Variables (Optional)

To add custom build variables, edit `.github/workflows/release.yml`:

```yaml
env:
  APP_NAME: pdf-sanitizer
  RUST_BACKTRACE: 1
```

---

## Keeping Dependencies Fresh

GitHub has security scanning. To update dependencies:

```bash
# Update Rust
rustup update

# Update Node packages
pnpm update

# Update Cargo dependencies
cd src-tauri
cargo update
```

Then commit and test before releasing.

---

## Support

If builds fail:
1. Check workflow logs in GitHub Actions tab
2. Common fixes usually shown in error messages
3. Push fix to main and re-tag with incremented version

Example: If `v1.1.0` failed, next release is `v1.1.1` with fix
