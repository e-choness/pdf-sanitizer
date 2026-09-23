# What gets removed

Each option in **Settings** controls one pass over the document. Passes walk
every object in the file — not just the obvious places — because actions and
scripts can hang off pages, annotations, form fields, outline items and
action chains.

## Remove metadata <Badge type="tip" text="on by default" />

- The document information dictionary (`/Info`): title, author, subject,
  keywords, creator/producer software, creation and modification dates.
- XMP metadata streams (`/Metadata`) on the catalog and on any other object.
- Application private data (`/PieceInfo`).

## Remove scripts <Badge type="tip" text="on by default" />

- Document-level JavaScript (the `/JavaScript` name tree) and XFA forms,
  which can carry scripts.
- Auto-run triggers: `/OpenAction` and additional actions (`/AA`) on the
  document, pages, annotations and form fields.
- Actions of these types, wherever they appear:

  | Action | Why it is removed |
  |---|---|
  | `JavaScript` | Runs code in the viewer |
  | `Launch` | Starts a program or opens a file |
  | `ImportData`, `SubmitForm`, `ResetForm` | Read or send form data |
  | `GoToE`, `GoToR` | Jump into embedded or other (remote) files |
  | `Rendition`, `Movie`, `Sound` | Play media, can run scripts |

Harmless navigation — internal links and bookmarks that jump to a page in the
same document — is kept.

## Remove embedded files <Badge type="tip" text="on by default" />

- The `/EmbeddedFiles` name tree (document attachments).
- File-attachment annotations (the paper-clip icons on pages).

## Strip external links <Badge type="info" text="off by default" />

- Link annotations that open a URL (`URI` actions) or another file (`GoToR`).
- The document base URI (`/URI` on the catalog).

Internal links are kept.

## Font subsetting <Badge type="info" text="off by default" />

Removes the outlines of glyphs that are never used from embedded TrueType
fonts, which can shrink documents that embed complete fonts.

Glyph numbers are kept as they are (unused glyphs become empty rather than
being renumbered), so text, widths and character mappings stay valid. If the
app cannot tell with certainty which glyphs a font uses, that font is left
unchanged.

## Compress images <Badge type="info" text="off by default" />

Re-encodes large images as JPEG at the chosen quality (Low 45, Medium 70,
High 85). An image is only converted if it is:

- 8-bit RGB or grayscale,
- at least 64 × 64 pixels,
- without a transparency mask, and
- stored in a form the app can decode.

The new version is only kept if it is at least 5% smaller than the original.

## Always applied

After the passes, objects that nothing references any more (such as removed
scripts and attachments) are dropped, and streams are compressed when the file
is written.

## Verification

Before the original is touched, the cleaned file is opened again and checked:

- it parses, and has a document catalog and page tree;
- it has the **same number of pages** as the original;
- every page content stream can still be decoded;
- if *Remove scripts* is on, no JavaScript, `/AA` or `/OpenAction` remains.

If any check fails, the temporary file is deleted, the original is left
untouched and the row shows *"Sanitized result failed verification"*.

## Limitations

- **Password-protected PDFs** that need a password to *open* cannot be
  processed. PDFs that open without a password but restrict printing or
  copying (an owner password only) are decrypted and processed normally.
- PDF Sanitizer removes active and hidden content. It does **not** re-render
  pages, so it does not neutralize malformed data designed to exploit a bug
  in a specific viewer's parser. For untrusted files from hostile sources,
  combine it with an up-to-date, sandboxed viewer.
