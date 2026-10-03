// SPDX-FileCopyrightText: 2026 Altifigence
// SPDX-License-Identifier: Apache-2.0
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const packedRoot = path.resolve(process.argv[2] ?? '');
if (!process.argv[2]) throw new Error('Usage: node scripts/packed-consumer-smoke.mjs /path/to/extracted/package');
const manifest = JSON.parse(await readFile(path.join(packedRoot, 'package.json'), 'utf8'));
assert.equal(manifest.name, '@altifigence/dds-vscode-plugin');
const adapter = await import(pathToFileURL(path.join(packedRoot, manifest.exports['.'].import)));
const state = { installed: true, installationScope: 'device', applicationAvailable: true, launch: 'native' };
const calls = [];
const integration = adapter.createDesktopVsCodeIntegration(async (command, args) => {
  calls.push([command, args]);
  return command === 'open_external_editor' ? true : state;
});
assert.deepEqual(await integration.read(), state);
await integration.setInstalled(true);
await integration.open();
assert.deepEqual(calls, [
  ['read_vscode_integration', undefined],
  ['set_vscode_integration', { installed: true }],
  ['open_external_editor', { id: 'visual_studio_code' }],
]);
assert.equal(adapter.hasVsCodeIntegration(adapter.vsCodeIntegrationPatch(true)), true);
let count = 0;
async function audit(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const target = path.join(directory, entry.name);
    const relative = path.relative(packedRoot, target).replaceAll('\\', '/');
    assert(!entry.isSymbolicLink(), `Package contains a symlink: ${relative}`);
    assert(!/^(native\/target|node_modules|\.cargo)(\/|$)/.test(relative), `Package contains build/cache data: ${relative}`);
    assert(!/\.(exe|dll|vsix|msix|zip|log)$/i.test(relative), `Package contains a binary/archive/log: ${relative}`);
    if (entry.isDirectory()) await audit(target);
    else count++;
  }
}
await audit(packedRoot);
execFileSync(process.execPath, [path.join(packedRoot, 'scripts/verify-source.mjs')], { cwd: packedRoot, stdio: 'inherit' });
console.log(`Packed external consumer passed; audited ${count} files, no executables, VSIX, caches or logs.`);
