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
              ? 'export const useState = (...args) => globalThis.workspaceFixture.state(...args); export const useRef = (...args) => globalThis.workspaceFixture.ref(...args); export const useCallback = (...args) => globalThis.workspaceFixture.callback(...args); export const useEffect = (...args) => globalThis.workspaceFixture.effect(...args);'
              : path.endsWith('/event')
                ? 'export async function listen(name, handler) { const f = globalThis.workspaceFixture; f.listeners.set(name, handler); return () => f.listeners.delete(name); }'
                : path.endsWith('useDownloader')
                  ? 'export function useDownloader() { return {}; }'
                  : path.endsWith('useBrowserSession')
                    ? `import { useState, useCallback } from 'react'; export function useBrowserSession() { const [selection, setSelection] = useState({browser:null,profileId:null}); return {selection, testing:false, attach:(browser,profileId)=>setSelection({browser,profileId}), restore:useCallback(({browser,browserProfile})=>setSelection({browser,profileId:browserProfile??null}),[])}; }`
                    : `export const desktopAvailable = true; ${['archiveDestination', 'cancelExport', 'cancelInspection', 'chooseArchive', 'dependencyStatus', 'dismissRecovery', 'inspectSource', 'pendingExport', 'startExport'].map((name) => `export const ${name} = (...args) => globalThis.workspaceFixture.native.${name}(...args);`).join('\n')}`;
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
  const calls = { dismiss: [], start: [], inspect: [], choose: [] };
  const same = (a, b) =>
    a && b && a.length === b.length && a.every((v, i) => Object.is(v, b[i]));
  const f = {
    listeners: new Map(),
    native: {
      dependencyStatus: async () => [],
      pendingExport: async () => null,
      archiveDestination: async (title) => `/Downloads/${title}.zip`,
      chooseArchive: async (title) => {
        calls.choose.push(title);
        return '/Music/Album.zip';
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
    for (let i = 0; i < 5; i++) {
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
  assert.deepEqual(h.calls.start[1], [plan, path, id]);
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
