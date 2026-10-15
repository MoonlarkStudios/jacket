#!/usr/bin/env python3
"""Check the development environment selected by mise without changing it."""
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tomllib


def main():
    root = Path(__file__).resolve().parents[1]
    config = tomllib.loads((root / 'mise.toml').read_text())
    rust = tomllib.loads((root / 'rust-toolchain.toml').read_text())['toolchain']['channel']
    expected = {'node': config['tools']['node'], 'rustc': rust, 'python': config['tools']['python'], 'uv': config['tools']['uv']}
    commands = {
        'node': ['--version'], 'rustc': ['--version'], 'cargo': ['--version'],
        'python': ['--version'], 'uv': ['--version'], 'yt-dlp': ['--version'],
        'ffmpeg': ['-version'], 'ffprobe': ['-version'],
    }
    errors = []
    for name, args in commands.items():
        executable = shutil.which(name)
        if executable is None:
            errors.append(f'{name} is missing; run mise install and mise run setup.')
            continue
        result = subprocess.run([executable, *args], capture_output=True, text=True)
        lines = (result.stdout or result.stderr).splitlines()
        version = lines[0] if lines else '(no version output)'
        print(f'{name}: {version} [{executable}]')
        if result.returncode:
            errors.append(f'{name} failed its version check: {version}')
        elif name in expected:
            actual = version.removeprefix('v') if name == 'node' else version.split()[1]
            if actual != expected[name]:
                errors.append(f'{name} should be {expected[name]}, got {actual}; run through mise.')
    for component in ('rustfmt', 'clippy'):
        result = subprocess.run(['cargo', component.replace('rustfmt', 'fmt'), '--version'], capture_output=True, text=True) if shutil.which('cargo') else None
        if result is None or result.returncode:
            errors.append(f'{component} is missing; run mise install.')
    if platform.system() == 'Darwin':
        result = subprocess.run(['/usr/bin/xcode-select', '-p'], capture_output=True, text=True)
        if result.returncode:
            errors.append('Install Xcode command line tools with xcode-select --install.')
        else:
            print(f'Xcode: {result.stdout.strip()}')
        # Staging follows an absolute Python shebang, not a standalone binary.
        launcher = shutil.which('yt-dlp')
        if launcher:
            with Path(launcher).open('rb') as file:
                shebang = file.readline(4096)
            interpreter = Path(shebang[2:].decode(errors='replace').strip())
            if not shebang.startswith(b'#!') or not interpreter.is_file():
                errors.append('macOS packaging requires Homebrew Python-based yt-dlp; run mise run setup:media and check PATH.')
    if errors:
        for error in errors:
            print(f'ERROR: {error}', file=sys.stderr)
        return 1
    print('Development environment is ready.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
