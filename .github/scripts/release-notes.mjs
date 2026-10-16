import { readFile } from 'node:fs/promises';

const marker = '<!-- jacket-release-template:v1 -->';
const managedSections = ['downloads', 'verification'];

export function installerNames(tag) {
  const match =
    /^v((?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))(?:-((?:alpha|beta|rc)\.[1-9]\d*))?$/.exec(
      tag,
    );
  if (!match) throw new Error(`Invalid release tag: ${tag}`);
  const version = tag.slice(1);
  return {
    deb: `Jacket_${version}_amd64.deb`,
    rpm: `Jacket-${match[1]}-${match[2] ? `0.${match[2]}` : '1'}.x86_64.rpm`,
  };
}

function sectionPattern(name) {
  return new RegExp(
    `<!-- jacket-${name}:start -->[\\s\\S]*?<!-- jacket-${name}:end -->`,
    'g',
  );
}

function singleSection(body, name) {
  const matches = body.match(sectionPattern(name)) || [];
  if (
    matches.length !== 1 ||
    body.split(`<!-- jacket-${name}:start -->`).length !== 2 ||
    body.split(`<!-- jacket-${name}:end -->`).length !== 2
  )
    throw new Error(`Expected one managed ${name} section in release notes`);
  return matches[0];
}

export async function renderReleaseNotes({
  body = '',
  tag,
  sha,
  runId,
  repo,
  changes,
  template,
}) {
  const names = installerNames(tag);
  if (!/^[a-f0-9]{40}$/.test(sha))
    throw new Error('Release notes require a full commit SHA');
  if (!Number.isSafeInteger(runId) || runId <= 0)
    throw new Error('Release notes require a workflow run ID');
  if (
    !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repo) ||
    repo.split('/').some((part) => part === '.' || part === '..')
  )
    throw new Error('Invalid release repository');
  const repoUrl = `https://github.com/${repo}`;
  const values = {
    version: tag.slice(1),
    release_kind: tag.includes('-') ? 'preview' : 'release',
    deb_filename: names.deb,
    rpm_filename: names.rpm,
    raw_source_url: `https://raw.githubusercontent.com/${repo}/${sha}`,
    source_url: `${repoUrl}/blob/${sha}`,
    sha,
    commit_url: `${repoUrl}/commit/${sha}`,
    run_url: `${repoUrl}/actions/runs/${runId}`,
    repo_url: repoUrl,
  };
  const source =
    template ??
    (await readFile(
      new URL('../release-template.md', import.meta.url),
      'utf8',
    ));
  if (!source.startsWith(marker))
    throw new Error('Missing release template version marker');
  for (const name of managedSections) singleSection(source, name);
  const original = (body ?? '')
    .replace(
      /\n?<!-- jacket-artifacts:start -->[\s\S]*?<!-- jacket-artifacts:end -->/g,
      '',
    )
    .trim();
  // Release Please's first compare link may refer to an unreleased baseline.
  // Keep the actual changelog, removing only its redundant version heading.
  const changelog = (changes ?? original)
    .replace(/^## \[[^\]]+\]\([^\n]+\)[^\n]*\n+/, '')
    .trim();
  if (source.split('{{changes}}').length !== 2)
    throw new Error('Expected one changelog slot in the release template');
  const filled = source.replace(/\{\{([^{}]+)\}\}/g, (token, key) => {
    if (key === 'changes') return '{{changes}}';
    if (!Object.hasOwn(values, key))
      throw new Error(`Unknown release template token: ${token}`);
    return values[key];
  });
  if (!original.includes(marker))
    return `${filled.replace('{{changes}}', () => changelog || 'See the linked build commit for the changes in this release.').trim()}\n`;
  if (original.split(marker).length !== 2)
    throw new Error('Expected one release template version marker');
  // Once initialized, only these sections belong to automation. All editorial
  // changes, release highlights and acceptance results survive rebuilds.
  let updated = original;
  for (const name of managedSections) {
    singleSection(updated, name);
    updated = updated.replace(sectionPattern(name), () =>
      singleSection(filled, name),
    );
  }
  return `${updated.trim()}\n`;
}
