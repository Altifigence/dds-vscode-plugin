// SPDX-FileCopyrightText: 2026 Altifigence
// SPDX-License-Identifier: Apache-2.0
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const dds = process.argv[2];
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const inventory = JSON.parse(await readFile(path.join(root, 'SOURCE_INVENTORY.json'), 'utf8'));
const bytesBySource = new Map();
function select(bytes, start, end) {
  const offsets = [0];
  for (let index = 0; index < bytes.length; index++) if (bytes[index] === 10) offsets.push(index + 1);
  return bytes.subarray(offsets[start - 1], offsets[end] ?? bytes.length);
}
for (const source of inventory.sources) {
  if (source.snapshotPath) {
    const bytes = await readFile(path.join(root, source.snapshotPath));
    assert.equal(digest(bytes), source.sourceSha256, `${source.snapshotPath} differs from the original`);
    bytesBySource.set(source.sourcePath, bytes);
  }
  if (dds) {
    const bytes = await readFile(path.join(dds, source.sourcePath));
    assert.equal(digest(bytes), source.sourceSha256, `${source.sourcePath} differs from supplied DDS source`);
    bytesBySource.set(source.sourcePath, bytes);
  }
}
for (const compilation of inventory.compilations) {
  const bytes = await readFile(path.join(root, compilation.outputPath));
  assert.equal(digest(bytes), compilation.sha256, `${compilation.outputPath} hash differs`);
  const source = bytesBySource.get(compilation.sourcePath);
  if (compilation.transformation === 'public-import-paths-only' && source) {
    let expected = source.toString('utf8');
    for (const [from, to] of compilation.replacements) expected = expected.replaceAll(from, to);
    assert.deepEqual(bytes, Buffer.from(expected), `${compilation.outputPath} changed beyond imports`);
  }
  if (compilation.transformation === 'concatenate-exact-line-ranges') {
    let offset = 0;
    const expected = [];
    for (const range of compilation.ranges) {
      if (source) {
        const selected = select(source, range.startLine, range.endLine);
        assert.equal(digest(selected), range.sha256, `${compilation.sourcePath} line selection changed`);
        expected.push(selected);
      } else {
        // Standalone consumers can verify the concatenation and fragment digest
        // without access to the other, unexported DDS source.
        const lines = bytes.subarray(offset).toString('utf8').match(/.*(?:\n|$)/g).filter(Boolean);
        const count = range.endLine - range.startLine + 1;
        const selected = Buffer.from(lines.slice(0, count).join(''));
        assert.equal(digest(selected), range.sha256, `${compilation.outputPath} selected fragment changed`);
        offset += selected.length;
      }
    }
    if (source) assert.deepEqual(bytes, Buffer.concat(expected), `${compilation.outputPath} changed`);
    else assert.equal(offset, bytes.length, `${compilation.outputPath} contains extra source`);
  }
}
console.log(`Verified ${inventory.sources.filter(source => source.completeFile).length} exact source files and ${inventory.compilations.length} compiled exports${dds ? ' against supplied DDS' : ''}.`);
