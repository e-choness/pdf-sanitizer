#!/usr/bin/env node
// Release helper. docs/changelog.md (Keep a Changelog format) is the single
// source of truth for release notes.
//
//   node scripts/release.mjs prepare <version>
//       Move the [Unreleased] entries under "## [<version>] - <today>",
//       update the compare links, and set <version> in tauri.conf.json,
//       package.json, src-tauri/Cargo.toml and src-tauri/Cargo.lock.
//
//   node scripts/release.mjs notes <tag>
//       Print the changelog section for <tag> (e.g. v1.2.0) to stdout.
//       Falls back to [Unreleased] for pre-releases or versions without a
//       section, and prints nothing if both are empty.
//
//   node scripts/release.mjs check <tag>
//       Exit non-zero unless <tag> matches the version in tauri.conf.json.

import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const CHANGELOG = path.join(root, 'docs', 'changelog.md')
const REPO = 'https://github.com/e-choness/pdf-sanitizer'
const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/

const read = (file) => fs.readFileSync(file, 'utf8')

// Keep each file's existing line endings (Windows checkouts may use CRLF).
function write(file, text) {
  const eol = read(file).includes('\r\n') ? '\r\n' : '\n'
  fs.writeFileSync(file, text.replace(/\r?\n/g, eol))
}

const lines = (text) => text.split(/\r?\n/)
const stripV = (tag) => tag.replace(/^refs\/tags\//, '').replace(/^v/, '')

function fail(msg) {
  console.error(`error: ${msg}`)
  process.exit(1)
}

/** Body of the "## [name]" section, without its heading, trimmed. */
function section(text, name) {
  const all = lines(text)
  const start = all.findIndex((l) => l.startsWith(`## [${name}]`))
  if (start === -1) return ''
  let end = all.findIndex((l, i) => i > start && (l.startsWith('## ') || /^\[[^\]]+\]:\s/.test(l)))
  if (end === -1) end = all.length
  return all.slice(start + 1, end).join('\n').trim()
}

function appVersion() {
  return JSON.parse(read(path.join(root, 'src-tauri', 'tauri.conf.json'))).version
}

function notes(tag) {
  if (!tag) fail('usage: release.mjs notes <tag>')
  const text = read(CHANGELOG)
  const body = section(text, stripV(tag)) || section(text, 'Unreleased')
  if (body) process.stdout.write(body + '\n')
}

function check(tag) {
  if (!tag) fail('usage: release.mjs check <tag>')
  const want = stripV(tag)
  const have = appVersion()
  if (want !== have) {
    fail(
      `tag ${tag} does not match the app version ${have} in src-tauri/tauri.conf.json.\n` +
        `Run "node scripts/release.mjs prepare ${want}", commit, and tag that commit.`,
    )
  }
  console.log(`version ${have} matches ${tag}`)
}

function prepare(version) {
  if (!version || !SEMVER.test(version)) fail('usage: release.mjs prepare <x.y.z>')
  const previous = appVersion()
  let text = read(CHANGELOG)

  if (section(text, version)) fail(`docs/changelog.md already has a [${version}] section`)
  if (!section(text, 'Unreleased')) {
    fail('the [Unreleased] section in docs/changelog.md is empty — add the changes first')
  }

  const today = new Date().toISOString().slice(0, 10)
  text = text.replace(/^## \[Unreleased\].*$/m, `## [Unreleased]\n\n## [${version}] - ${today}`)

  // Compare links: [Unreleased] now starts at the new tag; add one for it.
  const link = `[${version}]: ${REPO}/compare/v${previous}...v${version}`
  if (/^\[Unreleased\]:.*$/m.test(text)) {
    text = text.replace(
      /^\[Unreleased\]:.*$/m,
      `[Unreleased]: ${REPO}/compare/v${version}...HEAD\n${link}`,
    )
  } else {
    text = text.trimEnd() + `\n\n[Unreleased]: ${REPO}/compare/v${version}...HEAD\n${link}\n`
  }
  write(CHANGELOG, text)

  // tauri.conf.json and package.json: first "version" field.
  for (const rel of ['src-tauri/tauri.conf.json', 'package.json']) {
    const file = path.join(root, rel)
    write(file, read(file).replace(/"version":\s*"[^"]*"/, `"version": "${version}"`))
  }

  // Cargo.toml: the version line inside [package].
  const cargoToml = path.join(root, 'src-tauri', 'Cargo.toml')
  write(
    cargoToml,
    read(cargoToml).replace(/(\[package\][^[]*?\nversion\s*=\s*)"[^"]*"/, `$1"${version}"`),
  )

  // Cargo.lock: the pdf-sanitizer package entry, so CI's --locked builds pass.
  const cargoLock = path.join(root, 'src-tauri', 'Cargo.lock')
  if (fs.existsSync(cargoLock)) {
    write(
      cargoLock,
      read(cargoLock).replace(
        /(name = "pdf-sanitizer"\r?\nversion = )"[^"]*"/,
        `$1"${version}"`,
      ),
    )
  }

  console.log(`Prepared ${version} (was ${previous}). Review the changes, then:

  git commit -am "release: v${version}"
  git tag v${version}
  git push origin HEAD v${version}`)
}

const [cmd, arg] = process.argv.slice(2)
const commands = { prepare, notes, check }
if (!commands[cmd]) fail('usage: release.mjs <prepare|notes|check> <version|tag>')
commands[cmd](arg)
