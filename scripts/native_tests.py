#!/usr/bin/env python3
"""Offline checks for native dependency boundaries and safe packaging."""

import importlib.util
import json
import platform
from pathlib import Path
import plistlib
import shutil
import tarfile
import tempfile
import tomllib
import unittest
from unittest import mock

import native_installers as installers

ROOT = Path(__file__).resolve().parents[1]


def load_script(name, path=None):
    spec = importlib.util.spec_from_file_location(name, path or ROOT / 'scripts' / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


packaging = load_script('native_package')
doctor = load_script('native_doctor')
licenses = load_script('native_licenses')


class EditionMatching(unittest.TestCase):
    def setUp(self):
        self.bridge = load_script('youtube_music', packaging.CATALOG_HELPER)
        self.client = mock.Mock()

    def test_explicit_versions_get_metadata_and_keep_partial_rows(self):
        main = {'title': 'Album', 'other_versions': [
            {'browseId': 'MPREb_main', 'title': 'Album'},
            {'browseId': 'MPREb_deluxe', 'title': 'Album (Deluxe Edition)'},
            {'browseId': 'MPREb_deluxe', 'title': 'Duplicate'},
            {'browseId': 'MPREb_partial', 'title': 'Album (Remastered)', 'year': '2024'},
        ]}
        albums = {
            'MPREb_main': main,
            'MPREb_deluxe': {'title': 'Album (Deluxe Edition)', 'year': '2023', 'trackCount': 18},
        }
        self.client.get_album.side_effect = albums.__getitem__
        rows = self.bridge.album_editions(self.client, 'MPREb_main', (KeyError,))
        self.assertEqual([(r['title'], r['year'], r['track_count']) for r in rows], [
            ('Album (Deluxe Edition)', 2023, 18), ('Album (Remastered)', 2024, None),
        ])
        self.client.search.assert_not_called()

    def test_album_search_rejects_a_similarly_named_single_and_unrelated_tracks(self):
        credits = [{'name': 'Stevie Ray Vaughan', 'id': 'stevie'},
                   {'name': 'Double Trouble', 'id': 'band'}]
        base = {'title': 'Texas Flood (Legacy Edition)', 'artists': credits, 'type': 'Album',
                'tracks': [{'title': 'Love Struck Baby'}, {'title': 'Pride and Joy'}]}
        single = {'browseId': 'MPREb_single', 'title': 'Texas Flood (Remastered 2024)',
                  'artists': credits[:1] + [{'name': 'Albert King', 'id': 'albert'}],
                  'type': 'Single'}
        unrelated = {'browseId': 'MPREb_other', 'title': 'Texas Flood',
                     'artists': credits, 'type': 'Album', 'tracks': [{'title': 'Different song'}]}
        alternate = {**unrelated, 'browseId': 'MPREb_match', 'year': 1999, 'trackCount': 2,
                     'tracks': [{'title': 'Pride and Joy (Remastered)'}, {'title': 'Love Struck Baby'}]}
        self.client.get_album.side_effect = {
            'MPREb_main': base, 'MPREb_other': unrelated, 'MPREb_match': alternate,
        }.__getitem__
        self.client.search.return_value = [single, unrelated, alternate]

        rows = self.bridge.album_editions(self.client, 'MPREb_main', (KeyError,))
        self.assertEqual([(r['id'], r['year'], r['track_count']) for r in rows], [
            ('MPREb_match', 1999, 2),
        ])
        self.client.search.assert_called_once_with('Texas Flood Stevie Ray Vaughan', filter='albums', limit=12)
        self.assertNotIn(mock.call('MPREb_single'), self.client.get_album.call_args_list)


class ArtistReleases(unittest.TestCase):
    def test_artist_preview_leaves_full_album_and_single_lookups_to_expansion(self):
        bridge = load_script('youtube_music', packaging.CATALOG_HELPER)
        client = mock.Mock()
        albums = [{'browseId': f'MPREb_album{n}', 'title': f'Album {n}'} for n in range(125)]
        singles = [{'browseId': 'MPREb_single', 'title': 'A single'}]
        client.get_artist.return_value = {
            'name': 'Example Artist',
            'albums': {'browseId': 'album-shelf', 'params': 'all-albums', 'results': albums[:3]},
            'singles': {'browseId': 'single-shelf', 'params': 'all-singles', 'results': []},
            'songs': {'results': [{'videoId': 'abcdefghijk', 'title': 'A song'}]},
        }
        client.get_artist_albums.side_effect = [albums, singles]

        preview = bridge.artist_detail(client, 'UCexample')
        self.assertTrue(preview['has_more_releases'])
        self.assertEqual([row['id'] for row in preview['hits']],
                         [row['browseId'] for row in albums[:3]] + ['abcdefghijk'])
        client.get_artist_albums.assert_not_called()

        expanded = bridge.artist_detail(client, 'UCexample', full_releases=True)
        self.assertEqual(client.get_artist_albums.call_args_list, [
            mock.call('album-shelf', 'all-albums', limit=None),
            mock.call('single-shelf', 'all-singles', limit=None),
        ])
        self.assertEqual([row['id'] for row in expanded['hits']],
                         [row['browseId'] for row in albums + singles] + ['abcdefghijk'])
        self.assertEqual(expanded['hits'][-2]['kind'], 'Single')
        self.assertEqual(expanded['hits'][-2]['artist'], 'Example Artist')

    def test_artist_shelf_without_a_full_list_keeps_all_preview_releases(self):
        bridge = load_script('youtube_music', packaging.CATALOG_HELPER)
        client = mock.Mock()
        client.get_artist.return_value = {
            'name': 'Example Artist',
            'albums': {'results': [
                {'browseId': f'MPREb_album{n}', 'title': f'Album {n}'} for n in range(16)
            ]},
        }
        page = bridge.artist_detail(client, 'UCexample', full_releases=True)
        self.assertFalse(page['has_more_releases'])
        self.assertEqual(len(page['hits']), 16)
        client.get_artist_albums.assert_not_called()


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
    def stage_catalog_fixture(self, root, target):
        catalog = root / 'catalog-tools'
        (catalog / 'bin').mkdir(parents=True)
        (catalog / 'sources').mkdir()
        launcher = catalog / 'bin/youtube-music'
        launcher.write_text('#!/bin/sh\nprintf "1.12.4\\n"\n')
        launcher.chmod(0o755)
        shutil.copy2(packaging.CATALOG_HELPER, catalog / 'youtube_music.py')
        source = catalog / 'sources/ytmusicapi.tar.gz'
        source.write_bytes(b'synthetic Python source')
        lock = root / 'catalog.lock.json'
        lock.write_text(json.dumps({'packages': {'ytmusicapi': {
            'version': '1.12.4',
            'source': {'filename': source.name, 'sha256': packaging.digest(source)},
        }}}))
        files = {item.relative_to(catalog).as_posix(): packaging.digest(item)
                 for item in catalog.rglob('*') if item.is_file()}
        (catalog / 'manifest.json').write_text(json.dumps({
            'platform': target, 'lockSha256': packaging.digest(lock), 'files': files,
        }))
        return lock

    def test_catalog_wheels_match_the_development_search_dependencies(self):
        requirements = dict(line.split('==') for line in (
            ROOT / 'scripts/youtube_music_requirements.txt').read_text().splitlines()
            if line and not line.startswith('#'))
        bundled = json.loads(packaging.CATALOG_LOCK.read_text())['packages']
        self.assertEqual(requirements, {name: item['version'] for name, item in bundled.items()})
    def test_macos_candidate_sets_bundle_identifier_icon_and_deployment_target(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'Cargo.toml').write_text('[workspace.package]\nversion = "0.1.0-beta.3"\n')
            (root / 'mise.toml').write_text('[env]\nMACOSX_DEPLOYMENT_TARGET = "26.0"\n')
            icon = root / packaging.MACOS_ICON
            icon.parent.mkdir(parents=True)
            icon.write_bytes((ROOT / packaging.MACOS_ICON).read_bytes())
            runtime = root / 'audio-tools'
            (runtime / 'bin').mkdir(parents=True)
            for name in packaging.REQUIRED_TOOLS:
                tool = runtime / 'bin' / name
                tool.write_text('#!/bin/sh\nexit 0\n')
                tool.chmod(0o755)
            manifest_path = runtime / 'manifest.json'
            manifest = {'platform': 'Darwin', 'architecture': 'arm64', 'minimumMacOS': '26.0.0',
                        'files': {f'bin/{path.name}': packaging.digest(path)
                                  for path in (runtime / 'bin').iterdir()}}
            manifest_path.write_text(json.dumps(manifest))
            dependencies = root / 'dependencies'
            dependencies.mkdir()
            (dependencies / 'jacket-native-dependency-sources.tar.gz').write_bytes(b'synthetic native sources')
            catalog_lock = self.stage_catalog_fixture(root, 'macos')
            options = mock.Mock(platform='macos', runtime=runtime, dependencies=dependencies,
                                executable=runtime / 'bin/node', output=root / 'output')
            with (mock.patch.object(packaging, 'ROOT', root),
                  mock.patch.object(packaging, 'CATALOG_LOCK', catalog_lock),
                  mock.patch.object(packaging, 'check_catalog_launcher') as catalog_smoke,
                  mock.patch.object(packaging.platform, 'system', return_value='Darwin'),
                  mock.patch.object(packaging.platform, 'machine', return_value='arm64'),
                  mock.patch.object(packaging, 'verify_dependencies', return_value=dependencies),
                  mock.patch.object(packaging, 'copy_notices'),
                  mock.patch.object(packaging.subprocess, 'run') as codesign):
                candidate = packaging.build_candidate(options)
                self.assertEqual(codesign.call_count, 2)
                catalog_smoke.assert_called_once()
                self.assertTrue((candidate / "Contents/Resources/catalog-tools/bin/youtube-music").is_file())
                manifest['minimumMacOS'] = '26.0.1'
                manifest_path.write_text(json.dumps(manifest))
                with self.assertRaisesRegex(ValueError, 'deployment target is macOS 26.0'):
                    packaging.build_candidate(options)
            contents = candidate / 'Contents'
            with (contents / 'Info.plist').open('rb') as stream:
                info = plistlib.load(stream)
            self.assertEqual(info['CFBundleIdentifier'], 'com.moonlarkstudios.jacket')
            self.assertEqual(info['LSMinimumSystemVersion'], '26.0')
            self.assertEqual((contents / 'Resources' / info['CFBundleIconFile']).read_bytes(), icon.read_bytes())
            with tarfile.open(options.output / 'jacket-0.1.0-beta.3-native-source.tar.gz') as source:
                self.assertIn(f'jacket-source/{packaging.MACOS_ICON}', source.getnames())

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
            desktop_file = root / 'apps/jacket/packaging/linux' / installers.DESKTOP_FILE
            desktop_file.parent.mkdir(parents=True)
            shutil.copy2(installers.PACKAGING / installers.DESKTOP_FILE, desktop_file)
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
            companion = root / 'target/native-runtime/linux/jacket-linux-runtime-sources.tar.gz'
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
            catalog_lock = self.stage_catalog_fixture(root, 'linux')
            options = mock.Mock(platform='linux', runtime=runtime, dependencies=dependencies,
                                executable=executable, output=root / 'output')
            with (mock.patch.object(packaging, 'ROOT', root),
                  mock.patch.object(packaging, 'CATALOG_LOCK', catalog_lock)):
                candidate = packaging.build_candidate(options)
                self.assertTrue((candidate / 'bin/jacket').stat().st_mode & 0o111)
                desktop_entry = (candidate / 'com.moonlark.audio-dl.desktop').read_text()
                self.assertIn('Icon=com.moonlark.audio-dl\n', desktop_entry)
                self.assertTrue((candidate / 'resources/com.moonlark.audio-dl.png').is_file())
                packaging.verify_runtime(candidate / 'resources/audio-tools')
                catalog = candidate / 'resources/catalog-tools'
                packaging.verify_catalog_runtime(catalog, 'linux')
                packaging.check_catalog_launcher(catalog)
                with self.assertRaisesRegex(ValueError, 'already exists'):
                    packaging.build_candidate(options)
            checksums = (root / 'output/SHA256SUMS').read_text().splitlines()
            self.assertEqual(len(checksums), 4)
            for line in checksums:
                checksum, filename = line.split('  ', 1)
                self.assertEqual(packaging.digest(root / 'output' / filename), checksum)
            with tarfile.open(root / 'output/jacket-0.1.0-beta.3-native-linux.tar.gz') as package:
                self.assertTrue(package.getmember('jacket-linux/bin/jacket').mode & 0o111)
                self.assertTrue(package.getmember(
                    'jacket-linux/resources/catalog-tools/bin/youtube-music').mode & 0o111)
            # A valid sidecar hash is insufficient if the sources belong to an
            # older runtime. Mismatched provenance must stop candidate creation.
            (runtime / 'build-info.json').write_text('{"inputs": {}, "changed": true}')
            with self.assertRaisesRegex(ValueError, 'different runtime'):
                packaging.verify_linux_sources(companion, runtime)


class LinuxReleaseAssets(unittest.TestCase):
    def test_package_versions_preserve_prerelease_ordering_conventions(self):
        self.assertEqual(installers.package_versions('0.2.0-rc.1'), ('0.2.0~rc.1-1', '0.2.0', '0.rc.1'))
        self.assertEqual(installers.package_names('0.2.0-rc.1'),
                         ('jacket_0.2.0-rc.1_amd64.deb', 'jacket-0.2.0-0.rc.1.x86_64.rpm'))
        self.assertEqual(installers.package_versions('0.2.0'), ('0.2.0-1', '0.2.0', '1'))
        for invalid in ('0.2.0-rc.0', 'v0.2.0', '0.02.0', '0.2.0+build'):
            with self.assertRaises(ValueError):
                installers.package_versions(invalid)

    def test_exact_asset_inventory_rejects_a_modified_installer(self):
        version = '0.2.0-rc.1'
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            names = (*installers.package_names(version), installers.RUNTIME_SOURCES,
                     installers.CRATE_SOURCES, 'runtime-build-info.json')
            for name in names:
                (output / name).write_text('fixture')
            lock = output / 'runtime-build-info.json'
            with tarfile.open(output / f'jacket-{version}-source.tar.gz', 'w:gz',
                              pax_headers={'comment': 'candidate'}) as source:
                source.add(lock, 'jacket-source/Cargo.lock')
            (output / 'build.json').write_text(json.dumps({
                'version': version, 'commit': 'candidate', 'target': 'x86_64-unknown-linux-gnu',
                'cargoLockSha256': packaging.digest(lock),
            }))
            installers.verify_assets(output, version, 'candidate', seal=True)
            installers.verify_assets(output, version, 'candidate')
            with self.assertRaisesRegex(ValueError, 'different build'):
                installers.verify_assets(output, version, 'another commit')
            with self.assertRaisesRegex(ValueError, 'Asset set'):
                installers.verify_assets(output, version, 'candidate', signed=True)
            (output / names[0]).write_text('changed fixture')
            with self.assertRaisesRegex(ValueError, 'checksums'):
                installers.verify_assets(output, version, 'candidate')


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
