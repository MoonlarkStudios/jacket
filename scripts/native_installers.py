#!/usr/bin/env python3
"""Stage, build and verify Jacket's Linux installers and matching source assets."""
import argparse
from datetime import date
from email.parser import Parser
import gzip
from hashlib import file_digest
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
from xml.etree import ElementTree

from native_package import (LINUX_DESKTOP_ID, ROOT, copy_catalog_runtime, copy_notices, digest,
                            verify_catalog_runtime,
                            verify_dependencies, verify_linux_sources, verify_runtime,
                            verify_runtime_platform, write_sources)

PACKAGING = ROOT / 'apps/jacket/packaging/linux'
RUNTIME_DIR = ROOT / 'target/native-runtime/linux'
RUNTIME_SOURCES = 'jacket-linux-runtime-sources.tar.gz'
CRATE_SOURCES = 'jacket-native-dependency-sources.tar.gz'
DESKTOP_FILE = f'{LINUX_DESKTOP_ID}.desktop'
METAINFO_FILE = f'{LINUX_DESKTOP_ID}.metainfo.xml'
APP_HOME = Path('usr/libexec/jacket')
OUTPUT = ROOT / 'target/linux-installers'
VERSION = re.compile(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(alpha|beta|rc)\.([1-9]\d*))?')
FLATPAK_RUNTIME = '26.08'
FLATPAK_RUNTIME_REPO = 'https://dl.flathub.org/repo/flathub.flatpakrepo'


def package_versions(version):
    match = VERSION.fullmatch(version)
    if not match:
        raise ValueError(f'Unsupported Jacket release version: {version}')
    core = '.'.join(match.groups()[:3])
    channel, number = match.groups()[3:]
    if channel:
        return f'{core}~{channel}.{number}-1', core, f'0.{channel}.{number}'
    return f'{core}-1', core, '1'


def package_names(version):
    _, rpm, revision = package_versions(version)
    # GitHub can normalize special characters in uploaded names. The DEB
    # keeps its tilde version in control metadata for correct upgrade sorting.
    return (f'jacket_{version}_amd64.deb', f'jacket-{rpm}-{revision}.x86_64.rpm',
            f'jacket-{version}-x86_64.flatpak')


def check_version(tag=None):
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']
    package_versions(version)
    if tag is not None and tag != f'v{version}':
        raise ValueError(f'Tag {tag} must match the workspace version v{version}')
    lockfile = tomllib.loads((ROOT / 'Cargo.lock').read_text())
    lock = {package['name']: package for package in lockfile['package'] if 'source' not in package}
    for path in [ROOT / 'apps/jacket/Cargo.toml', *sorted(ROOT.glob('crates/*/Cargo.toml'))]:
        crate = tomllib.loads(path.read_text())['package']
        if crate['version'] != {'workspace': True} or lock[crate['name']]['version'] != version:
            raise ValueError(f'{crate["name"]} does not match the locked workspace version')
    release_notes(version)
    return version


def release_notes(version):
    changelog = (ROOT / 'CHANGELOG.md').read_text()
    for section in re.split(r'^## ', changelog, flags=re.MULTILINE)[1:]:
        heading, _, notes = section.partition('\n')
        if heading.startswith(f'[{version}]') and notes.strip():
            return notes.strip()
    raise ValueError(f'CHANGELOG.md needs release notes for {version}')


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, text=True).strip()


def verify_glibc_baseline(executables):
    for executable in executables:
        elf = subprocess.check_output(['readelf', '--version-info', executable], text=True)
        versions = {tuple(map(int, version.split('.')))
                    for version in re.findall(r'Name: GLIBC_([0-9.]+)', elf)}
        if not versions or max(versions) > (2, 39):
            raise ValueError(f'{executable.name} does not meet the glibc 2.39 baseline. Build on Ubuntu 24.04.')


