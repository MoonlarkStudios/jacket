import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

// Exercise the hook's public transitions with controlled native promises.
// The small hook runtime provides React's state/ref/effect behavior without
// requiring a DOM or changing the application's production dependencies.
const result = await build({
  entryPoints: ['src/useWorkspace.ts'],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
  plugins: [
    {
      name: 'workspace-fixture',
      setup(build) {
        build.onResolve(
          {
            filter:
              /^(react|@tauri-apps\/api\/event|\.\/desktop|\.\/useBrowserSession|\.\/useDownloader)$/,
          },
          (args) => ({ path: args.path, namespace: 'fixture' }),
        );
        build.onLoad({ filter: /.*/, namespace: 'fixture' }, ({ path }) => {
          const contents =
            path === 'react'
              ? 'export const useState = (...args) => globalThis.workspaceFixture.state(...args); export const useRef = (...args) => globalThis.workspaceFixture.ref(...args); export const useCallback = (...args) => globalThis.workspaceFixture.callback(...args); export const useMemo = (...args) => globalThis.workspaceFixture.memo(...args); export const useEffect = (...args) => globalThis.workspaceFixture.effect(...args);'
              : path.endsWith('/event')
                ? 'export async function listen(name, handler) { const f = globalThis.workspaceFixture; f.listeners.set(name, handler); return () => f.listeners.delete(name); }'
                : path.endsWith('useDownloader')
                  ? 'export function useDownloader() { return {}; }'
                  : path.endsWith('useBrowserSession')
                    ? `import { useState, useCallback } from 'react'; export function useBrowserSession() { const [selection, setSelection] = useState({browser:null,profileId:null}); return {selection, testing:false, attach:(browser,profileId)=>setSelection({browser,profileId}), restore:useCallback(({browser,browserProfile})=>setSelection({browser,profileId:browserProfile??null}),[])}; }`
                    : `export const desktopAvailable = true; ${['archiveDestination', 'cancelExport', 'cancelInspection', 'chooseArchive', 'chooseFolder', 'dependencyStatus', 'dismissRecovery', 'inspectSource', 'pendingExport', 'startExport', 'loadReviewDraft', 'saveReviewDraft', 'discardReviewDraft'].map((name) => `export const ${name} = (...args) => globalThis.workspaceFixture.native.${name}(...args);`).join('\n')}`;
          return { contents };
        });
      },
    },
  ],
});
const { useWorkspace } = await import(
  `data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`
);

function fixture() {
  const hooks = [];
  let cursor = 0;
  let dirty = true;
  let value;
  let effects = [];
  const calls = {
    dismiss: [],
    start: [],
    inspect: [],
    choose: [],
    chooseFolder: [],
    loadReview: [],
    saveReview: [],
    discardReview: [],
  };
  const same = (a, b) =>
    a && b && a.length === b.length && a.every((v, i) => Object.is(v, b[i]));
  const f = {
    listeners: new Map(),
    native: {
      dependencyStatus: async () => [],
      pendingExport: async () => null,
      loadReviewDraft: async () => {
        calls.loadReview.push(true);
        return null;
      },
      saveReviewDraft: async (draft) => {
        calls.saveReview.push(structuredClone(draft));
        return true;
      },
      discardReviewDraft: async (...args) => {
        calls.discardReview.push(args);
        return true;
      },
      archiveDestination: async (title) => `/Downloads/${title}.zip`,
      chooseArchive: async (title) => {
        calls.choose.push(title);
        return '/Music/Album.zip';
      },
      chooseFolder: async (title) => {
        calls.chooseFolder.push(title);
        return '/Music/Album';
      },
      dismissRecovery: async (id) => {
        calls.dismiss.push(id);
      },
      startExport: async (...args) => {
        calls.start.push(args);
      },
      inspectSource: async (...args) => {
        calls.inspect.push(args);
        return album;
      },
      cancelExport: async () => {},
      cancelInspection: async () => {},
    },
    state(initial) {
      const index = cursor++;
      if (!hooks[index])
        hooks[index] = {
          value: typeof initial === 'function' ? initial() : initial,
        };
      return [
        hooks[index].value,
        (next) => {
          hooks[index].value =
            typeof next === 'function' ? next(hooks[index].value) : next;
          dirty = true;
        },
      ];
    },
    ref(initial) {
      const index = cursor++;
      hooks[index] ??= { current: initial };
      return hooks[index];
    },
    callback(fn, deps) {
      const index = cursor++;
      if (!same(hooks[index]?.deps, deps)) hooks[index] = { value: fn, deps };
      return hooks[index].value;
    },
    memo(fn, deps) {
      const index = cursor++;
      if (!same(hooks[index]?.deps, deps)) hooks[index] = { value: fn(), deps };
      return hooks[index].value;
    },
    effect(fn, deps) {
      const index = cursor++;
      if (!same(hooks[index]?.deps, deps)) {
        const previous = hooks[index];
        hooks[index] = { deps };
        effects.push(() => {
          previous?.cleanup?.();
          hooks[index].cleanup = fn();
        });
      }
    },
  };
  globalThis.workspaceFixture = f;
  function render() {
    let count = 0;
    while (dirty) {
      assert.ok(count++ < 30, 'state updates settle');
      dirty = false;
      cursor = 0;
      // biome-ignore lint/correctness/useHookAtTopLevel: controlled hook renderer, each pass preserves hook order.
      value = useWorkspace();
      const pending = effects;
      effects = [];
      for (const effect of pending) effect();
    }
    return value;
  }
  async function flush() {
    for (let i = 0; i < 12; i++) {
      await Promise.resolve();
      render();
    }
    return value;
  }
  return { f, calls, render, flush };
}

const metadata = {
  title: 'Song',
  artist: 'Artist',
  album: 'Album',
  albumArtist: 'Artist',
  trackNumber: 7,
  discNumber: null,
  releaseYear: 2026,
};
const album = {
  title: 'Album',
  sourceUrl: 'https://music.youtube.com/playlist?list=example',
  suggestedMode: 'album',
  artworkUrl: null,
  warnings: [],
  tracks: [0, 1].map((index) => ({
    id: `track-${index}`,
    sourceUrl: `https://youtube.com/watch?v=${index}`,
    duration: 120,
    artworkUrl: null,
    uploader: 'Artist',
    sourceMetadata: { ...metadata },
    metadata: { ...metadata },
  })),
};
const deferred = () => {
  let resolve;
  let reject;
  const promise = new Promise((a, b) => {
    resolve = a;
    reject = b;
  });
  return { promise, resolve, reject };
};

