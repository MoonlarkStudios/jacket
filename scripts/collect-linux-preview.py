#!/usr/bin/env python3
"""Collect both installers, exact source companions, build identity and hashes."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
from release_support import check_versions, package_names, package_versions, verify_artifacts

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'target/linux-preview'


def main():
    if subprocess.run(['git', 'diff', '--quiet', 'HEAD', '--'], cwd=ROOT).returncode or subprocess.check_output(['git', 'ls-files', '--others', '--exclude-standard'], cwd=ROOT).strip():
        raise RuntimeError('Commit the release source before collecting preview installers')
    OUTPUT.mkdir(parents=True, exist_ok=True)
    version = check_versions()
    versions = package_versions(version)
    names = package_names(version)
    # Cached build folders can contain older releases. Never collect them.
    for old in [*OUTPUT.glob('*.deb'), *OUTPUT.glob('*.rpm')]:
        old.unlink()
    for kind in ['deb', 'rpm']:
        filename = names[kind]
        packages = list((ROOT / 'target/release/bundle' / kind).glob(filename))
        if len(packages) != 1:
            raise RuntimeError(f'Expected exactly one {kind} installer, found {packages}')
        package = packages[0]
        if kind == 'deb':
            fields = subprocess.check_output(['dpkg-deb', '--show', '--showformat=${Package} ${Architecture} ${Version}', str(package)], text=True)
            if fields != f"jacket amd64 {versions['deb']}":
                raise RuntimeError(f'Unexpected DEB metadata: {fields}')
        else:
            fields = subprocess.check_output(['rpm', '-qp', '--queryformat', '%{NAME} %{ARCH} %{LICENSE} %{VERSION} %{RELEASE}', str(package)], text=True)
            if fields != f"jacket x86_64 GPL-3.0-or-later {versions['rpmVersion']} {versions['rpmRelease']}":
                raise RuntimeError(f'Unexpected RPM metadata: {fields}')
        shutil.copy2(package, OUTPUT / package.name)
    source = OUTPUT / 'jacket-source.tar.gz'
    subprocess.run(['git', 'archive', '--format=tar.gz', f'--output={source}', 'HEAD'], cwd=ROOT, check=True)
    shutil.copy2(ROOT / 'src-tauri/resources/linux/audio-tools/build-info.json', OUTPUT / 'runtime-build-info.json')
    inventory = ROOT / 'src-tauri/resources/linux/application-licenses/inventory.json'
    if any(not package['notices'] for package in json.loads(inventory.read_text())):
        raise RuntimeError('Unreviewed missing dependency notices; refusing to collect installers')
    shutil.copy2(inventory, OUTPUT / 'dependency-inventory.json')
    shutil.copy2(ROOT / 'docs/linux-preview.md', OUTPUT / 'README.md')
    for name in ['LICENSE', 'THIRD_PARTY_NOTICES.md']:
        shutil.copy2(ROOT / name, OUTPUT / name)
    if json.loads((OUTPUT / 'package-versions.json').read_text()) != versions:
        raise RuntimeError('Missing or mismatched native package versions')
    identity = {'version': version, 'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(), 'target': 'x86_64-unknown-linux-gnu', 'status': 'preview-candidate', 'baseline': 'Ubuntu 24.04', 'desktopAcceptance': 'manual GNOME/KDE Wayland/X11 acceptance remains required'}
    (OUTPUT / 'build.json').write_text(json.dumps(identity, indent=2) + '\n')
    if not (OUTPUT / 'jacket-linux-runtime-sources.tar.gz').is_file():
        raise RuntimeError('Missing corresponding runtime sources; refusing to collect installers')
    if not (OUTPUT / 'jacket-dependency-sources.tar.gz').is_file():
        raise RuntimeError('Missing application dependency sources; refusing to collect installers')
    hashes = []
    for path in sorted(OUTPUT.iterdir()):
        if path.is_file() and path.name != 'SHA256SUMS':
            with path.open('rb') as stream:
                hashes.append(f"{hashlib.file_digest(stream, 'sha256').hexdigest()}  {path.name}\n")
    (OUTPUT / 'SHA256SUMS').write_text(''.join(hashes))
    verify_artifacts(OUTPUT, identity['commit'], f'v{version}')


if __name__ == '__main__':
    main()
