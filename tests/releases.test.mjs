import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import {
  selectDraft,
  uploadDraft,
  validateTitle,
} from '../.github/scripts/releases.mjs';

const sha = 'a'.repeat(40);
const tag = 'v0.1.0-beta.1';
const context = {
  repo: { owner: 'MoonlarkStudios', repo: 'jacket' },
  runId: 42,
};

function fixture() {
  const release = {
    id: 1,
    tag_name: tag,
    draft: true,
    target_commitish: sha,
    html_url: 'https://example.test/draft',
    body: 'Reviewed release notes',
    assets: [],
  };
  const writes = [];
  const repos = {
    listReleases: async () => ({ data: [release] }),
    getCommit: async () => ({ data: { sha } }),
    compareCommitsWithBasehead: async () => ({ data: { status: 'ahead' } }),
    getRelease: async () => ({ data: release }),
    deleteReleaseAsset: async ({ asset_id }) => {
      writes.push('delete');
      release.assets = release.assets.filter((asset) => asset.id !== asset_id);
    },
    uploadReleaseAsset: async ({ name, data }) => {
      writes.push('upload');
      release.assets.push({
        id: release.assets.length + 1,
        name,
        size: data.length,
        digest: `sha256:${createHash('sha256').update(data).digest('hex')}`,
      });
    },
    updateRelease: async (options) => {
      assert.equal(options.draft, true, 'automation must never publish');
      assert.equal(
        options.tag_name,
        tag,
        'draft edits must retain the actual release tag',
      );
      assert.equal(
        options.target_commitish,
        sha,
        'draft edits must retain the exact release commit',
      );
      writes.push('update');
      Object.assign(release, options);
      return { data: release };
    },
  };
  const core = {
    summary: { addRaw() {}, async write() {} },
  };
  const github = {
    rest: { repos },
    async paginate(method, options) {
      assert.equal(method, repos.listReleases);
      assert.deepEqual(options, { ...context.repo, per_page: 100 });
      return (await method(options)).data;
    },
  };
  return { release, writes, repos, github, core };
}

test('Conventional Commit titles allow scopes and breaking changes', () => {
  for (const title of [
    'feat: add album search',
    'fix(linux): retain unfinished work',
    'feat!: change exported manifest',
    'chore(main): release 0.1.0-beta.1',
  ])
    validateTitle(title);
  for (const title of [
    'Add search',
    'feat: ',
    'fix: bad\nextra',
    'feat scope: wrong',
  ]) {
    assert.throws(() => validateTitle(title), /Conventional Commit/);
  }
});

test('draft lookup uses authenticated pagination instead of the published-tag endpoint', async () => {
  const state = fixture();
  state.repos.getReleaseByTag = async () => {
    throw new Error('Published-tag endpoint cannot locate drafts');
  };
  state.github.paginate = async (method, options) => {
    assert.equal(method, state.repos.listReleases);
    assert.deepEqual(options, { ...context.repo, per_page: 100 });
    return [{ tag_name: 'v0.2.0', draft: true }, state.release];
  };
  const selected = await selectDraft({
    ...state,
    context,
    tag,
    expectedSha: sha,
  });
  assert.equal(selected.release, state.release);
  assert.equal(selected.sha, sha);
});

test('missing and ambiguous release listings are rejected without mutations', async () => {
  const state = fixture();
  for (const releases of [[], [state.release, { ...state.release, id: 2 }]]) {
    state.repos.listReleases = async () => ({ data: releases });
    await assert.rejects(
      selectDraft({ ...state, context, tag }),
      /exactly one release/,
    );
  }
  assert.deepEqual(state.writes, []);
});

test('published releases, moved tags, and commits outside main are rejected', async () => {
  const state = fixture();
  state.release.draft = false;
  await assert.rejects(
    selectDraft({ ...state, context, tag }),
    /Published releases/,
  );
  state.release.draft = true;
  await assert.rejects(
    selectDraft({ ...state, context, tag, expectedSha: 'b'.repeat(40) }),
    /moved/,
  );
  state.repos.compareCommitsWithBasehead = async () => ({
    data: { status: 'diverged' },
  });
  await assert.rejects(
    selectDraft({ ...state, context, tag }),
    /belong to main/,
  );
  assert.deepEqual(state.writes, []);
});

