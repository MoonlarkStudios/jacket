#!/usr/bin/env python3
"""Offline regression coverage for the lightweight installer artifact."""
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest

from release_support import package_names

spec = importlib.util.spec_from_file_location('installers', Path(__file__).with_name('prepare-linux-installers.py'))
installers = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installers)
VERSION = '0.1.0-beta.3'


class InstallerArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.source = Path(self.temporary.name) / 'linux-preview'
        self.output = self.source.with_name('linux-installers')
        self.source.mkdir()
        self.names = set(package_names(VERSION).values())
        payloads = {name: f'fixture {name}'.encode() for name in self.names}
        payloads['jacket-source.tar.gz'] = b'large source companion excluded from installer tests'
        for name, data in payloads.items():
            (self.source / name).write_bytes(data)
        self.manifest = ''.join(f'{hashlib.sha256(data).hexdigest()}  {name}\n' for name, data in sorted(payloads.items()))
        (self.source / 'SHA256SUMS').write_text(self.manifest)

    def prepare(self):
        installers.prepare_installers(self.source, self.output, VERSION)

    def test_only_current_installers_and_original_checksums_are_copied(self):
        self.output.mkdir()
        (self.output / 'stale-package.deb').write_bytes(b'old')
        self.prepare()
        self.assertEqual({path.name for path in self.output.iterdir()}, self.names | {'SHA256SUMS'})
        expected = ''.join(line + '\n' for line in self.manifest.splitlines() if line.split('  ')[1] in self.names)
        self.assertEqual((self.output / 'SHA256SUMS').read_text(), expected)
        for name in self.names:
            self.assertEqual((self.output / name).read_bytes(), (self.source / name).read_bytes())
        self.assertEqual((self.source / 'SHA256SUMS').read_text(), self.manifest)
        self.assertTrue((self.source / 'jacket-source.tar.gz').is_file())

    def test_tampered_package_fails_before_existing_output_is_removed(self):
        self.output.mkdir()
        marker = self.output / 'existing'
        marker.write_text('keep on failure')
        (self.source / next(iter(self.names))).write_bytes(b'tampered')
        with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
            self.prepare()
        self.assertEqual(marker.read_text(), 'keep on failure')

    def test_missing_or_extra_installers_fail(self):
        package = self.source / next(iter(self.names))
        original = package.read_bytes()
        package.unlink()
        with self.assertRaisesRegex(ValueError, 'exactly the current'):
            self.prepare()
        package.write_bytes(original)
        (self.source / 'stale.deb').write_bytes(b'old')
        with self.assertRaisesRegex(ValueError, 'exactly the current'):
            self.prepare()

    def test_missing_duplicate_and_unexpected_installer_checksums_fail(self):
        name = next(iter(self.names))
        entry = next(line for line in self.manifest.splitlines() if line.endswith(f'  {name}'))
        for manifest in [self.manifest.replace(entry + '\n', ''), self.manifest + entry + '\n', self.manifest + 'a' * 64 + '  stale.rpm\n']:
            with self.subTest(manifest=manifest):
                (self.source / 'SHA256SUMS').write_text(manifest)
                with self.assertRaises(ValueError):
                    self.prepare()

    def test_output_cannot_replace_preview_or_follow_directory_symlink(self):
        with self.assertRaisesRegex(ValueError, 'generated linux-installers sibling'):
            installers.prepare_installers(self.source, self.source, VERSION)
        self.output.symlink_to(self.source, target_is_directory=True)
        with self.assertRaises(ValueError):
            self.prepare()
        self.assertTrue((self.source / 'SHA256SUMS').is_file())


if __name__ == '__main__':
    unittest.main()
