# Getting started

PDF Sanitizer is a desktop app that removes potentially dangerous or
privacy-leaking content from PDF files — scripts, auto-run actions, embedded
files, outbound links and metadata — and replaces each file with a cleaned
copy. Everything runs locally; the app makes no network requests.

## Requirements

- Windows 10 or 11 (x64)
- [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) —
  already installed on Windows 11 and on up-to-date Windows 10

## Install

1. Open the [latest release](https://github.com/e-choness/pdf-sanitizer/releases/latest).
2. Download `pdf-sanitizer.exe`.
3. Run it. There is no installer — the `.exe` is the whole app, so you can
   keep it anywhere, including a USB stick.

::: tip Windows SmartScreen
The executable is not code-signed, so Windows may show *"Windows protected
your PC"* the first time. Click **More info → Run anyway**.
:::

::: details Pre-release builds
Beta and release-candidate builds are published as
[pre-releases](https://github.com/e-choness/pdf-sanitizer/releases) with
versions like `v1.1.0-beta.1`. They are for testing and may change before the
stable release.
:::

## Sanitize your first file

1. Drag one or more PDFs onto the window, or click **Add files…**.
2. In **Settings → Output**, choose a **Backup folder**. Originals are moved
   here, so pick somewhere with enough free space.
3. Click **Sanitize**.

When a file finishes, the cleaned PDF sits at the original path and the
untouched original is in the backup folder.

## Next steps

- [Using the app](./usage) — batches, stopping, retrying and what happens to
  your files
- [What gets removed](./sanitization) — exactly what each option strips
- [Settings](./settings) — every option and its default