def stage(options, version):
    output = options.output.resolve()
    if output.exists() or output.is_symlink():
        raise ValueError(f'{output} already exists; use a new output directory')
    target = Path(os.environ.get('CARGO_TARGET_DIR', 'target'))
    executable = ROOT / target / 'release/jacket'
    if subprocess.check_output([executable, '--version'], text=True).strip() != f'Jacket {version}':
        raise ValueError('The release executable version does not match Cargo.toml')
    runtime = verify_runtime(RUNTIME_DIR / 'audio-tools')
    verify_runtime_platform(runtime, 'linux')
    catalog = verify_catalog_runtime(RUNTIME_DIR / 'catalog-tools', 'linux')
    verify_linux_sources(RUNTIME_DIR / RUNTIME_SOURCES, runtime)
    verify_glibc_baseline([executable, *[runtime / 'bin' / name for name in ('ffmpeg', 'ffprobe', 'node')]])
    dependencies = verify_dependencies(ROOT / 'target/native-dependencies', 'linux')
    if options.commit and (git('rev-parse', 'HEAD') != options.commit or git('status', '--porcelain')):
        raise ValueError('A CI package must build from the exact clean commit')
    release_date = git('show', '-s', '--format=%cs', options.commit) if options.commit else date.today().isoformat()

    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.jacket-installers-', dir=output.parent) as directory:
        staged = Path(directory) / 'output'
        payload = staged / 'root'
        assets = staged / 'assets'
        assets.mkdir(parents=True)
        app = payload / APP_HOME
        (app / 'bin').mkdir(parents=True)
        shutil.copy2(executable, app / 'bin/jacket')
        (payload / 'usr/bin').mkdir(parents=True)
        (payload / 'usr/bin/jacket').symlink_to('../libexec/jacket/bin/jacket')
        shutil.copytree(runtime, app / 'resources/audio-tools', symlinks=True)
        copy_catalog_runtime(catalog, app / 'resources', 'linux')
        copy_notices(app / 'resources', dependencies)
        licenses = payload / 'usr/share/licenses/jacket'
        licenses.mkdir(parents=True)
        for filename in ('LICENSE', 'THIRD_PARTY_NOTICES.md'):
            shutil.copy2(ROOT / filename, licenses)
        docs = payload / 'usr/share/doc/jacket'
        docs.mkdir(parents=True)
        shutil.copy2(PACKAGING / 'copyright', docs)
        (docs / 'changelog.gz').write_bytes(gzip.compress((ROOT / 'CHANGELOG.md').read_bytes(), mtime=0))
        applications = payload / 'usr/share/applications'
        applications.mkdir(parents=True)
        shutil.copy2(PACKAGING / DESKTOP_FILE, applications)
        for size in (16, 24, 32, 64, 128, 256, 512):
            icons = payload / f'usr/share/icons/hicolor/{size}x{size}/apps'
            icons.mkdir(parents=True)
            shutil.copy2(ROOT / f'branding/jacket/png/jacket-icon-{size}.png', icons / f'{LINUX_DESKTOP_ID}.png')
        metainfo = payload / 'usr/share/metainfo'
        metainfo.mkdir(parents=True)
        metadata = ElementTree.parse(PACKAGING / METAINFO_FILE)
        releases = ElementTree.SubElement(metadata.getroot(), 'releases')
        ElementTree.SubElement(releases, 'release', version=version, date=release_date,
                               type='development' if '-' in version else 'stable')
        ElementTree.indent(metadata, space='  ')
        metadata.write(metainfo / METAINFO_FILE, encoding='UTF-8', xml_declaration=True)

        source = assets / f'jacket-{version}-source.tar.gz'
        if options.commit:
            subprocess.run(['git', 'archive', '--format=tar.gz', '--prefix=jacket-source/',
                            f'--output={source}', options.commit], cwd=ROOT, check=True)
        else:
            write_sources(source)
        shutil.copy2(RUNTIME_DIR / RUNTIME_SOURCES, assets)
        shutil.copy2(dependencies / CRATE_SOURCES, assets)
        shutil.copy2(runtime / 'build-info.json', assets / 'runtime-build-info.json')
        build = {
            'version': version, 'commit': options.commit, 'target': 'x86_64-unknown-linux-gnu',
            'executableSha256': digest(executable), 'cargoLockSha256': digest(ROOT / 'Cargo.lock'),
            'builder': Path('/etc/os-release').read_text(),
            'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
        }
        (assets / 'build.json').write_text(json.dumps(build, indent=2) + '\n')
        staged.rename(output)


