import { createHash } from 'node:crypto';
import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';

const tagPattern =
  /^v(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-(?:alpha|beta|rc)\.[1-9]\d*)?$/;

export function validateTitle(title) {
  if (
    !/^(?:feat|fix|perf|refactor|docs|test|build|ci|chore|revert|style)(?:\([a-zA-Z0-9_./-]+\))?!?: \S.+$/.test(
      title,
    )
  ) {
    throw new Error(
      'Use a Conventional Commit PR title, such as "feat: add album search" or "fix(linux): preserve unfinished downloads". The squash commit must use this title.',
    );
  }
}

export async function checkTitle({ github, context, core }) {
  const number =
    context.payload.pull_request?.number ||
    Number(context.payload.inputs?.pr_number);
  if (!Number.isSafeInteger(number) || number <= 0)
    throw new Error('A pull request number is required');
  const { data: pr } = await github.rest.pulls.get({
    ...context.repo,
    pull_number: number,
  });
  validateTitle(pr.title);
  core.info(`Conventional Commit title accepted for PR #${number}`);
}

export async function selectDraft({ github, context, tag, expectedSha }) {
  if (!tagPattern.test(tag)) throw new Error(`Invalid release tag: ${tag}`);
  const { data: release } = await github.rest.repos.getReleaseByTag({
    ...context.repo,
    tag,
  });
  if (!release.draft)
    throw new Error(
      'Published releases are immutable to this workflow; choose a new version',
    );
  const { data: commit } = await github.rest.repos.getCommit({
    ...context.repo,
    ref: tag,
  });
  if (expectedSha && expectedSha !== commit.sha)
    throw new Error('Release tag moved away from its expected commit');
  const { data: comparison } =
    await github.rest.repos.compareCommitsWithBasehead({
      ...context.repo,
      basehead: `${commit.sha}...main`,
    });
  if (!['ahead', 'identical'].includes(comparison.status))
    throw new Error('Release commit must already belong to main');
  if (release.target_commitish !== commit.sha)
    throw new Error('Draft release must target the immutable tag commit SHA');
  return { release, sha: commit.sha };
}

export async function uploadDraft({
  github,
  context,
  core,
  tag,
  sha,
  directory,
}) {
  let { release } = await selectDraft({
    github,
    context,
    tag,
    expectedSha: sha,
  });
  const files = (await readdir(directory)).sort();
  for (const name of files) {
    // Re-check before every mutation, including reruns after a partial upload.
    const { data: current } = await github.rest.repos.getRelease({
      ...context.repo,
      release_id: release.id,
    });
    if (!current.draft)
      throw new Error(
        'Release was published while preparing assets; refusing to modify it',
      );
    const data = await readFile(join(directory, name));
    const digest = `sha256:${createHash('sha256').update(data).digest('hex')}`;
    const existing = current.assets.find((asset) => asset.name === name);
    if (existing?.digest === digest && existing.size === data.length) continue;
    if (existing)
      await github.rest.repos.deleteReleaseAsset({
        ...context.repo,
        asset_id: existing.id,
      });
    await github.rest.repos.uploadReleaseAsset({
      ...context.repo,
      release_id: release.id,
      name,
      data,
      headers: {
        'content-type': 'application/octet-stream',
        'content-length': data.length,
      },
    });
  }
  ({ data: release } = await github.rest.repos.getRelease({
    ...context.repo,
    release_id: release.id,
  }));
  if (!release.draft) throw new Error('Release is no longer a draft');
  for (const name of files) {
    const data = await readFile(join(directory, name));
    const asset = release.assets.find((entry) => entry.name === name);
    if (
      !asset ||
      asset.size !== data.length ||
      asset.digest !==
        `sha256:${createHash('sha256').update(data).digest('hex')}`
    )
      throw new Error(`Release asset verification failed: ${name}`);
  }
  if (release.assets.some((asset) => !files.includes(asset.name)))
    throw new Error(
      'Draft contains unexpected assets; review them before retrying',
    );
  const run = `https://github.com/${context.repo.owner}/${context.repo.repo}/actions/runs/${context.runId}`;
  const notes = `<!-- jacket-artifacts:start -->
## Linux x86_64 preview

Download the **.deb** for Ubuntu 24.04 / Debian 13, or the **.rpm** for Fedora 44. Install the new package through your package manager to update Jacket. Older distributions and Linux ARM are not supported by this preview.

The accompanying source archives, dependency inventory, license notices, build information, and SHA256SUMS belong to these exact installers. Keep them together when redistributing.

Build commit: \`${sha}\`. [Automated build and package checks](${run}) passed.

### Maintainer acceptance before publishing

- [ ] Inspect installer-test screenshots and complete the applicable [release checklist](https://github.com/${context.repo.owner}/${context.repo.repo}/blob/${sha}/docs/releasing.md).
- [ ] Record real GNOME/KDE Wayland/X11 and download/recovery results, or clearly list untested scenarios and known issues here.
- [ ] Review these notes and confirm that all installers and source companions are attached.

This preview does not include macOS or Windows installers, signed APT/DNF repositories, or whole-app automatic updates.
<!-- jacket-artifacts:end -->`;
  const original = (release.body || '')
    .replace(
      /\n?<!-- jacket-artifacts:start -->[\s\S]*?<!-- jacket-artifacts:end -->/g,
      '',
    )
    .trim();
  await github.rest.repos.updateRelease({
    ...context.repo,
    release_id: release.id,
    body: `${original}\n\n${notes}`,
    draft: true,
    prerelease: tag.includes('-'),
  });
  core.summary.addRaw(
    `Draft release assets verified: [${tag}](${release.html_url}). Publication requires a maintainer.\n`,
  );
  await core.summary.write();
}
