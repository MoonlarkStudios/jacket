#!/usr/bin/env python3
"""Collect both installers, exact source companions, build identity and hashes."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'target/linux-preview'


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    config = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())
    version = config['version']
    # Cached build folders can contain older releases. Never collect them.
    for old in [*OUTPUT.glob('*.deb'), *OUTPUT.glob('*.rpm')]:
        old.unlink()
    for kind in ['deb', 'rpm']:
        filename = f'Jacket_{version}_amd64.deb' if kind == 'deb' else f'Jacket-{version}-1.x86_64.rpm'
        packages = list((ROOT / 'target/release/bundle' / kind).glob(filename))
        if len(packages) != 1:
            raise RuntimeError(f'Expected exactly one {kind} installer, found {packages}')
        package = packages[0]
        if kind == 'deb':
            fields = subprocess.check_output(['dpkg-deb', '-f', str(package), 'Package', 'Architecture'], text=True)
            if 'jacket' not in fields or 'amd64' not in fields:
                raise RuntimeError(f'Unexpected DEB metadata: {fields}')
        else:
            fields = subprocess.check_output(['rpm', '-qp', '--queryformat', '%{NAME} %{ARCH} %{LICENSE}', str(package)], text=True)
            if fields != 'jacket x86_64 GPL-3.0-or-later':
                raise RuntimeError(f'Unexpected RPM metadata: {fields}')
        shutil.copy2(package, OUTPUT / package.name)
    source = OUTPUT / 'jacket-source.tar.gz'
    subprocess.run(['git', 'archive', '--format=tar.gz', f'--output={source}', 'HEAD'], cwd=ROOT, check=True)
    shutil.copy2(ROOT / 'src-tauri/resources/linux/audio-tools/build-info.json', OUTPUT / 'runtime-build-info.json')
    shutil.copy2(ROOT / 'docs/linux-preview.md', OUTPUT / 'README.md')
    identity = {'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(), 'target': 'x86_64-unknown-linux-gnu', 'status': 'preview-candidate', 'baseline': 'Ubuntu 24.04', 'desktopAcceptance': 'manual GNOME/KDE Wayland/X11 acceptance remains required'}
    (OUTPUT / 'build.json').write_text(json.dumps(identity, indent=2) + '\n')
    if not (OUTPUT / 'jacket-linux-runtime-sources.tar.gz').is_file():
        raise RuntimeError('Missing corresponding runtime sources; refusing to collect installers')
    hashes = []
    for path in sorted(OUTPUT.iterdir()):
        if path.is_file() and path.name != 'SHA256SUMS':
            with path.open('rb') as stream:
                hashes.append(f"{hashlib.file_digest(stream, 'sha256').hexdigest()}  {path.name}\n")
    (OUTPUT / 'SHA256SUMS').write_text(''.join(hashes))


if __name__ == '__main__':
    main()
