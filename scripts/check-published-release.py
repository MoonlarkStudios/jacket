#!/usr/bin/env python3
"""Read-only validation of anonymously downloadable, immutable release assets."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
import time
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, urlopen

from release_support import SOURCE_FILES, package_names, package_versions, verify_artifacts


def github_api(path):
    return json.loads(subprocess.check_output(['gh', 'api', path], text=True))


def validate_metadata(release, repo, tag, commit):
    if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repo) or any(part in {'.', '..'} for part in repo.split('/')):
        raise ValueError('Invalid repository name')
    if not tag.startswith('v'):
        raise ValueError('Expected a v-prefixed release tag')
    version = tag[1:]
    package_versions(version)
    if '-' in version and version.endswith('.0'):
        raise ValueError('A published prerelease counter must start at 1')
    if not re.fullmatch(r'[a-f0-9]{40}', commit):
        raise ValueError('Expected an immutable commit SHA')
    if release.get('draft') is not False or not release.get('published_at'):
        raise ValueError('Release must already be published; this checker never publishes')
    if release.get('immutable') is not True:
        raise ValueError('Published release must be immutable')
    if release.get('tag_name') != tag or release.get('target_commitish') != commit:
        raise ValueError('Published release tag/commit mismatch')
    if release.get('prerelease') != ('-' in version):
        raise ValueError('Release prerelease flag disagrees with its version')
    expected = set(package_names(version).values()) | SOURCE_FILES | {'SHA256SUMS'}
    assets = release.get('assets', [])
    if len(assets) != len(expected) or {asset['name'] for asset in assets} != expected:
        raise ValueError('Published asset inventory is incomplete, duplicated or unexpected')
    urls = set()
    for asset in assets:
        url = f"https://github.com/{repo}/releases/download/{quote(tag, safe='')}/{quote(asset['name'], safe='')}"
        if asset.get('browser_download_url') != url:
            raise ValueError(f"Unexpected public asset URL: {asset['name']}")
        if asset.get('state') != 'uploaded' or type(asset.get('size')) is not int or asset['size'] <= 0:
            raise ValueError(f"Incomplete asset: {asset['name']}")
        if not re.fullmatch(r'sha256:[a-f0-9]{64}', asset.get('digest') or ''):
            raise ValueError(f"Missing server SHA-256 digest: {asset['name']}")
        urls.add(url)
    # Catch manually added stale draft links or links to the wrong installer.
    for url in re.findall(r'https://github\.com/[^\s)<>"\x27]+/releases/download/[^\s)<>"\x27]+', release.get('body') or ''):
        if url.startswith(f'https://github.com/{repo}/releases/download/') and url not in urls:
            raise ValueError('Release notes contain a stale or unknown asset download link')
    return assets


def download_public_asset(asset, destination, opener=None, sleep=time.sleep):
    """Download without any GitHub token, retrying short CDN propagation delays."""
    opener = opener or urlopen
    delays = (0, 2, 5, 10, 20)
    for attempt, delay in enumerate(delays):
        if delay:
            sleep(delay)
        try:
            # Never pass GH_TOKEN or API authorization headers to the download.
            request = Request(asset['browser_download_url'], headers={'User-Agent': 'Jacket-release-verifier'})
            size = 0
            digest = hashlib.sha256()
            with opener(request, timeout=120) as response, destination.open('wb') as output:
                if response.status != 200:
                    raise ValueError(f"Unexpected public download status for {asset['name']}: {response.status}")
                while chunk := response.read(1024 * 1024):
                    size += len(chunk)
                    if size > asset['size']:
                        raise ValueError(f"Public download exceeded recorded size: {asset['name']}")
                    digest.update(chunk)
                    output.write(chunk)
            if size != asset['size'] or f'sha256:{digest.hexdigest()}' != asset['digest']:
                raise ValueError(f"Public download size/checksum mismatch: {asset['name']}")
            return
        except HTTPError as error:
            if error.code not in {404, 408, 429, 500, 502, 503, 504} or attempt == len(delays) - 1:
                raise ValueError(f"Public download failed for {asset['name']}: HTTP {error.code}") from error
        except (URLError, TimeoutError) as error:
            if attempt == len(delays) - 1:
                raise ValueError(f"Public download timed out or failed: {asset['name']}") from error


def verify_published_release(repo, tag):
    # Validate user-controlled path components before invoking the API.
    if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repo) or any(part in {'.', '..'} for part in repo.split('/')) or not tag.startswith('v'):
        raise ValueError('Invalid repository or release tag')
    package_versions(tag[1:])
    release = github_api(f'repos/{repo}/releases/tags/{tag}')
    commit = github_api(f'repos/{repo}/commits/{tag}')['sha']
    assets = validate_metadata(release, repo, tag, commit)
    with tempfile.TemporaryDirectory(prefix='jacket-release-verify-') as temporary:
        directory = Path(temporary)
        for asset in assets:
            download_public_asset(asset, directory / asset['name'])
            print(f"Verified public download: {asset['name']}")
        verify_artifacts(directory, commit, tag)
        with tarfile.open(directory / 'jacket-source.tar.gz', 'r:gz') as archive:
            if archive.pax_headers.get('comment') != commit:
                raise ValueError('Application source archive does not identify the release commit')
            config = json.load(archive.extractfile('src-tauri/tauri.conf.json'))
            if config['version'] != tag[1:]:
                raise ValueError('Archived application version disagrees with release tag')
    print(f'Published release verified: {repo} {tag}; {len(assets)} public assets, immutable tag {commit}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', required=True)
    parser.add_argument('--tag', required=True)
    args = parser.parse_args()
    try:
        verify_published_release(args.repo, args.tag)
    except (ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'Release verification failed: {error}\n')


if __name__ == '__main__':
    main()
