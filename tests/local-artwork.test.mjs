import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

const result = await build({
  stdin: {
    contents:
      "export { LocalArtwork } from './src/components/LocalArtwork'; export { setDesktopAvailable } from './src/desktop';",
    resolveDir: process.cwd(),
  },
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
  plugins: [
    {
      name: 'local-artwork-fixture',
      setup(build) {
        build.onResolve(
          {
            filter:
              /^(react|react\/jsx-runtime|@mantine\/core|@tauri-apps\/api\/webview|\.\.\/desktop|\.\/src\/desktop)$/,
          },
          ({ path }) => ({
            path: path.endsWith('/desktop') ? 'desktop' : path,
            namespace: 'fixture',
          }),
        );
        build.onLoad({ filter: /.*/, namespace: 'fixture' }, ({ path }) => ({
          contents:
            path === 'react'
              ? 'export const useState = (...args) => globalThis.localArtworkFixture.state(...args); export const useRef = (...args) => globalThis.localArtworkFixture.ref(...args); export const useEffect = (...args) => globalThis.localArtworkFixture.effect(...args);'
              : path === 'react/jsx-runtime'
                ? 'export const jsx = (type, props, key) => ({ type, props, key }); export const jsxs = jsx;'
                : path === '@mantine/core'
                  ? ['Alert', 'Button', 'Group', 'Slider', 'Stack', 'Text']
                      .map((name) => `export const ${name} = '${name}';`)
                      .join('\n')
                  : path.endsWith('/webview')
                    ? 'export const getCurrentWebview = () => globalThis.localArtworkFixture.webview;'
                    : `export let desktopAvailable = true; export const setDesktopAvailable = (value) => { desktopAvailable = value; }; ${['chooseArtwork', 'importArtwork', 'cropArtwork', 'artworkAsset'].map((name) => `export const ${name} = (...args) => globalThis.localArtworkFixture.native.${name}(...args);`).join('\n')}`,
        }));
      },
    },
  ],
});
const { LocalArtwork, setDesktopAvailable } = await import(
  `data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`
);

const image = {
  id: 'imported',
  previewDataUrl: 'data:image/jpeg;base64,aW1hZ2U=',
  width: 1200,
  height: 800,
};
const saved = { ...image, id: 'saved', width: 800, height: 800 };
const cropped = { ...saved, id: 'cropped' };

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function fixture(overrides = {}, desktop = true) {
  setDesktopAvailable(desktop);
  const hooks = [];
  let cursor = 0;
  let tree;
  let dirty = true;
  let effects = [];
  let unmounted = false;
  let updatesAfterUnmount = 0;
  let drop;
  let unlistened = 0;
  let subscribed = 0;
  const selected = [];
  const busyChanges = [];
  const calls = { choose: 0, import: [], crop: [], asset: [] };
  const props = {
    currentAssetId: null,
    busy: false,
    onSelect: (asset) => selected.push(asset),
    onBusyChange: (busy) => busyChanges.push(busy),
    ...overrides,
  };
  const f = {
    webview: {
      onDragDropEvent(handler) {
        subscribed++;
        drop = handler;
        return Promise.resolve(() => unlistened++);
      },
    },
    native: {
      async chooseArtwork() {
        calls.choose++;
        return image;
      },
      async importArtwork(path) {
        calls.import.push(path);
        return image;
      },
      async cropArtwork(...args) {
        calls.crop.push(args);
        return cropped;
      },
      async artworkAsset(id) {
        calls.asset.push(id);
        return saved;
      },
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
          if (unmounted) updatesAfterUnmount++;
          const value =
            typeof next === 'function' ? next(hooks[index].value) : next;
          if (!Object.is(value, hooks[index].value)) dirty = true;
          hooks[index].value = value;
        },
      ];
    },
    ref(initial) {
      const index = cursor++;
      hooks[index] ??= { current: initial };
      return hooks[index];
    },
    effect(fn, deps) {
      const index = cursor++;
      const previous = hooks[index];
      if (previous?.deps?.every((value, i) => Object.is(value, deps[i])))
        return;
      hooks[index] = { deps };
      effects.push(() => {
        previous?.cleanup?.();
        hooks[index].cleanup = fn();
      });
    },
  };
  globalThis.localArtworkFixture = f;

  function render() {
    let pass = 0;
    while (dirty && !unmounted) {
      assert.ok(pass++ < 20, 'updates settle');
      cursor = 0;
      dirty = false;
      tree = LocalArtwork(props);
      const pending = effects;
      effects = [];
      for (const effect of pending) effect();
    }
  }
  async function flush() {
    for (let i = 0; i < 6; i++) {
      await Promise.resolve();
      render();
    }
  }
  function elements(node) {
    if (Array.isArray(node)) return node.flatMap(elements);
    if (!node || typeof node !== 'object') return [];
    return [node, ...elements(node.props?.children)];
  }
  function find(type, name) {
    return elements(tree).find(
      (node) =>
        node.type === type &&
        (!name ||
          node.props.children === name ||
          node.props['aria-label'] === name ||
          node.props.alt === name),
    )?.props;
  }
  render();
  return {
    f,
    find,
    flush,
    calls,
    selected,
    busyChanges,
    async choose() {
      find('Button', 'Choose cover image').onClick();
      await flush();
    },
    changeProps(patch) {
      Object.assign(props, patch);
      dirty = true;
      render();
    },
    drop(payload) {
      drop({ payload });
    },
    unmount() {
      unmounted = true;
      for (const hook of hooks) hook.cleanup?.();
    },
    get updatesAfterUnmount() {
      return updatesAfterUnmount;
    },
    get subscribed() {
      return subscribed;
    },
    get unlistened() {
      return unlistened;
    },
  };
}

