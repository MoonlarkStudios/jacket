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
globalThis.audioDlCommandResponses = new Map();
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
            ? 'export const isTauri = () => true; export async function invoke(command, args) { globalThis.audioDlInvocations.push({command, args}); if (command === "downloader_selection_path") return args.path; if (globalThis.audioDlCommandResponses.has(command)) return globalThis.audioDlCommandResponses.get(command); if (globalThis.audioDlArchiveError) throw new Error("Folder unavailable"); return globalThis.audioDlArchiveResponse || (args.directory || "/Users/me/Downloads/") + args.filename; }'
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

test('folder export asks for one parent directory and lets native code plan the collection path', async () => {
  values.set('audio-dl.save-directory', '/Users/me/Music/');
  globalThis.audioDlOpenResponse = '/Users/me/Music';
  globalThis.audioDlCommandResponses.set(
    'folder_destination',
    '/Users/me/Music/Album _ live (2)',
  );
  assert.equal(
    await desktop.chooseFolder('Album / live'),
    '/Users/me/Music/Album _ live (2)',
  );
  assert.deepEqual(globalThis.audioDlOpenRequests.at(-1), {
    title: 'Choose a folder for your album',
    directory: true,
    multiple: false,
    defaultPath: '/Users/me/Music/',
  });
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'folder_destination',
    args: { directory: '/Users/me/Music', title: 'Album / live' },
  });
  assert.equal(values.get('audio-dl.save-directory'), '/Users/me/Music/');

  globalThis.audioDlOpenResponse = 'C:\\Music\\';
  globalThis.audioDlCommandResponses.set(
    'folder_destination',
    'C:\\Music\\Album',
  );
  await desktop.chooseFolder('Album');
  assert.equal(values.get('audio-dl.save-directory'), 'C:\\Music\\');
  assert.equal(
    globalThis.audioDlInvocations.at(-1).args.directory,
    'C:\\Music\\',
  );
  const invocations = globalThis.audioDlInvocations.length;
  globalThis.audioDlOpenResponse = null;
  assert.equal(await desktop.chooseFolder('Album'), null);
  assert.equal(globalThis.audioDlInvocations.length, invocations);
  assert.equal(values.get('audio-dl.save-directory'), 'C:\\Music\\');
});

test('cover import uses a chosen source once while later crop and reload use only its opaque asset ID', async () => {
  const asset = {
    id: 'a'.repeat(64),
    previewDataUrl: 'data:image/jpeg;base64,fixture',
    width: 600,
    height: 400,
  };
  globalThis.audioDlCommandResponses.set('import_artwork', asset);
  globalThis.audioDlCommandResponses.set('crop_artwork', asset);
  globalThis.audioDlCommandResponses.set('artwork_asset', asset);
  globalThis.audioDlOpenResponse = '/Pictures/Original cover.png';
  assert.deepEqual(await desktop.chooseArtwork(), asset);
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'import_artwork',
    args: { path: '/Pictures/Original cover.png', crop: null },
  });
  assert.deepEqual(globalThis.audioDlOpenRequests.at(-1).filters, [
    { name: 'Cover image', extensions: ['jpg', 'jpeg', 'png'] },
  ]);
  const crop = { x: 100, y: 0, size: 400 };
  assert.deepEqual(await desktop.cropArtwork(asset.id, crop), asset);
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'crop_artwork',
    args: { id: asset.id, crop },
  });
  await desktop.artworkAsset(asset.id);
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'artwork_asset',
    args: { id: asset.id },
  });
  globalThis.audioDlOpenResponse = null;
  const invocations = globalThis.audioDlInvocations.length;
  assert.equal(await desktop.chooseArtwork(), null);
  assert.equal(globalThis.audioDlInvocations.length, invocations);
});

test('review transport preserves versioned revision identity and explicit discard outcomes', async () => {
  const draft = {
    schemaVersion: 1,
    sessionId: 'review-session',
    revision: 8,
    inspection: { title: 'Committed album' },
    selected: [1, 3],
    preferences: { sharedArtworkAsset: 'b'.repeat(64), outputKind: 'folder' },
  };
  globalThis.audioDlCommandResponses.set('review_draft_load', draft);
  globalThis.audioDlCommandResponses.set('review_draft_save', true);
  globalThis.audioDlCommandResponses.set('review_draft_discard', false);
  assert.deepEqual(await desktop.loadReviewDraft(), draft);
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'review_draft_load',
    args: undefined,
  });
  assert.equal(await desktop.saveReviewDraft(draft), true);
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'review_draft_save',
    args: { draft },
  });
  assert.equal(await desktop.discardReviewDraft('review-session', 9), false);
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'review_draft_discard',
    args: { sessionId: 'review-session', revision: 9 },
  });
});

test('preview and folder playback use their specific native commands', async () => {
  const plan = {
    title: 'Album',
    outputKind: 'folder',
    auth: { browser: null },
    tracks: [],
  };
  const preview = {
    rootName: 'Album',
    outputKind: 'folder',
    tracks: [],
    files: ['playlist.m3u'],
    warnings: [],
  };
  globalThis.audioDlCommandResponses.set('preview_export', preview);
  globalThis.audioDlCommandResponses.set('open_export_playlist', null);
  assert.deepEqual(await desktop.previewExport(plan), preview);
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'preview_export',
    args: { plan },
  });
  await desktop.openPlaylist('/Music/Album (2)');
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'open_export_playlist',
    args: { outputPath: '/Music/Album (2)' },
  });
});

test('export binds an optional committed review identity without adding it to the audio plan', async () => {
  const plan = { title: 'Album', auth: { browser: null }, tracks: [] };
  const identity = { sessionId: 'review-session', revision: 9 };
  globalThis.audioDlCommandResponses.set('start_export', null);
  await desktop.startExport(plan, '/Music/Album.zip', 'job', identity);
  assert.deepEqual(globalThis.audioDlInvocations.at(-1), {
    command: 'start_export',
    args: {
      plan,
      outputPath: '/Music/Album.zip',
      jobId: 'job',
      reviewIdentity: identity,
    },
  });
  assert.equal(Object.hasOwn(plan, 'reviewIdentity'), false);
  await desktop.startExport(plan, '/Music/Another.zip', 'memory-only-job');
  assert.equal(globalThis.audioDlInvocations.at(-1).args.reviewIdentity, null);
});
