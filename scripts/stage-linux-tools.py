#!/usr/bin/env python3
"""Build Jacket's x86_64 Linux runtime from hash-pinned inputs.

Only GTK/WebKit, Python 3, CA certificates and the base C/C++ runtime are system
dependencies. FFmpeg's optional libraries are not discovered from the builder.
The companion source archive is part of the deliverable, not an optional log.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / 'scripts/linux-runtime.lock.json'
OUTPUT = ROOT / 'src-tauri/resources/linux/audio-tools'
DOWNLOADS = ROOT / 'target/linux-runtime-downloads'
BUILD = ROOT / 'target/linux-runtime-build'
SOURCES = ROOT / 'target/linux-preview'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def fetch(item, cache=DOWNLOADS):
    cache.mkdir(parents=True, exist_ok=True)
    destination = cache / item['filename']
    if destination.exists() and digest(destination) == item['sha256']:
        return destination
    print(f"Downloading {item['filename']}", flush=True)
    request = urllib.request.Request(item['url'], headers={'User-Agent': 'Jacket-Linux-builder'})
    with tempfile.NamedTemporaryFile(dir=cache, delete=False) as temporary:
        candidate = Path(temporary.name)
        try:
            with urllib.request.urlopen(request, timeout=120) as response:
                shutil.copyfileobj(response, temporary)
            temporary.close()
            if digest(candidate) != item['sha256']:
                raise RuntimeError(f"Checksum mismatch: {item['filename']}")
            candidate.replace(destination)
        finally:
            candidate.unlink(missing_ok=True)
    return destination


def unpack(archive, destination):
    destination.mkdir(parents=True, exist_ok=True)
    if zipfile.is_zipfile(archive):
        with zipfile.ZipFile(archive) as package:
            for member in package.infolist():
                path = Path(member.filename)
                if path.is_absolute() or '..' in path.parts or (member.external_attr >> 16) & 0o170000 == 0o120000:
                    raise RuntimeError(f'Unsafe wheel member: {member.filename}')
            package.extractall(destination)
    else:
        with tarfile.open(archive) as package:
            package.extractall(destination, filter='data')


def run(command, cwd=None, env=None):
    print('+ ' + ' '.join(map(str, command)), flush=True)
    subprocess.run(list(map(str, command)), cwd=cwd, env=env, check=True)


def verify_manifest(directory):
    manifest = json.loads((directory / 'manifest.json').read_text())
    expected = manifest['files']
    actual = {path.relative_to(directory).as_posix() for path in directory.rglob('*') if path.is_file()}
    if actual != set(expected) | {'manifest.json'}:
        raise RuntimeError('Bundled runtime files do not match the manifest')
    for name, checksum in expected.items():
        path = directory / name
        if path.is_symlink() or digest(path) != checksum:
            raise RuntimeError(f'Bundled runtime changed: {name}')
    return manifest


def validate(directory):
    manifest = verify_manifest(directory)
    if manifest['target'] != 'x86_64-unknown-linux-gnu':
        raise RuntimeError('Unexpected Linux runtime target')
    for name, arguments in [('yt-dlp', ['--ignore-config', '--version']), ('node', ['--version']), ('ffmpeg', ['-version']), ('ffprobe', ['-version'])]:
        executable = directory / 'bin' / name
        if not os.access(executable, os.X_OK):
            raise RuntimeError(f'Bundled {name} is not executable')
        subprocess.run([str(executable), *arguments], check=True, stdout=subprocess.DEVNULL)
        if name != 'yt-dlp':
            result = subprocess.run(['ldd', str(executable)], check=True, capture_output=True, text=True)
            if 'not found' in result.stdout or str(BUILD) in result.stdout:
                raise RuntimeError(f'Nonportable {name}: {result.stdout}')
    output = subprocess.check_output([str(directory / 'bin/node'), '--version'], text=True)
    if int(output.lstrip('v').split('.')[0]) < 22:
        raise RuntimeError('YouTube requires Node 22 or later')
    # PNG codecs require zlib, which --disable-autodetect would otherwise omit.
    # Exercise both encoding and decoding before spending time building the app.
    with tempfile.TemporaryDirectory() as temporary:
        png = Path(temporary) / 'cover.png'
        jpeg = Path(temporary) / 'cover.jpg'
        ffmpeg = directory / 'bin/ffmpeg'
        run([ffmpeg, '-hide_banner', '-loglevel', 'error', '-f', 'lavfi', '-i', 'color=c=red:s=16x16', '-frames:v', '1', '-threads', '1', png])
        run([ffmpeg, '-hide_banner', '-loglevel', 'error', '-i', png, '-frames:v', '1', '-threads', '1', jpeg])
    # Verify that both the challenge solver and downloader import in isolated
    # system Python, without pip, user site packages, or a developer's venv.
    code = "import sys; sys.path.insert(0, sys.argv[1]); import yt_dlp, yt_dlp_ejs; from yt_dlp.extractor.youtube import YoutubeIE; print(YoutubeIE.IE_NAME)"
    run(['/usr/bin/python3', '-I', '-B', '-c', code, directory / 'lib/python'])
    print('Linux runtime hashes, artwork codecs, executables, imports, and shared libraries verified.', flush=True)


def source_archive(inputs, paths, runtime):
    SOURCES.mkdir(parents=True, exist_ok=True)
    name = 'jacket-linux-runtime-sources.tar.gz'
    destination = SOURCES / name
    with tarfile.open(destination, 'w:gz') as archive:
        for key, item in inputs.items():
            if item['kind'] == 'source':
                archive.add(paths[key], arcname=f'sources/{item["filename"]}')
        for relative in ['scripts/stage-linux-tools.py', 'scripts/linux-runtime.lock.json', 'docs/linux-preview.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md']:
            archive.add(ROOT / relative, arcname=relative)
        archive.add(runtime / 'build-info.json', arcname='build-info.json')
    (SOURCES / (name + '.sha256')).write_text(f'{digest(destination)}  {name}\n')


def stage():
    if platform.system() != 'Linux' or platform.machine() != 'x86_64':
        raise RuntimeError('Build this preview on x86_64 Linux (Ubuntu 24.04 baseline)')
    lock = json.loads(LOCK.read_text())
    if OUTPUT.exists():
        manifest = verify_manifest(OUTPUT)
        if manifest['lockSha256'] == digest(LOCK) and manifest.get('builderSha256') == digest(Path(__file__)):
            validate(OUTPUT)
            # A cached runtime without its source companion is not distributable.
            if (SOURCES / 'jacket-linux-runtime-sources.tar.gz').is_file():
                return
    inputs = lock['inputs']
    paths = {key: fetch(item) for key, item in inputs.items()}
    staging = OUTPUT.with_name(OUTPUT.name + '.staging')
    for directory in [BUILD, staging]:
        if directory.exists():
            shutil.rmtree(directory)
        directory.mkdir(parents=True)
    prefix = BUILD / 'prefix'
    environment = dict(os.environ, CFLAGS='-O2 -fPIC', CXXFLAGS='-O2 -fPIC', PKG_CONFIG_PATH=str(prefix / 'lib/pkgconfig'))
    jobs = str(min(os.cpu_count() or 2, 4))
    configurations = {}
    for key in ['zlib', 'lame', 'opus', 'ffmpeg']:
        extracted = BUILD / key
        unpack(paths[key], extracted)
        source = next(extracted.iterdir())
        if key == 'zlib':
            options = ['--static']
        elif key == 'lame':
            options = ['--disable-shared', '--enable-static', '--disable-frontend', '--disable-decoder', '--with-pic']
        elif key == 'opus':
            options = ['--disable-shared', '--enable-static', '--disable-extra-programs', '--disable-doc', '--with-pic']
        else:
            options = ['--disable-autodetect', '--disable-debug', '--disable-doc', '--disable-ffplay', '--disable-network', '--disable-shared', '--enable-static', '--enable-zlib', '--enable-libmp3lame', '--enable-libopus', '--pkg-config-flags=--static', f'--extra-cflags=-I{prefix}/include', f'--extra-ldflags=-L{prefix}/lib']
        command = ['./configure', f'--prefix={prefix}', *options]
        configurations[key] = command
        run(command, cwd=source, env=environment)
        run(['make', '-j', jobs], cwd=source, env=environment)
        run(['make', 'install'], cwd=source, env=environment)
    (staging / 'bin').mkdir()
    for name in ['ffmpeg', 'ffprobe']:
        shutil.copy2(prefix / 'bin' / name, staging / 'bin' / name)
    node = BUILD / 'node'
    unpack(paths['node'], node)
    node = next(node.iterdir())
    shutil.copy2(node / 'bin/node', staging / 'bin/node')
    licenses = staging / 'share/licenses'
    licenses.mkdir(parents=True)
    shutil.copy2(node / 'LICENSE', licenses / 'Node-LICENSE')
    for key, filename in [('ffmpeg', 'COPYING.LGPLv2.1'), ('ffmpeg', 'LICENSE.md'), ('zlib', 'LICENSE'), ('lame', 'COPYING'), ('opus', 'COPYING')]:
        source = next((BUILD / key).iterdir())
        shutil.copy2(source / filename, licenses / f'{key}-{filename}')
    for key in ['yt-dlp', 'yt-dlp-ejs']:
        unpack(paths[key], staging / 'lib/python')
    launcher = staging / 'bin/yt-dlp'
    launcher.write_text('#!/bin/sh\nset -eu\nJACKET_RUNTIME_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)\nexec /usr/bin/python3 -I -B "$JACKET_RUNTIME_DIR/lib/python/yt_dlp/__main__.py" "$@"\n')
    launcher.chmod(0o755)
    # Python must not leave extra files in the package's read-only runtime.
    # -B also avoids modifying the hash manifest during package validation.
    info = {'target': lock['target'], 'inputs': inputs, 'configure': configurations,
            'compiler': subprocess.check_output(['cc', '--version'], text=True).splitlines()[0],
            'builder': Path('/etc/os-release').read_text(),
            'systemDependencies': ['python3 >= 3.10', 'ca-certificates', 'GTK3', 'WebKitGTK 4.1', 'xdg-utils', 'glibc >= 2.39', 'libstdc++', 'libgcc']}
    (staging / 'build-info.json').write_text(json.dumps(info, indent=2) + '\n')
    (staging / 'README.txt').write_text('Jacket Linux audio runtime. See build-info.json for exact inputs and build commands.\nThe matching jacket-linux-runtime-sources.tar.gz must accompany distributed installers.\nyt-dlp uses system Python in isolated mode. Node and FFmpeg are private to Jacket.\n')
    files = {path.relative_to(staging).as_posix(): digest(path) for path in sorted(staging.rglob('*')) if path.is_file()}
    (staging / 'manifest.json').write_text(json.dumps({'target': lock['target'], 'lockSha256': digest(LOCK), 'builderSha256': digest(Path(__file__)), 'files': files}, indent=2) + '\n')
    validate(staging)
    source_archive(inputs, paths, staging)
    if OUTPUT.exists():
        shutil.rmtree(OUTPUT)
    staging.rename(OUTPUT)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--validate', type=Path)
    args = parser.parse_args()
    if args.validate:
        validate(args.validate.resolve())
    else:
        stage()