test('picker imports a centered square draft and selects only after crop confirmation', async () => {
  const h = fixture();
  await h.choose();
  assert.equal(h.calls.choose, 1);
  assert.deepEqual(h.selected, []);
  assert.equal(h.find('Slider', 'Crop size').value, 800);
  assert.equal(h.find('Slider', 'Horizontal crop position').value, 200);
  assert.equal(h.find('Slider', 'Vertical crop position').value, 0);
  assert.equal(h.find('Slider', 'Vertical crop position').disabled, true);
  const preview = h.find('img', 'Square cover crop preview');
  assert.equal(preview.style.width, '150%');
  assert.equal(preview.style.left, '-25%');
  h.find('Button', 'Apply cover').onClick();
  await h.flush();
  assert.deepEqual(h.calls.crop, [['imported', { x: 200, y: 0, size: 800 }]]);
  assert.deepEqual(h.selected, [cropped]);
  assert.deepEqual(h.busyChanges, [true, false, true, false]);
  assert.equal(h.find('img', 'Square cover crop preview'), undefined);
  h.unmount();
});

test('crop resize keeps its center and clamps both position controls inside the image', async () => {
  const h = fixture();
  await h.choose();
  h.find('Slider', 'Crop size').onChange(400);
  await h.flush();
  assert.equal(h.find('Slider', 'Horizontal crop position').value, 400);
  assert.equal(h.find('Slider', 'Vertical crop position').value, 200);
  h.find('Slider', 'Horizontal crop position').onChange(10000);
  h.find('Slider', 'Vertical crop position').onChange(-10000);
  await h.flush();
  h.find('Button', 'Apply cover').onClick();
  await h.flush();
  assert.deepEqual(h.calls.crop[0][1], { x: 800, y: 0, size: 400 });
  h.unmount();
});

test('restoring a saved cover and cancelling a replacement never recrop or change selection', async () => {
  const h = fixture({ currentAssetId: 'saved' });
  await h.flush();
  assert.ok(h.find('img', 'Current imported cover'));
  assert.deepEqual(h.calls.asset, ['saved']);
  await h.choose();
  h.find('Button', 'Cancel crop').onClick();
  await h.flush();
  assert.ok(h.find('img', 'Current imported cover'));
  assert.deepEqual(h.calls.crop, []);
  assert.deepEqual(h.selected, []);
  h.unmount();
});

test('picker cancellation preserves the selected cover and releases parent busy state', async () => {
  const h = fixture({ currentAssetId: 'saved' });
  h.f.native.chooseArtwork = async () => null;
  await h.choose();
  assert.ok(h.find('img', 'Current imported cover'));
  assert.equal(h.find('Button', 'Apply cover'), undefined);
  assert.deepEqual(h.selected, []);
  assert.deepEqual(h.busyChanges, [true, false]);
  h.unmount();
});

test('native drop imports the first supported image and rejects unsupported paths', async () => {
  const h = fixture();
  h.drop({
    type: 'drop',
    paths: ['/images/readme.txt', '/images/COVER.JPEG', '/images/second.png'],
  });
  await h.flush();
  assert.deepEqual(h.calls.import, ['/images/COVER.JPEG']);
  h.drop({ type: 'drop', paths: ['/images/file.svg'] });
  await h.flush();
  assert.match(h.find('Alert').children, /Drop a JPEG or PNG/);
  assert.equal(h.calls.import.length, 1);
  assert.equal(h.subscribed, 1);
  h.unmount();
  assert.equal(h.unlistened, 1);
});

