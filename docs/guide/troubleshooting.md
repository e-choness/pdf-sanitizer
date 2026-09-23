# Troubleshooting

Error messages shown on a file's row, and what to do about them.

### Encrypted PDF – can't be sanitized without the password

The PDF needs a password to open. PDF Sanitizer can't decrypt it. Open it in
a viewer with the password, save or print an unprotected copy, and sanitize
that.

PDFs that open *without* a password but restrict printing or copying are
processed normally.

### Not a PDF file (missing %PDF header)

The file does not start with a PDF header — it may be another format with a
`.pdf` extension, or truncated.

### Couldn't parse this PDF

The file is damaged or uses structures the PDF parser does not support. Try
re-saving it from a PDF viewer ("Save as" or "Print to PDF") and sanitize the
new copy.

### Sanitized result failed verification; original left untouched

The cleaned file did not pass the [checks](./sanitization#verification), so
it was discarded. Your original has not been changed. Try again with *Font
subsetting* and *Compress images* turned off; if it still fails, please
[open an issue](https://github.com/e-choness/pdf-sanitizer/issues).

### Backup folder is not writable

No backup folder is set, or the app can't create or write to it. Choose a
folder you own in **Settings → Output**.

### Disk error / swap failed

The file could not be read, written or moved — most often because it is open
in another program, the disk is full, or it is on a read-only or network
location. Close other programs using the file and click **Retry failed**.

If the message says *"Original is in …"*, the original was already moved to
the backup folder and could not be put back automatically; copy it back from
that path.

## The window is blank

The app needs the Microsoft Edge WebView2 Runtime. Install it from
[Microsoft](https://developer.microsoft.com/microsoft-edge/webview2/) and
start the app again.

## Reporting a bug

[Open an issue](https://github.com/e-choness/pdf-sanitizer/issues/new) with
the app version, the exact error text and, if you can share it, a sample PDF
that reproduces the problem.