def build_deb(output, version):
    root = output.resolve() / 'root'
    binary = root / APP_HOME / 'bin/jacket'
    control = (PACKAGING / 'control').read_text()
    debian = root / 'DEBIAN'
    with tempfile.TemporaryDirectory(dir=output) as directory:
        work = Path(directory)
        (work / 'debian').mkdir()
        (work / 'debian/control').write_text(control)
        shlibs = subprocess.check_output(
            ['dpkg-shlibdeps', '-O', binary, *[root / APP_HOME / 'resources/audio-tools/bin' / tool
                                              for tool in ('ffmpeg', 'ffprobe', 'node')]],
            cwd=work, text=True).strip().removeprefix('shlibs:Depends=')
    source, package = (Parser().parsestr(stanza) for stanza in control.strip().split('\n\n'))
    package['Version'] = package_versions(version)[0]
    for key in ('Section', 'Priority', 'Maintainer', 'Homepage'):
        package[key] = source[key]
    package.replace_header('Depends', package['Depends'].replace('${shlibs:Depends}', shlibs))
    package['Installed-Size'] = str((sum(path.stat().st_size for path in root.rglob('*')
                                         if path.is_file() and not path.is_symlink()) + 1023) // 1024)
    debian.mkdir()
    try:
        (debian / 'control').write_text(package.as_string().lstrip('\n'))
        with tempfile.TemporaryDirectory(dir=output) as directory:
            temporary = Path(directory) / 'installer.deb'
            subprocess.run(['dpkg-deb', '--root-owner-group', '--threads-max=4', '--build', root, temporary], check=True)
            temporary.replace(output / 'assets' / package_names(version)[0])
    finally:
        shutil.rmtree(debian)


def build_rpm(output, version):
    _, rpm, release = package_versions(version)
    with tempfile.TemporaryDirectory(dir=output) as directory:
        top = Path(directory).resolve()
        subprocess.run(['rpmbuild', '-bb', '--target', 'x86_64',
                        '--define', f'_topdir {top}',
                        '--define', f'jacket_payload {output.resolve() / "root"}',
                        '--define', f'jacket_version {rpm}', '--define', f'jacket_revision {release}',
                        PACKAGING / 'jacket.spec'], check=True)
        name = package_names(version)[1]
        (top / 'RPMS/x86_64' / name).replace(output / 'assets' / name)


def build_flatpak(output, version):
    output = output.resolve()
    branch = 'beta' if '-' in version else 'stable'
    with tempfile.TemporaryDirectory(dir=output) as directory:
        work = Path(directory)
        app, repo = work / 'app', work / 'repo'
        subprocess.run(['flatpak', 'build-init', '--arch=x86_64', app, LINUX_DESKTOP_ID,
                        'org.freedesktop.Sdk', 'org.freedesktop.Platform', FLATPAK_RUNTIME],
                       check=True)
        shutil.copytree(output / 'root/usr', app / 'files', symlinks=True, dirs_exist_ok=True)
        # The app uses native file choosers and publishes through temporary
        # files next to the destination. Limit that access to export folders.
        subprocess.run(['flatpak', 'build-finish', '--command=jacket',
                        '--share=network', '--share=ipc', '--socket=wayland',
                        '--socket=fallback-x11', '--device=dri',
                        '--filesystem=xdg-music:create', '--filesystem=xdg-download:create', app],
                       check=True)
        subprocess.run(['flatpak', 'build-export', repo, app, branch], check=True)
        bundle = work / package_names(version)[2]
        subprocess.run(['flatpak', 'build-bundle', f'--runtime-repo={FLATPAK_RUNTIME_REPO}',
                        repo, bundle, LINUX_DESKTOP_ID, branch], check=True)
        bundle.replace(output / 'assets' / bundle.name)


def verify_assets(directory, version, commit=None, signed=False, *, seal=False):
    expected = {*package_names(version), f'jacket-{version}-source.tar.gz', RUNTIME_SOURCES,
                CRATE_SOURCES, 'runtime-build-info.json', 'build.json'}
    if signed:
        expected.add('jacket-rpm-signing-key.asc')
    checksums = directory / 'SHA256SUMS'
    actual = {path.name for path in directory.iterdir()}
    if actual - {'SHA256SUMS'} != expected:
        raise ValueError(f'Asset set mismatch: {sorted((actual - {"SHA256SUMS"}) ^ expected)}')
    if any(path.is_symlink() or not path.is_file() or path.stat().st_size == 0
           for path in directory.iterdir()):
        raise ValueError('Release assets must be non-empty regular files')
    build = json.loads((directory / 'build.json').read_text())
    if (build['version'] != version or build['target'] != 'x86_64-unknown-linux-gnu'
            or build['commit'] != commit):
        raise ValueError('The release assets belong to a different build')
    if commit is not None:
        with tarfile.open(directory / f'jacket-{version}-source.tar.gz') as source:
            if source.pax_headers.get('comment') != commit:
                raise ValueError('Application source archive belongs to a different commit')
            lock = source.extractfile('jacket-source/Cargo.lock')
            if lock is None:
                raise ValueError('Application source archive is missing Cargo.lock')
            with lock:
                lock_checksum = file_digest(lock, 'sha256').hexdigest()
            if lock_checksum != build['cargoLockSha256']:
                raise ValueError('Application source archive contains a different Cargo.lock')
    manifest = ''.join(f'{digest(directory / name)}  {name}\n' for name in sorted(expected))
    if seal:
        checksums.write_text(manifest)
    elif checksums.read_text() != manifest:
        raise ValueError('Release asset checksums do not match')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('version', 'notes', 'stage', 'deb', 'rpm', 'flatpak', 'seal', 'verify'))
    parser.add_argument('--output', type=Path, default=OUTPUT)
    parser.add_argument('--tag')
    parser.add_argument('--commit')
    parser.add_argument('--signed', action='store_true')
    options = parser.parse_args()
    try:
        if options.command == 'verify' and options.tag:
            if not options.tag.startswith('v'):
                raise ValueError('Expected a v-prefixed release tag')
            version = options.tag[1:]
            package_versions(version)
        else:
            version = check_version(options.tag)
        if options.command == 'version':
            print(version)
        elif options.command == 'notes':
            print(release_notes(version))
        elif options.command == 'stage':
            stage(options, version)
        elif options.command == 'deb':
            build_deb(options.output, version)
        elif options.command == 'rpm':
            build_rpm(options.output, version)
        elif options.command == 'flatpak':
            build_flatpak(options.output, version)
        else:
            verify_assets(options.output / 'assets', version, options.commit,
                          options.signed, seal=options.command == 'seal')
    except (OSError, ValueError, KeyError, tarfile.TarError, subprocess.SubprocessError) as error:
        parser.exit(1, f'Linux installers: {error}\n')


if __name__ == '__main__':
    main()
