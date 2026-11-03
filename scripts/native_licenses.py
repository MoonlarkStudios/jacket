#!/usr/bin/env python3
"""Retain notices and exact resolved Cargo sources for native app candidates."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
NOTICE_PREFIXES = ('license', 'licence', 'copying', 'copyright', 'notice', 'authors')


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def host_target():
    version = subprocess.check_output(['rustc', '-vV'], text=True)
    for line in version.splitlines():
        if line.startswith('host: '):
            return line.removeprefix('host: ').strip()
    raise ValueError('rustc did not report its host target')


def notice_files(package):
    source = Path(package['manifest_path']).parent
    notices = {path.relative_to(source): path for path in source.rglob('*')
               if path.is_file() and not path.is_symlink()
               and path.name.lower().startswith(NOTICE_PREFIXES)}
    if package.get('license_file'):
        declared = Path(package['license_file'])
        if not declared.is_absolute():
            declared = source / declared
        declared = declared.resolve(strict=True)
        if not declared.is_relative_to(source.resolve()):
            raise ValueError(f'{package["name"]}: license file is outside its published source')
        notices[declared.relative_to(source.resolve())] = declared
    return notices


def supplement(package, overrides):
    key = f'Rust:{package["name"]}@{package["version"]}'
    origin = overrides.get(key)
    if origin is None:
        return None
    source = ROOT / 'third-party/licenses' / origin['file']
    if not source.resolve(strict=True).is_relative_to((ROOT / 'third-party/licenses').resolve()):
        raise ValueError(f'Invalid supplement path: {key}')
    if digest(source) != origin['sha256']:
        raise ValueError(f'Supplemental notice checksum changed: {key}')
    return source, origin


def reuse_inventory(output, target, overrides_hash):
    """Reuse only an unchanged inventory for this exact graph and target."""
    try:
        if output.is_symlink() or any(path.is_symlink() for path in output.rglob('*')):
            raise ValueError('inventory contains a symlink')
        manifest = json.loads((output / 'manifest.json').read_text())
        if manifest['target'] != target:
            raise ValueError('inventory belongs to a different target')
        if manifest['cargoLockSha256'] != digest(ROOT / 'Cargo.lock'):
            raise ValueError('inventory belongs to a different Cargo.lock')
        if manifest.get('noticeOverridesSha256') != overrides_hash:
            raise ValueError('inventory uses different or older supplemental notices')
        files = manifest['files']
        if not isinstance(files, dict) or not files:
            raise ValueError('inventory file manifest is invalid')
        actual = {path.relative_to(output).as_posix() for path in output.rglob('*') if path.is_file()}
        if actual != set(files) | {'manifest.json', 'SHA256SUMS'}:
            raise ValueError('inventory is incomplete or has extra files')
        for name, checksum in files.items():
            relative = PurePosixPath(name)
            if relative.is_absolute() or '..' in relative.parts or '\\' in name:
                raise ValueError('inventory contains an unsafe path')
            if digest(output / name) != checksum:
                raise ValueError(f'notices changed: {name}')
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise ValueError(f'{output}: {error}. Move or remove this generated inventory, then recollect it.') from error
    print(f'Reused verified native dependency sources and notices in {output}')
    return output


def collect(output, target, metadata=None, check_only=False):
    if metadata is None:
        metadata = json.loads(subprocess.check_output([
            'cargo', 'metadata', '--locked', '--offline', '--format-version=1',
            f'--filter-platform={target}',
        ], cwd=ROOT))
    resolved = {node['id'] for node in metadata['resolve']['nodes']}
    workspace = set(metadata['workspace_members'])
    overrides = {}
    for name in ('linux-license-overrides.json', 'native-license-overrides.json'):
        path = ROOT / 'scripts' / name
        if path.is_file():
            overrides.update(json.loads(path.read_text()))
    packages = []
    missing = []
    for package in sorted(metadata['packages'], key=lambda item: (item['name'], item['version'])):
        if package['id'] not in resolved or package['id'] in workspace:
            continue
        notices = notice_files(package)
        extra = supplement(package, overrides) if not notices else None
        if not notices and extra is None:
            missing.append(f'{package["name"]}@{package["version"]} ({package.get("license") or "unspecified"})')
        packages.append((package, notices, extra))
    if missing:
        raise ValueError('Review missing native dependency notices before packaging:\n' + '\n'.join(missing))
    if check_only:
        print(f'Verified notices for {len(packages)} resolved native Cargo dependencies ({target})')
        return None
    overrides_hash = hashlib.sha256(json.dumps(overrides, sort_keys=True).encode()).hexdigest()
    if output.is_symlink():
        raise ValueError('Refusing to reuse a native dependency inventory symlink')
    output = output.resolve()
    if output.exists():
        return reuse_inventory(output, target, overrides_hash)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.native-notices-', dir=output.parent) as temporary:
        staged = Path(temporary) / output.name
        notices_root = staged / 'application-licenses'
        notices_root.mkdir(parents=True)
        archive_path = staged / 'jacket-native-dependency-sources.tar.gz'
        inventory = []
        with tarfile.open(archive_path, 'w:gz', compresslevel=1) as archive:
            for package, notices, extra in packages:
                name = f'{package["name"]}-{package["version"]}'
                destination = notices_root / 'rust' / name
                copied = []
                for relative, source in sorted(notices.items()):
                    path = destination / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(source, path)
                    path.chmod(0o644)
                    copied.append(relative.as_posix())
                origin = None
                if extra:
                    source, origin = extra
                    destination.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(source, destination / source.name)
                    copied.append(source.name)
                    archive.add(source, arcname=f'supplemental-licenses/{source.name}')
                inventory.append({
                    'name': package['name'], 'version': package['version'],
                    'license': package.get('license'), 'repository': package.get('repository'),
                    'source': package.get('source'), 'notices': copied, 'supplementalNotice': origin,
                })
                source = Path(package['manifest_path']).parent
                archive.add(source, arcname=f'rust/{name}')
            record = {'target': target, 'cargoLockSha256': digest(ROOT / 'Cargo.lock'), 'packages': inventory}
            (notices_root / 'inventory.json').write_text(json.dumps(record, indent=2) + '\n')
            (notices_root / 'README.txt').write_text(
                'Native Cargo dependency notices. inventory.json identifies the exact target, '
                'Cargo.lock hash, resolved packages and supplemental notice provenance.\n'
                'Build/test dependencies can be included. System components retain their own licenses.\n'
                'Resolved source trees accompany the candidate as jacket-native-dependency-sources.tar.gz.\n')
            archive.add(notices_root / 'inventory.json', arcname='inventory.json')
        files = {path.relative_to(staged).as_posix(): digest(path)
                 for path in staged.rglob('*') if path.is_file()}
        (staged / 'manifest.json').write_text(json.dumps({
            'target': target, 'cargoLockSha256': digest(ROOT / 'Cargo.lock'),
            'noticeOverridesSha256': overrides_hash, 'files': files,
        }, indent=2) + '\n')
        (staged / 'SHA256SUMS').write_text(f'{digest(archive_path)}  {archive_path.name}\n')
        staged.rename(output)
    print(f'Retained {len(inventory)} native Cargo source trees and their notices in {output}')
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'target/native-dependencies')
    parser.add_argument('--target', default=None)
    parser.add_argument('--check', action='store_true', help='Verify notices without writing source archives')
    options = parser.parse_args()
    try:
        collect(options.output, options.target or host_target(), check_only=options.check)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, tarfile.TarError) as error:
        parser.exit(1, f'Native notice collection failed: {error}\n')


if __name__ == '__main__':
    main()
