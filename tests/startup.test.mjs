import assert from 'node:assert/strict';
import { test } from 'node:test';
import vm from 'node:vm';
import { build } from 'esbuild';

const bundle = await build({
  entryPoints: ['src/bootstrap.ts'],
  bundle: true,
  write: false,
  format: 'iife',
  plugins: [
    {
      name: 'failed-application-module',
      setup(build) {
        build.onResolve({ filter: /^\.\/main$/ }, () => ({
          path: 'main',
          namespace: 'failure',
        }));
        build.onLoad({ filter: /.*/, namespace: 'failure' }, () => ({
          contents: 'throw new Error("Outdated optimized dependency");',
        }));
      },
    },
  ],
});

function browserFixture() {
  const clicks = new Map();
  const events = new Map();
  const recovery = {
    role: 'status',
    setAttribute(name, value) {
      this[name] = value;
    },
  };
  const message = { textContent: 'Opening Jacket…' };
  const retry = {
    hidden: true,
    addEventListener: (name, fn) => clicks.set(name, fn),
  };
  const elements = new Map([
    ['startup-recovery', recovery],
    ['startup-message', message],
    ['startup-retry', retry],
  ]);
  let reloads = 0;
  vm.runInNewContext(bundle.outputFiles[0].text, {
    document: { getElementById: (id) => elements.get(id) },
    window: {
      addEventListener: (name, fn) => events.set(name, fn),
      location: { reload: () => reloads++ },
    },
    console: { error() {} },
  });
  return { recovery, message, retry, clicks, events, reloads: () => reloads };
}

test('a failed application import retains a visible recovery view and working reload action', async () => {
  const page = browserFixture();
  await new Promise(setImmediate);
  assert.equal(page.recovery.role, 'alert');
  assert.match(page.message.textContent, /couldn’t open/);
  assert.equal(page.retry.hidden, false);
  page.clicks.get('click')();
  assert.equal(page.reloads(), 1);
});

test('a stale Vite chunk exposes recovery instead of leaving a blank root or reloading in a loop', () => {
  const page = browserFixture();
  let prevented = false;
  page.events.get('vite:preloadError')({
    preventDefault: () => {
      prevented = true;
    },
  });
  assert.equal(prevented, true);
  assert.equal(page.retry.hidden, false);
  assert.equal(page.recovery.role, 'alert');
  assert.equal(page.reloads(), 0);
});