function savedReview() {
  const inspection = structuredClone(album);
  inspection.tracks[0].metadata.trackNumber = null;
  inspection.tracks[0].metadata.title = 'A deliberately unnumbered track';
  inspection.tracks[1].metadata.title = 'Reviewed second track';
  return {
    schemaVersion: 1,
    sessionId: 'saved-review-session',
    revision: 12,
    url: inspection.sourceUrl,
    inspection,
    selected: [1],
    preferences: {
      title: 'My reviewed album',
      mode: 'album',
      albumTitle: 'My album tags',
      albumArtist: 'Reviewed artist',
      year: 1999,
      format: 'preserve',
      artworkMode: 'shared',
      sharedArtwork: null,
      sharedArtworkAsset: 'a'.repeat(64),
      outputKind: 'folder',
    },
  };
}

function stopExport(h, id, stage = 'failed') {
  h.f.listeners.get('export-progress')({
    payload: {
      jobId: id,
      stage,
      message: 'Export stopped',
      completed: 1,
      total: 2,
      error: stage === 'failed' ? 'Disk unavailable' : null,
      result: null,
    },
  });
}

test('New download Undo restores edited review, selection, numbering, example and browser selection', async () => {
  const h = fixture();
  h.render().loadInspection(album, true);
  let w = await h.flush();
  w.setUrl(album.sourceUrl);
  w.browserSession.attach('firefox', 'profile-original');
  w.setPreferences((p) => ({
    ...p,
    title: 'My custom ZIP',
    format: 'mp3',
    albumTitle: 'Edited album',
    artworkMode: 'none',
  }));
  w.setSelected(new Set([1]));
  w.updateTrack(1, {
    ...album.tracks[1],
    metadata: { ...metadata, title: 'Edited song' },
  });
  w = h.render();
  w.numberTracks();
  w = h.render();
  assert.equal(await w.reset(), true);
  w = h.render();
  assert.equal(w.inspection, null);
  assert.deepEqual(w.previousReview, { title: 'My custom ZIP', trackCount: 2 });
  w.browserSession.attach('chrome', 'different');
  w = h.render();
  assert.equal(w.undoReset(), true);
  w = h.render();
  assert.equal(w.preferences.title, 'My custom ZIP');
  assert.equal(w.preferences.albumTitle, 'Edited album');
  assert.equal(w.preferences.format, 'mp3');
  assert.equal(w.inspection.tracks[1].metadata.title, 'Edited song');
  assert.deepEqual([...w.selected], [1]);
  assert.equal(w.url, album.sourceUrl);
  assert.equal(w.exampleMode, true);
  assert.equal(w.browser, 'firefox');
  assert.equal(w.browserProfile, 'profile-original');
  assert.deepEqual(w.numberingUndo, [7, 7]);
  assert.equal(w.canUndoReset, false);
  w.undoNumbering();
  assert.deepEqual(
    h.render().inspection.tracks.map((t) => t.metadata.trackNumber),
    [7, 7],
  );
});

test('Undo of failed download preserves cache job ID and destination for a safe resume', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  await w.exportZip();
  w = h.render();
  const [plan, path, id] = h.calls.start[0];
  h.f.listeners.get('export-progress')({
    payload: {
      jobId: id,
      stage: 'failed',
      message: 'Disk full',
      completed: 1,
      total: 2,
      error: 'Disk full',
      result: null,
    },
  });
  w = h.render();
  assert.equal(await w.reset(), true);
  w = h.render();
  assert.deepEqual(h.calls.dismiss, [id]);
  assert.equal(w.undoReset(), true);
  w = await h.flush();
  assert.equal(w.progress.completed, 1);
  assert.equal(w.savePath, path);
  assert.equal(w.destinationPath, path);
  await w.exportZip();
  assert.deepEqual(h.calls.start[1].slice(0, 3), [plan, path, id]);
  assert.equal(h.calls.start[1][3].sessionId, h.calls.start[0][3].sessionId);
  assert.ok(h.calls.start[1][3].revision > h.calls.start[0][3].revision);
  assert.equal(h.calls.choose.length, 1);
});

test('in-flight inspection cannot be overwritten by Undo; failures retain it and success retires it', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  await w.reset();
  w = h.render();
  const inspection = deferred();
  h.f.native.inspectSource = () => inspection.promise;
  const operation = w.inspect('https://youtube.com/watch?v=new');
  assert.equal(w.undoReset(), false);
  w = await h.flush();
  assert.equal(w.canUndoReset, false);
  inspection.reject(new Error('Offline'));
  await operation;
  w = h.render();
  assert.equal(w.canUndoReset, true);
  assert.equal(w.undoReset(), true);
  w = h.render();
  assert.equal(w.inspection.title, 'Album');
  await w.reset();
  w = h.render();
  h.f.native.inspectSource = async () => ({ ...album, title: 'New album' });
  await w.inspect('https://youtube.com/watch?v=new');
  w = h.render();
  assert.equal(w.inspection.title, 'New album');
  assert.equal(w.canUndoReset, false);
});

test('native recovery dismissal failure leaves the review intact and reset blocks simultaneous operations', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  await w.exportZip();
  const id = h.calls.start[0][2];
  h.f.listeners.get('export-progress')({
    payload: {
      jobId: id,
      stage: 'cancelled',
      message: 'Cancelled',
      completed: 0,
      total: 2,
      error: null,
      result: null,
    },
  });
  w = h.render();
  const dismissal = deferred();
  h.f.native.dismissRecovery = () => dismissal.promise;
  const resetting = w.reset();
  assert.equal(await w.reset(), false);
  await w.inspect('https://youtube.com/watch?v=new');
  await w.exportZip();
  assert.equal(h.calls.inspect.length, 0);
  assert.equal(h.calls.start.length, 1);
  dismissal.reject(new Error('Recovery unavailable'));
  assert.equal(await resetting, false);
  w = h.render();
  assert.equal(w.inspection.title, 'Album');
  assert.match(w.error, /Recovery unavailable/);
  assert.equal(w.canUndoReset, false);
  assert.equal(w.busy, false);
});

test('starting over after a partial inspection failure still offers the prior complete review', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  await w.reset();
  w = h.render();
  const inspection = deferred();
  let inspectionId;
  h.f.native.inspectSource = (_source, _browser, id) => {
    inspectionId = id;
    return inspection.promise;
  };
  const operation = w.inspect('https://youtube.com/watch?v=partial');
  await h.flush();
  h.f.listeners.get('inspection-progress')({
    payload: {
      inspectionId,
      title: 'Partial album',
      sourceUrl: 'https://youtube.com/watch?v=partial',
      suggestedMode: 'album',
      artworkUrl: null,
      index: 0,
      total: 10,
      track: album.tracks[0],
    },
  });
  inspection.reject(new Error('Offline'));
  await operation;
  w = h.render();
  assert.equal(w.inspection.title, 'Partial album');
  assert.equal(w.canUndoReset, true);
  await w.reset();
  w = h.render();
  assert.equal(w.previousReview.title, 'Album');
  assert.equal(w.undoReset(), true);
  w = h.render();
  assert.equal(w.inspection.tracks.length, 2);
  assert.equal(w.inspectionStatus, 'complete');
});

