import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

const bundle = await build({
  entryPoints: ['src/useDesktopShell.ts'],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
  plugins: [
    {
      name: 'desktop-shell-fixture',
      setup(build) {
        build.onResolve(
          { filter: /^(react|\.\/desktop|@tauri-apps\/api\/(core|event))$/ },
          ({ path }) => ({ path, namespace: 'fixture' }),
        );
        build.onLoad({ filter: /.*/, namespace: 'fixture' }, ({ path }) => ({
          contents:
            path === 'react'
              ? `export const useEffect = (effect) => globalThis.shellFixture.effects.push(effect);
                 export const useRef = (current) => ({ current });`
              : path === './desktop'
                ? 'export const desktopAvailable = true;'
                : path.endsWith('/event')
                  ? `export async function listen(name, callback) {
                       const f = globalThis.shellFixture;
                       f.calls.push('listen:' + name);
                       f.listeners.set(name, callback);
                       return () => f.listeners.delete(name);
                     }`
                  : `export async function invoke(command, args) {
                       const f = globalThis.shellFixture;
                       f.calls.push(command);
                       if (command === 'take_menu_request') return f.pending.shift() || null;
                       f.status = args;
                     }`,
        }));
      },
    },
  ],
});
const { useDesktopShell } = await import(
  `data:text/javascript;base64,${Buffer.from(bundle.outputFiles[0].text).toString('base64')}`
);

async function settle() {
  await new Promise(setImmediate);
}

async function mount({
  pending = [],
  busy = false,
  draft = false,
  reset,
} = {}) {
  const f = {
    pending,
    effects: [],
    listeners: new Map(),
    calls: [],
    errors: [],
    resets: 0,
    examples: [],
    settings: 0,
  };
  globalThis.shellFixture = f;
  // biome-ignore lint/correctness/useHookAtTopLevel: The harness supplies mocked effects and refs without mounting React.
  useDesktopShell(
    {
      busy,
      setError: (error) => f.errors.push(error),
      loadInspection: (value, example) => f.examples.push({ value, example }),
    },
    () => f.settings++,
    async () => {
      f.resets++;
      return reset ? reset() : true;
    },
    draft,
  );
  const cleanups = f.effects.map((effect) => effect());
  f.unmount = () =>
    cleanups.forEach((cleanup) => {
      cleanup?.();
    });
  await settle();
  return f;
}

test('startup menu requests are retained and drained after subscribing', async () => {
  const f = await mount({ pending: [{ kind: 'new' }, { kind: 'settings' }] });
  assert.ok(
    f.calls.indexOf('listen:jacket-menu-request') <
      f.calls.indexOf('take_menu_request'),
  );
  assert.equal(f.resets, 1);
  assert.equal(f.settings, 1);
  assert.deepEqual(f.status, { busy: false });
  assert.deepEqual(f.errors, []);
  f.unmount();
  assert.equal(f.listeners.size, 0);
});

test('native menu navigation cannot replace a running operation but can open Settings', async () => {
  const f = await mount({
    busy: true,
    pending: [{ kind: 'new' }, { kind: 'example' }, { kind: 'settings' }],
  });
  assert.equal(f.resets, 0);
  assert.equal(f.examples.length, 0);
  assert.equal(f.settings, 1);
  assert.equal(f.errors.length, 2);
  assert.ok(f.errors.every((error) => /current operation/.test(error)));
  assert.deepEqual(f.status, { busy: true });
  f.unmount();
});

test('native menu navigation preserves an open metadata draft', async () => {
  const f = await mount({
    draft: true,
    pending: [{ kind: 'new' }, { kind: 'settings' }, { kind: 'example' }],
  });
  assert.equal(f.resets, 0);
  assert.equal(f.settings, 0);
  assert.equal(f.examples.length, 0);
  assert.equal(f.errors.length, 3);
  assert.ok(f.errors.every((error) => /current editor/.test(error)));
  f.unmount();
});

test('a failed recovery dismissal cannot replace the current album with example content', async () => {
  const f = await mount({ pending: [{ kind: 'example' }], reset: () => false });
  assert.equal(f.resets, 1);
  assert.equal(f.examples.length, 0);
  f.unmount();
});

test('example preview loads only after the old workspace is reset', async () => {
  const f = await mount({ pending: [{ kind: 'example' }] });
  assert.equal(f.resets, 1);
  assert.equal(f.examples.length, 1);
  assert.equal(f.examples[0].example, true);
  assert.ok(f.examples[0].value.tracks.length > 0);
  f.unmount();
});

test('menu requests arriving during async reset are processed serially', async () => {
  let release;
  const blocked = new Promise((resolve) => {
    release = resolve;
  });
  const f = await mount({ pending: [{ kind: 'new' }], reset: () => blocked });
  assert.equal(f.resets, 1);
  f.pending.push({ kind: 'settings' });
  f.listeners.get('jacket-menu-request')();
  await settle();
  assert.equal(f.settings, 0);
  release(true);
  await settle();
  assert.equal(f.resets, 1);
  assert.equal(f.settings, 1);
  assert.deepEqual(f.pending, []);
  f.unmount();
});