test('mutable release targets and malformed tags are rejected', async () => {
  const state = fixture();
  state.release.target_commitish = 'main';
  await assert.rejects(selectDraft({ ...state, context, tag }), /immutable/);
  for (const invalid of [
    'main',
    'v0.1.0-beta.0',
    'v0.1.0+build',
    '../v0.1.0',
  ]) {
    await assert.rejects(
      selectDraft({ ...state, context, tag: invalid }),
      /Invalid release tag/,
    );
  }
});

test('draft uploads verify server digests and safely resume partial uploads', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'jacket-release-'));
  try {
    await writeFile(join(directory, 'SHA256SUMS'), 'fixture');
    const state = fixture();
    await uploadDraft({ ...state, context, tag, sha, directory });
    assert.equal(state.release.draft, true);
    assert.equal(state.release.prerelease, true);
    assert.match(state.release.body, /Reviewed release notes/);
    assert.match(state.release.body, /actions\/runs\/42/);
    assert.deepEqual(state.writes, ['upload', 'update']);
    state.writes.length = 0;
    await uploadDraft({ ...state, context, tag, sha, directory });
    assert.deepEqual(state.writes, ['update']);
    assert.equal(
      state.release.body.split('<!-- jacket-artifacts:start -->').length,
      2,
    );
    await writeFile(join(directory, 'SHA256SUMS'), 'replacement');
    state.writes.length = 0;
    await uploadDraft({ ...state, context, tag, sha, directory });
    assert.deepEqual(state.writes, ['delete', 'upload', 'update']);
  } finally {
    await rm(directory, { recursive: true });
  }
});

test('server changes to draft identity prevent a ready summary', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'jacket-release-'));
  try {
    await writeFile(join(directory, 'asset'), 'fixture');
    for (const change of [
      { tag_name: 'untagged-placeholder' },
      { target_commitish: 'main' },
      { draft: false },
    ]) {
      const state = fixture();
      state.repos.updateRelease = async () => ({
        data: { ...state.release, ...change },
      });
      state.core.summary.addRaw = () =>
        assert.fail('An invalid draft must not be marked ready');
      await assert.rejects(
        uploadDraft({ ...state, context, tag, sha, directory }),
        /Draft identity changed/,
      );
    }
  } finally {
    await rm(directory, { recursive: true });
  }
});

test('publishing during an upload prevents further mutations', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'jacket-release-'));
  try {
    await writeFile(join(directory, 'asset'), 'fixture');
    const state = fixture();
    state.repos.getRelease = async () => {
      state.release.draft = false;
      return { data: state.release };
    };
    await assert.rejects(
      uploadDraft({ ...state, context, tag, sha, directory }),
      /published while/,
    );
    assert.deepEqual(state.writes, []);
  } finally {
    await rm(directory, { recursive: true });
  }
});

test('missing server digests prevent a draft from being marked ready', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'jacket-release-'));
  try {
    await writeFile(join(directory, 'asset'), 'fixture');
    const state = fixture();
    state.repos.uploadReleaseAsset = async () => {
      state.release.assets.push({ name: 'asset', size: 7 });
    };
    await assert.rejects(
      uploadDraft({ ...state, context, tag, sha, directory }),
      /verification failed/,
    );
    assert.deepEqual(state.writes, []);
  } finally {
    await rm(directory, { recursive: true });
  }
});

test('release configuration keeps every Rust and frontend version in one release', async () => {
  const config = JSON.parse(
    await readFile(new URL('../release-please-config.json', import.meta.url)),
  );
  const release = config.packages['.'];
  assert.equal(release.draft, true);
  assert.equal(release['force-tag-creation'], true);
  assert.equal(release['bump-minor-pre-major'], true);
  assert.equal(release.versioning, 'prerelease');
  assert.deepEqual(
    release['extra-files'].map((file) => file.path),
    ['src-tauri/tauri.conf.json', 'Cargo.toml', 'Cargo.lock'],
  );
});
