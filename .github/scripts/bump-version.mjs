// Bumps the project version (major, minor, or patch) across every file that
// declares it, so a manual cross-platform release always ships a unique,
// consistent version number.
//
// Usage: node bump-version.mjs <major|minor|patch>
// Writes the new version to GITHUB_OUTPUT as `version=x.y.z` when available.

import { readFileSync, writeFileSync, appendFileSync } from 'node:fs';

const bumpType = process.argv[2];
if (!['major', 'minor', 'patch'].includes(bumpType)) {
  console.error(`Invalid bump type "${bumpType}". Expected major, minor, or patch.`);
  process.exit(1);
}

const PACKAGE_JSON = 'package.json';
const CARGO_TOML = 'src-tauri/Cargo.toml';
const CARGO_LOCK = 'src-tauri/Cargo.lock';
const TAURI_CONF = 'src-tauri/tauri.conf.json';
const RUST_PACKAGE_NAME = 'power-interview-ai';

const currentVersion = JSON.parse(readFileSync(PACKAGE_JSON, 'utf8')).version;
const [major, minor, patch] = currentVersion.split('.').map(Number);

let nextVersion;
if (bumpType === 'major') nextVersion = `${major + 1}.0.0`;
else if (bumpType === 'minor') nextVersion = `${major}.${minor + 1}.0`;
else nextVersion = `${major}.${minor}.${patch + 1}`;

function replaceVersionLine(content, regex, file) {
  if (!regex.test(content)) {
    console.error(`Could not find a version field to update in ${file}`);
    process.exit(1);
  }
  return content.replace(regex, (line) => line.replace(currentVersion, nextVersion));
}

// package.json and tauri.conf.json each have a single top-level "version" field.
for (const file of [PACKAGE_JSON, TAURI_CONF]) {
  const content = readFileSync(file, 'utf8');
  const updated = replaceVersionLine(content, /"version":\s*"[^"]+"/, file);
  writeFileSync(file, updated);
}

// Cargo.toml has multiple `version = "..."` lines (dependencies too), so only
// touch the one inside the [package] section.
{
  const content = readFileSync(CARGO_TOML, 'utf8');
  const packageSection = content.match(/\[package\][\s\S]*?(?=\n\[|$)/);
  if (!packageSection) {
    console.error(`Could not find [package] section in ${CARGO_TOML}`);
    process.exit(1);
  }
  const updatedSection = replaceVersionLine(packageSection[0], /version\s*=\s*"[^"]+"/, CARGO_TOML);
  writeFileSync(CARGO_TOML, content.replace(packageSection[0], updatedSection));
}

// Cargo.lock mirrors the crate's own [[package]] entry.
{
  const content = readFileSync(CARGO_LOCK, 'utf8');
  const entryRegex = new RegExp(
    `(\\[\\[package\\]\\]\\nname = "${RUST_PACKAGE_NAME}"\\nversion = ")[^"]+(")`
  );
  if (!entryRegex.test(content)) {
    console.error(`Could not find "${RUST_PACKAGE_NAME}" entry in ${CARGO_LOCK}`);
    process.exit(1);
  }
  writeFileSync(CARGO_LOCK, content.replace(entryRegex, `$1${nextVersion}$2`));
}

console.log(`Bumped version: ${currentVersion} -> ${nextVersion}`);

if (process.env.GITHUB_OUTPUT) {
  appendFileSync(process.env.GITHUB_OUTPUT, `version=${nextVersion}\n`);
}
