#!/usr/bin/env python3
"""Check a real installed release app under Xvfb; not full desktop acceptance."""
import os
from pathlib import Path
import signal
import subprocess
import time

evidence = Path('/evidence')
evidence.mkdir(exist_ok=True)
label = os.environ.get('PREVIEW_LABEL', 'linux')
environment = dict(os.environ, WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS='1',
                   WEBKIT_DISABLE_DMABUF_RENDERER='1', LIBGL_ALWAYS_SOFTWARE='1')
with (evidence / f'{label}.log').open('w') as log:
    manager = subprocess.Popen(['openbox'], stdout=log, stderr=log)
    time.sleep(1)
    app = subprocess.Popen(['/usr/bin/jacket'], env=environment, stdout=log, stderr=log, start_new_session=True)
    try:
        deadline = time.monotonic() + 45
        window = None
        while time.monotonic() < deadline:
            if app.poll() is not None:
                raise RuntimeError(f'Jacket exited during startup: {app.returncode}')
            result = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', '^Jacket$'], capture_output=True, text=True)
            if result.returncode == 0:
                window = result.stdout.splitlines()[0]
                break
            time.sleep(0.5)
        if window is None:
            raise RuntimeError('Jacket never created a visible window')
        time.sleep(5)
        if app.poll() is not None:
            raise RuntimeError('Jacket exited after opening its window')
        subprocess.run(['import', '-window', window, str(evidence / f'{label}.png')], check=True)
        subprocess.run(['xdotool', 'windowactivate', '--sync', window, 'key', 'alt+F4'], check=True)
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired as error:
            raise RuntimeError('Idle window close did not exit Jacket') from error
        if app.returncode != 0:
            raise RuntimeError(f'Jacket failed on idle close: {app.returncode}')
    finally:
        if app.poll() is None:
            os.killpg(app.pid, signal.SIGKILL)
            app.wait()
        manager.terminate()
        manager.wait(timeout=5)
print(f'{label}: installed release window opened and closed successfully')
