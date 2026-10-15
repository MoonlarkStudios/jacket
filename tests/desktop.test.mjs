import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

const values = new Map();
globalThis.localStorage = {
  getItem: (key) => values.get(key) ?? null,
  setItem: (key, value) => values.set(key, value),
};
globalThis.audioDlSaveRequests = [];
globalThis.audioDlSaveResponse = null;
globalThis.audioDlInvocations = [];
globalThis.audioDlOpenRequests = [];
globalThis.audioDlOpenResponse = null;
globalThis.audioDlArchiveResponse = null;
globalThis.audioDlArchiveError = false;
const result = await build({
  entryPoints: ['src/desktop.ts'],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
  plugins: [
    {
      name: 'native-dialog-fixture',
      setup(build) {
        build.onResolve({ filter: /^@tauri-apps\// }, (args) => ({
          path: args.path,
          namespace: 'fixture',
        }));
        build.onLoad({ filter: /.*/, namespace: 'fixture' }, (args) => ({
          contents: args.path.endsWith('/core')
            ? 'export const isTauri = () => true; export async function invoke(command, args) { globalThis.audioDlInvocations.push({command, args}); if (command === "downloader_selection_path") return args.path; if (globalThis.audioDlArchiveError) throw new Error("Folder unavailable"); return globalThis.audioDlArchiveResponse || (args.directory || "/Users/me/Downloads/") + args.filename; }'
            : 'export async function save(options) { globalThis.audioDlSaveRequests.push(options); return globalThis.audioDlSaveResponse; } export async function open(options) { globalThis.audioDlOpenRequests.push(options); return globalThis.audioDlOpenResponse; }',
        }));
      },
    },
  ],
});
const desktop = await import(
  `data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`
);

test('the native save dialog defaults to the last chosen folder', async () => {
  globalThis.audioDlSaveResponse = '/Users/me/Music/First.zip';
  assert.equal(
    await desktop.chooseArchive('First'),
    '/Users/me/Music/First.zip',
  );
  assert.equal(values.get('audio-dl.save-directory'), '/Users/me/Music/');
  assert.equal(
    globalThis.audioDlSaveRequests.at(-1).defaultPath,
    '/Users/me/Downloads/First.zip',
  );
  globalThis.audioDlSaveResponse = '/Users/me/Music/Second.zip';
  await desktop.chooseArchive('Second');
  assert.equal(
    globalThis.audioDlSaveRequests.at(-1).defaultPath,
    '/Users/me/Music/Second.zip',
  );
});

test('cancelling the save dialog keeps the remembered folder', async () => {
  values.set('audio-dl.save-directory', 'C:\\Music\\');
  globalThis.audioDlSaveResponse = null;
  assert.equal(await desktop.chooseArchive('Album / live'), null);
  assert.equal(
    globalThis.audioDlSaveRequests.at(-1).defaultPath,
    'C:\\Music\\Album _ live.zip',
  );
  assert.equal(values.get('audio-dl.save-directory'), 'C:\\Music\\');
});

test('archive preview resolves its filename and remembered folder without a save dialog', async () => {
  values.set('audio-dl.save-directory', '/Users/me/Music/');
  globalThis.audioDlArchiveResponse = '/Users/me/Music/Album (2).zip';
  const dialogs = globalThis.audioDlSaveRequests.length;
  assert.equal(
    await desktop.archiveDestination('Album / live'),
    '/Users/me/Music/Album (2).zip',
  );
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'archive_destination',
    args: { directory: '/Users/me/Music/', filename: 'Album _ live.zip' },
  });
  assert.equal(globalThis.audioDlSaveRequests.length, dialogs);
  values.delete('audio-dl.save-directory');
  await desktop.archiveDestination('First album');
  assert.equal(globalThis.audioDlInvocations.at(-1).args.directory, null);
  globalThis.audioDlArchiveResponse = null;
});

test('the save dialog uses the collision-safe preview filename and still opens when folder resolution fails', async () => {
  globalThis.audioDlArchiveResponse = '/Users/me/Downloads/Album (3).zip';
  globalThis.audioDlSaveResponse = null;
  await desktop.chooseArchive('Album');
  assert.equal(
    globalThis.audioDlSaveRequests.at(-1).defaultPath,
    '/Users/me/Downloads/Album (3).zip',
  );
  globalThis.audioDlArchiveResponse = null;
  globalThis.audioDlArchiveError = true;
  values.set('audio-dl.save-directory', 'C:\\Music\\');
  await desktop.chooseArchive('Album');
  assert.equal(
    globalThis.audioDlSaveRequests.at(-1).defaultPath,
    'C:\\Music\\Album.zip',
  );
  globalThis.audioDlArchiveError = false;
});

test('local downloader selection requests one executable and leaves cancellation empty', async () => {
  globalThis.audioDlOpenResponse = null;
  assert.equal(
    await desktop.chooseDownloader('/opt/homebrew/bin/yt-dlp'),
    null,
  );
  assert.equal(globalThis.audioDlOpenRequests.at(-1).multiple, false);
  assert.equal(globalThis.audioDlOpenRequests.at(-1).directory, false);
  assert.equal(
    globalThis.audioDlOpenRequests.at(-1).defaultPath,
    '/opt/homebrew/bin/yt-dlp',
  );
  globalThis.audioDlOpenResponse = '/Users/me/bin/yt-dlp';
  assert.equal(await desktop.chooseDownloader(), '/Users/me/bin/yt-dlp');
});

test('browser-session transport sends exact profile IDs and leaves cookie paths native', async () => {
  await desktop.inspectSource(
    'https://www.youtube.com/watch?v=fixture',
    'chrome',
    'inspection-fixture',
    'chrome-profile-fixture',
  );
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'inspect_source',
    args: {
      url: 'https://www.youtube.com/watch?v=fixture',
      auth: { browser: 'chrome', browserProfile: 'chrome-profile-fixture' },
      inspectionId: 'inspection-fixture',
    },
  });
  await desktop.browserProfiles('firefox');
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'browser_profiles',
    args: { browser: 'firefox' },
  });
  await desktop.testBrowserAccess(
    'chrome',
    'chrome-profile-fixture',
    'https://www.youtube.com/watch?v=fixture',
  );
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'test_browser_access',
    args: {
      auth: { browser: 'chrome', browserProfile: 'chrome-profile-fixture' },
      url: 'https://www.youtube.com/watch?v=fixture',
    },
  });
  await desktop.sessionAction('disconnect');
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'session_action',
    args: { action: 'disconnect', url: null },
  });
});
