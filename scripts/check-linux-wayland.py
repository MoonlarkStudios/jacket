#!/usr/bin/env python3
"""Check native Wayland startup on a real desktop; retain the protocol trace."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile


RENDERING_OVERRIDES = (
    'GDK_GL', 'GDK_DEBUG', '__NV_DISABLE_EXPLICIT_SYNC',
    'WEBKIT_DISABLE_DMABUF_RENDERER', 'WEBKIT_DMABUF_RENDERER_FORCE_SHM',
    'WEBKIT_DISABLE_COMPOSITING_MODE', 'WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS',
    'LIBGL_ALWAYS_SOFTWARE', 'JACKET_NVIDIA_WAYLAND_WORKAROUND',
)


def check_environment(environment):
    if not environment.get('WAYLAND_DISPLAY'):
        raise ValueError('Run this check in a real Wayland desktop session.')
    overrides = [key for key in RENDERING_OVERRIDES if key in environment]
    if overrides:
        raise ValueError('Unset rendering overrides before acceptance: ' + ', '.join(overrides))


def check_trace(trace, require_nvidia_sync=False):
    if 'wl_surface' not in trace:
        raise ValueError('No native Wayland surface was recorded.')
    if re.search(r'wl_display[#@]\d+\.error\(', trace) or 'Error 71' in trace:
        raise ValueError('The compositor rejected a Wayland protocol request.')
    if 'Failed to create GBM buffer' in trace:
        raise ValueError('WebKit could not allocate its rendering buffers.')
    if require_nvidia_sync and (
        'prepared GTK OpenGL painting for NVIDIA Wayland' not in trace
        or '.set_acquire_point(' not in trace
    ):
        raise ValueError('The NVIDIA workaround and active explicit sync were not both observed.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('/usr/bin/jacket'))
    parser.add_argument('--evidence', type=Path, default=Path('target/linux-wayland'))
    parser.add_argument('--seconds', type=int, default=15)
    parser.add_argument('--require-nvidia-sync', action='store_true')
    args = parser.parse_args()
    if args.seconds < 5:
        parser.error('--seconds must be at least 5')
    args.evidence.mkdir(parents=True, exist_ok=True)
    log_path = args.evidence / 'wayland.log'
    report_path = args.evidence / 'startup.json'
    # Remove a previous success before attempting a new check.
    report_path.unlink(missing_ok=True)
    check_environment(os.environ)
    binary = args.binary.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='jacket-wayland-') as temporary:
        environment = dict(os.environ, GDK_BACKEND='wayland', WAYLAND_DEBUG='1',
                           XDG_CONFIG_HOME=f'{temporary}/config',
                           XDG_DATA_HOME=f'{temporary}/data',
                           XDG_CACHE_HOME=f'{temporary}/cache')
        with log_path.open('w') as log:
            app = subprocess.Popen([str(binary)], env=environment, stdout=log,
                                   stderr=log, start_new_session=True)
            try:
                try:
                    code = app.wait(timeout=args.seconds)
                except subprocess.TimeoutExpired:
                    pass
                else:
                    raise RuntimeError(f'Jacket exited during startup ({code}); see {log_path}')
                check_trace(log_path.read_text(), args.require_nvidia_sync)
            finally:
                if app.poll() is None:
                    os.killpg(app.pid, signal.SIGTERM)
                    try:
                        app.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(app.pid, signal.SIGKILL)
                        app.wait()
    report_path.write_text(json.dumps({
        'binary': str(binary), 'binarySha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        'seconds': args.seconds,
        'nativeWaylandStartup': 'passed',
        'nvidiaExplicitSyncRequired': args.require_nvidia_sync,
        'manualDesktopAcceptance': 'still required',
    }, indent=2) + '\n')
    print(f'Native Wayland startup passed; evidence: {args.evidence}')
    print('Still verify visible content, resize, dialogs, download, close and restart.')


if __name__ == '__main__':
    main()
