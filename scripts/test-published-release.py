#!/usr/bin/env python3
"""Offline regression coverage for public release validation and download failures."""
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

from release_support import SOURCE_FILES, package_names, package_versions

spec = importlib.util.spec_from_file_location('published_release', Path(__file__).with_name('check-published-release.py'))
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)
REPO = 'MoonlarkStudios/jacket'
TAG = 'v0.1.0-beta.1'
COMMIT = 'a' * 40


def fixture(source_commit=COMMIT):
    names = set(package_names(TAG[1:]).values()) | SOURCE_FILES
    payloads = {name: f'fixture {name}'.encode() for name in names}
    payloads['build.json'] = json.dumps({'version': TAG[1:], 'commit': COMMIT, 'target': 'x86_64-unknown-linux-gnu'}).encode()
    payloads['package-versions.json'] = json.dumps(package_versions(TAG[1:])).encode()
    payloads['dependency-inventory.json'] = json.dumps([{'name': 'example', 'notices': ['LICENSE']}]).encode()
    source = io.BytesIO()
    with tarfile.open(fileobj=source, mode='w:gz', format=tarfile.PAX_FORMAT, pax_headers={'comment': source_commit}) as archive:
        data = json.dumps({'version': TAG[1:]}).encode()
        info = tarfile.TarInfo('src-tauri/tauri.conf.json')
        info.size = len(data)
        archive.addfile(info, io.BytesIO(data))
    payloads['jacket-source.tar.gz'] = source.getvalue()
    payloads['SHA256SUMS'] = ''.join(f'{hashlib.sha256(payloads[name]).hexdigest()}  {name}\n' for name in sorted(names)).encode()
    assets = [{'name': name, 'size': len(data), 'digest': 'sha256:' + hashlib.sha256(data).hexdigest(), 'state': 'uploaded', 'browser_download_url': f'https://github.com/{REPO}/releases/download/{TAG}/{name}'} for name, data in sorted(payloads.items())]
    release = {'draft': False, 'immutable': True, 'published_at': '2026-10-16T20:00:00Z', 'prerelease': True, 'tag_name': TAG, 'target_commitish': COMMIT, 'assets': assets, 'body': ''}
    return release, payloads


class Response(io.BytesIO):
    status = 200


class PublishedReleaseTests(unittest.TestCase):
    def test_complete_published_metadata(self):
        release, _ = fixture()
        self.assertEqual(len(checker.validate_metadata(release, REPO, TAG, COMMIT)), 14)

    def test_draft_mutable_or_mismatched_release_rejected(self):
        release, _ = fixture()
        for change in [{'draft': True}, {'published_at': None}, {'immutable': False}, {'target_commitish': 'main'}, {'tag_name': 'v0.1.0-beta.2'}, {'prerelease': False}]:
            with self.subTest(change=change):
                invalid = {**release, **change}
                with self.assertRaises(ValueError):
                    checker.validate_metadata(invalid, REPO, TAG, COMMIT)

    def test_missing_duplicate_or_untrusted_assets_rejected(self):
        release, _ = fixture()
        for edit in ['missing', 'duplicate', 'url', 'digest', 'size']:
            with self.subTest(edit=edit):
                invalid = copy.deepcopy(release)
                if edit == 'missing':
                    invalid['assets'].pop()
                elif edit == 'duplicate':
                    invalid['assets'][-1] = invalid['assets'][0]
                elif edit == 'url':
                    invalid['assets'][0]['browser_download_url'] = 'https://example.test/installer'
                elif edit == 'digest':
                    invalid['assets'][0]['digest'] = None
                else:
                    invalid['assets'][0]['size'] = 0
                with self.assertRaises(ValueError):
                    checker.validate_metadata(invalid, REPO, TAG, COMMIT)

    def test_stale_note_download_link_rejected(self):
        release, _ = fixture()
        release['body'] = f'[Download](https://github.com/{REPO}/releases/download/untagged-old/Jacket.deb)'
        with self.assertRaisesRegex(ValueError, 'stale or unknown'):
            checker.validate_metadata(release, REPO, TAG, COMMIT)

    def test_public_download_uses_no_authorization_and_checks_bytes(self):
        release, payloads = fixture()
        asset = release['assets'][0]
        def opener(request, **_):
            self.assertNotIn('authorization', {name.lower() for name, _ in request.header_items()})
            self.assertEqual(request.full_url, asset['browser_download_url'])
            return Response(payloads[asset['name']])
        with tempfile.TemporaryDirectory() as temporary, patch.dict('os.environ', {'GH_TOKEN': 'must-never-reach-download'}):
            destination = Path(temporary) / asset['name']
            checker.download_public_asset(asset, destination, opener=opener)
            self.assertEqual(destination.read_bytes(), payloads[asset['name']])

    def test_corrupt_and_truncated_downloads_fail(self):
        release, _ = fixture()
        asset = release['assets'][0]
        with tempfile.TemporaryDirectory() as temporary:
            for data in [b'bad', b'x' * (asset['size'] + 1)]:
                with self.subTest(data=data):
                    with self.assertRaisesRegex(ValueError, 'checksum mismatch|exceeded recorded size'):
                        checker.download_public_asset(asset, Path(temporary) / 'asset', opener=lambda *_, **__: Response(data))

    def test_cdn_propagation_retries_then_succeeds(self):
        release, payloads = fixture()
        asset = release['assets'][0]
        calls = []
        delays = []
        def opener(*_, **__):
            calls.append(True)
            if len(calls) == 1:
                raise HTTPError('https://github.com/example', 404, 'Not found', {}, None)
            return Response(payloads[asset['name']])
        with tempfile.TemporaryDirectory() as temporary:
            checker.download_public_asset(asset, Path(temporary) / 'asset', opener=opener, sleep=delays.append)
        self.assertEqual(len(calls), 2)
        self.assertEqual(delays, [2])

    def test_permanent_or_repeated_http_errors_fail(self):
        release, _ = fixture()
        asset = release['assets'][0]
        for status, count in [(403, 1), (404, 5)]:
            calls = []
            def opener(*_, **__):
                calls.append(True)
                raise HTTPError('https://github.com/example', status, 'Error', {}, None)
            with tempfile.TemporaryDirectory() as temporary, self.subTest(status=status):
                with self.assertRaisesRegex(ValueError, f'HTTP {status}'):
                    checker.download_public_asset(asset, Path(temporary) / 'asset', opener=opener, sleep=lambda _: None)
            self.assertEqual(len(calls), count)

    def test_full_validation_checks_all_files_and_source_identity(self):
        for source_commit, succeeds in [(COMMIT, True), ('b' * 40, False)]:
            release, payloads = fixture(source_commit)
            def api(path):
                if '/releases/tags/' in path:
                    return release
                return {'sha': COMMIT}
            def opener(request, **_):
                return Response(payloads[request.full_url.rsplit('/', 1)[1]])
            with patch.object(checker, 'github_api', side_effect=api), patch.object(checker, 'urlopen', side_effect=opener), contextlib.redirect_stdout(io.StringIO()), self.subTest(source_commit=source_commit):
                if succeeds:
                    checker.verify_published_release(REPO, TAG)
                else:
                    with self.assertRaisesRegex(ValueError, 'source archive'):
                        checker.verify_published_release(REPO, TAG)


if __name__ == '__main__':
    unittest.main()
