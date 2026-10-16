#!/usr/bin/env python3
"""Release integrity regressions: drift, tampering, wrong commits and source omissions."""
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from release_support import ROOT, SOURCE_FILES, check_versions, package_names, package_versions, verify_artifacts


class ReleaseIntegrity(unittest.TestCase):
    def test_versions_are_synchronized_and_unreleased_baseline_cannot_be_tagged(self):
        version = check_versions()
        with self.assertRaisesRegex(ValueError, 'Release tag'):
            check_versions(tag='v9.9.9')
        if version.endswith('-beta.0'):
            with self.assertRaisesRegex(ValueError, 'counter'):
                check_versions(tag=f'v{version}')

    def test_mismatched_lockfile_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in ['src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml', 'crates/audio-engine/Cargo.toml', 'package.json', 'package-lock.json', 'Cargo.toml', 'Cargo.lock', '.release-please-manifest.json']:
                destination = root / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(ROOT / name, destination)
            lock = json.loads((root / 'package-lock.json').read_text())
            lock['packages']['']['version'] = '9.9.9'
            (root / 'package-lock.json').write_text(json.dumps(lock))
            with self.assertRaisesRegex(ValueError, 'Version mismatch'):
                check_versions(root)

    def test_prereleases_have_native_ordering_and_stable_rpm_release(self):
        self.assertEqual(package_versions('0.1.0-beta.10'), {'app': '0.1.0-beta.10', 'deb': '0.1.0~beta.10', 'rpmVersion': '0.1.0', 'rpmRelease': '0.beta.10'})
        self.assertEqual(package_versions('0.1.0')['rpmRelease'], '1')
        for version in ['v0.1.0', '1.2', '01.2.3', '1.2.3-beta', '1.2.3-beta.01', '1.2.3+build', '1.2.3-dev.1', '../bad']:
            with self.subTest(version=version), self.assertRaises(ValueError):
                package_versions(version)

    def fixture(self, root):
        names = SOURCE_FILES | set(package_names('0.1.0-beta.1').values())
        for name in names:
            (root / name).write_text('fixture')
        (root / 'build.json').write_text(json.dumps({'version': '0.1.0-beta.1', 'commit': 'a' * 40, 'target': 'x86_64-unknown-linux-gnu'}))
        (root / 'package-versions.json').write_text(json.dumps(package_versions('0.1.0-beta.1')))
        (root / 'dependency-inventory.json').write_text(json.dumps([{'notices': ['LICENSE']}]))
        self.hashes(root)

    def hashes(self, root):
        (root / 'SHA256SUMS').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n' for path in sorted(root.iterdir()) if path.name != 'SHA256SUMS'))

    def test_complete_candidate_is_accepted_and_wrong_commit_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root)
            verify_artifacts(root, 'a' * 40, 'v0.1.0-beta.1')
            with self.assertRaisesRegex(ValueError, 'exact tag commit'):
                verify_artifacts(root, 'b' * 40, 'v0.1.0-beta.1')

    def test_missing_corresponding_sources_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root)
            (root / 'jacket-dependency-sources.tar.gz').unlink()
            with self.assertRaisesRegex(ValueError, 'missing'):
                verify_artifacts(root, 'a' * 40, 'v0.1.0-beta.1')

    def test_tampered_installer_and_duplicate_checksums_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root)
            (root / package_names('0.1.0-beta.1')['deb']).write_text('tampered')
            with self.assertRaisesRegex(ValueError, 'Checksum mismatch'):
                verify_artifacts(root, 'a' * 40, 'v0.1.0-beta.1')
            self.hashes(root)
            checksums = root / 'SHA256SUMS'
            checksums.write_text(checksums.read_text() + checksums.read_text().splitlines()[0] + '\n')
            with self.assertRaisesRegex(ValueError, 'duplicate'):
                verify_artifacts(root, 'a' * 40, 'v0.1.0-beta.1')


if __name__ == '__main__':
    unittest.main()
