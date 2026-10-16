"""Shared application version, package version and release-artifact validation."""
import hashlib
import json
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
VERSION = re.compile(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(alpha|beta|rc)\.(0|[1-9]\d*))?')
SOURCE_FILES = {'jacket-source.tar.gz', 'jacket-linux-runtime-sources.tar.gz', 'jacket-linux-runtime-sources.tar.gz.sha256', 'jacket-dependency-sources.tar.gz', 'dependency-inventory.json', 'runtime-build-info.json', 'build.json', 'package-versions.json', 'README.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md'}


def package_versions(version):
    match = VERSION.fullmatch(version)
    if not match:
        raise ValueError(f'Unsupported release version: {version}; use X.Y.Z or X.Y.Z-beta.N (alpha/rc also supported), without build metadata')
    core = '.'.join(match.group(1, 2, 3))
    pre = f'{match[4]}.{match[5]}' if match[4] else None
    return {'app': version, 'deb': f'{core}~{pre}' if pre else core, 'rpmVersion': core, 'rpmRelease': f'0.{pre}' if pre else '1'}


def package_names(version):
    versions = package_versions(version)
    return {'deb': f'Jacket_{version}_amd64.deb', 'rpm': f"Jacket-{versions['rpmVersion']}-{versions['rpmRelease']}.x86_64.rpm"}


def check_versions(root=ROOT, tag=None):
    config = json.loads((root / 'src-tauri/tauri.conf.json').read_text())
    version = config['version']
    package_versions(version)
    npm = json.loads((root / 'package.json').read_text())
    lock = json.loads((root / 'package-lock.json').read_text())
    cargo = tomllib.loads((root / 'Cargo.toml').read_text())
    cargo_lock = tomllib.loads((root / 'Cargo.lock').read_text())
    manifest = json.loads((root / '.release-please-manifest.json').read_text())
    versions = {'package.json': npm['version'], 'package-lock.json': lock['version'], 'package-lock root': lock['packages']['']['version'], 'Cargo workspace': cargo['workspace']['package']['version'], 'release manifest': manifest['.']}
    for name, path in [('audio-engine', 'crates/audio-engine/Cargo.toml'), ('audio-dl-desktop', 'src-tauri/Cargo.toml')]:
        crate = tomllib.loads((root / path).read_text())
        if crate['package']['version'] != {'workspace': True}:
            raise ValueError(f'{path} must inherit the workspace version')
        versions[name] = next(package['version'] for package in cargo_lock['package'] if package['name'] == name)
    for path, actual in versions.items():
        if actual != version:
            raise ValueError(f'Version mismatch: {path} is {actual}, expected {version}')
    if tag is not None:
        if tag != f'v{version}' or version.endswith('.0') and '-' in version:
            raise ValueError(f'Release tag {tag} must match v{version}; prerelease counter must start at 1')
    return version


def verify_artifacts(directory, commit, tag):
    version = tag.removeprefix('v')
    names = set(package_names(version).values()) | SOURCE_FILES
    actual = {path.name for path in directory.iterdir()}
    if any(path.is_symlink() or not path.is_file() for path in directory.iterdir()):
        raise ValueError('Release assets must be regular files')
    if actual != names | {'SHA256SUMS'}:
        raise ValueError(f'Unexpected release file set: missing {names - actual}, extra {actual - names - {"SHA256SUMS"}}')
    recorded = {}
    for line in (directory / 'SHA256SUMS').read_text().splitlines():
        match = re.fullmatch(r'([a-f0-9]{64})  ([A-Za-z0-9_.-]+)', line)
        if not match or match[2] in recorded:
            raise ValueError('Invalid or duplicate checksum entry')
        recorded[match[2]] = match[1]
    if set(recorded) != names:
        raise ValueError('Checksum manifest does not cover exactly the release files')
    for name, expected in recorded.items():
        path = directory / name
        if path.is_symlink() or not path.is_file() or not path.stat().st_size:
            raise ValueError(f'Invalid release file: {name}')
        with path.open('rb') as stream:
            if hashlib.file_digest(stream, 'sha256').hexdigest() != expected:
                raise ValueError(f'Checksum mismatch: {name}')
    identity = json.loads((directory / 'build.json').read_text())
    if identity['commit'] != commit or identity['version'] != version or identity['target'] != 'x86_64-unknown-linux-gnu':
        raise ValueError('Release artifacts do not match the exact tag commit, version and target')
    if json.loads((directory / 'package-versions.json').read_text()) != package_versions(version):
        raise ValueError('Package versions do not match the release')
    inventory = json.loads((directory / 'dependency-inventory.json').read_text())
    if not inventory or any(not package['notices'] for package in inventory):
        raise ValueError('Missing dependency notices')
    return identity