test('choosing another destination retains progress and cache, never starts a download, and cancel keeps the prior path', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  await w.exportZip();
  const id = h.calls.start[0][2];
  const progress = {
    jobId: id,
    stage: 'failed',
    message: 'Disk full',
    completed: 1,
    total: 2,
    error: 'Disk full',
    result: null,
  };
  h.f.listeners.get('export-progress')({ payload: progress });
  w = h.render();
  const dialog = deferred();
  h.f.native.chooseArchive = () => dialog.promise;
  const choosing = w.chooseAnotherDestination();
  assert.equal(await w.reset(), false);
  await w.exportZip();
  assert.equal(h.calls.start.length, 1);
  dialog.resolve('/Other/Album.zip');
  await choosing;
  w = h.render();
  assert.deepEqual(w.progress, progress);
  assert.equal(w.savePath, '/Other/Album.zip');
  assert.equal(h.calls.start.length, 1);
  h.f.native.chooseArchive = async () => null;
  await w.chooseAnotherDestination();
  w = h.render();
  assert.equal(w.savePath, '/Other/Album.zip');
  await w.exportZip();
  assert.equal(h.calls.start[1][1], '/Other/Album.zip');
  assert.equal(h.calls.start[1][2], id);
});

test('renaming a failed download clears the retry destination hint and asks for a new destination', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  await w.exportZip();
  const id = h.calls.start[0][2];
  h.f.listeners.get('export-progress')({
    payload: {
      jobId: id,
      stage: 'failed',
      message: 'Connection lost',
      completed: 1,
      total: 2,
      error: 'Connection lost',
      result: null,
    },
  });
  w = h.render();
  assert.equal(w.destinationPath, '/Music/Album.zip');
  w.setPreferences((previous) => ({ ...previous, title: 'Renamed album' }));
  w = await h.flush();
  assert.equal(w.destinationPath, null);
  assert.equal(w.savePath, '/Downloads/Renamed album.zip');
  const dialogs = h.calls.choose.length;
  h.f.native.chooseArchive = async (title) => {
    h.calls.choose.push(title);
    return '/Other/Renamed album.zip';
  };
  await w.exportZip();
  assert.equal(h.calls.choose.length, dialogs + 1);
  assert.equal(h.calls.start[1][1], '/Other/Renamed album.zip');
  assert.equal(h.calls.start[1][2], id);
});

test('saved reviews restore committed null numbering, URL, selection, artwork and output preferences exactly', async () => {
  const h = fixture();
  const draft = savedReview();
  h.f.native.loadReviewDraft = async () => structuredClone(draft);
  h.render();
  const w = await h.flush();
  assert.equal(w.url, draft.url);
  assert.deepEqual(w.inspection, draft.inspection);
  assert.equal(w.inspection.tracks[0].metadata.trackNumber, null);
  assert.deepEqual([...w.selected], [1]);
  assert.deepEqual(w.preferences, draft.preferences);
  assert.equal(w.browser, null, 'ordinary reviews start with public access');
  assert.equal(w.browserProfile, null);
  assert.equal(w.inspectionStatus, 'complete');
  const saved = h.calls.saveReview.at(-1);
  assert.equal(saved.sessionId, draft.sessionId);
  assert.ok(saved.revision > draft.revision);
  assert.deepEqual(saved.inspection, draft.inspection);
  assert.equal(w.draftStatus, 'saved');
});

test('review persistence contains committed edits and selected occurrences without browser credentials', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  w.browserSession.attach('firefox', 'private-profile');
  w.setSelected(new Set([1]));
  w.setPreferences((p) => ({
    ...p,
    sharedArtworkAsset: 'b'.repeat(64),
    sharedArtwork: null,
    artworkMode: 'shared',
    outputKind: 'folder',
  }));
  w.updateTrack(1, {
    ...album.tracks[1],
    metadata: { ...metadata, artist: 'Committed artist', trackNumber: null },
  });
  w = await h.flush();
  const draft = h.calls.saveReview.at(-1);
  assert.deepEqual(draft.selected, [1]);
  assert.equal(draft.inspection.tracks[1].metadata.artist, 'Committed artist');
  assert.equal(draft.inspection.tracks[1].metadata.trackNumber, null);
  assert.deepEqual(draft.inspection.tracks[1].sourceMetadata, metadata);
  assert.equal(draft.preferences.sharedArtworkAsset, 'b'.repeat(64));
  assert.equal(draft.preferences.outputKind, 'folder');
  assert.deepEqual(Object.keys(draft).sort(), [
    'inspection',
    'preferences',
    'revision',
    'schemaVersion',
    'selected',
    'sessionId',
    'url',
  ]);
  assert.doesNotMatch(
    JSON.stringify(draft),
    /private-profile|browserProfile|cookie|managedSession/,
  );
  assert.equal(w.browserProfile, 'private-profile');
});

test('unfinished folder export recovery takes precedence over an unrelated saved review', async () => {
  const h = fixture();
  const review = savedReview();
  review.url = 'https://music.youtube.com/playlist?list=other-review';
  review.inspection.sourceUrl = review.url;
  const plan = {
    title: 'Recovered export',
    sourceUrl: album.sourceUrl,
    mode: 'album',
    format: 'mp3',
    artworkMode: 'shared',
    sharedArtworkUrl: null,
    sharedArtworkAsset: 'c'.repeat(64),
    outputKind: 'folder',
    auth: { browser: 'chrome', browserProfile: 'recover-profile' },
    tracks: structuredClone(album.tracks),
  };
  h.f.native.pendingExport = async () => ({
    jobId: 'retained-job',
    plan,
    outputPath: '/Music/Recovered export',
    progress: { stage: 'preparing', completed: 1, total: 2 },
    error: null,
  });
  h.f.native.loadReviewDraft = async () => {
    h.calls.loadReview.push(true);
    return review;
  };
  h.render();
  let w = await h.flush();
  assert.equal(h.calls.loadReview.length, 1);
  assert.deepEqual(w.savedReview, {
    title: review.preferences.title,
    trackCount: review.selected.length,
  });
  assert.equal(
    h.calls.saveReview.length,
    0,
    'recovery does not replace another saved review',
  );
  assert.equal(w.preferences.title, 'Recovered export');
  assert.equal(w.preferences.outputKind, 'folder');
  assert.equal(w.preferences.sharedArtworkAsset, 'c'.repeat(64));
  assert.equal(w.browserProfile, 'recover-profile');
  assert.equal(w.progress.stage, 'interrupted');
  assert.equal(w.progress.completed, 1);
  assert.equal(w.destinationPath, '/Music/Recovered export');
  await w.exportZip();
  w = await h.flush();
  assert.equal(h.calls.chooseFolder.length, 0);
  assert.equal(h.calls.choose.length, 0);
  assert.equal(h.calls.start[0][1], '/Music/Recovered export');
  assert.equal(h.calls.start[0][2], 'retained-job');
  assert.equal(h.calls.start[0][0].outputKind, 'folder');
  assert.equal(w.exporting, true);
});

