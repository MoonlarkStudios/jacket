import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import {
  installerNames,
  renderReleaseNotes,
} from '../.github/scripts/release-notes.mjs';

const options = {
  tag: 'v0.1.0-beta.1',
  sha: 'a'.repeat(40),
  runId: 42,
  repo: 'MoonlarkStudios/jacket',
};

test('release template provides usable draft downloads, exact versions and source links', async () => {
  const body = await renderReleaseNotes({
    ...options,
    body: '## [0.1.0-beta.1](https://example.test/missing-baseline) (2026-10-16)\n\n### Features\n\n* Linux installers',
  });
  assert.match(body, /Assets\*\* below/);
  assert.match(body, /Jacket_0\.1\.0-beta\.1_amd64\.deb/);
  assert.match(body, /Jacket-0\.1\.0-0\.beta\.1\.x86_64\.rpm/);
  assert.match(body, /Linux x86_64 preview/);
  assert.match(body, /fictional example album/);
  assert.match(body, /### Features\n\n\* Linux installers/);
  assert.match(body, new RegExp(`/blob/${options.sha}/docs/usage.md`));
  assert.match(body, /actions\/runs\/42/);
  assert.doesNotMatch(
    body,
    /missing-baseline|releases\/download\/|untagged-|\{\{/,
  );
});

test('installer filenames preserve native prerelease and stable ordering', () => {
  for (const [tag, rpm] of [
    ['v0.1.0-alpha.1', 'Jacket-0.1.0-0.alpha.1.x86_64.rpm'],
    ['v0.1.0-beta.10', 'Jacket-0.1.0-0.beta.10.x86_64.rpm'],
    ['v0.1.0-rc.1', 'Jacket-0.1.0-0.rc.1.x86_64.rpm'],
    ['v0.1.0', 'Jacket-0.1.0-1.x86_64.rpm'],
  ]) {
    assert.deepEqual(installerNames(tag), {
      deb: `Jacket_${tag.slice(1)}_amd64.deb`,
      rpm,
    });
  }
  for (const tag of [
    'main',
    'v0.1.0-beta.0',
    'v0.1.0+build',
    'v01.0.0',
    'v0.1.0/../bad',
  ])
    assert.throws(() => installerNames(tag), /Invalid release tag/);
});

test('stable releases use the stable installer and release label', async () => {
  const body = await renderReleaseNotes({ ...options, tag: 'v0.1.0' });
  assert.match(body, /Linux x86_64 release/);
  assert.match(body, /Jacket-0\.1\.0-1\.x86_64\.rpm/);
  assert.doesNotMatch(body, /Jacket_[^\n]+beta/);
});

test('rebuilds preserve editorial changes and acceptance results outside managed sections', async () => {
  let body = await renderReleaseNotes({
    ...options,
    body: 'Highlights: better audio exports.',
  });
  body = body.replace(
    '## Tested scope and known limitations',
    '## Tested on my GNOME desktop',
  );
  body += '\nHuman acceptance: native dialogs passed; KDE still untested.\n';
  const rebuilt = await renderReleaseNotes({ ...options, runId: 99, body });
  assert.match(rebuilt, /Highlights: better audio exports\./);
  assert.match(rebuilt, /Tested on my GNOME desktop/);
  assert.match(
    rebuilt,
    /Human acceptance: native dialogs passed; KDE still untested/,
  );
  assert.match(rebuilt, /actions\/runs\/99/);
  assert.doesNotMatch(rebuilt, /actions\/runs\/42/);
  assert.equal(rebuilt.split('<!-- jacket-downloads:start -->').length, 2);
  assert.equal(rebuilt.split('<!-- jacket-verification:start -->').length, 2);
});

test('malformed managed sections fail rather than replacing human notes', async () => {
  const body = await renderReleaseNotes(options);
  await assert.rejects(
    renderReleaseNotes({
      ...options,
      body: body.replace('<!-- jacket-downloads:end -->', ''),
    }),
    /managed downloads/,
  );
  await assert.rejects(
    renderReleaseNotes({
      ...options,
      body: `${body}\n<!-- jacket-downloads:start -->duplicate<!-- jacket-downloads:end -->`,
    }),
    /managed downloads/,
  );
});

test('template errors and invalid release identity fail before rendering', async () => {
  const template = await readFile(
    new URL('../.github/release-template.md', import.meta.url),
    'utf8',
  );
  await assert.rejects(
    renderReleaseNotes({
      ...options,
      template: template.replace('{{version}}', '{{unknown}}'),
    }),
    /Unknown release template token/,
  );
  await assert.rejects(
    renderReleaseNotes({ ...options, sha: 'main' }),
    /full commit SHA/,
  );
  await assert.rejects(
    renderReleaseNotes({ ...options, runId: 0 }),
    /workflow run ID/,
  );
});

test('changelog text is inserted literally', async () => {
  const body = await renderReleaseNotes({
    ...options,
    changes: 'Keep $& and {{user_text}} exactly as written.',
  });
  assert.match(body, /Keep \$& and \{\{user_text\}\} exactly as written\./);
});

// A manually created draft can have a null body before its first upload.
test('empty draft bodies and prototype-named template tokens are handled safely', async () => {
  const rendered = await renderReleaseNotes({ ...options, body: null });
  assert.match(rendered, /See the linked build commit/);
  const template = await readFile(
    new URL('../.github/release-template.md', import.meta.url),
    'utf8',
  );
  await assert.rejects(
    renderReleaseNotes({
      ...options,
      template: template.replace('{{version}}', '{{constructor}}'),
    }),
    /Unknown release template token/,
  );
  await assert.rejects(
    renderReleaseNotes({ ...options, repo: 'MoonlarkStudios/..' }),
    /Invalid release repository/,
  );
});
