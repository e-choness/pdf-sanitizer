---
layout: home

hero:
  name: PDF Sanitizer
  text: Clean PDFs before you open or share them
  tagline: Strips scripts, attachments, outbound links and metadata from PDF files. A single offline Windows app — nothing leaves your machine.
  image:
    src: /logo.svg
    alt: PDF Sanitizer
  actions:
    - theme: brand
      text: Get started
      link: /guide/getting-started
    - theme: alt
      text: Download for Windows
      link: https://github.com/e-choness/pdf-sanitizer/releases/latest
    - theme: alt
      text: View on GitHub
      link: https://github.com/e-choness/pdf-sanitizer

features:
  - icon: 🛡️
    title: Removes active content
    details: JavaScript, auto-run actions, launch/submit actions, XFA forms, embedded files and file-attachment annotations.
    link: /guide/sanitization
  - icon: 🔒
    title: Fully offline
    details: No network access, no telemetry, no account. PDFs are processed in memory on your own machine.
  - icon: ✅
    title: Verified before replacing
    details: Every output is re-parsed and checked (page count, content streams, no leftover scripts). If a check fails, the original is left untouched.
    link: /guide/sanitization#verification
  - icon: 🗂️
    title: Originals kept safe
    details: The original file is moved to a backup folder you choose; the sanitized copy takes its place at the same path.
    link: /guide/usage
  - icon: ⚡
    title: Batch processing
    details: Drop in many files and process up to 8 in parallel, with per-file progress, stop and retry.
  - icon: 🪶
    title: Smaller files, optionally
    details: Subset embedded TrueType fonts and re-encode large images as JPEG to cut file size.
    link: /guide/settings
---
