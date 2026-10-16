#!/usr/bin/env python3
"""Offline integrity and extraction regressions for the Linux runtime builder."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest import mock
import zipfile

spec = importlib.util.spec_from_file_location('linux_staging', Path(__file__).with_name('stage-linux-tools.py'))
staging = importlib.util.module_from_spec(spec)
spec.loader.exec_module(staging)


class LinuxRuntimeIntegrity(unittest.TestCase):
    def test_download_checksum_failure_never_creates_a_usable_input(self):
        with tempfile.TemporaryDirectory() as temporary:
            cache = Path(temporary)
            item = {'filename': 'source.tar.xz', 'url': 'https://example.test/source', 'sha256': 'a' * 64}
            with mock.patch.object(staging.urllib.request, 'urlopen', return_value=io.BytesIO(b'tampered')):
                with self.assertRaisesRegex(RuntimeError, 'Checksum mismatch'):
                    staging.fetch(item, cache)
            self.assertEqual(list(cache.iterdir()), [])

    def test_cached_download_is_reused_only_when_its_hash_matches(self):
        with tempfile.TemporaryDirectory() as temporary:
            cache = Path(temporary)
            item = {'filename': 'input', 'url': 'https://example.test/source', 'sha256': hashlib.sha256(b'original').hexdigest()}
            (cache / 'input').write_bytes(b'original')
            with mock.patch.object(staging.urllib.request, 'urlopen', side_effect=AssertionError('Unexpected network')):
                self.assertEqual(staging.fetch(item, cache), cache / 'input')
            (cache / 'input').write_bytes(b'corrupt cache')
            with mock.patch.object(staging.urllib.request, 'urlopen', return_value=io.BytesIO(b'original')):
                self.assertEqual(staging.fetch(item, cache).read_bytes(), b'original')

    def test_archive_traversal_cannot_write_outside_the_staging_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / 'bad.tar'
            with tarfile.open(archive, 'w') as package:
                entry = tarfile.TarInfo('../escaped')
                entry.size = 1
                package.addfile(entry, io.BytesIO(b'x'))
            with self.assertRaises(tarfile.FilterError):
                staging.unpack(archive, root / 'unpacked')
            self.assertFalse((root / 'escaped').exists())
            wheel = root / 'bad.whl'
            with zipfile.ZipFile(wheel, 'w') as package:
                package.writestr('../escaped', 'x')
            with self.assertRaisesRegex(RuntimeError, 'Unsafe wheel'):
                staging.unpack(wheel, root / 'unpacked')

    def test_changed_extra_and_symlinked_runtime_files_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            payload = root / 'tool'
            payload.write_bytes(b'original')
            (root / 'manifest.json').write_text(json.dumps({'files': {'tool': staging.digest(payload)}}))
            staging.verify_manifest(root)
            payload.write_bytes(b'tampered')
            with self.assertRaisesRegex(RuntimeError, 'changed'):
                staging.verify_manifest(root)
            payload.write_bytes(b'original')
            (root / 'extra').write_bytes(b'untracked')
            with self.assertRaisesRegex(RuntimeError, 'match the manifest'):
                staging.verify_manifest(root)
            (root / 'extra').unlink()
            payload.unlink()
            payload.symlink_to(root / 'manifest.json')
            with self.assertRaisesRegex(RuntimeError, 'changed'):
                staging.verify_manifest(root)

    def test_every_runtime_input_has_an_immutable_https_url_and_sha256(self):
        lock = json.loads(staging.LOCK.read_text())
        self.assertEqual(lock['target'], 'x86_64-unknown-linux-gnu')
        for item in lock['inputs'].values():
            self.assertTrue(item['url'].startswith('https://'))
            self.assertEqual(len(bytes.fromhex(item['sha256'])), 32)
            self.assertEqual(Path(item['filename']).name, item['filename'])
            self.assertNotIn('/latest/', item['url'])
        self.assertEqual(lock['inputs']['yt-dlp-ejs']['filename'], 'yt_dlp_ejs-0.8.0-py3-none-any.whl')


if __name__ == '__main__':
    unittest.main()
