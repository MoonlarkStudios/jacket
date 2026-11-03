#!/usr/bin/env python3
"""Offline checks for native dependency boundaries and safe packaging."""

import importlib.util
import json
import platform
from pathlib import Path
import tarfile
import tempfile
import tomllib
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]


def load_script(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


packaging = load_script('native_package')
doctor = load_script('native_doctor')
licenses = load_script('native_licenses')


class DependencyBoundaries(unittest.TestCase):
    def test_native_workspace_has_no_webview_or_legacy_build_members(self):
        workspace = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']
        expected = {'apps/jacket', 'crates/audio-engine', 'crates/jacket-core', 'crates/jacket-desktop'}
        self.assertEqual(set(workspace['members']), expected)
        for member in expected:
            manifest = tomllib.loads((ROOT / member / 'Cargo.toml').read_text())
            declarations = [manifest, *manifest.get('target', {}).values()]
            for declaration in declarations:
                for section in ('dependencies', 'build-dependencies'):
                    dependencies = declaration.get(section, {})
                    for name in dependencies:
                        self.assertFalse(name.startswith(('tauri', 'webkit', 'gtk')), f'{member}: {name}')
            if member.startswith('crates/'):
                self.assertNotIn('iced', manifest.get('dependencies', {}), member)

    def test_default_tasks_do_not_invoke_node_package_tools(self):
        tasks = tomllib.loads((ROOT / 'mise.toml').read_text())['tasks']
        for name in ('dev', 'desktop', 'build', 'check', 'test', 'lint'):
            command = str(tasks[name].get('run', ''))
            self.assertNotIn('npm ', command)
            self.assertNotIn('tauri ', command)


class RuntimeIntegrity(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.runtime = Path(self.temporary.name) / 'audio-tools'
        (self.runtime / 'bin').mkdir(parents=True)
        for name in packaging.REQUIRED_TOOLS:
            executable = self.runtime / 'bin' / name
            executable.write_text('#!/bin/sh\nexit 0\n')
            executable.chmod(0o755)
        self.write_manifest()

    def write_manifest(self):
        files = {path.relative_to(self.runtime).as_posix(): packaging.digest(path)
                 for path in self.runtime.rglob('*') if path.is_file() and path.name != 'manifest.json'}
        (self.runtime / 'manifest.json').write_text(json.dumps({'files': files}))

    def test_staged_runtime_must_be_complete_unchanged_and_executable(self):
        self.assertEqual(packaging.verify_runtime(self.runtime), self.runtime.resolve())
        executable = self.runtime / 'bin/ffmpeg'
        executable.chmod(0o644)
        with self.assertRaisesRegex(ValueError, 'not executable'):
            packaging.verify_runtime(self.runtime)
        executable.chmod(0o755)
        executable.write_text('changed bytes')
        with self.assertRaisesRegex(ValueError, 'checksum changed'):
            packaging.verify_runtime(self.runtime)

    def test_unmanifested_credentials_cannot_be_packed_with_runtime(self):
        (self.runtime / 'cookies.txt').write_text('synthetic fixture')
        with self.assertRaisesRegex(ValueError, 'do not match'):
            packaging.verify_runtime(self.runtime)

    def test_manifest_traversal_and_external_links_are_rejected(self):
        manifest = self.runtime / 'manifest.json'
        original = manifest.read_text()
        manifest.write_text(json.dumps({'files': {'../outside': '0' * 64}}))
        with self.assertRaisesRegex(ValueError, 'Unsafe'):
            packaging.verify_runtime(self.runtime)
        manifest.write_text(original)
        outside = Path(self.temporary.name) / 'outside'
        outside.write_text('outside the runtime')
        executable = self.runtime / 'bin/ffmpeg'
        executable.unlink()
        executable.symlink_to(outside)
        self.write_manifest()
        with self.assertRaisesRegex(ValueError, 'external symlink'):
            packaging.verify_runtime(self.runtime)

    @unittest.skipUnless(platform.system() == 'Linux', 'Linux runtime platform validation')
    def test_foreign_runtime_cannot_be_packaged_for_linux(self):
        manifest = self.runtime / 'manifest.json'
        value = json.loads(manifest.read_text())
        value['target'] = 'aarch64-apple-darwin'
        manifest.write_text(json.dumps(value))
        with self.assertRaisesRegex(ValueError, 'matching x86_64 Linux'):
            packaging.verify_runtime_platform(self.runtime, 'linux')

    def test_source_snapshot_contains_uncommitted_native_files_and_no_build_cache(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / 'apps/jacket/src'
            source.mkdir(parents=True)
            (source / 'main.rs').write_text('fn main() {}')
            (root / 'target').mkdir()
            (root / 'target/private.log').write_text('excluded')
            (root / '.env').write_text('excluded')
            archive = root / 'source.tar.gz'
            with mock.patch.object(packaging, 'ROOT', root):
                packaging.write_sources(archive)
            with tarfile.open(archive) as package:
                self.assertEqual(package.getnames(), ['jacket-source/apps/jacket/src/main.rs'])


class CandidatePackaging(unittest.TestCase):
    @unittest.skipUnless(platform.system() == 'Linux', 'Linux portable candidate smoke')
    def test_complete_candidate_retains_tools_and_publishes_matching_source_checksums(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'Cargo.toml').write_text('[workspace.package]\nversion = "0.1.0-beta.3"\n')
            (root / 'Cargo.lock').write_text('synthetic locked native graph')
            (root / 'LICENSE').write_text('synthetic license fixture')
            (root / 'THIRD_PARTY_NOTICES.md').write_text('synthetic notices fixture')
            (root / 'third-party/licenses').mkdir(parents=True)
            icon = root / 'branding/jacket/png/jacket-icon-256.png'
            icon.parent.mkdir(parents=True)
            icon.write_bytes(b'synthetic icon')
            runtime = root / 'runtime'
            (runtime / 'bin').mkdir(parents=True)
            for name in packaging.REQUIRED_TOOLS:
                tool = runtime / 'bin' / name
                tool.write_text('#!/bin/sh\nexit 0\n')
                tool.chmod(0o755)
            recipe = root / 'scripts/stage-linux-tools.py'
            recipe.parent.mkdir()
            recipe.write_text('synthetic build recipe')
            lock = root / 'scripts/linux-runtime.lock.json'
            lock.write_text('synthetic hash-pinned lock')
            source = root / 'fixture.tar.gz'
            source.write_bytes(b'synthetic matching source archive')
            (runtime / 'build-info.json').write_text(json.dumps({'inputs': {
                'fixture': {'kind': 'source', 'filename': source.name, 'sha256': packaging.digest(source)}}}))
            files = {item.relative_to(runtime).as_posix(): packaging.digest(item)
                     for item in runtime.rglob('*') if item.is_file()}
            (runtime / 'manifest.json').write_text(json.dumps({
                'target': 'x86_64-unknown-linux-gnu', 'builderSha256': packaging.digest(recipe),
                'lockSha256': packaging.digest(lock), 'files': files}))
            executable = root / 'jacket'
            executable.write_text('#!/bin/sh\nexit 0\n')
            executable.chmod(0o755)
            companion = root / 'target/linux-preview/jacket-linux-runtime-sources.tar.gz'
            companion.parent.mkdir(parents=True)
            with tarfile.open(companion, 'w:gz') as package:
                package.add(runtime / 'build-info.json', arcname='build-info.json')
                package.add(recipe, arcname='scripts/stage-linux-tools.py')
                package.add(lock, arcname='scripts/linux-runtime.lock.json')
                package.add(source, arcname=f'sources/{source.name}')
            companion.with_suffix(companion.suffix + '.sha256').write_text(
                f'{packaging.digest(companion)}  {companion.name}\n')
            dependencies = root / 'dependencies'
            (dependencies / 'application-licenses').mkdir(parents=True)
            (dependencies / 'application-licenses/inventory.json').write_text('{"packages": []}')
            (dependencies / 'jacket-native-dependency-sources.tar.gz').write_bytes(b'synthetic native sources')
            files = {item.relative_to(dependencies).as_posix(): packaging.digest(item)
                     for item in dependencies.rglob('*') if item.is_file()}
            (dependencies / 'manifest.json').write_text(json.dumps({
                'target': 'x86_64-unknown-linux-gnu', 'cargoLockSha256': packaging.digest(root / 'Cargo.lock'),
                'files': files}))
            (dependencies / 'SHA256SUMS').write_text('synthetic source hash fixture')
            options = mock.Mock(platform='linux', runtime=runtime, dependencies=dependencies,
                                executable=executable, output=root / 'output')
            with mock.patch.object(packaging, 'ROOT', root):
                candidate = packaging.build_candidate(options)
                self.assertTrue((candidate / 'bin/jacket').stat().st_mode & 0o111)
                packaging.verify_runtime(candidate / 'resources/audio-tools')
                with self.assertRaisesRegex(ValueError, 'already exists'):
                    packaging.build_candidate(options)
            checksums = (root / 'output/SHA256SUMS').read_text().splitlines()
            self.assertEqual(len(checksums), 4)
            for line in checksums:
                checksum, filename = line.split('  ', 1)
                self.assertEqual(packaging.digest(root / 'output' / filename), checksum)
            with tarfile.open(root / 'output/jacket-0.1.0-beta.3-native-linux.tar.gz') as package:
                self.assertTrue(package.getmember('jacket-linux/bin/jacket').mode & 0o111)
            # A valid sidecar hash is insufficient if the sources belong to an
            # older runtime. Mismatched provenance must stop candidate creation.
            (runtime / 'build-info.json').write_text('{"inputs": {}, "changed": true}')
            with self.assertRaisesRegex(ValueError, 'different runtime'):
                packaging.verify_linux_sources(companion, runtime)


class NativeLicenseCollection(unittest.TestCase):
    def test_collected_sources_notices_and_lock_identity_match(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'Cargo.lock').write_text('synthetic resolved fixture')
            source = root / 'registry/fixture'
            source.mkdir(parents=True)
            (source / 'Cargo.toml').write_text('[package]\nname = "fixture"\nversion = "1.0.0"\n')
            (source / 'LICENSE').write_text('synthetic upstream notice')
            package = {'id': 'fixture', 'name': 'fixture', 'version': '1.0.0',
                       'manifest_path': str(source / 'Cargo.toml'), 'license': 'MIT', 'license_file': None,
                       'repository': 'https://example.invalid/fixture', 'source': 'registry+fixture'}
            metadata = {'workspace_members': [], 'resolve': {'nodes': [{'id': 'fixture'}]}, 'packages': [package]}
            with mock.patch.object(licenses, 'ROOT', root):
                result = licenses.collect(root / 'output', 'x86_64-unknown-linux-gnu', metadata)
                manifest = json.loads((result / 'manifest.json').read_text())
                self.assertEqual(manifest['cargoLockSha256'], packaging.digest(root / 'Cargo.lock'))
                self.assertEqual((result / 'application-licenses/rust/fixture-1.0.0/LICENSE').read_text(),
                                 'synthetic upstream notice')
                with tarfile.open(result / 'jacket-native-dependency-sources.tar.gz') as archive:
                    self.assertIn('rust/fixture-1.0.0/LICENSE', archive.getnames())
                source_archive = result / 'jacket-native-dependency-sources.tar.gz'
                timestamp = source_archive.stat().st_mtime_ns
                self.assertEqual(licenses.collect(result, 'x86_64-unknown-linux-gnu', metadata), result)
                self.assertEqual(source_archive.stat().st_mtime_ns, timestamp)
                with self.assertRaisesRegex(ValueError, 'different target'):
                    licenses.collect(result, 'aarch64-apple-darwin', metadata)
                original_lock = (root / 'Cargo.lock').read_text()
                (root / 'Cargo.lock').write_text('changed graph')
                with self.assertRaisesRegex(ValueError, 'different Cargo.lock'):
                    licenses.collect(result, 'x86_64-unknown-linux-gnu', metadata)
                (root / 'Cargo.lock').write_text(original_lock)
                retained_notice = result / 'application-licenses/rust/fixture-1.0.0/LICENSE'
                retained_notice.write_text('modified retained notice')
                with self.assertRaisesRegex(ValueError, 'notices changed'):
                    licenses.collect(result, 'x86_64-unknown-linux-gnu', metadata)
                self.assertEqual(retained_notice.read_text(), 'modified retained notice')
                (source / 'LICENSE').unlink()
                with self.assertRaisesRegex(ValueError, 'missing native dependency notices'):
                    licenses.collect(root / 'missing-output', 'x86_64-unknown-linux-gnu', metadata)
                self.assertFalse((root / 'missing-output').exists())
                supplement = root / 'third-party/licenses/fixture-license'
                supplement.parent.mkdir(parents=True)
                supplement.write_text('unreviewed changed supplement')
                (root / 'scripts').mkdir()
                (root / 'scripts/native-license-overrides.json').write_text(json.dumps({
                    'Rust:fixture@1.0.0': {'file': supplement.name, 'sha256': '0' * 64}}))
                with self.assertRaisesRegex(ValueError, 'checksum changed'):
                    licenses.collect(root / 'changed-output', 'x86_64-unknown-linux-gnu', metadata)
                self.assertFalse((root / 'changed-output').exists())


class ToolDiagnostics(unittest.TestCase):
    def test_missing_tool_is_reported_without_running_commands(self):
        with mock.patch.object(doctor.shutil, 'which', return_value=None):
            self.assertEqual(doctor.inspect_tool('ffmpeg', ['-version']), 'ffmpeg: missing')

    def test_unsupported_javascript_runtime_is_reported(self):
        process = mock.Mock(stdout='v20.0.0\n')
        with mock.patch.object(doctor.shutil, 'which', return_value='/fixture/node'):
            with mock.patch.object(doctor.subprocess, 'run', return_value=process):
                self.assertIn('22 or later', doctor.inspect_tool('node', ['--version']))


if __name__ == '__main__':
    unittest.main(verbosity=2)
