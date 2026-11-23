#!/usr/bin/env python3
"""Stage a pinned YouTube Music helper for native Jacket packages."""
import argparse
import json
from pathlib import Path, PurePosixPath
import platform
import shutil
import tempfile
from urllib.request import urlopen
from zipfile import ZipFile

from native_package import CATALOG_HELPER, CATALOG_LOCK, ROOT, digest, verify_catalog_runtime


DOWNLOADS = ROOT / 'target/catalog-runtime-downloads'


def fetch(item):
    DOWNLOADS.mkdir(parents=True, exist_ok=True)
    destination = DOWNLOADS / item['filename']
    if destination.is_file() and digest(destination) == item['sha256']:
        return destination
    with tempfile.NamedTemporaryFile(dir=DOWNLOADS, delete=False) as stream:
        candidate = Path(stream.name)
        try:
            with urlopen(item['url'], timeout=30) as response:
                shutil.copyfileobj(response, stream)
            stream.close()
            if digest(candidate) != item['sha256']:
                raise ValueError(f'Catalog input checksum changed: {item["filename"]}')
            candidate.replace(destination)
        finally:
            candidate.unlink(missing_ok=True)
    return destination


def unpack_wheel(wheel, destination):
    if not wheel.name.endswith(('-py3-none-any.whl', '-py2.py3-none-any.whl')):
        raise ValueError(f'Expected a platform-independent catalog wheel: {wheel.name}')
    with ZipFile(wheel) as archive:
        for member in archive.infolist():
            path = PurePosixPath(member.filename)
            if (path.is_absolute() or '..' in path.parts or '\\' in member.filename
                    or (member.external_attr >> 16) & 0o170000 == 0o120000):
                raise ValueError(f'Unsafe catalog wheel member: {member.filename}')
        archive.extractall(destination)


def launcher(target):
    # macOS relocates the Python framework inside the adjoining audio runtime.
    # Linux uses the Python already declared by Jacket's installers.
    if target == 'macos':
        interpreter = '''
unset PYTHONPATH PYTHONSTARTUP PYTHONINSPECT PYTHONUSERBASE PYTHONPYCACHEPREFIX
export PYTHONHOME="$runtime/../audio-tools/python" PYTHONNOUSERSITE=1
exec "$runtime/../audio-tools/bin/python" -B -S "$runtime/launch.py" "$@"
'''
    else:
        interpreter = '''
exec /usr/bin/python3 -I -B -S "$runtime/launch.py" "$@"
'''
    return '''#!/bin/sh
set -eu
runtime=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
''' + interpreter.lstrip()


def stage(destination, target):
    inputs = json.loads(CATALOG_LOCK.read_text())['packages']
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.catalog-tools-', dir=destination.parent) as temporary:
        staged = Path(temporary) / 'catalog-tools'
        (staged / 'bin').mkdir(parents=True)
        (staged / 'sources').mkdir()
        package_dir = staged / 'lib/python'
        package_dir.mkdir(parents=True)
        for item in inputs.values():
            unpack_wheel(fetch(item['wheel']), package_dir)
            shutil.copy2(fetch(item['source']), staged / 'sources')
        bridge = staged / 'youtube_music.py'
        shutil.copyfile(CATALOG_HELPER, bridge)
        (staged / 'launch.py').write_text('''from pathlib import Path
import runpy
import sys

root = Path(__file__).resolve().parent
sys.path.insert(0, str(root / "lib/python"))
runpy.run_path(str(root / "youtube_music.py"), run_name="__main__")
''')
        executable = staged / 'bin/youtube-music'
        executable.write_text(launcher(target))
        executable.chmod(0o755)
        files = {path.relative_to(staged).as_posix(): digest(path)
                 for path in sorted(staged.rglob('*')) if path.is_file()}
        manifest = {'platform': target, 'lockSha256': digest(CATALOG_LOCK), 'files': files}
        (staged / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
        verify_catalog_runtime(staged, target)
        if destination.is_symlink():
            raise ValueError('Refusing to replace a catalog runtime symlink')
        if destination.exists():
            shutil.rmtree(destination)
        staged.rename(destination)
        print(f'Staged YouTube Music catalog tools at {destination}', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--destination', type=Path)
    options = parser.parse_args()
    target = {'Darwin': 'macos', 'Linux': 'linux'}.get(platform.system())
    if target is None:
        parser.exit(1, 'Catalog tools staging supports Linux and macOS.\n')
    destination = options.destination or ROOT / 'target/native-runtime' / target / 'catalog-tools'
    try:
        stage(destination.absolute(), target)
    except (OSError, ValueError, KeyError) as error:
        parser.exit(1, f'Catalog tools staging failed: {error}\n')
