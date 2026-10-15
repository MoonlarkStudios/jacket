#!/usr/bin/env node
// Developer-only feasibility probe. No cookie extraction or production auth.
import { spawn } from 'node:child_process';
import { constants } from 'node:fs';
import { access, lstat, mkdir } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const OWNED_DIRECTORY = 'com.moonlark.audio-dl.auth-prototype';

export function prototypePaths(home = os.homedir()) {
  if (!path.isAbsolute(home))
    throw new Error('Home directory must be absolute.');
  const root = path.join(
    home,
    'Library',
    'Application Support',
    OWNED_DIRECTORY,
  );
  const profile = path.join(root, 'chrome-profile');
  return { root, profile, browserProfile: path.join(profile, 'Default') };
}

export function launchArguments(home = os.homedir()) {
  return [
    `--user-data-dir=${prototypePaths(home).profile}`,
    '--no-first-run',
    '--no-default-browser-check',
    'https://www.youtube.com/',
  ];
}

export function exactProfileArgument(home = os.homedir()) {
  return `chrome:${prototypePaths(home).browserProfile}`;
}

// Only a future exporter uses this pure helper. This CLI never reads cookies.
// Preserve HttpOnly, secure, hostOnly and expiry fields for later serialization.
export function filterYouTubeCookies(cookies) {
  if (!Array.isArray(cookies)) throw new TypeError('Expected cookie fixtures.');
  return cookies.flatMap((cookie) => {
    if (!cookie || typeof cookie.domain !== 'string') return [];
    const domain = cookie.domain.toLowerCase().replace(/^\./, '');
    if (!/^(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)*youtube\.com$/.test(domain))
      return [];
    return [{ ...cookie }];
  });
}

async function existingDirectory(directory, privateDirectory = false) {
  let info;
  try {
    info = await lstat(directory);
  } catch (error) {
    if (error.code === 'ENOENT') return false;
    throw error;
  }
  if (info.isSymbolicLink() || !info.isDirectory()) {
    throw new Error(`Refusing a symlink or non-directory: ${directory}`);
  }
  if (privateDirectory) {
    if (typeof process.getuid === 'function' && info.uid !== process.getuid())
      throw new Error(
        `Prototype directory belongs to another user: ${directory}`,
      );
    if ((info.mode & 0o077) !== 0)
      throw new Error(
        `Prototype directory must be owner-only (0700): ${directory}`,
      );
  }
  return true;
}

// Fixed paths only: no --profile option and no fallback to Chrome's own folders.
export async function validateProfileLocation(
  home = os.homedir(),
  create = false,
) {
  const { root, profile, browserProfile } = prototypePaths(home);
  const directories = [
    home,
    path.join(home, 'Library'),
    path.join(home, 'Library', 'Application Support'),
    root,
    profile,
    browserProfile,
  ];
  for (const directory of directories) {
    const privateDirectory = directory === root || directory === profile;
    const exists = await existingDirectory(directory, privateDirectory);
    if (!exists && create && directory !== browserProfile) {
      await mkdir(directory, { mode: 0o700 });
      await existingDirectory(directory, privateDirectory);
    }
  }
  return { root, profile, browserProfile };
}

async function executable(candidate) {
  try {
    await access(candidate, constants.X_OK);
    return candidate;
  } catch {
    return null;
  }
}

export async function diagnose({
  home = os.homedir(),
  searchPath = process.env.PATH ?? '',
} = {}) {
  const chromeCandidates = [
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    path.join(
      home,
      'Applications',
      'Google Chrome.app',
      'Contents',
      'MacOS',
      'Google Chrome',
    ),
  ];
  const tools = [
    path.join(ROOT, 'src-tauri', 'resources', 'audio-tools', 'bin', 'yt-dlp'),
  ];
  for (const directory of searchPath.split(path.delimiter)) {
    // Ignore empty/relative PATH components; never search the current directory.
    if (path.isAbsolute(directory)) tools.push(path.join(directory, 'yt-dlp'));
  }
  const [chrome, ytDlp] = await Promise.all([
    Promise.all(chromeCandidates.map(executable)).then(
      (values) => values.find(Boolean) ?? null,
    ),
    Promise.all(tools.map(executable)).then(
      (values) => values.find(Boolean) ?? null,
    ),
  ]);
  const profile = await validateProfileLocation(home);
  return {
    platform: process.platform,
    chrome,
    ytDlp,
    profile,
    chromeArguments: launchArguments(home),
    futureYtDlpProfileArgument: exactProfileArgument(home),
    status:
      'Diagnostic only: no browser launch, cookie access, tool execution, or network requests.',
    pending: [
      'Human-operated sign-in',
      'Authenticated exact-profile extraction',
      'Keychain storage and cleanup',
    ],
  };
}

export async function main(args = process.argv.slice(2)) {
  if (args.length !== 1 || !['diagnose', 'open'].includes(args[0])) {
    throw new Error(
      'Usage: node scripts/auth-prototype.mjs diagnose | open\nOnly open launches an isolated browser. No sign-in or cookie export is automated.',
    );
  }
  const info = await diagnose();
  if (args[0] === 'diagnose') {
    console.log(JSON.stringify(info, null, 2));
    return;
  }
  if (process.platform !== 'darwin')
    throw new Error('The open probe currently supports macOS only.');
  if (!info.chrome)
    throw new Error('Install Google Chrome before using this probe.');
  await validateProfileLocation(os.homedir(), true);
  // Spawn the real browser binary directly with fixed arguments; no shell/CDP.
  const child = spawn(info.chrome, info.chromeArguments, {
    detached: true,
    stdio: 'ignore',
  });
  await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('spawn', resolve);
  });
  child.unref();
  console.log(
    `Opened Chrome with the isolated prototype profile:\n${info.profile.profile}\nSign-in is manual and optional. Close this separate browser when done.\nThis probe does not extract cookies, store a Keychain session, or revoke a Google session.`,
  );
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
