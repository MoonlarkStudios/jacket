#!/usr/bin/env python3
"""Build local native app candidates from a complete, verified audio runtime.

This script never publishes, notarizes, installs, or silently uses host audio
binaries. Stage the runtime with the matching mise task before packaging.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import plistlib
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
IDENTIFIER = 'com.moonlark.audio-dl'
REQUIRED_TOOLS = ('yt-dlp', 'ffmpeg', 'ffprobe', 'node')


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def verify_runtime(directory):
    directory = directory.resolve(strict=True)
    manifest = json.loads((directory / 'manifest.json').read_text())
    files = manifest['files']
    if not isinstance(files, dict) or not files:
        raise ValueError('Audio runtime manifest has no files')
    for name in files:
        relative = PurePosixPath(name)
        if relative.is_absolute() or '..' in relative.parts or '\\' in name:
            raise ValueError(f'Unsafe runtime manifest path: {name}')
    actual = {path.relative_to(directory).as_posix()
              for path in directory.rglob('*') if path.is_file()}
    if actual != set(files) | {'manifest.json'}:
        raise ValueError('Audio runtime files do not match its manifest; stage it again')
    for path in directory.rglob('*'):
        # macOS Python/framework runtime staging can preserve internal symlinks.
        # Resolve them here before copying, rejecting links outside the runtime.
        if path.is_symlink() and not path.resolve(strict=True).is_relative_to(directory):
            raise ValueError(f'Audio runtime contains an external symlink: {path.name}')
    for name, expected in files.items():
        if digest(directory / name) != expected:
            raise ValueError(f'Audio runtime checksum changed: {name}')
    for name in REQUIRED_TOOLS:
        executable = directory / 'bin' / name
        if not executable.is_file() or not os.access(executable, os.X_OK):
            raise ValueError(f'Bundled {name} is missing or not executable')
    return directory


def verify_runtime_platform(directory, target_platform):
    manifest = json.loads((directory / 'manifest.json').read_text())
    if target_platform == 'linux':
        if platform.machine() != 'x86_64' or manifest.get('target') != 'x86_64-unknown-linux-gnu':
            raise ValueError('The Linux candidate requires its matching x86_64 Linux audio runtime')
    elif manifest.get('platform') != 'Darwin' or manifest.get('architecture') != platform.machine():
        raise ValueError('The macOS candidate requires a matching host-architecture macOS audio runtime')


def verify_linux_sources(companion, runtime):
    checksum = companion.with_suffix(companion.suffix + '.sha256').read_text().split()
    if not checksum or digest(companion) != checksum[0]:
        raise ValueError('Linux runtime source companion checksum changed; stage it again')
    manifest = json.loads((runtime / 'manifest.json').read_text())
    with tarfile.open(companion, 'r:gz') as archive:
        member = archive.getmember('build-info.json')
        if member.size > 1024 * 1024 or not member.isfile():
            raise ValueError('Linux runtime source build information is invalid')
        stream = archive.extractfile(member)
        if stream is None:
            raise ValueError('Linux runtime source build information is missing')
        with stream:
            build_info = stream.read()
        if build_info != (runtime / 'build-info.json').read_bytes():
            raise ValueError('Linux runtime source companion belongs to a different runtime')
        # Check the source payloads and build recipe against the hash-verified
        # runtime metadata without extracting an untrusted archive to disk.
        expected = {'scripts/stage-linux-tools.py': manifest['builderSha256'],
                    'scripts/linux-runtime.lock.json': manifest['lockSha256']}
        for item in json.loads(build_info)['inputs'].values():
            if item['kind'] == 'source':
                expected[f'sources/{item["filename"]}'] = item['sha256']
        for name, checksum in expected.items():
            member = archive.getmember(name)
            if not member.isfile():
                raise ValueError(f'Linux runtime source is not a regular file: {name}')
            stream = archive.extractfile(member)
            if stream is None:
                raise ValueError(f'Linux runtime source is missing: {name}')
            with stream:
                actual = hashlib.file_digest(stream, 'sha256').hexdigest()
            if actual != checksum:
                raise ValueError(f'Linux runtime source changed: {name}')


def verify_dependencies(directory, target_platform):
    directory = directory.resolve(strict=True)
    manifest = json.loads((directory / 'manifest.json').read_text())
    if manifest['cargoLockSha256'] != digest(ROOT / 'Cargo.lock'):
        raise ValueError('Native dependency notices belong to a different Cargo.lock; recollect them')
    expected_target = 'x86_64-unknown-linux-gnu' if target_platform == 'linux' else f'{"aarch64" if platform.machine() == "arm64" else "x86_64"}-apple-darwin'
    if manifest['target'] != expected_target:
        raise ValueError('Native dependency notices belong to a different target')
    files = manifest['files']
    if any(path.is_symlink() for path in directory.rglob('*')):
        raise ValueError('Native dependency inventory contains a symlink')
    actual = {path.relative_to(directory).as_posix() for path in directory.rglob('*') if path.is_file()}
    if actual != set(files) | {'manifest.json', 'SHA256SUMS'}:
        raise ValueError('Native dependency inventory is incomplete or has extra files')
    for name, checksum in files.items():
        relative = PurePosixPath(name)
        if relative.is_absolute() or '..' in relative.parts or '\\' in name:
            raise ValueError('Native dependency inventory contains an unsafe path')
        path = directory / name
        if path.is_symlink() or digest(path) != checksum:
            raise ValueError(f'Native dependency notices changed: {name}')
    return directory


def copy_notices(resources, dependencies):
    notices = resources / 'notices'
    notices.mkdir()
    for name in ('LICENSE', 'THIRD_PARTY_NOTICES.md'):
        shutil.copy2(ROOT / name, notices / name)
    shutil.copytree(dependencies / 'application-licenses', notices / 'application-licenses')
    font_license = ROOT / 'assets/fonts/OFL.txt'
    if font_license.is_file():
        shutil.copy2(font_license, notices / 'Geist-OFL.txt')


def write_sources(destination):
    """Snapshot the native build inputs, including an uncommitted local port."""
    roots = ('.gitattributes', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'rustfmt.toml',
             'mise.toml', 'LICENSE', 'THIRD_PARTY_NOTICES.md', 'README.md',
             'CONTRIBUTING.md', 'AGENTS.md', 'apps/jacket', 'crates/audio-engine',
             'crates/jacket-core', 'crates/jacket-desktop', 'branding', 'scripts',
             'third-party', 'assets', 'docs', '.github', 'src-tauri/icons/icon.icns',
             'src-tauri/tauri.conf.json', 'src-tauri/tauri.macos.conf.json')
    with tarfile.open(destination, 'w:gz') as archive:
        for relative in roots:
            path = ROOT / relative
            if not path.exists():
                continue
            if path.is_file():
                archive.add(path, arcname=f'jacket-source/{relative}', recursive=False)
                continue
            for item in sorted(path.rglob('*')):
                if not item.is_file() or item.is_symlink():
                    continue
                if '__pycache__' in item.parts or item.suffix == '.pyc':
                    continue
                archive.add(item, arcname=f'jacket-source/{item.relative_to(ROOT)}', recursive=False)


def build_candidate(options):
    if options.platform == 'macos' and platform.system() != 'Darwin':
        raise ValueError('The macOS bundle must be built and signed on macOS')
    if options.platform == 'linux' and platform.system() != 'Linux':
        raise ValueError('The Linux bundle must be built on Linux')
    runtime = verify_runtime(options.runtime)
    verify_runtime_platform(runtime, options.platform)
    dependencies = verify_dependencies(options.dependencies, options.platform)
    companion = ROOT / 'target/linux-preview/jacket-linux-runtime-sources.tar.gz'
    if options.platform == 'linux':
        if not companion.is_file():
            raise ValueError('Linux runtime source companion is missing; rerun mise run tools:stage:linux')
        verify_linux_sources(companion, runtime)
    executable = options.executable.resolve(strict=True)
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise ValueError('The native executable is not executable; run mise run build')
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']
    bundle_version = version.split('-', 1)[0]
    output = options.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    destination = output / ('Jacket.app' if options.platform == 'macos' else 'jacket-linux')
    if destination.is_symlink():
        raise ValueError('Refusing to replace a bundle symlink')
    if destination.exists():
        raise ValueError(f'{destination} already exists; move or remove this generated candidate first')
    with tempfile.TemporaryDirectory(prefix='.native-package-', dir=output) as temporary:
        staged = Path(temporary) / destination.name
        if options.platform == 'macos':
            binaries = staged / 'Contents/MacOS'
            resources = staged / 'Contents/Resources'
            resources.mkdir(parents=True)
            binaries.mkdir(parents=True)
            info = {
                'CFBundleDevelopmentRegion': 'en',
                'CFBundleExecutable': 'jacket',
                'CFBundleIdentifier': IDENTIFIER,
                'CFBundleName': 'Jacket',
                'CFBundleDisplayName': 'Jacket',
                'CFBundlePackageType': 'APPL',
                'CFBundleShortVersionString': bundle_version,
                'CFBundleVersion': bundle_version,
                'CFBundleIconFile': 'Jacket.icns',
                'LSApplicationCategoryType': 'public.app-category.music',
                'LSMinimumSystemVersion': '26.0',
                'NSHighResolutionCapable': True,
            }
            with (staged / 'Contents/Info.plist').open('wb') as stream:
                plistlib.dump(info, stream)
            shutil.copy2(ROOT / 'src-tauri/icons/icon.icns', resources / 'Jacket.icns')
        else:
            binaries = staged / 'bin'
            resources = staged / 'resources'
            resources.mkdir(parents=True)
            binaries.mkdir(parents=True)
            shutil.copy2(ROOT / 'branding/jacket/png/jacket-icon-256.png', resources / f'{IDENTIFIER}.png')
            (staged / f'{IDENTIFIER}.desktop').write_text(
                '[Desktop Entry]\nType=Application\nName=Jacket\n'
                'Comment=Prepare tagged audio for your music library\n'
                f'Exec=jacket\nIcon={IDENTIFIER}\nCategories=AudioVideo;Audio;\nTerminal=false\n'
            )
            (staged / 'README.txt').write_text(
                'Run bin/jacket in a graphical Linux session.\n'
                'The audio runtime is bundled; system Python 3, CA certificates, '
                'X11/Wayland, xdg-utils and GPU/display drivers remain system dependencies.\n'
                'This is a local qualification candidate, not an installer.\n'
                'The .desktop file is a template: install bin/jacket and the icon '
                'into your PATH/icon theme or set absolute Exec/Icon paths.\n'
            )
        shutil.copy2(executable, binaries / 'jacket')
        shutil.copytree(runtime, resources / 'audio-tools', symlinks=True)
        verify_runtime(resources / 'audio-tools')
        copy_notices(resources, dependencies)
        if options.platform == 'macos':
            subprocess.run(['/usr/bin/codesign', '--force', '--sign', '-', str(staged)], check=True)
            subprocess.run(['/usr/bin/codesign', '--verify', '--deep', '--strict', str(staged)], check=True)
        staged.rename(destination)
    dependency_sources = output / 'jacket-native-dependency-sources.tar.gz'
    shutil.copy2(dependencies / dependency_sources.name, dependency_sources)
    sources = output / f'jacket-{version}-native-source.tar.gz'
    write_sources(sources)
    if options.platform == 'linux':
        shutil.copy2(companion, output / companion.name)
    archive = output / f'jacket-{version}-native-{options.platform}.tar.gz'
    with tarfile.open(archive, 'w:gz') as package:
        package.add(destination, arcname=destination.name)
    artifacts = [sources, archive, dependency_sources]
    if options.platform == 'linux':
        artifacts.append(output / 'jacket-linux-runtime-sources.tar.gz')
    (output / 'SHA256SUMS').write_text(''.join(f'{digest(path)}  {path.name}\n' for path in artifacts))
    print(destination)
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', choices=('macos', 'linux'), required=True)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--dependencies', type=Path, default=ROOT / 'target/native-dependencies')
    target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
    if not target.is_absolute():
        target = ROOT / target
    parser.add_argument('--executable', type=Path, default=target / 'release/jacket')
    parser.add_argument('--output', type=Path, default=ROOT / 'target/native-candidates')
    options = parser.parse_args()
    try:
        build_candidate(options)
    except (OSError, ValueError, KeyError, tarfile.TarError, subprocess.SubprocessError) as error:
        parser.exit(1, f'Native packaging failed: {error}\n')


if __name__ == '__main__':
    main()