test('failed partial inspection does not overwrite the last complete saved review', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  const saved = structuredClone(h.calls.saveReview.at(-1));
  const saves = h.calls.saveReview.length;
  await w.reset();
  w = h.render();
  const inspection = deferred();
  let inspectionId;
  h.f.native.inspectSource = (_url, _browser, id) => {
    inspectionId = id;
    return inspection.promise;
  };
  const operation = w.inspect('https://youtube.com/watch?v=partial');
  await h.flush();
  h.f.listeners.get('inspection-progress')({
    payload: {
      inspectionId,
      title: 'Incomplete replacement',
      sourceUrl: 'https://youtube.com/watch?v=partial',
      suggestedMode: 'album',
      artworkUrl: null,
      index: 0,
      total: 5,
      track: album.tracks[0],
    },
  });
  inspection.reject(new Error('Source unavailable'));
  await operation;
  w = await h.flush();
  assert.equal(w.inspectionStatus, 'failed');
  assert.equal(w.inspection.title, 'Incomplete replacement');
  assert.equal(h.calls.saveReview.length, saves);
  assert.deepEqual(h.calls.saveReview.at(-1), saved);
});

test('discard blocks intervening operations and late autosave completion cannot restore a saved status', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  const lateSave = deferred();
  h.f.native.saveReviewDraft = (draft) => {
    h.calls.saveReview.push(structuredClone(draft));
    return lateSave.promise;
  };
  w.updateTrack(0, {
    ...album.tracks[0],
    metadata: { ...metadata, title: 'Latest committed edit' },
  });
  w = h.render();
  assert.equal(w.draftStatus, 'saving');
  const discard = deferred();
  h.f.native.discardReviewDraft = (...args) => {
    h.calls.discardReview.push(args);
    return discard.promise;
  };
  const operation = w.discardDraft();
  w = h.render();
  assert.equal(w.busy, true);
  const saves = h.calls.saveReview.length;
  assert.equal(await w.reset(), false);
  await w.inspect('https://youtube.com/watch?v=intervening');
  await w.exportZip();
  w.applyBulk({ metadata: { artist: 'Must not apply' } });
  assert.equal(h.calls.inspect.length, 0);
  assert.equal(h.calls.start.length, 0);
  assert.equal(h.calls.saveReview.length, saves);
  assert.equal(h.calls.discardReview.length, 1);
  assert.ok(h.calls.discardReview[0][1] > h.calls.saveReview.at(-1).revision);
  lateSave.resolve(true);
  await h.flush();
  discard.resolve(true);
  await operation;
  w = await h.flush();
  assert.equal(w.inspection, null);
  assert.equal(w.draftStatus, null);
  assert.equal(w.busy, false);
  assert.equal(w.canUndoReset, false);
  assert.equal(h.calls.saveReview.length, saves);
});

test('failed review discard keeps the edited review and reports the native failure', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  h.f.native.discardReviewDraft = async () => false;
  await w.discardDraft();
  w = await h.flush();
  assert.deepEqual(w.inspection, album);
  assert.match(w.error, /discard|review|saved/i);
  assert.equal(w.busy, false);
});

test('switching output kind during retry chooses the matching destination and retains the media cache job', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  await w.exportZip();
  const id = h.calls.start[0][2];
  stopExport(h, id);
  w = h.render();
  w.setPreferences((p) => ({ ...p, outputKind: 'folder' }));
  w = await h.flush();
  assert.equal(w.destinationPath, null);
  assert.equal(w.savePath, null);
  await w.exportZip();
  assert.deepEqual(h.calls.chooseFolder, ['Album']);
  assert.equal(h.calls.start[1][0].outputKind, 'folder');
  assert.equal(h.calls.start[1][1], '/Music/Album');
  assert.equal(h.calls.start[1][2], id);
  stopExport(h, id, 'cancelled');
  w = h.render();
  await w.exportZip();
  assert.equal(
    h.calls.chooseFolder.length,
    1,
    'retry reuses its folder destination',
  );
  assert.equal(h.calls.start[2][1], '/Music/Album');
  stopExport(h, id);
  w = h.render();
  w.setPreferences((p) => ({ ...p, outputKind: 'zip' }));
  w = await h.flush();
  assert.equal(w.destinationPath, null);
  await w.exportZip();
  assert.equal(h.calls.choose.length, 2);
  assert.equal(h.calls.start[3][0].outputKind, 'zip');
  assert.equal(h.calls.start[3][1], '/Music/Album.zip');
  assert.equal(h.calls.start[3][2], id);
});

test('bulk changes and Undo each persist one complete selected-occurrence transaction', async () => {
  const h = fixture();
  const source = structuredClone(album);
  source.tracks = source.tracks.map((track, index) => ({
    ...track,
    id: 'repeated',
    sourceUrl: 'https://youtube.com/watch?v=repeated',
    metadata: { ...track.metadata, title: `Prefix ${index + 1}` },
  }));
  h.render().loadInspection(source);
  let w = await h.flush();
  w.setSelected(new Set([1]));
  w = await h.flush();
  const before = structuredClone(w.inspection);
  const saves = h.calls.saveReview.length;
  w.applyBulk({
    metadata: { artist: 'Bulk artist' },
    titleCleanup: { prefix: 'Prefix ', suffix: '' },
  });
  w = await h.flush();
  assert.equal(h.calls.saveReview.length, saves + 1);
  assert.deepEqual(w.inspection.tracks[0], before.tracks[0]);
  assert.equal(w.inspection.tracks[1].metadata.title, '2');
  assert.equal(w.inspection.tracks[1].metadata.artist, 'Bulk artist');
  assert.deepEqual(
    w.inspection.tracks[1].sourceMetadata,
    before.tracks[1].sourceMetadata,
  );
  assert.equal(w.bulkUndo.changes.length, 1);
  w.undoBulk();
  w = await h.flush();
  assert.equal(h.calls.saveReview.length, saves + 2);
  assert.deepEqual(w.inspection, before);
  assert.equal(w.bulkUndo, null);
});

