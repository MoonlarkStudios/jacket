#!/usr/bin/env python3
"""Retain dependency notices and exact Rust/frontend sources for the preview."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'src-tauri/resources/linux/application-licenses'
SOURCES = ROOT / 'target/linux-preview/jacket-dependency-sources.tar.gz'
NOTICE_PREFIXES = ('license', 'licence', 'copying', 'copyright', 'notice', 'authors')


def supplement(ecosystem, name, version, destination, notices, overrides):
    origin = overrides.get(f'{ecosystem}:{name}@{version}')
    if notices or origin is None:
        return None
    source = ROOT / 'third-party/licenses' / origin['file']
    if hashlib.sha256(source.read_bytes()).hexdigest() != origin['sha256']:
        raise RuntimeError(f'Supplemental license checksum mismatch: {name}')
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination / source.name)
    notices.append(source.name)
    return origin


def collect_notices(source, destination):
    notices = []
    for path in sorted(source.rglob('*')):
        if path.is_file() and not path.is_symlink() and path.name.lower().startswith(NOTICE_PREFIXES):
            relative = path.relative_to(source)
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)
            target.chmod(0o644)
            notices.append(relative.as_posix())
    return notices


def main():
    overrides = json.loads((ROOT / 'scripts/linux-license-overrides.json').read_text())
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version=1', '--filter-platform=x86_64-unknown-linux-gnu'], cwd=ROOT))
    resolved = {node['id'] for node in metadata['resolve']['nodes']}
    workspace = set(metadata['workspace_members'])
    if OUTPUT.exists():
        shutil.rmtree(OUTPUT)
    OUTPUT.mkdir(parents=True)
    SOURCES.parent.mkdir(parents=True, exist_ok=True)
    inventory = []
    with tarfile.open(SOURCES, 'w:gz') as archive:
        for package in sorted(metadata['packages'], key=lambda package: (package['name'], package['version'])):
            if package['id'] not in resolved or package['id'] in workspace:
                continue
            source = Path(package['manifest_path']).parent
            name = f"{package['name']}-{package['version']}"
            notices = collect_notices(source, OUTPUT / 'rust' / name)
            origin = supplement('Rust', package['name'], package['version'], OUTPUT / 'rust' / name, notices, overrides)
            inventory.append({'ecosystem': 'Rust', 'name': package['name'], 'version': package['version'], 'license': package['license'], 'repository': package['repository'], 'source': package['source'] or 'Jacket source companion (vendored)', 'notices': notices, 'supplementalNotice': origin})
            archive.add(source, arcname=f'rust/{name}')
        modules = ROOT / 'node_modules'
        directories = subprocess.check_output(['npm', 'ls', '--omit=dev', '--all', '--parseable'], cwd=ROOT, text=True).splitlines()
        for directory in sorted(set(directories)):
            source = Path(directory).resolve()
            if source == ROOT:
                continue
            if not source.is_relative_to(modules):
                raise RuntimeError('Refusing a frontend dependency outside node_modules')
            package = json.loads((source / 'package.json').read_text())
            name = f"{package['name'].replace('/', '__')}-{package['version']}"
            notices = collect_notices(source, OUTPUT / 'npm' / name)
            origin = supplement('npm', package['name'], package['version'], OUTPUT / 'npm' / name, notices, overrides)
            inventory.append({'ecosystem': 'npm', 'name': package['name'], 'version': package['version'], 'license': package.get('license'), 'repository': package.get('repository'), 'notices': notices, 'supplementalNotice': origin})
            archive.add(source, arcname=f'npm/{name}')
        (OUTPUT / 'inventory.json').write_text(json.dumps(inventory, indent=2) + '\n')
        (OUTPUT / 'README.txt').write_text('Third-party application dependency notices. Exact registry/crate/frontend sources accompany the preview as jacket-dependency-sources.tar.gz.\nBuild and test dependencies can be included in the Rust inventory; system libraries are supplied by your distribution.\nEach dependency retains its original license and copyright. See inventory.json for versions, origins and notice locations.\n')
        archive.add(OUTPUT / 'inventory.json', arcname='inventory.json')
        archive.add(ROOT / 'third-party/licenses', arcname='supplemental-licenses')
    missing = [f"{item['ecosystem']}:{item['name']}@{item['version']}" for item in inventory if not item['notices']]
    print(f'Collected {len(inventory)} dependency source trees and their available notices.')
    if missing:
        raise RuntimeError('Review dependencies with no separate notice file before packaging: ' + ', '.join(missing))


if __name__ == '__main__':
    main()
