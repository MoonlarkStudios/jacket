import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

const bundle = await build({
  entryPoints: ['src/useBrowserSession.ts'],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
  plugins: [
    {
      name: 'browser-session-fixture',
      setup(build) {
        build.onResolve({ filter: /^(react|\.\/desktop)$/ }, ({ path }) => ({
          path,
          namespace: 'fixture',
        }));
        build.onLoad({ filter: /.*/, namespace: 'fixture' }, ({ path }) => ({
          contents:
            path === 'react'
              ? `
      export function useState(initial) {
        const f = globalThis.browserSessionFixture, index = f.cursor++;
        if (index >= f.slots.length) f.slots.push(typeof initial === 'function' ? initial() : initial);
        return [f.slots[index], value => { f.slots[index] = typeof value === 'function' ? value(f.slots[index]) : value; }];
      }
      export const useCallback = fn => fn;
      export function useRef(current) { return useState(() => ({current}))[0]; }
    `
              : `
      export async function browserProfiles(browser) { const f = globalThis.browserSessionFixture; f.calls.push(['profiles',browser]); return f.profiles(browser); }
      export async function testBrowserAccess(browser, profile, url) { const f = globalThis.browserSessionFixture; f.calls.push(['test',browser,profile,url]); return f.access(); }
      export async function cancelExport(id) { globalThis.browserSessionFixture.calls.push(['cancel',id]); }
    `,
        }));
      },
    },
  ],
});
const { useBrowserSession } = await import(
  `data:text/javascript;base64,${Buffer.from(bundle.outputFiles[0].text).toString('base64')}`
);

function mount() {
  const fixture = {
    cursor: 0,
    slots: [],
    calls: [],
    profiles: async () => ({ profiles: [], notice: null }),
    access: async () => ({
      accessible: true,
      message: 'Accessible',
      guidance: 'Public access is not account verification.',
    }),
  };
  globalThis.browserSessionFixture = fixture;
  return {
    fixture,
    render() {
      fixture.cursor = 0;
      // biome-ignore lint/correctness/useHookAtTopLevel: React hooks are supplied by the state-preserving test harness.
      return useBrowserSession();
    },
  };
}
function deferred() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

test('public access is the default and stopping session use never touches the browser', () => {
  const { fixture, render } = mount();
  assert.deepEqual(render().selection, { browser: null, profileId: null });
  render().attach('chrome', 'profile-music');
  assert.deepEqual(render().selection, {
    browser: 'chrome',
    profileId: 'profile-music',
  });
  render().stop();
  assert.deepEqual(render().selection, { browser: null, profileId: null });
  assert.deepEqual(fixture.calls, []);
});

test('recovery restores an exact existing profile without enabling the owned prototype', () => {
  const { render } = mount();
  render().restore({ browser: 'firefox', browserProfile: 'profile-container' });
  assert.deepEqual(render().selection, {
    browser: 'firefox',
    profileId: 'profile-container',
  });
  render().restore({ browser: 'chrome' });
  assert.deepEqual(render().selection, { browser: 'chrome', profileId: null });
  render().restore({ browser: null, managedSession: 'experimental-session' });
  assert.deepEqual(render().selection, { browser: null, profileId: null });
});

test('a slower profile lookup cannot replace the browser the user selected later', async () => {
  const { fixture, render } = mount();
  const chrome = deferred(),
    firefox = deferred();
  fixture.profiles = (browser) =>
    browser === 'chrome' ? chrome.promise : firefox.promise;
  const first = render().refresh('chrome');
  const second = render().refresh('firefox');
  firefox.resolve({
    profiles: [{ id: 'firefox-profile', label: 'Music' }],
    notice: null,
  });
  await second;
  chrome.resolve({
    profiles: [{ id: 'chrome-profile', label: 'Personal' }],
    notice: null,
  });
  await first;
  assert.equal(render().catalog.browser, 'firefox');
  assert.equal(render().refreshing, false);
});

test('access tests reread the chosen session and a failed reconnect cannot retain a passing result', async () => {
  const { fixture, render } = mount();
  const url = 'https://www.youtube.com/watch?v=fixture';
  await render().test('chrome', 'profile-music', url);
  assert.equal(render().result.value.accessible, true);
  fixture.access = async () => ({
    accessible: false,
    message: 'Session expired',
    guidance: 'Sign in in the selected profile, then test again.',
  });
  await render().test('chrome', 'profile-music', url);
  assert.equal(render().result.value.accessible, false);
  assert.equal(fixture.calls.filter(([kind]) => kind === 'test').length, 2);
  render().clearResult();
  assert.equal(render().result, null);
});

test('an access test can be cancelled and duplicate clicks cannot launch two reads', async () => {
  const { fixture, render } = mount();
  const response = deferred();
  fixture.access = () => response.promise;
  const first = render().test(
    'chrome',
    'profile-music',
    'https://www.youtube.com/watch?v=fixture',
  );
  await render().test(
    'chrome',
    'profile-music',
    'https://www.youtube.com/watch?v=fixture',
  );
  await render().cancelTest();
  assert.equal(fixture.calls.filter(([kind]) => kind === 'test').length, 1);
  assert.deepEqual(fixture.calls.at(-1), ['cancel', 'browser-access']);
  response.resolve({ accessible: false, message: 'Cancelled', guidance: '' });
  await first;
  assert.equal(render().testing, false);
});
