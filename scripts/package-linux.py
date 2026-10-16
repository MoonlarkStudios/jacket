#!/usr/bin/env python3
"""Build the app at its SemVer, then give native packages correctly ordered versions."""
import json
from pathlib import Path
import re
import subprocess
import tempfile
from release_support import ROOT, check_versions, package_names, package_versions


def run(*arguments):
    subprocess.run(arguments, cwd=ROOT, check=True)


def main():
    version = check_versions()
    versions = package_versions(version)
    names = package_names(version)
    overlay = 'src-tauri/tauri.bundle.linux.json'
    # Compile with the actual application version embedded in Tauri's context.
    run('npm', 'exec', '--', 'tauri', 'build', '--config', overlay, '--no-bundle', '--', '--locked')
    # Bundle without recompiling. RPM disallows SemVer's prerelease hyphen;
    # its release field expresses prereleases as 0.beta.N, before stable's 1.
    metadata = json.dumps({'version': versions['rpmVersion'], 'bundle': {'linux': {'rpm': {'release': versions['rpmRelease']}}}})
    run('npm', 'exec', '--', 'tauri', 'bundle', '--config', overlay, '--config', metadata, '--bundles', 'deb,rpm')
    directory = ROOT / 'target/release/bundle/deb'
    original = directory / f"Jacket_{versions['rpmVersion']}_amd64.deb"
    destination = directory / names['deb']
    if versions['deb'] != versions['rpmVersion']:
        # Debian's ~ sorts before the corresponding final release. Preserve
        # Tauri's complete payload, scripts, dependencies and desktop metadata.
        with tempfile.TemporaryDirectory(prefix='jacket-deb-') as temporary:
            expanded = Path(temporary) / 'package'
            run('dpkg-deb', '--raw-extract', str(original), str(expanded))
            control = expanded / 'DEBIAN/control'
            content, count = re.subn(r'^Version: .+$', f"Version: {versions['deb']}", control.read_text(), flags=re.MULTILINE)
            if count != 1:
                raise RuntimeError('Expected exactly one Debian Version field')
            control.write_text(content)
            run('dpkg-deb', '--build', '--root-owner-group', str(expanded), str(destination))
        original.unlink()
    output = ROOT / 'target/linux-preview'
    output.mkdir(parents=True, exist_ok=True)
    (output / 'package-versions.json').write_text(json.dumps(versions, indent=2) + '\n')


if __name__ == '__main__':
    main()
