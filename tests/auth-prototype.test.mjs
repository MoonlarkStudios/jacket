import assert from 'node:assert/strict';
import {
  chmod,
  lstat,
  mkdir,
  mkdtemp,
  readdir,
  rm,
  symlink,
} from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import {
  diagnose,
  exactProfileArgument,
  filterYouTubeCookies,
  launchArguments,
  main,
  prototypePaths,
  validateProfileLocation,
} from '../scripts/auth-prototype.mjs';

async function withHome(fn) {
  const home = await mkdtemp(path.join(os.tmpdir(), 'audio-dl-auth-fixture-'));
  try {
    await fn(home);
  } finally {
    await rm(home, { recursive: true, force: true });
  }
}

test('diagnose is read-only and uses an exact owned profile without personal profile discovery', async () => {
  await withHome(async (home) => {
    const info = await diagnose({ home, searchPath: ':relative/path' });
    assert.deepEqual(await readdir(home), []);
    assert.equal(info.ytDlp === null || path.isAbsolute(info.ytDlp), true);
    assert.equal(
      info.futureYtDlpProfileArgument,
      `chrome:${prototypePaths(home).profile}/Default`,
    );
    assert.deepEqual(info.chromeArguments, launchArguments(home));
    assert.ok(
      !info.chromeArguments.some((arg) =>
        /remote-debugging|user-agent/.test(arg),
      ),
    );
    assert.ok(!info.profile.profile.includes('/Google/Chrome'));
  });
});

test('only explicit open is accepted; missing/extra/profile arguments do not launch or create anything', async () => {
  for (const args of [
    [],
    ['export'],
    ['open', '--profile=/personal'],
    ['diagnose', 'open'],
  ]) {
    await assert.rejects(main(args), /Usage:/);
  }
  assert.throws(() => prototypePaths('relative/home'), /absolute/);
  assert.equal(
    exactProfileArgument('/fixture'),
    'chrome:/fixture/Library/Application Support/com.moonlark.audio-dl.auth-prototype/chrome-profile/Default',
  );
});

test('create prepares private owned directories without fabricating Chrome Default or credentials', async () => {
  await withHome(async (home) => {
    const locations = await validateProfileLocation(home, true);
    for (const directory of [locations.root, locations.profile]) {
      assert.equal((await lstat(directory)).mode & 0o777, 0o700);
    }
    assert.deepEqual(await readdir(locations.profile), []);
    await validateProfileLocation(home);
  });
});

test('profile location rejects unsafe existing directories and symlink substitution', async () => {
  await withHome(async (home) => {
    const { root, profile, browserProfile } = await validateProfileLocation(
      home,
      true,
    );
    await chmod(root, 0o755);
    await assert.rejects(validateProfileLocation(home), /owner-only/);
    await chmod(root, 0o700);
    const other = path.join(home, 'unrelated');
    await mkdir(other);
    await symlink(other, browserProfile);
    await assert.rejects(validateProfileLocation(home), /symlink/);
    await rm(browserProfile);
    await rm(profile, { recursive: true });
    await symlink(other, profile);
    await assert.rejects(validateProfileLocation(home, true), /symlink/);
    assert.deepEqual(await readdir(other), []);
  });
});

test('cookie fixture filter permits YouTube domains only and preserves export attributes without mutation', () => {
  const fixtures = [
    {
      domain: '.youtube.com',
      name: 'FIXTURE_SESSION',
      value: 'synthetic',
      httpOnly: true,
      secure: true,
      session: true,
    },
    {
      domain: 'music.youtube.com',
      name: 'FIXTURE_PREF',
      value: 'synthetic',
      hostOnly: true,
      expirationDate: 1900000000,
    },
    { domain: 'YOUTUBE.COM', name: 'FIXTURE_CASE', value: 'synthetic' },
    ...[
      'google.com',
      '.accounts.google.com',
      'youtu.be',
      'youtube.com.attacker.test',
      'notyoutube.com',
      '.youtube.com.',
      '..youtube.com',
      ' youtube.com',
      'https://youtube.com',
      'youtube.com/path',
      'youtube.com\n',
      '-bad.youtube.com',
    ].map((domain) => ({ domain, value: 'synthetic' })),
    null,
    {},
  ];
  const snapshot = structuredClone(fixtures);
  const result = filterYouTubeCookies(fixtures);
  assert.deepEqual(result, fixtures.slice(0, 3));
  assert.deepEqual(fixtures, snapshot);
  assert.notEqual(result[0], fixtures[0]);
  assert.throws(() => filterYouTubeCookies('not a fixture list'), /fixtures/);
});
