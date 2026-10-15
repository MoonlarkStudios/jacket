import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

const result = await build({
  entryPoints: ['src/operationPresentation.ts'],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
});
const { getDownloadPresentation, presentOperationError } = await import(
  `data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`
);

function progress(overrides = {}) {
  return {
    jobId: 'job',
    stage: 'downloading',
    message: 'Downloading First track',
    completed: 0,
    total: 12,
    result: null,
    error: null,
    ...overrides,
  };
}

test('long downloads show the active track while progress counts only prepared tracks', () => {
  const first = getDownloadPresentation(progress());
  assert.equal(first.title, 'Downloading track 1 of 12');
  assert.equal(first.currentTrack, 'First track');
  assert.equal(first.percentage, 0);
  const tagging = getDownloadPresentation(
    progress({
      stage: 'preparing',
      message: 'Tagging Third track',
      completed: 2,
    }),
  );
  assert.equal(tagging.title, 'Preparing track 3 of 12');
  assert.equal(tagging.currentTrack, 'Third track');
  assert.equal(tagging.countLabel, '2 of 12 tracks prepared');
});

test('a prepared event does not falsely present the next track as active', () => {
  const prepared = getDownloadPresentation(
    progress({
      stage: 'preparing',
      message: 'Prepared Second track',
      completed: 2,
    }),
  );
  assert.equal(prepared.title, 'Prepared track 2 of 12');
  assert.equal(prepared.currentTrack, 'Second track');
});

test('packaging is an indeterminate stage after actual track preparation reaches 100%', () => {
  const packaging = getDownloadPresentation(
    progress({
      stage: 'packaging',
      message: 'Building your audio archive',
      completed: 12,
    }),
  );
  assert.equal(packaging.title, 'Creating ZIP…');
  assert.equal(packaging.percentage, 100);
  assert.equal(packaging.indeterminate, true);
  assert.equal(packaging.currentTrack, null);
  assert.equal(
    getDownloadPresentation(progress({ stage: 'queued' })).indeterminate,
    true,
  );
});

test('connection retries retain the track title and terminal states retain prepared counts', () => {
  const retrying = getDownloadPresentation(
    progress({
      message: 'Retrying A song after a connection problem',
      completed: 3,
    }),
  );
  assert.equal(retrying.currentTrack, 'A song');
  for (const stage of ['cancelled', 'interrupted', 'failed']) {
    assert.equal(
      getDownloadPresentation(progress({ stage, completed: 3 })).countLabel,
      '3 of 12 tracks prepared',
    );
  }
});

test('malformed counts cannot produce impossible percentages', () => {
  assert.equal(
    getDownloadPresentation(progress({ completed: 15 })).percentage,
    100,
  );
  assert.equal(
    getDownloadPresentation(progress({ completed: -1 })).percentage,
    0,
  );
  assert.equal(
    getDownloadPresentation(progress({ completed: Number.NaN })).percentage,
    0,
  );
  assert.equal(getDownloadPresentation(progress({ total: 0 })).percentage, 0);
  assert.equal(
    getDownloadPresentation(progress({ completed: 1, total: 1 })).countLabel,
    '1 of 1 track prepared',
  );
});

test('browser sessions are offered only for account and browser access errors', () => {
  for (const error of [
    "HTTP Error 403: Sign in to confirm you're not a bot",
    'This is a private video. Sign in if you have access.',
    'YouTube account cookies are no longer valid',
    "Unlock or allow access to Jacket's local session in Keychain, then try again.",
    'Reconnect the Jacket YouTube session before continuing.',
  ])
    assert.equal(presentOperationError(error).kind, 'session', error);
  for (const error of [
    'HTTP Error 403: Forbidden',
    'HTTP Error 503: Service Unavailable',
    'Audio tool timed out; retry this item.',
    'Connection reset by peer',
  ])
    assert.equal(presentOperationError(error).kind, 'network', error);
});

test('disk, tool, source and unavailable errors have distinct recovery guidance', () => {
  assert.equal(
    presentOperationError('No space left on device (os error 28)').kind,
    'storage',
  );
  assert.equal(
    presentOperationError('Read-only file system (os error 30)').kind,
    'destination',
  );
  assert.equal(
    presentOperationError('Permission denied (os error 13)').kind,
    'destination',
  );
  assert.equal(
    presentOperationError('yt-dlp: No such file or directory').kind,
    'tools',
  );
  assert.equal(presentOperationError('ffmpeg is not installed').kind, 'tools');
  assert.equal(
    presentOperationError('Requested format is not available').kind,
    'tools',
  );
  assert.equal(
    presentOperationError('Use an HTTPS YouTube or YouTube Music link.').kind,
    'source',
  );
  assert.equal(
    presentOperationError('This video has been removed by the uploader').kind,
    'unavailable',
  );
  assert.equal(
    presentOperationError('This video is not available in your country').kind,
    'unavailable',
  );
  assert.equal(
    presentOperationError('yt-dlp failed: Video unavailable').kind,
    'unavailable',
  );
  assert.equal(
    presentOperationError('HTTP Error 507: Insufficient Storage').kind,
    'network',
  );
  assert.equal(presentOperationError(null).kind, 'unknown');
});
