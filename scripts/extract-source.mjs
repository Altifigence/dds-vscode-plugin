// SPDX-FileCopyrightText: 2026 Altifigence
// SPDX-License-Identifier: Apache-2.0
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const dds = process.argv[2];
if (!dds) throw new Error('Usage: node scripts/extract-source.mjs /path/to/DDS');
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const sources = [
  ['src-tauri/src/external_editors.rs', true],
  ['src-tauri/src/external_editor_integration.rs', true],
  ['src-tauri/src/external_editors_tests.rs', true],
  ['src/desktopVsCodeIntegration.ts', true],
  ['ui/packages/console-core/src/lib/vsCodeIntegration.ts', true],
  ['ui/packages/console-core/src/lib/vsCodeIntegrationChanges.ts', true],
  ['security/desktop-runtime-security/src/lib.rs', false],
  ['engines/desktop-wsl-backend/src/transport.rs', false],
  ['engines/desktop-wsl-backend/src/lib.rs', false],
];
const inventory = {
  schema: 'dds-public-source-inventory-v1',
  sourceCommit: execFileSync('git', ['-C', dds, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
  license: 'Apache-2.0',
  scope: 'Only the listed original source files and inclusive line ranges are granted under Apache-2.0 in this public repository. Other DDS source is outside this grant.',
  sources: [],
  compilations: [],
};
const bytesBySource = new Map();
async function save(relative, bytes) {
  const filename = path.join(root, relative);
  await mkdir(path.dirname(filename), { recursive: true });
  await writeFile(filename, bytes);
}
for (const [sourcePath, completeFile] of sources) {
  const bytes = await readFile(path.join(dds, sourcePath));
  const entry = { sourcePath, sourceSha256: sha256(bytes), completeFile,
    ...(completeFile ? { snapshotPath: `dds-source/${sourcePath}` } : {}) };
  inventory.sources.push(entry);
  bytesBySource.set(sourcePath, bytes);
  if (completeFile) await save(entry.snapshotPath, bytes);
}
function select(bytes, start, end) {
  // Split on LF without rewriting CRLF or the final newline.
  const offsets = [0];
  for (let index = 0; index < bytes.length; index++) if (bytes[index] === 10) offsets.push(index + 1);
  if (start < 1 || end < start || end > offsets.length) throw new Error('Invalid line range');
  return bytes.subarray(offsets[start - 1], offsets[end] ?? bytes.length);
}
async function fragment(outputPath, sourcePath, ranges) {
  const source = bytesBySource.get(sourcePath);
  const fragments = ranges.map(([startLine, endLine]) => {
    const bytes = select(source, startLine, endLine);
    return { startLine, endLine, sha256: sha256(bytes), bytes };
  });
  const result = Buffer.concat(fragments.map(fragment => fragment.bytes));
  await save(outputPath, result);
  inventory.compilations.push({ outputPath, sourcePath, transformation: 'concatenate-exact-line-ranges',
    sha256: sha256(result), ranges: fragments.map(({ bytes, ...range }) => range) });
}
await fragment('native/src/source/editor_policy.rs', sources[0][0],
  [[1, 9], [12, 12], [16, 16], [19, 143], [157, 206], [217, 225], [274, 310]]);
await fragment('native/src/source/vscode_policy.rs', sources[1][0], [[7, 22], [117, 212]]);
await fragment('native/src/source/vscode_policy_tests.rs', sources[1][0], [[225, 268]]);
await fragment('native/src/source/environment.rs', sources[6][0], [[19, 70], [150, 167], [598, 604]]);
await fragment('native/src/source/wsl_project.rs', sources[7][0], [[29, 74]]);
await fragment('native/src/source/run_process.rs', sources[7][0], [[996, 1105]]);
await fragment('native/src/source/validate_project.rs', sources[8][0], [[149, 162]]);
for (const sourcePath of sources.slice(3, 6).map(([sourcePath]) => sourcePath)) {
  const original = bytesBySource.get(sourcePath);
  const imports = [
    ["'@altifigence-internal/console-core/lib/vsCodeIntegration.js'", "'./vsCodeIntegration.js'"],
    ["'@altifigence-internal/console-core/lib/vsCodeIntegrationChanges.js'", "'./vsCodeIntegrationChanges.js'"],
  ];
  const outputPath = `src/${path.basename(sourcePath)}`;
  let result = original.toString('utf8');
  for (const [from, to] of imports) result = result.replaceAll(from, to);
  const bytes = Buffer.from(result);
  await save(outputPath, bytes);
  inventory.compilations.push({ outputPath, sourcePath, transformation: 'public-import-paths-only',
    sha256: sha256(bytes), replacements: sourcePath.startsWith('src/') ? imports : [] });
}
await save('SOURCE_INVENTORY.json', JSON.stringify(inventory, null, 2) + '\n');
console.log(`Extracted ${inventory.sources.length} allowlisted inputs at DDS ${inventory.sourceCommit}.`);
