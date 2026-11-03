#!/usr/bin/env python3
"""Capture isolated fictional native workflows for manual visual qualification."""

import argparse
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    target = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target'))
    if not target.is_absolute():
        target = ROOT / target
    parser.add_argument('--executable', type=Path, default=target / 'debug/jacket')
    parser.add_argument('--output', type=Path, default=ROOT / 'target/native-snapshots')
    options = parser.parse_args()
    output = options.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    try:
        for compact in (False, True):
            for theme in ("dark", "light", "mocha"):
                for panel in (None, 'preview', 'settings', 'appearance', 'options', 'editor', 'bulk', 'crop'):
                    name = '-'.join(('compact' if compact else 'normal',
                                     theme, panel or 'review'))
                    destination = output / f'{name}.png'
                    command = [str(options.executable.resolve()), '--screenshot', str(destination)]
                    if compact:
                        command.append('--compact')
                    if theme != "dark":
                        command.append(f"--{theme}")
                    if panel:
                        command.extend(['--panel', panel])
                    subprocess.run(command, check=True, timeout=60)
                    if not destination.is_file() or destination.stat().st_size == 0:
                        raise ValueError(f'No screenshot was produced: {name}')
        for scenario in ('song', 'empty', 'stress'):
            for theme in ("dark", "light", "mocha"):
                destination = output / f"normal-{theme}-{scenario}.png"
                command = [str(options.executable.resolve()), '--screenshot', str(destination), f'--{scenario}']
                if theme != "dark":
                    command.append(f"--{theme}")
                subprocess.run(command, check=True, timeout=60)
                if not destination.is_file() or destination.stat().st_size == 0:
                    raise ValueError(f'No screenshot was produced: {scenario}')
        print(f'Captured 57 native views in {output}; inspect each for visual acceptance.')
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f'Native snapshot capture failed: {error}\n')


if __name__ == '__main__':
    main()
