#!/usr/bin/env python3
"""Regenerate Jacket's macOS bundle icon from the checked-in brand artwork."""

import platform
import shutil
import subprocess
import tempfile
from pathlib import Path

from native_package import MACOS_ICON, ROOT


def main():
    if platform.system() != 'Darwin':
        raise SystemExit('Regenerate the macOS icon on macOS with iconutil.')
    source = ROOT / 'branding/jacket/png'
    with tempfile.TemporaryDirectory(prefix='jacket-icons-') as temporary:
        iconset = Path(temporary) / 'Jacket.iconset'
        iconset.mkdir()
        for size in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                suffix = '@2x' if scale == 2 else ''
                shutil.copyfile(source / f'jacket-icon-{size * scale}.png',
                                iconset / f'icon_{size}x{size}{suffix}.png')
        generated = iconset.with_suffix('.icns')
        subprocess.run(['/usr/bin/iconutil', '-c', 'icns', '-o', str(generated), str(iconset)], check=True)
        shutil.copyfile(generated, ROOT / MACOS_ICON)


if __name__ == '__main__':
    main()
