import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

const result = await build({
  entryPoints: ['src/downloader.ts'],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
});
const { automaticUpdateKey, downloaderPresentation } = await import(
  `data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`
);
const status = {
  mode: 'managed',
  autoUpdate: true,
  installedVersion: '2026.08.19',
  latestVersion: '2026.10.10.233141',
  channel: 'nightly',
  updateAvailable: true,
  downgrade: false,
  pendingActivation: false,
  error: null,
};

test('automatic updates require a newer managed release and respect explicit pause and local mode', () => {
  assert.equal(automaticUpdateKey(status), 'nightly:2026.10.10.233141');
  for (const patch of [
    { mode: 'local' },
    { autoUpdate: false },
    { updateAvailable: false },
    { downgrade: true },
    { error: 'Offline' },
  ])
    assert.equal(automaticUpdateKey({ ...status, ...patch }), null);
  assert.equal(automaticUpdateKey(null), null);
  assert.equal(
    automaticUpdateKey({
      ...status,
      pendingActivation: true,
      error: 'Offline',
    }),
    'nightly:2026.10.10.233141',
  );
});

test('status distinguishes a prepared update from an active transfer, local use, and a failed check', () => {
  assert.match(
    downloaderPresentation({ ...status, pendingActivation: true }, null, null)
      .label,
    /Update ready/,
  );
  assert.equal(
    downloaderPresentation(status, 'verifying', null).tone,
    'working',
  );
  assert.match(
    downloaderPresentation(status, 'downloading', null).label,
    /background/,
  );
  assert.match(
    downloaderPresentation({ ...status, mode: 'local' }, null, null).label,
    /local install/,
  );
  assert.equal(
    downloaderPresentation({ ...status, error: 'Offline' }, null, null).tone,
    'attention',
  );
  assert.match(
    downloaderPresentation({ ...status, error: 'Offline' }, null, null).label,
    /current version/,
  );
  assert.match(
    downloaderPresentation(status, 'downloading', null, true).label,
    /Stopping/,
  );
});