test('startup draft read failure surfaces its message and can be explicitly discarded', async () => {
  const h = fixture();
  h.f.native.loadReviewDraft = async () => {
    throw new Error('This saved review needs a newer version of Jacket.');
  };
  h.render();
  let w = await h.flush();
  assert.equal(w.inspection, null);
  assert.match(w.error, /newer version/);
  await w.discardDraft();
  w = await h.flush();
  assert.equal(h.calls.discardReview.length, 1);
  assert.equal(w.draftStatus, null);
  assert.equal(w.error, null);
  w.loadInspection(album);
  w = await h.flush();
  assert.equal(w.draftStatus, 'saved');
  assert.equal(h.calls.saveReview.at(-1).inspection.title, 'Album');
});

test('a delayed startup review cannot replace a source the user has already inspected', async () => {
  const h = fixture();
  const draft = deferred();
  h.f.native.loadReviewDraft = () => draft.promise;
  h.render();
  let w = await h.flush();
  assert.equal(w.inspection, null);
  await w.inspect(album.sourceUrl);
  w = await h.flush();
  assert.equal(w.inspection.title, 'Album');
  draft.resolve(savedReview());
  w = await h.flush();
  assert.equal(w.inspection.title, 'Album');
  assert.equal(w.preferences.title, 'Album');
  assert.deepEqual([...w.selected], [0, 1]);
  assert.equal(h.calls.saveReview.at(-1).inspection.title, 'Album');
});

test('a delayed stale save response cannot replace the newest review save status', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  const stale = deferred();
  let saveCount = 0;
  h.f.native.saveReviewDraft = (draft) => {
    h.calls.saveReview.push(structuredClone(draft));
    saveCount += 1;
    return saveCount === 1 ? stale.promise : Promise.resolve(true);
  };
  w.setPreferences((p) => ({ ...p, title: 'First revision' }));
  w = h.render();
  assert.equal(w.draftStatus, 'saving');
  w.setPreferences((p) => ({ ...p, title: 'Newest revision' }));
  w = await h.flush();
  assert.equal(w.draftStatus, 'saved');
  assert.equal(h.calls.saveReview.at(-1).preferences.title, 'Newest revision');
  stale.resolve(false);
  w = await h.flush();
  assert.equal(w.draftStatus, 'saved');
  assert.equal(w.preferences.title, 'Newest revision');
  assert.ok(
    h.calls.saveReview.at(-1).revision > h.calls.saveReview.at(-2).revision,
  );
});

test('browser example reviews never write or replace the real saved review', async () => {
  const h = fixture();
  h.render().loadInspection(album, true);
  let w = await h.flush();
  w.applyBulk({ metadata: { artist: 'Example-only edit' } });
  w.setPreferences((p) => ({ ...p, outputKind: 'folder' }));
  w = await h.flush();
  assert.equal(w.inspection.tracks[0].metadata.artist, 'Example-only edit');
  assert.equal(h.calls.saveReview.length, 0);
  await w.exportZip();
  assert.equal(h.calls.start.length, 0);
});

test('discard does not erase committed review data when unfinished export recovery cannot be dismissed', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  await w.exportZip();
  const id = h.calls.start[0][2];
  stopExport(h, id);
  w = h.render();
  w.updateTrack(1, {
    ...album.tracks[1],
    metadata: { ...metadata, title: 'Edits newer than the export recovery' },
  });
  w = await h.flush();
  const saved = structuredClone(h.calls.saveReview.at(-1));
  h.f.native.dismissRecovery = async () => {
    throw new Error('Recovery could not be dismissed');
  };
  await w.discardDraft();
  w = await h.flush();
  assert.equal(
    w.inspection.tracks[1].metadata.title,
    'Edits newer than the export recovery',
  );
  assert.match(w.error, /Recovery could not be dismissed/);
  assert.equal(
    h.calls.discardReview.length,
    0,
    'the saved review remains available when recovery dismissal fails',
  );
  assert.deepEqual(h.calls.saveReview.at(-1), saved);
  assert.equal(w.busy, false);
});

function recoveredFolderPlan() {
  return {
    title: 'Recovered export',
    sourceUrl: album.sourceUrl,
    mode: 'album',
    format: 'mp3',
    artworkMode: 'shared',
    sharedArtworkUrl: null,
    sharedArtworkAsset: 'c'.repeat(64),
    outputKind: 'folder',
    auth: { browser: 'chrome', browserProfile: 'recover-profile' },
    tracks: structuredClone(album.tracks),
  };
}

function pendingFolderExport() {
  return {
    jobId: 'retained-job',
    plan: recoveredFolderPlan(),
    outputPath: '/Music/Recovered export',
    progress: { stage: 'preparing', completed: 1, total: 2 },
    error: 'Connection interrupted',
  };
}

test('same-source saved edits survive restart while recovery keeps its cache job, destination and browser choice', async () => {
  const h = fixture();
  const pending = pendingFolderExport();
  const review = savedReview();
  review.preferences.title = pending.plan.title;
  review.inspection.tracks[1].metadata.trackNumber = null;
  h.f.native.pendingExport = async () => pending;
  h.f.native.loadReviewDraft = async () => structuredClone(review);
  h.render();
  let w = await h.flush();
  assert.deepEqual(w.inspection, review.inspection);
  assert.equal(w.inspection.tracks[0].metadata.trackNumber, null);
  assert.equal(w.inspection.tracks[1].metadata.trackNumber, null);
  assert.deepEqual([...w.selected], [1]);
  assert.deepEqual(w.preferences, review.preferences);
  assert.equal(w.url, review.url);
  assert.equal(w.browser, 'chrome');
  assert.equal(w.browserProfile, 'recover-profile');
  assert.equal(w.progress.stage, 'interrupted');
  assert.equal(w.progress.jobId, 'retained-job');
  assert.equal(w.progress.completed, 1);
  assert.equal(w.destinationPath, '/Music/Recovered export');
  assert.equal(w.savedReview, null);
  for (const saved of h.calls.saveReview) {
    assert.deepEqual(saved.inspection, review.inspection);
    assert.deepEqual(saved.selected, review.selected);
  }
  await w.exportZip();
  w = await h.flush();
  assert.equal(h.calls.choose.length, 0);
  assert.equal(h.calls.chooseFolder.length, 0);
  assert.equal(h.calls.start[0][1], pending.outputPath);
  assert.equal(h.calls.start[0][2], pending.jobId);
  const plan = h.calls.start[0][0];
  assert.equal(plan.tracks.length, 1);
  assert.equal(plan.tracks[0].metadata.title, 'Reviewed second track');
  assert.equal(plan.tracks[0].metadata.trackNumber, null);
  assert.equal(plan.tracks[0].metadata.album, 'My album tags');
  assert.equal(plan.format, 'preserve');
  assert.equal(plan.sharedArtworkAsset, 'a'.repeat(64));
  assert.equal(plan.auth.browserProfile, 'recover-profile');
  assert.equal(w.exporting, true);
});

