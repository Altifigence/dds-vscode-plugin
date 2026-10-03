// SPDX-FileCopyrightText: 2026 Altifigence
// SPDX-License-Identifier: Apache-2.0
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createVsCodeIntegrationChanges, createDesktopVsCodeIntegration } from '../dist/index.js';

test('SSR notification hooks are safe without a browser', () => {
  const changes = createVsCodeIntegrationChanges('device');
  changes.publish();
  changes.subscribe(() => assert.fail('SSR must not notify'))();
});

test('notifications invalidate exact scopes, leave no settings in storage, and unsubscribe', async () => {
  const browser = new EventTarget();
  const values = new Map();
  const writes = [];
  browser.localStorage = {
    setItem(key, value) { values.set(key, value); writes.push([key, value]); },
    removeItem(key) { values.delete(key); },
  };
  globalThis.window = browser;
  try {
    let notifications = 0;
    const changes = createVsCodeIntegrationChanges('device');
    const unsubscribe = changes.subscribe(() => notifications++);
    createVsCodeIntegrationChanges('cloud:other-project').publish();
    assert.equal(notifications, 0);
    changes.publish();
    assert.equal(notifications, 1);
    assert.equal(values.size, 0);
    assert.deepEqual(Object.keys(JSON.parse(writes.at(-1)[1])).sort(), ['change', 'scope']);
    const storageEvent = new Event('storage');
    Object.assign(storageEvent, { key: 'dds.vscode-integration-change.v1', newValue: '{broken-json' });
    browser.dispatchEvent(storageEvent);
    assert.equal(notifications, 1);
    storageEvent.newValue = JSON.stringify({ scope: 'device', installed: true });
    browser.dispatchEvent(storageEvent);
    assert.equal(notifications, 2);
    unsubscribe();
    changes.publish();
    assert.equal(notifications, 2);

    const failed = createDesktopVsCodeIntegration(async () => { throw new Error('write failed'); });
    let failuresInvalidated = 0;
    const stop = failed.subscribeChanges(() => failuresInvalidated++);
    await assert.rejects(failed.setInstalled(true), /write failed/);
    assert.equal(failuresInvalidated, 1);
    stop();
    browser.localStorage = { setItem() { throw new Error('blocked'); }, removeItem() {} };
    assert.doesNotThrow(() => changes.publish());
  } finally {
    delete globalThis.window;
  }
});
