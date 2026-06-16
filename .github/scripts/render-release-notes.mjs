// Renders the release notes from the template, filling in the version/tag,
// the "what's changed" section (user-supplied via workflow input, or a
// default when left empty), and the command-line install snippets (which
// reference the actual built installer filenames).
//
// CHANGES is markdown. The "Run workflow" web form only offers a single-line
// text box, so literal "\n" sequences are converted to real line breaks,
// letting users type multi-line markdown (lists, headings, etc.) there too.
//
// Env: TAG (e.g. v1.5.2), VERSION (e.g. 1.5.2), CHANGES (optional),
//      GITHUB_REPOSITORY (e.g. PowerInterviewAI/client-app, set by Actions).

import {
  readFileSync,
  writeFileSync,
  appendFileSync,
  readdirSync,
  statSync,
  existsSync,
} from 'node:fs';
import { basename, join } from 'node:path';

const { TAG, VERSION, CHANGES, GITHUB_REPOSITORY } = process.env;
if (!TAG || !VERSION) {
  console.error('TAG and VERSION env vars are required');
  process.exit(1);
}

const DEFAULT_CHANGES = 'Manual cross-platform Tauri build.';

const changes = CHANGES?.trim() ? CHANGES.trim().replaceAll('\\n', '\n') : DEFAULT_CHANGES;

// Find the built installer filenames so the curl commands always match this
// release's assets. GitHub stores asset names with spaces replaced by dots.
function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const p = join(dir, entry);
    if (statSync(p).isDirectory()) out.push(...walk(p));
    else out.push(p);
  }
  return out;
}
const artifactNames = existsSync('release-artifacts')
  ? walk('release-artifacts').map((f) => basename(f))
  : [];
const toAssetName = (name) => name.replace(/ /g, '.');
const winAsset = toAssetName(artifactNames.find((f) => f.endsWith('-setup.exe')) ?? '');
const macAsset = toAssetName(artifactNames.find((f) => f.endsWith('.dmg')) ?? '');

const template = readFileSync('.github/release-notes-template.md', 'utf8');
const notes = template
  .replaceAll('{{TAG}}', TAG)
  .replaceAll('{{VERSION}}', VERSION)
  .replaceAll('{{CHANGES}}', changes)
  .replaceAll('{{REPO}}', GITHUB_REPOSITORY ?? 'PowerInterviewAI/client-app-tauri')
  .replaceAll('{{WIN_ASSET}}', winAsset)
  .replaceAll('{{MAC_ASSET}}', macAsset);

writeFileSync('release-notes.md', notes);
console.log(notes);

if (process.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, `## Release notes preview\n\n${notes}\n`);
}