test('continuing another saved review dismisses recovery first, restores exact edits and starts publicly', async () => {
  const h = fixture();
  const pending = pendingFolderExport();
  const review = savedReview();
  review.url = 'https://music.youtube.com/playlist?list=another-review';
  review.inspection.sourceUrl = review.url;
  h.f.native.pendingExport = async () => pending;
  h.f.native.loadReviewDraft = async () => structuredClone(review);
  h.render();
  let w = await h.flush();
  assert.equal(w.inspection.sourceUrl, pending.plan.sourceUrl);
  assert.equal(h.calls.saveReview.length, 0);
  const dismissal = deferred();
  h.f.native.dismissRecovery = (jobId) => {
    h.calls.dismiss.push(jobId);
    return dismissal.promise;
  };
  const continuing = w.continueSavedReview();
  w = h.render();
  assert.equal(w.busy, true);
  assert.deepEqual(h.calls.dismiss, ['retained-job']);
  assert.equal(w.inspection.sourceUrl, pending.plan.sourceUrl);
  await w.inspect('https://youtube.com/watch?v=intervening');
  await w.exportZip();
  assert.equal(h.calls.inspect.length, 0);
  assert.equal(h.calls.start.length, 0);
  dismissal.resolve();
  await continuing;
  w = await h.flush();
  assert.deepEqual(w.inspection, review.inspection);
  assert.deepEqual(w.preferences, review.preferences);
  assert.deepEqual([...w.selected], [1]);
  assert.equal(w.url, review.url);
  assert.equal(w.browser, null);
  assert.equal(w.browserProfile, null);
  assert.equal(w.progress, null);
  assert.equal(w.destinationPath, null);
  assert.equal(w.savedReview, null);
  assert.equal(w.busy, false);
  assert.equal(h.calls.discardReview.length, 0);
  for (const saved of h.calls.saveReview) {
    assert.deepEqual(saved.inspection, review.inspection);
    assert.equal(saved.sessionId, review.sessionId);
    assert.ok(saved.revision > review.revision);
    assert.doesNotMatch(
      JSON.stringify(saved),
      /recover-profile|browserProfile/,
    );
  }
});

test('failed recovery dismissal keeps both the pending export and the other saved review available', async () => {
  const h = fixture();
  const pending = pendingFolderExport();
  const review = savedReview();
  review.url = 'https://music.youtube.com/playlist?list=another-review';
  review.inspection.sourceUrl = review.url;
  h.f.native.pendingExport = async () => pending;
  h.f.native.loadReviewDraft = async () => structuredClone(review);
  h.render();
  let w = await h.flush();
  h.f.native.dismissRecovery = async () => {
    throw new Error('Recovery could not be dismissed');
  };
  await w.continueSavedReview();
  w = await h.flush();
  assert.equal(w.inspection.sourceUrl, pending.plan.sourceUrl);
  assert.equal(w.preferences.title, pending.plan.title);
  assert.equal(w.browserProfile, 'recover-profile');
  assert.equal(w.progress.jobId, pending.jobId);
  assert.equal(w.destinationPath, pending.outputPath);
  assert.deepEqual(w.savedReview, {
    title: review.preferences.title,
    trackCount: review.selected.length,
  });
  assert.equal(h.calls.saveReview.length, 0);
  assert.equal(h.calls.discardReview.length, 0);
  assert.match(w.error, /Recovery could not be dismissed/);
  assert.equal(w.busy, false);
  h.f.native.dismissRecovery = async () => {};
  await w.continueSavedReview();
  w = await h.flush();
  assert.deepEqual(w.inspection, review.inspection);
  assert.equal(w.savedReview, null);
  assert.equal(w.error, null);
});

test('committed edits made while startup review loading is delayed are saved once loading settles', async () => {
  const h = fixture();
  const pending = pendingFolderExport();
  const delayedReview = deferred();
  h.f.native.pendingExport = async () => pending;
  h.f.native.loadReviewDraft = () => delayedReview.promise;
  h.render();
  let w = await h.flush();
  assert.equal(w.inspection.title, pending.plan.title);
  assert.equal(h.calls.saveReview.length, 0);
  w.updateTrack(1, {
    ...album.tracks[1],
    metadata: { ...metadata, title: 'Committed while startup was loading' },
  });
  w = await h.flush();
  assert.equal(
    w.inspection.tracks[1].metadata.title,
    'Committed while startup was loading',
  );
  delayedReview.resolve(savedReview());
  w = await h.flush();
  assert.equal(
    w.inspection.tracks[1].metadata.title,
    'Committed while startup was loading',
  );
  assert.equal(h.calls.saveReview.length, 1);
  assert.equal(
    h.calls.saveReview[0].inspection.tracks[1].metadata.title,
    'Committed while startup was loading',
  );
  assert.equal(w.draftStatus, 'saved');
});

function completeExport(h, jobId, warnings = []) {
  h.f.listeners.get('export-progress')({
    payload: {
      jobId,
      stage: 'complete',
      message: 'Your collection is ready',
      completed: 2,
      total: 2,
      error: null,
      result: {
        outputPath: '/Music/Album.zip',
        trackCount: 2,
        warnings,
      },
    },
  });
}

test('download waits for the exact committed review save before passing its completion identity', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  const pending = deferred();
  h.f.native.saveReviewDraft = (draft) => {
    h.calls.saveReview.push(structuredClone(draft));
    return pending.promise;
  };
  w.setSelected(new Set([1]));
  w.setPreferences((p) => ({
    ...p,
    sharedArtworkAsset: 'd'.repeat(64),
    artworkMode: 'shared',
  }));
  w.updateTrack(1, {
    ...album.tracks[1],
    metadata: { ...metadata, title: 'Exactly this committed title' },
  });
  w = h.render();
  const operation = w.exportZip();
  w = await h.flush();
  assert.equal(w.choosing, true);
  assert.equal(h.calls.start.length, 0);
  const exact = h.calls.saveReview.at(-1);
  assert.deepEqual(exact.selected, [1]);
  assert.equal(
    exact.inspection.tracks[1].metadata.title,
    'Exactly this committed title',
  );
  assert.equal(exact.preferences.sharedArtworkAsset, 'd'.repeat(64));
  pending.resolve(true);
  await operation;
  w = await h.flush();
  const [plan, , , identity] = h.calls.start[0];
  assert.deepEqual(identity, {
    sessionId: exact.sessionId,
    revision: exact.revision,
  });
  assert.equal(
    plan.tracks[0].metadata.title,
    exact.inspection.tracks[1].metadata.title,
  );
  assert.equal(plan.sharedArtworkAsset, exact.preferences.sharedArtworkAsset);
  assert.equal(w.exporting, true);
});

