#!/usr/bin/env python3
"""Sync approved Jacket artwork to the web app and native icon containers.

Uses the checked-in brand exports and Tauri's installed icon generator.
The ICNS retains Tauri's native encodings with optical 16/32 px entries.
"""
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BRAND = ROOT / 'branding/jacket'
WEB = ROOT / 'public/branding'
ICONS = ROOT / 'src-tauri/icons'


def png(size):
    data = (BRAND / f'png/jacket-icon-{size}.png').read_bytes()
    if data[:8] != b'\x89PNG\r\n\x1a\n' or struct.unpack('>II', data[16:24]) != (size, size):
        raise ValueError(f'Expected a {size} x {size} Jacket PNG')
    return data


def tauri_icons(source, output, *options):
    result = subprocess.run([
        'node', str(ROOT / 'node_modules/@tauri-apps/cli/tauri.js'),
        'icon', str(source), '--output', str(output), *options,
    ], cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if result.returncode:
        raise RuntimeError(result.stdout)


def icns_entries(data):
    if data[:4] != b'icns' or struct.unpack('>I', data[4:8])[0] != len(data):
        raise ValueError('Invalid generated ICNS header')
    entries = {}
    offset = 8
    while offset < len(data):
        kind = data[offset:offset + 4]
        length = struct.unpack('>I', data[offset + 4:offset + 8])[0]
        if length < 8 or offset + length > len(data):
            raise ValueError('Invalid generated ICNS entry')
        entries[kind] = data[offset:offset + length]
        offset += length
    return entries


def main():
    WEB.mkdir(parents=True, exist_ok=True)
    ICONS.mkdir(parents=True, exist_ok=True)
    for source, target in {
        'jacket-logo-light.svg': 'jacket-logo.svg',
        'jacket-mark.svg': 'jacket-mark.svg',
        'jacket-app-icon.svg': 'jacket-app-icon.svg',
        'jacket-app-icon-small.svg': 'jacket-favicon.svg',
    }.items():
        shutil.copyfile(BRAND / 'svg' / source, WEB / target)
    for size, name in [(32, '32x32.png'), (128, '128x128.png'),
                       (256, '128x128@2x.png'), (1024, 'icon.png')]:
        (ICONS / name).write_bytes(png(size))
    (WEB / 'favicon-32.png').write_bytes(png(32))
    # Apple touch icons require 180 px; derive it with the already-installed
    # Tauri CLI renderer rather than introducing another image dependency.
    with tempfile.TemporaryDirectory(prefix='jacket-icons-') as temp:
        output = Path(temp)
        tauri_icons(BRAND / 'png/jacket-icon-1024.png', output, '--png', '180')
        shutil.copyfile(output / '180x180.png', WEB / 'apple-touch-icon.png')
        tauri_icons(BRAND / 'png/jacket-icon-1024.png', output / 'full')
        tauri_icons(BRAND / 'svg/jacket-app-icon-small.svg', output / 'small')
        full = icns_entries((output / 'full/icon.icns').read_bytes())
        small = icns_entries((output / 'small/icon.icns').read_bytes())
        # Legacy 16/32 px RGB/alpha entries and the 16 pt Retina PNG.
        for kind in [b'is32', b's8mk', b'il32', b'l8mk', b'ic11']:
            if kind not in full or kind not in small:
                raise ValueError(f'Tauri did not generate the required {kind!r} entry')
            full[kind] = small[kind]
        payload = b''.join(full.values())
        (ICONS / 'icon.icns').write_bytes(b'icns' + struct.pack('>I', len(payload) + 8) + payload)
    # PNG-backed ICO entries are supported by modern Windows. Preserve the
    # small optical artwork instead of resampling every entry from one master.
    sizes = [16, 32, 64, 128, 256]
    payloads = [png(size) for size in sizes]
    offset = 6 + 16 * len(sizes)
    entries = []
    for size, data in zip(sizes, payloads):
        entries.append(struct.pack('<BBBBHHII', size % 256, size % 256, 0, 0,
                                   1, 32, len(data), offset))
        offset += len(data)
    (ICONS / 'icon.ico').write_bytes(
        struct.pack('<HHH', 0, 1, len(sizes)) + b''.join(entries) + b''.join(payloads)
    )
    print('Synced Jacket web artwork, favicons, PNGs, ICO, and macOS ICNS.')


if __name__ == '__main__':
    main()
