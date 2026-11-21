#!/usr/bin/env python3
"""Package the checked-in brand PNGs as Jacket's macOS bundle icon."""

import argparse
import platform
import struct
import subprocess
import tempfile
from pathlib import Path

from native_package import MACOS_ICON, ROOT

# Each ICNS resource identifies a size in points and a display scale.
ICON_REPRESENTATIONS = (
    (b'icp4', 16, 1),
    (b'ic11', 16, 2),
    (b'icp5', 32, 1),
    (b'ic12', 32, 2),
    (b'ic07', 128, 1),
    (b'ic13', 128, 2),
    (b'ic08', 256, 1),
    (b'ic14', 256, 2),
    (b'ic09', 512, 1),
    (b'ic10', 512, 2),
)
PNG_HEADER = b'\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR'
ICNS_HEADER = struct.Struct('>4sI')


def read_square_png(path, pixels):
    image = path.read_bytes()
    if (len(image) < 24 or not image.startswith(PNG_HEADER)
            or struct.unpack_from('>II', image, 16) != (pixels, pixels)):
        raise ValueError(f'{path} must be a {pixels} × {pixels} PNG.')
    return image


def icns_header(kind, payload_size):
    return ICNS_HEADER.pack(kind, ICNS_HEADER.size + payload_size)


def bundle_icon(source):
    headers = []
    resources = []
    for kind, points, scale in ICON_REPRESENTATIONS:
        pixels = points * scale
        png = read_square_png(source / f'jacket-icon-{pixels}.png', pixels)
        header = icns_header(kind, len(png))
        headers.append(header)
        resources.append(header + png)
    table = b''.join(headers)
    contents = icns_header(b'TOC ', len(table)) + table + b''.join(resources)
    return icns_header(b'icns', len(contents)) + contents


def check_macos_icon(icon):
    with tempfile.TemporaryDirectory(prefix='jacket-icons-') as temporary:
        iconset = Path(temporary) / 'Jacket.iconset'
        subprocess.run(['/usr/bin/iconutil', '-c', 'iconset', '-o', str(iconset),
                        str(icon)], check=True)
        for _, points, scale in ICON_REPRESENTATIONS:
            suffix = '@2x' if scale == 2 else ''
            read_square_png(iconset / f'icon_{points}x{points}{suffix}.png', points * scale)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='Verify the bundle icon without rewriting it.')
    args = parser.parse_args()
    destination = ROOT / MACOS_ICON
    try:
        expected = bundle_icon(ROOT / 'branding/jacket/png')
        if args.check:
            if not destination.is_file() or destination.read_bytes() != expected:
                raise ValueError(f'{MACOS_ICON} is out of date. Run mise run branding:sync.')
            if platform.system() == 'Darwin':
                check_macos_icon(destination)
        else:
            destination.write_bytes(expected)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'{error}\n')


if __name__ == '__main__':
    main()
