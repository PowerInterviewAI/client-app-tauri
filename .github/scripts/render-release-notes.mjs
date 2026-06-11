// Renders the release notes from the template, filling in the version/tag
// and the "what's changed" section (user-supplied via workflow input, or a
// default when left empty).
//
// Env: TAG (e.g. v1.5.2), VERSION (e.g. 1.5.2), CHANGES (optional).

import { readFileSync, writeFileSync } from 'node:fs';

const { TAG, VERSION, CHANGES } = process.env;
if (!TAG || !VERSION) {
  console.error('TAG and VERSION env vars are required');
  process.exit(1);
}

const DEFAULT_CHANGES = 'Manual cross-platform Tauri build.';

const template = readFileSync('.github/release-notes-template.md', 'utf8');
const notes = template
  .replaceAll('{{TAG}}', TAG)
  .replaceAll('{{VERSION}}', VERSION)
  .replaceAll('{{CHANGES}}', CHANGES?.trim() || DEFAULT_CHANGES);

writeFileSync('release-notes.md', notes);
console.log(notes);
