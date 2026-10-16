#!/usr/bin/env python3
"""Ask the native package manager to verify beta counter and beta-to-stable ordering."""
import shutil
import subprocess
from release_support import package_versions

versions = [package_versions(version) for version in ['0.1.0-alpha.1', '0.1.0-beta.1', '0.1.0-beta.2', '0.1.0-beta.10', '0.1.0-rc.1', '0.1.0', '0.1.1-beta.1', '0.1.1']]
if shutil.which('dpkg'):
    for older, newer in zip(versions, versions[1:]):
        subprocess.run(['dpkg', '--compare-versions', older['deb'], 'lt', newer['deb']], check=True)
else:
    import rpm
    for older, newer in zip(versions, versions[1:]):
        left = ('0', older['rpmVersion'], older['rpmRelease'])
        right = ('0', newer['rpmVersion'], newer['rpmRelease'])
        assert rpm.labelCompare(left, right) < 0, (left, right)
print('Native package manager: alpha < beta.1 < beta.2 < beta.10 < rc < stable < next beta < next stable')
