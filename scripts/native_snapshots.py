#!/usr/bin/env python3
"""Capture isolated fictional native workflows for manual visual qualification."""

import argparse
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
THEMES = ('dark', 'light', 'mocha')
ALBUM_PANELS = (
    None, 'preview', 'settings', 'appearance', 'options', 'editor', 'bulk', 'crop',
    'shared-tags', 'track-order',
)


def capture_cases():
    """Yield size, theme, filename suffix and flags for each distinct view."""
    for compact in (False, True):
        for theme in THEMES:
            for panel in ALBUM_PANELS:
                yield compact, theme, panel or 'review', (['--panel', panel] if panel else [])
            for page in ('catalog', 'catalog-results', 'catalog-media', 'catalog-youtube-music',
                         'catalog-partial', 'catalog-unavailable',
                         'catalog-editions', 'catalog-album', 'catalog-match', 'catalog-variant',
                         'catalog-artist', 'catalog-song', 'catalog-song-matched'):
                yield compact, theme, page, ['--panel', page]
            for scenario, panels in (
                ('song', (None, 'preview', 'editor', 'options', 'crop')),
                ('playlist', (None, 'preview', 'editor', 'shared-tags', 'track-order')),
            ):
                for panel in panels:
                    name = f'{scenario}-{panel}' if panel else scenario
                    flags = [f'--{scenario}'] + (['--panel', panel] if panel else [])
                    yield compact, theme, name, flags
            yield compact, theme, 'long-song', ['--song', '--long']
            yield compact, theme, 'organized-song', ['--song', '--organized']
            yield compact, theme, 'organized-preview', ['--song', '--organized', '--panel', 'preview']
            yield compact, theme, 'music-folder', ['--song', '--organized', '--panel', 'music-folder']
            yield compact, theme, 'song-saved', ['--song', '--panel', 'saved']
            yield compact, theme, 'album-saved', ['--panel', 'saved']
            yield compact, theme, 'opening-song', ['--song', '--panel', 'opening']
            yield compact, theme, 'opening-playlist', ['--playlist', '--panel', 'opening-tracks']
    for scenario in ('empty', 'stress'):
        for theme in THEMES:
            yield False, theme, scenario, [f'--{scenario}']


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
    count = 0
    try:
        for compact, theme, suffix, flags in capture_cases():
            name = '-'.join(('compact' if compact else 'normal', theme, suffix))
            destination = output / f'{name}.png'
            command = [str(options.executable.resolve()), '--screenshot', str(destination)]
            if compact:
                command.append('--compact')
            if theme != 'dark':
                command.append(f'--{theme}')
            command.extend(flags)
            subprocess.run(command, check=True, timeout=60)
            if not destination.is_file() or destination.stat().st_size == 0:
                raise ValueError(f'No screenshot was produced: {name}')
            count += 1
        print(f'Captured {count} native views in {output}; inspect each for visual acceptance.')
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f'Native snapshot capture failed: {error}\n')


if __name__ == '__main__':
    main()
