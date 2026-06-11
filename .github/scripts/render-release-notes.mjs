// Renders the release notes from the template, filling in the version/tag
// and the "what's changed" section (user-supplied via workflow input, or a
// default when left empty).
//
// CHANGES is markdown. The "Run workflow" web form only offers a single-line
// text box, so literal "\n" sequences are converted to real line breaks,
// letting users type multi-line markdown (lists, headings, etc.) there too.
//
// Env: TAG (e.g. v1.5.2), VERSION (e.g. 1.5.2), CHANGES (optional).

import { readFileSync, writeFileSync, appendFileSync } from 'node:fs';

const { TAG, VERSION, CHANGES } = process.env;
if (!TAG || !VERSION) {
  console.error('TAG and VERSION env vars are required');
  process.exit(1);
}

const DEFAULT_CHANGES = 'Manual cross-platform Tauri build.';

const changes = CHANGES?.trim() ? CHANGES.trim().replaceAll('\\n', '\n') : DEFAULT_CHANGES;

const template = readFileSync('.github/release-notes-template.md', 'utf8');
const notes = template
  .replaceAll('{{TAG}}', TAG)
  .replaceAll('{{VERSION}}', VERSION)
  .replaceAll('{{CHANGES}}', changes);

writeFileSync('release-notes.md', notes);
console.log(notes);

if (process.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, `## Release notes preview\n\n${notes}\n`);
}
