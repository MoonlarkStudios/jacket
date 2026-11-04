#!/usr/bin/env python3
"""Prepare the small, checksummed installer artifact used by container tests."""
import hashlib
from pathlib import Path
import re
import shutil

from release_support import check_versions, package_names

ROOT = Path(__file__).resolve().parents[1]


def prepare_installers(source, destination, version):
    # The full preview is verified by collect-linux-preview.py first. Retain its
    # installer checksums without hashing the large source companions again.
    expected = set(package_names(version).values())
    recorded = {}
    for line in (source / 'SHA256SUMS').read_text().splitlines():
        match = re.fullmatch(r'([a-f0-9]{64})  ([A-Za-z0-9_.-]+)', line)
        if not match or match[2] in recorded:
            raise ValueError('Invalid or duplicate checksum entry')
        recorded[match[2]] = match[1]
    actual = {path.name for path in source.iterdir() if path.suffix in {'.deb', '.rpm'}}
    listed = {name for name in recorded if Path(name).suffix in {'.deb', '.rpm'}}
    if actual != expected or listed != expected:
        raise ValueError('Expected exactly the current DEB and RPM installers and checksums')
    for name in expected:
        package = source / name
        if package.is_symlink() or not package.is_file() or not package.stat().st_size:
            raise ValueError(f'Invalid installer: {name}')
        with package.open('rb') as stream:
            if hashlib.file_digest(stream, 'sha256').hexdigest() != recorded[name]:
                raise ValueError(f'Installer checksum mismatch: {name}')
    # Only replace the known generated sibling, never the complete preview.
    if destination.name != 'linux-installers' or destination.parent.resolve() != source.parent.resolve() or destination.resolve() == source.resolve():
        raise ValueError('Installer output must be the generated linux-installers sibling')
    if destination.is_symlink() or destination.exists() and not destination.is_dir():
        raise ValueError('Installer output must be a regular directory')
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir()
    for name in sorted(expected):
        shutil.copy2(source / name, destination / name)
    (destination / 'SHA256SUMS').write_text(''.join(f'{recorded[name]}  {name}\n' for name in sorted(expected)))


if __name__ == '__main__':
    prepare_installers(ROOT / 'target/linux-preview', ROOT / 'target/linux-installers', check_versions())