test('failed exact review flush preserves the review and prevents export until a successful retry', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  h.f.native.saveReviewDraft = async (draft) => {
    h.calls.saveReview.push(structuredClone(draft));
    return false;
  };
  w.updateTrack(0, {
    ...album.tracks[0],
    metadata: { ...metadata, title: 'Preserve me' },
  });
  w = await h.flush();
  await w.exportZip();
  w = await h.flush();
  assert.equal(h.calls.start.length, 0);
  assert.equal(w.inspection.tracks[0].metadata.title, 'Preserve me');
  assert.equal(w.draftStatus, 'failed');
  assert.equal(w.busy, false);
  assert.match(w.error, /review could not be saved/);
  h.f.native.saveReviewDraft = async (draft) => {
    h.calls.saveReview.push(structuredClone(draft));
    return true;
  };
  await w.exportZip();
  w = await h.flush();
  assert.equal(h.calls.start.length, 1);
  assert.equal(
    h.calls.start[0][3].revision,
    h.calls.saveReview.at(-1).revision,
  );
  assert.equal(w.exporting, true);
});

test('superseding a review while destination selection is pending prevents an outdated export', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  const chooser = deferred();
  h.f.native.chooseArchive = () => chooser.promise;
  const operation = w.exportZip();
  w.setPreferences((p) => ({ ...p, title: 'Changed while choosing' }));
  w = await h.flush();
  chooser.resolve('/Music/Album.zip');
  await operation;
  w = await h.flush();
  assert.equal(h.calls.start.length, 0);
  assert.equal(w.preferences.title, 'Changed while choosing');
  assert.match(w.error, /review changed while choosing/);
});

test('a superseding committed revision prevents an export waiting on an older save', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  const older = deferred();
  let writes = 0;
  h.f.native.saveReviewDraft = (draft) => {
    h.calls.saveReview.push(structuredClone(draft));
    return ++writes === 1 ? older.promise : Promise.resolve(true);
  };
  w.setPreferences((p) => ({ ...p, title: 'First committed revision' }));
  w = h.render();
  const operation = w.exportZip();
  w = await h.flush();
  w.setPreferences((p) => ({ ...p, title: 'Newer committed revision' }));
  w = await h.flush();
  older.resolve(true);
  await operation;
  w = await h.flush();
  assert.equal(h.calls.start.length, 0);
  assert.equal(w.preferences.title, 'Newer committed revision');
  assert.equal(w.draftStatus, 'saved');
  assert.match(w.error, /review changed before the download started/);
});

test('successful completion suppresses late save acknowledgements and repeated output copies remain in memory', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  const older = deferred();
  let writes = 0;
  h.f.native.saveReviewDraft = (draft) => {
    h.calls.saveReview.push(structuredClone(draft));
    return ++writes === 1 ? older.promise : Promise.resolve(true);
  };
  w.setPreferences((p) => ({ ...p, title: 'Earlier edit' }));
  w = h.render();
  w.setPreferences((p) => ({ ...p, title: 'Final album' }));
  w = await h.flush();
  await w.exportZip();
  const id = h.calls.start[0][2];
  completeExport(h, id, ['Some tags were omitted by this codec.']);
  w = await h.flush();
  const saves = h.calls.saveReview.length;
  assert.equal(w.progress.stage, 'complete');
  assert.equal(w.draftStatus, null);
  older.resolve(true);
  w = await h.flush();
  assert.equal(w.draftStatus, null);
  assert.equal(h.calls.saveReview.length, saves);
  assert.equal(
    h.calls.discardReview.length,
    0,
    'native completion owns cleanup',
  );
  w.prepareAnotherCopy();
  w = await h.flush();
  w.setPreferences((p) => ({ ...p, format: 'mp3', outputKind: 'folder' }));
  w.setSelected(new Set([1, 0]));
  w.updateTrack(0, structuredClone(w.inspection.tracks[0]));
  w = await h.flush();
  assert.equal(w.progress, null);
  assert.equal(
    h.calls.saveReview.length,
    saves,
    'output and no-op edits do not recreate a finished review',
  );
  await w.exportZip();
  assert.equal(h.calls.start[1][3], null);
  assert.equal(h.calls.saveReview.length, saves);
  completeExport(h, h.calls.start[1][2]);
  w = await h.flush();
  assert.equal(w.draftStatus, null);
});

test('New download and Undo preserve completed review memory-only status', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  await w.exportZip();
  completeExport(h, h.calls.start[0][2]);
  w = await h.flush();
  const saves = h.calls.saveReview.length;
  assert.equal(await w.reset(), true);
  w = await h.flush();
  assert.equal(w.inspection, null);
  assert.equal(w.draftStatus, null);
  assert.equal(w.undoReset(), true);
  w = await h.flush();
  assert.equal(w.progress.stage, 'complete');
  assert.equal(w.draftStatus, null);
  assert.equal(h.calls.saveReview.length, saves);
  w.prepareAnotherCopy();
  w = await h.flush();
  assert.equal(h.calls.saveReview.length, saves);
});

test('a deliberate metadata edit after completion creates a fresh unfinished generation', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  await w.exportZip();
  const finished = h.calls.start[0][3];
  completeExport(h, h.calls.start[0][2]);
  w = await h.flush();
  w.prepareAnotherCopy();
  w = await h.flush();
  w.applyBulk({ metadata: { artist: 'A corrected artist' } });
  w = await h.flush();
  const unfinished = h.calls.saveReview.at(-1);
  assert.notEqual(unfinished.sessionId, finished.sessionId);
  assert.equal(unfinished.revision, 1);
  assert.equal(
    unfinished.inspection.tracks[0].metadata.artist,
    'A corrected artist',
  );
  assert.equal(w.draftStatus, 'saved');
  await w.exportZip();
  assert.deepEqual(h.calls.start[1][3], {
    sessionId: unfinished.sessionId,
    revision: unfinished.revision,
  });
});

test('new artwork and changed selection each make completed albums unfinished again', async () => {
  for (const edit of [
    (w) =>
      w.setPreferences((p) => ({
        ...p,
        artworkMode: 'shared',
        sharedArtworkAsset: 'e'.repeat(64),
      })),
    (w) => w.setSelected(new Set([1])),
  ]) {
    const h = fixture();
    h.render().loadInspection(album);
    let w = await h.flush();
    await w.exportZip();
    const finished = h.calls.start[0][3];
    completeExport(h, h.calls.start[0][2]);
    w = await h.flush();
    w.prepareAnotherCopy();
    w = await h.flush();
    edit(w);
    w = await h.flush();
    assert.notEqual(h.calls.saveReview.at(-1).sessionId, finished.sessionId);
    assert.equal(w.draftStatus, 'saved');
  }
});

