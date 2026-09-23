# Settings

Settings are saved automatically as you change them and restored the next
time the app starts.

| Section | Setting | Default | Description |
|---|---|---|---|
| Content | Remove metadata | On | Author, dates, software, XMP |
| Content | Remove scripts | On | JavaScript, auto-run and dangerous actions |
| Content | Remove embedded files | On | Attachments and file-attachment annotations |
| Content | Strip external links | Off | Links to URLs and other files |
| Content | Font subsetting | Off | Drop unused glyphs from embedded TrueType fonts |
| Images | Compress images | Off | Re-encode large images as JPEG |
| Images | Quality | Medium | Low (45), Medium (70) or High (85) JPEG quality |
| Performance | Concurrent files | 4 | Files processed in parallel, 1–8 |
| Output | Backup folder | *(none)* | Where originals are moved. **Required** before processing |

See [What gets removed](./sanitization) for exactly what each content option
does.

## Choosing concurrency

Each file is processed on its own CPU thread and held in memory while it is
processed. Higher values finish large batches faster on multi-core machines;
lower values use less memory, which matters for very large PDFs.

## Backup folder

The folder is created if it does not exist, and the app checks that it can
write there before a batch starts. It can be on another drive — originals are
copied and then deleted when a plain move is not possible.

## Where settings are stored

| OS | Path |
|---|---|
| Windows | `%APPDATA%\pdf-sanitizer\settings.json` |
| macOS | `~/Library/Application Support/pdf-sanitizer/settings.json` |
| Linux | `~/.config/pdf-sanitizer/settings.json` |

If the file is unreadable, it is renamed to `settings.json.bak` and defaults
are used. Delete the file to reset all settings.
