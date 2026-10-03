// SPDX-FileCopyrightText: 2026 Altifigence
// SPDX-License-Identifier: Apache-2.0
import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  createDesktopVsCodeIntegration,
  hasVsCodeIntegration,
  parseVsCodeIntegrationState,
  vsCodeIntegrationPatch,
} from '../dist/index.js';

const state = { installed: true, installationScope: 'device', applicationAvailable: true, launch: 'native' };

test('only the registered DDS integration version counts as installed', () => {
  assert.equal(hasVsCodeIntegration(vsCodeIntegrationPatch(true)), true);
  assert.equal(hasVsCodeIntegration(vsCodeIntegrationPatch(false)), false);
  for (const document of [null, [], true, { developerTools: { externalEditor: 'visual_studio_code' } },
    { extensions: { 'altifigence.vscode': { installed: true, version: 'other' } } },
    { extensions: { 'altifigence.vscode': { installed: 'true', version: '1.0.0' } } }]) {
    assert.equal(hasVsCodeIntegration(document), false);
  }
});

test('native wire parser accepts exact safe states and rejects fields that cross host authority', () => {
  for (const launch of ['native', 'wsl', 'cloud-unsupported', 'project-unavailable', 'wsl-extension-missing', 'wsl-extension-unavailable']) {
    assert.deepEqual(parseVsCodeIntegrationState({ ...state, launch }), { ...state, launch });
  }
  for (const invalid of [null, [], { ...state, installationScope: 'project' },
    { ...state, installed: 'true' }, { ...state, applicationAvailable: null },
    { ...state, launch: 'shell' }, { ...state, executable: '/arbitrary/code' },
    { ...state, workspace: '/renderer/path' }, { ...state, args: ['--install-extension'] }]) {
    assert.throws(() => parseVsCodeIntegrationState(invalid), /Invalid VS Code integration state/);
  }
});

test('actual DDS adapter uses fixed commands and never sends project paths or argv', async () => {
  const calls = [];
  const integration = createDesktopVsCodeIntegration(async (command, args) => {
    calls.push({ command, args });
    return command === 'open_external_editor' ? true : state;
  });
  assert.deepEqual(await integration.read(), state);
  assert.deepEqual(await integration.setInstalled(false), state);
  await integration.open();
  assert.deepEqual(calls, [
    { command: 'read_vscode_integration', args: undefined },
    { command: 'set_vscode_integration', args: { installed: false } },
    { command: 'open_external_editor', args: { id: 'visual_studio_code' } },
  ]);
});

test('host failures and malformed launch responses remain failures', async () => {
  for (const response of [false, undefined, { opened: true }, 'true']) {
    const integration = createDesktopVsCodeIntegration(async () => response);
    await assert.rejects(integration.open(), /Invalid VS Code launch response/);
  }
  const integration = createDesktopVsCodeIntegration(async () => { throw new Error('permission denied'); });
  await assert.rejects(integration.setInstalled(true), /permission denied/);
});