test('failed and cancelled downloads retain the unfinished review and its retry completion identity', async () => {
  for (const stage of ['failed', 'cancelled']) {
    const h = fixture();
    h.render().loadInspection(album);
    let w = await h.flush();
    await w.exportZip();
    const [, , id, identity] = h.calls.start[0];
    stopExport(h, id, stage);
    w = await h.flush();
    assert.equal(w.draftStatus, 'saved');
    assert.equal(w.inspection.title, 'Album');
    assert.equal(h.calls.discardReview.length, 0);
    await w.exportZip();
    assert.deepEqual(h.calls.start[1][3], identity);
    assert.equal(h.calls.start[1][2], id);
  }
});

test('cancelled destination selection keeps the unfinished review without starting or clearing it', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  const saves = h.calls.saveReview.length;
  h.f.native.chooseArchive = async () => null;
  await w.exportZip();
  w = await h.flush();
  assert.equal(w.inspection.title, 'Album');
  assert.equal(w.draftStatus, 'saved');
  assert.equal(h.calls.start.length, 0);
  assert.equal(h.calls.saveReview.length, saves);
  assert.equal(h.calls.discardReview.length, 0);
});

test('untouched recovery uses its original identity and preserves a different saved session even for the same source', async () => {
  const h = fixture();
  const pending = pendingFolderExport();
  pending.reviewIdentity = { sessionId: 'exported-generation', revision: 5 };
  const unrelated = savedReview();
  assert.equal(unrelated.inspection.sourceUrl, pending.plan.sourceUrl);
  assert.notEqual(unrelated.sessionId, pending.reviewIdentity.sessionId);
  h.f.native.pendingExport = async () => pending;
  h.f.native.loadReviewDraft = async () => structuredClone(unrelated);
  h.render();
  let w = await h.flush();
  assert.equal(w.inspection.title, pending.plan.title);
  assert.ok(w.savedReview);
  await w.exportZip();
  assert.deepEqual(h.calls.start[0][3], pending.reviewIdentity);
  assert.equal(h.calls.saveReview.length, 0);
  completeExport(h, pending.jobId);
  w = await h.flush();
  assert.equal(w.draftStatus, null);
  assert.equal(h.calls.saveReview.length, 0);
  assert.equal(h.calls.discardReview.length, 0);
  assert.ok(w.savedReview);
  await w.continueSavedReview();
  w = await h.flush();
  assert.deepEqual(w.inspection, unrelated.inspection);
  assert.equal(w.draftStatus, 'saved');
  assert.equal(h.calls.saveReview.at(-1).sessionId, unrelated.sessionId);
});

test('native success can arrive before start acknowledgement without recreating a review', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  const start = deferred();
  h.f.native.startExport = (...args) => {
    h.calls.start.push(args);
    completeExport(h, args[2]);
    return start.promise;
  };
  const operation = w.exportZip();
  w = await h.flush();
  assert.equal(w.progress.stage, 'complete');
  assert.equal(w.draftStatus, null);
  const saves = h.calls.saveReview.length;
  start.resolve();
  await operation;
  w = await h.flush();
  assert.equal(w.progress.stage, 'complete');
  assert.equal(w.draftStatus, null);
  assert.equal(h.calls.saveReview.length, saves);
});

test('stale completion from an earlier job cannot finish a newly inspected review', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  await w.exportZip();
  const older = h.calls.start[0][2];
  stopExport(h, older);
  w = h.render();
  await w.reset();
  w = h.render();
  w.loadInspection({
    ...album,
    title: 'A new album',
    sourceUrl: 'https://youtube.com/watch?v=new-album',
  });
  w = await h.flush();
  completeExport(h, older);
  w = await h.flush();
  assert.equal(w.inspection.title, 'A new album');
  assert.equal(w.progress, null);
  assert.equal(w.draftStatus, 'saved');
});

test('discarding recovery with a same-source different saved session continues that saved review safely', async () => {
  const h = fixture();
  const pending = pendingFolderExport();
  pending.reviewIdentity = { sessionId: 'older-generation', revision: 4 };
  const review = savedReview();
  h.f.native.pendingExport = async () => pending;
  h.f.native.loadReviewDraft = async () => structuredClone(review);
  h.render();
  let w = await h.flush();
  assert.ok(w.savedReview);
  await w.discardDraft();
  w = await h.flush();
  assert.deepEqual(h.calls.discardReview, []);
  assert.deepEqual(h.calls.dismiss, [pending.jobId]);
  assert.deepEqual(w.inspection, review.inspection);
  assert.equal(w.progress, null);
  assert.equal(w.draftStatus, 'saved');
});

test('completion of an older exported revision leaves newer committed edits unfinished', async () => {
  const h = fixture();
  h.render().loadInspection(album);
  let w = await h.flush();
  await w.exportZip();
  const [, , id, exported] = h.calls.start[0];
  w = h.render();
  w.updateTrack(0, {
    ...album.tracks[0],
    metadata: { ...metadata, title: 'A newer committed title' },
  });
  w = await h.flush();
  const newer = h.calls.saveReview.at(-1);
  assert.equal(newer.sessionId, exported.sessionId);
  assert.ok(newer.revision > exported.revision);
  completeExport(h, id);
  w = await h.flush();
  assert.equal(w.draftStatus, 'saved');
  assert.equal(
    w.inspection.tracks[0].metadata.title,
    'A newer committed title',
  );
  w.prepareAnotherCopy();
  w = await h.flush();
  await w.exportZip();
  assert.deepEqual(h.calls.start[1][3], {
    sessionId: newer.sessionId,
    revision: newer.revision,
  });
});

test('export waits for startup review arbitration so a delayed unrelated review cannot be replaced', async () => {
  const h = fixture();
  const pending = pendingFolderExport();
  pending.reviewIdentity = { sessionId: 'pending-generation', revision: 3 };
  const lookup = deferred();
  h.f.native.pendingExport = async () => pending;
  h.f.native.loadReviewDraft = () => lookup.promise;
  h.render();
  let w = await h.flush();
  assert.equal(w.progress.stage, 'interrupted');
  assert.equal(w.draftReady, false);
  await w.exportZip();
  assert.equal(h.calls.start.length, 0);
  assert.equal(h.calls.saveReview.length, 0);
  lookup.resolve(savedReview());
  w = await h.flush();
  assert.equal(w.draftReady, true);
  await w.exportZip();
  assert.equal(h.calls.saveReview.length, 0);
  assert.deepEqual(h.calls.start[0][3], pending.reviewIdentity);
});