test('pending imports serialize native work and ignore a late result after unmount', async () => {
  const h = fixture();
  const pending = deferred();
  h.f.native.importArtwork = (path) => {
    h.calls.import.push(path);
    return pending.promise;
  };
  h.drop({ type: 'drop', paths: ['/first.png'] });
  h.drop({ type: 'drop', paths: ['/second.png'] });
  assert.deepEqual(h.calls.import, ['/first.png']);
  h.unmount();
  pending.resolve(image);
  await h.flush();
  assert.equal(h.updatesAfterUnmount, 0);
  assert.deepEqual(h.selected, []);
  assert.deepEqual(h.busyChanges, [true, false]);
});

test('late drag-drop registration is disposed when the component unmounts first', async () => {
  const h = fixture();
  h.unmount();
  // Resolve the subscription created before cleanup; its handler must be removed.
  await h.flush();
  assert.equal(h.unlistened, 1);
});

test('failed crop preserves the draft and current selection for retry', async () => {
  const h = fixture({ currentAssetId: 'saved' });
  await h.choose();
  h.f.native.cropArtwork = async () => {
    throw new Error('Image processing failed');
  };
  h.find('Button', 'Apply cover').onClick();
  await h.flush();
  assert.deepEqual(h.selected, []);
  assert.ok(h.find('img', 'Square cover crop preview'));
  assert.match(h.find('Alert').children, /Try applying the crop again/);
  assert.equal(h.find('Button', 'Apply cover').disabled, false);
  assert.deepEqual(h.busyChanges, [true, false, true, false]);
  h.unmount();
});

test('stale saved-cover lookups and updates after unmount are discarded', async () => {
  const h = fixture();
  const first = deferred();
  const second = deferred();
  h.f.native.artworkAsset = (id) =>
    id === 'old' ? first.promise : second.promise;
  h.changeProps({ currentAssetId: 'old' });
  h.changeProps({ currentAssetId: 'new' });
  second.resolve({ ...saved, previewDataUrl: 'data:new' });
  await h.flush();
  first.resolve({ ...saved, previewDataUrl: 'data:old' });
  await h.flush();
  assert.equal(h.find('img', 'Current imported cover').src, 'data:new');
  h.unmount();
  assert.equal(h.updatesAfterUnmount, 0);
});

test('browser preview disables native picker and does not subscribe to native events', async () => {
  const h = fixture({}, false);
  assert.equal(h.find('Button', 'Choose cover image').disabled, true);
  await h.choose();
  assert.equal(h.calls.choose, 0);
  assert.equal(h.subscribed, 0);
  h.unmount();
});

test('a failed import keeps the current cover and reports actionable guidance', async () => {
  const h = fixture({ currentAssetId: 'saved' });
  h.f.native.chooseArtwork = async () => {
    throw new Error('Choose an image smaller than 10 MiB.');
  };
  await h.choose();
  assert.ok(h.find('img', 'Current imported cover'));
  assert.match(h.find('Alert').children, /Try another JPEG or PNG/);
  assert.deepEqual(h.selected, []);
  assert.deepEqual(h.busyChanges, [true, false]);
  h.unmount();
});

test('a failed crop preview blocks confirmation until another image is imported', async () => {
  const h = fixture();
  await h.choose();
  h.find('img', 'Square cover crop preview').onError();
  await h.flush();
  assert.equal(h.find('Button', 'Apply cover').disabled, true);
  h.find('Button', 'Apply cover').onClick();
  await h.flush();
  assert.deepEqual(h.calls.crop, []);
  await h.choose();
  assert.equal(h.find('Button', 'Apply cover').disabled, false);
  h.unmount();
});

test('a late completed crop cannot select a cover after unmount', async () => {
  const h = fixture();
  await h.choose();
  const pending = deferred();
  h.f.native.cropArtwork = () => pending.promise;
  h.find('Button', 'Apply cover').onClick();
  h.unmount();
  pending.resolve(cropped);
  await h.flush();
  assert.deepEqual(h.selected, []);
  assert.equal(h.updatesAfterUnmount, 0);
  assert.deepEqual(h.busyChanges, [true, false, true, false]);
});
