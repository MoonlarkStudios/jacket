#!/usr/bin/env python3
"""Install a minimal older Jacket package to exercise package-manager upgrades.

This tests replacement and preservation of user files, not old-app schema migrations.
"""
from pathlib import Path
import shutil
import subprocess
import tempfile


def run(*args):
    subprocess.run(args, check=True)


with tempfile.TemporaryDirectory(prefix='jacket-upgrade-') as temporary:
    root = Path(temporary)
    if shutil.which('dpkg-deb'):
        control = root / 'package/DEBIAN'
        control.mkdir(parents=True)
        (control / 'control').write_text('Package: jacket\nVersion: 0.0.0\nArchitecture: amd64\nMaintainer: Moonlark Studios\nDescription: Disposable upgrade test baseline\n')
        run('dpkg-deb', '--build', '--root-owner-group', str(root / 'package'), str(root / 'baseline.deb'))
        run('dpkg', '-i', str(root / 'baseline.deb'))
    else:
        spec = root / 'baseline.spec'
        spec.write_text('Name: jacket\nVersion: 0.0.0\nRelease: 1\nSummary: Disposable upgrade test baseline\nLicense: GPL-3.0-or-later\nBuildArch: x86_64\n%description\nDisposable package-manager upgrade test fixture.\n%install\nmkdir -p %{buildroot}/usr/share/jacket-upgrade-fixture\necho baseline > %{buildroot}/usr/share/jacket-upgrade-fixture/marker\n%files\n/usr/share/jacket-upgrade-fixture/marker\n')
        run('rpmbuild', '-bb', '--define', f'_topdir {root}', str(spec))
        package = next((root / 'RPMS').rglob('*.rpm'))
        run('rpm', '-i', str(package))
