#!/usr/bin/env python3
"""Stage installed, trusted macOS audio tools and their actual runtime closure.

No network downloads. Rewrites copies only; the installed tools are untouched.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / 'src-tauri/resources/audio-tools'
SYSTEM = ('/usr/lib/', '/System/Library/')
MACH_MAGICS = {b'\xcf\xfa\xed\xfe', b'\xce\xfa\xed\xfe', b'\xfe\xed\xfa\xcf', b'\xfe\xed\xfa\xce', b'\xca\xfe\xba\xbe', b'\xbe\xba\xfe\xca'}


def configured_minimum_macos(root=ROOT):
    # Tauri merges the platform configuration over the shared configuration.
    # Keep validation aligned with the deployment target actually packaged.
    minimum = '10.13'
    for name in ('tauri.conf.json', 'tauri.macos.conf.json'):
        config = root / 'src-tauri' / name
        if config.is_file():
            settings = json.loads(config.read_text()).get('bundle', {}).get('macOS', {})
            minimum = settings.get('minimumSystemVersion', minimum)
    return minimum


def run(*args, env=None):
    try:
        return subprocess.check_output([str(a) for a in args], text=True, env=env, stderr=subprocess.STDOUT).strip()
    except subprocess.CalledProcessError as error:
        raise RuntimeError(f'{args[0]} failed: {error.output.strip()}') from error


def installed(name):
    value = shutil.which(name)
    if not value:
        raise RuntimeError(f'Install {name} before packaging. Staging never downloads executables.')
    return Path(value).resolve()


def macho(path):
    if not path.is_file():
        return False
    with path.open('rb') as file:
        return file.read(4) in MACH_MAGICS


def dependencies(path):
    return [line.strip().split(' (compatibility version')[0]
            for line in run('/usr/bin/otool', '-L', path).splitlines()[1:] if 'compatibility version' in line]


def rpaths(path):
    return re.findall(r'cmd LC_RPATH\s+cmdsize \d+\s+path (.+?) \(offset', run('/usr/bin/otool', '-l', path))


def resolve_dependency(name, owner):
    if name.startswith(SYSTEM):
        return None
    if name.startswith('@loader_path/'):
        result = owner.parent / name[len('@loader_path/'):]
    elif name.startswith('@rpath/'):
        candidates = [Path(p.replace('@loader_path', str(owner.parent))) / name[len('@rpath/'):] for p in rpaths(owner)]
        result = next((p for p in candidates if p.exists()), None)
        if result is None:
            raise RuntimeError(f'Cannot resolve {name} from {owner}')
    elif name.startswith('@executable_path/'):
        raise RuntimeError(f'Executable-relative dependency needs explicit handling: {owner}: {name}')
    else:
        result = Path(name)
    if not result.is_file():
        raise RuntimeError(f'Missing dependency {name} from {owner}')
    return result.resolve()


def copy_tree(source, target):
    def copy_first(source_file, target_file):
        # Match Python's first directory wins, and avoid overwriting read-only
        # Homebrew package copies reached through both .pth and global symlinks.
        if not Path(target_file).exists():
            shutil.copy2(source_file, target_file)
        return target_file
    shutil.copytree(source, target, dirs_exist_ok=True, copy_function=copy_first,
                    ignore=shutil.ignore_patterns('__pycache__', '*.pyc', '*.pth', '*.a', 'site-packages', 'test', 'tests'))


def owner_writable_tree(root):
    # Homebrew Python packages/license files often have mode 0444. Rust's
    # fs::copy preserves those modes, then cannot overwrite its target on the
    # next Tauri build. Change generated copies only, never installed inputs.
    if not root.exists():
        return
    for path in [root, *root.rglob('*')]:
        if not path.is_symlink():
            path.chmod(path.stat().st_mode | (0o700 if path.is_dir() else 0o600))


def clean_env():
    return {'PATH': '/usr/bin:/bin', 'HOME': '/var/empty', 'LANG': 'en_US.UTF-8', 'TMPDIR': '/tmp', 'PYTHONDONTWRITEBYTECODE': '1'}


def verify_manifest(root):
    manifest = json.loads((root / 'manifest.json').read_text())
    expected = set(manifest['files'])
    actual = {str(path.relative_to(root)) for path in root.rglob('*') if path.is_file()}
    extra = actual - expected - {'manifest.json'}
    missing = expected - actual
    if extra:
        raise RuntimeError(f'Unexpected bundled file: {sorted(extra)[0]} ({len(extra)} extra files)')
    if missing:
        raise RuntimeError(f'Missing bundled file: {sorted(missing)[0]}')
    for name, digest in manifest['files'].items():
        if hashlib.sha256((root / name).read_bytes()).hexdigest() != digest:
            raise RuntimeError(f'Staged file changed: {name}')


def validate(root):
    for path in root.rglob('*'):
        if macho(path):
            for name in dependencies(path):
                if name.startswith(SYSTEM):
                    continue
                if not name.startswith('@loader_path/') or not (path.parent / name[len('@loader_path/'):]).is_file():
                    raise RuntimeError(f'Nonportable dependency: {path}: {name}')
            run('/usr/bin/codesign', '--verify', '--strict', path)
    versions = {}
    for name, args in {'yt-dlp': ['--version'], 'ffmpeg': ['-version'], 'ffprobe': ['-version'], 'node': ['--version']}.items():
        versions[name] = run(root / 'bin' / name, *args, env=clean_env()).splitlines()[0]
    run(root / 'bin/node', '-e', 'if (eval("40 + 2") !== 42) process.exit(1)', env=clean_env())
    # Exercise Python native extensions, CA bundle and EJS instead of only testing import yt_dlp.
    run(root / 'bin/python', '-B', '-S', root / 'python/check-runtime.py', env={**clean_env(), 'PYTHONHOME': str(root / 'python')})
    # Exercise actual MP3 encoding and probing without downloading copyrighted audio.
    sample = root / 'validation.mp3'
    try:
        run(root / 'bin/ffmpeg', '-hide_banner', '-loglevel', 'error', '-f', 'lavfi', '-i', 'sine=duration=0.1', '-c:a', 'libmp3lame', '-y', sample, env=clean_env())
        result = json.loads(run(root / 'bin/ffprobe', '-v', 'error', '-show_streams', '-of', 'json', sample, env=clean_env()))
        if result['streams'][0]['codec_name'] != 'mp3':
            raise RuntimeError('Bundled MP3 codec smoke test failed')
    finally:
        sample.unlink(missing_ok=True)
    return versions


def stage(destination):
    if platform.system() != 'Darwin':
        raise RuntimeError('This staging mechanism currently targets macOS only')
    # Repair legacy generated Tauri outputs before a repeated build.
    for profile in ('debug', 'release'):
        owner_writable_tree(ROOT / 'target' / profile / 'audio-tools')
    print('Staging installed audio tools and runtimes…', flush=True)
    tools = {name: installed(name) for name in ('yt-dlp', 'ffmpeg', 'ffprobe', 'node')}
    with tools['yt-dlp'].open('rb') as launcher_file:
        first_line = launcher_file.readline(4096)
    if not first_line.startswith(b'#!'):
        raise RuntimeError('Expected installed Python-based yt-dlp; standalone binary staging is not configured')
    shebang = first_line.decode().strip()
    python = Path(shebang.removeprefix('#!'))
    if not python.is_file():
        raise RuntimeError('Expected the installed Homebrew Python yt-dlp launcher; standalone yt-dlp packaging is not configured')
    info = json.loads(run(python, '-I', '-c', 'import json,sys,sysconfig; print(json.dumps({"stdlib":sysconfig.get_path("stdlib"),"version":f"{sys.version_info.major}.{sys.version_info.minor}","paths":[p for p in sys.path if "site-packages" in p]}))'))
    # Homebrew bin/python is a framework launcher that posix_spawns Python.app.
    # Copy the actual interpreter, so the launcher cannot escape the staged tree.
    framework_interpreter = Path(info['stdlib']).parent.parent / 'Resources/Python.app/Contents/MacOS/Python'
    if framework_interpreter.is_file():
        python = framework_interpreter.resolve()
    # Work in a fresh staging directory so an interrupted stage cannot ship half a runtime.
    staged = destination.with_name(destination.name + '.staging')
    shutil.rmtree(staged, ignore_errors=True)
    (staged / 'bin').mkdir(parents=True)
    (staged / 'lib').mkdir()
    stdlib = staged / 'python/lib' / f'python{info["version"]}'
    copy_tree(Path(info['stdlib']), stdlib)
    packages = stdlib / 'site-packages'
    packages.mkdir()
    for path in info['paths']:
        source = Path(path)
        if source.is_dir():
            copy_tree(source, packages)
    # Native dependencies are recursively copied, including Python extension libraries.
    mapping = {}
    pending = []
    for name in ('ffmpeg', 'ffprobe', 'node'):
        target = staged / 'bin' / name
        shutil.copy2(tools[name], target)
        mapping[tools[name]] = target
        pending.append((tools[name], target))
    python_target = staged / 'bin/python'
    shutil.copy2(python, python_target)
    mapping[python] = python_target
    pending.append((python, python_target))
    for source_base, target_base in [(Path(info['stdlib']), stdlib)] + [(Path(p), packages) for p in info['paths'] if Path(p).is_dir()]:
        for source in source_base.rglob('*'):
            target = target_base / source.relative_to(source_base)
            if target.exists() and macho(source):
                mapping[source.resolve()] = target
                pending.append((source.resolve(), target))
    handled = set()
    while pending:
        source, target = pending.pop()
        if target in handled:
            continue
        handled.add(target)
        changes = []
        for name in dependencies(source):
            dependency = resolve_dependency(name, source)
            if dependency is None:
                continue
            if dependency == source:  # dylib's own install name, not a dependency
                continue
            if dependency not in mapping:
                digest = hashlib.sha256(str(dependency).encode()).hexdigest()[:12]
                copied = staged / 'lib' / f'{digest}-{dependency.name}'
                shutil.copy2(dependency, copied)
                mapping[dependency] = copied
                pending.append((dependency, copied))
            relative = os.path.relpath(mapping[dependency], target.parent)
            changes.extend(['-change', name, '@loader_path/' + relative])
        # Some Homebrew files are read-only; copies must be writable for relocation/signing.
        target.chmod(target.stat().st_mode | 0o200)
        if '.dylib' in target.name or target.name == 'Python':
            changes.extend(['-id', '@loader_path/' + target.name])
        if changes:
            run('/usr/bin/install_name_tool', *changes, target)
        # Relocated copies use a local ad-hoc signature. Release packaging must
        # replace this with Developer ID signing, including Node JIT entitlements.
        run('/usr/bin/codesign', '--force', '--sign', '-', target)
    launcher = staged / 'bin/yt-dlp'
    launcher.write_text('''#!/bin/sh
set -eu
runtime_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
unset PYTHONPATH PYTHONSTARTUP
export PYTHONHOME="$runtime_dir/python" PYTHONNOUSERSITE=1 PYTHONDONTWRITEBYTECODE=1
export SSL_CERT_FILE="$runtime_dir/python/lib/pythonVERSION/site-packages/certifi/cacert.pem"
exec "$runtime_dir/bin/python" -B -S "$runtime_dir/python/yt-dlp.py" "$@"
'''.replace('VERSION', info['version']))
    launcher.chmod(0o755)
    (staged / 'python/yt-dlp.py').write_text(f'''import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent / "lib/python{info['version']}/site-packages"))
from yt_dlp import main
main()
''')
    (staged / 'python/check-runtime.py').write_text(f'''import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent / "lib/python{info['version']}/site-packages"))
import ssl, sqlite3, bz2, lzma, ctypes, certifi, cffi, _cffi_backend, curl_cffi, Cryptodome, yt_dlp, yt_dlp_ejs
assert Path(certifi.where()).is_file()
''')
    # Preserve installed license texts, bottle provenance and SBOMs for review.
    provenance = staged / 'licenses'
    roots = set()
    for source in list(mapping) + list(tools.values()):
        for ancestor in source.parents:
            if (ancestor / 'INSTALL_RECEIPT.json').is_file():
                roots.add(ancestor)
                break
    for formula in sorted(roots):
        target = provenance / f'{formula.parent.name}-{formula.name}'
        target.mkdir(parents=True)
        for item in formula.iterdir():
            if item.is_file() and (item.name.lower().startswith(('license', 'copying', 'copyright', 'notice')) or item.name in ('INSTALL_RECEIPT.json', 'sbom.spdx.json')):
                shutil.copy2(item, target / item.name)
        if (formula / '.brew').is_dir():
            shutil.copytree(formula / '.brew', target / '.brew')
    node_license = tools['node'].parent.parent / 'LICENSE'
    if node_license.is_file():
        (provenance / 'node').mkdir(parents=True)
        shutil.copy2(node_license, provenance / 'node/LICENSE')
    owner_writable_tree(staged)
    print('Validating bundled dependencies, signatures and audio codecs…', flush=True)
    versions = validate(staged)
    minimum_os = (0, 0)
    architectures = set()
    for path in staged.rglob('*'):
        if not macho(path):
            continue
        archs = set(run('/usr/bin/lipo', '-archs', path).split())
        if platform.machine() not in archs:
            raise RuntimeError(f'{path} does not support this host architecture')
        architectures.update(archs)
        load_commands = run('/usr/bin/otool', '-l', path)
        for version in re.findall(r'cmd LC_BUILD_VERSION\s+cmdsize \d+\s+platform \d+\s+minos (\d+\.\d+(?:\.\d+)?)|cmd LC_VERSION_MIN_MACOSX\s+cmdsize \d+\s+version (\d+\.\d+(?:\.\d+)?)', load_commands):
            minimum_os = max(minimum_os, tuple(int(part) for part in next(part for part in version if part).split('.')))
    minimum_os = (minimum_os + (0, 0, 0))[:3]
    configured_minimum = configured_minimum_macos()
    if minimum_os > (tuple(int(part) for part in configured_minimum.split('.')) + (0, 0, 0))[:3]:
        raise RuntimeError(f'Bundled tools require macOS {minimum_os}; raise minimumSystemVersion or stage compatible builds')
    files = {str(p.relative_to(staged)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(staged.rglob('*')) if p.is_file()}
    (staged / 'manifest.json').write_text(json.dumps({'schemaVersion': 1, 'platform': platform.system(), 'architecture': platform.machine(), 'minimumMacOS': '.'.join(map(str, minimum_os)), 'versions': versions, 'installedSources': {k: str(v) for k, v in tools.items()}, 'files': files}, indent=2) + '\n')
    # Relocation checks a path with spaces, isolated environment, and package hashes.
    import tempfile
    print('Checking runtime relocation with an isolated environment…', flush=True)
    with tempfile.TemporaryDirectory(prefix='audio-dl portable ') as temp:
        relocated = Path(temp) / 'Audio Tools'
        shutil.copytree(staged, relocated)
        validate(relocated)
    shutil.rmtree(destination, ignore_errors=True)
    staged.rename(destination)
    print(json.dumps({'staged': str(destination), 'versions': versions, 'files': len(files)}, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--validate', type=Path, help='Validate an existing resource directory')
    parser.add_argument('--destination', type=Path, default=DEFAULT)
    options = parser.parse_args()
    if options.validate:
        verify_manifest(options.validate)
        print(json.dumps(validate(options.validate), indent=2))
    else:
        stage(options.destination.resolve())


if __name__ == '__main__':
    main()
