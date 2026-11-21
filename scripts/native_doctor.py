#!/usr/bin/env python3
"""Check the native app's development tools without changing the machine."""

import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys


TOOLS = {
    'rustc': ['--version'],
    'cargo': ['--version'],
    'ffmpeg': ['-version'],
    'ffprobe': ['-version'],
    'yt-dlp': ['--ignore-config', '--version'],
    'node': ['--version'],
}


def inspect_tool(name, arguments):
    executable = shutil.which(name)
    if executable is None:
        return f'{name}: missing'
    try:
        result = subprocess.run(
            [executable, *arguments], capture_output=True, text=True,
            timeout=15, check=True,
        )
    except (OSError, subprocess.SubprocessError) as error:
        return f'{name}: cannot run ({error})'
    first_line = result.stdout.splitlines()
    if not first_line:
        return f'{name}: no version reported'
    if name == 'node':
        try:
            if int(first_line[0].lstrip('v').split('.')[0]) < 22:
                return 'node: version 22 or later is required by yt-dlp'
        except ValueError:
            return 'node: unrecognized version'
    print(f'{name}: {first_line[0]}')
    return None


def main():
    errors = [error for name, arguments in TOOLS.items()
              if (error := inspect_tool(name, arguments)) is not None]
    if platform.system() == 'Linux':
        if shutil.which('pkg-config') is None:
            errors.append('pkg-config: missing; install the native build dependencies in CONTRIBUTING.md')
        else:
            result = subprocess.run(['pkg-config', '--exists', 'xkbcommon'], check=False)
            if result.returncode != 0:
                errors.append('xkbcommon headers: missing; install libxkbcommon-dev')
        if not (os.environ.get('DISPLAY') or os.environ.get('WAYLAND_DISPLAY')):
            print('Display: no active X11/Wayland session; builds and headless tests still work')
    if platform.system() == 'Darwin' and not Path('/usr/bin/xcrun').exists():
        errors.append('Xcode Command Line Tools: install with xcode-select --install')
    for error in errors:
        print(error, file=sys.stderr)
    return 1 if errors else 0


if __name__ == '__main__':
    raise SystemExit(main())
