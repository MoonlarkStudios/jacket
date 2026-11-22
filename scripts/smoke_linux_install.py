#!/usr/bin/env python3
"""Exercise Jacket's installed save workflow using Xvfb and real native dialogs."""
from contextlib import contextmanager
from hashlib import sha256
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

JACKET = '/usr/bin/jacket'
TOOLS = Path('/usr/libexec/jacket/resources/audio-tools/bin')
MEDIA_TAGS = {'title': 'First light', 'artist': 'The Paper Trails', 'album': 'Room to breathe'}


def run(*args, **kwargs):
    return subprocess.run(args, check=True, timeout=15, **kwargs)


def wait_for(description, predicate, timeout=20):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.1)
    raise RuntimeError(f'Timed out waiting for {description}')


@contextmanager
def process(args, log, *, failure_image=None, **kwargs):
    app = subprocess.Popen(args, stdout=log, stderr=subprocess.STDOUT,
                           start_new_session=True, **kwargs)
    try:
        yield app
    except Exception:
        if failure_image:
            subprocess.run(['import', '-window', 'root', failure_image], timeout=5)
        raise
    finally:
        if app.poll() is None:
            os.killpg(app.pid, signal.SIGTERM)
            try:
                app.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(app.pid, signal.SIGKILL)
                app.wait(timeout=5)


def window(title, app):
    def find():
        if app.poll() is not None:
            raise RuntimeError(f'Jacket exited with status {app.returncode} while opening {title}')
        result = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', f'^{title}$'],
                                text=True, capture_output=True, timeout=5)
        if result.returncode == 0:
            return result.stdout.splitlines()[0]
        return None
    return wait_for(f'a visible {title} window', find)


def key(window_id, chord):
    run('xdotool', 'windowactivate', '--sync', window_id, 'key', '--clearmodifiers', chord)


def click(window_id, x, y):
    run('xdotool', 'windowactivate', '--sync', window_id, 'mousemove', '--window',
        window_id, str(x), str(y), 'click', '1')


def choose(dialog, path):
    time.sleep(0.55)
    key(dialog, 'ctrl+l')
    time.sleep(0.4)
    key(dialog, 'ctrl+a')
    run('xclip', '-selection', 'clipboard', input=str(path), text=True, stdout=subprocess.DEVNULL)
    key(dialog, 'ctrl+v')
    time.sleep(0.35)
    run('xdotool', 'key', 'Return')
    time.sleep(0.35)
    if run('xdotool', 'getactivewindow', capture_output=True, text=True).stdout.strip() == dialog:
        run('xdotool', 'key', 'Return')


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value))


def prepare(home):
    data = home / '.local/share/com.moonlark.audio-dl'
    fixture = home / 'smoke'
    old = fixture / 'original'
    new = fixture / 'music'
    for directory in (old, new):
        directory.mkdir(parents=True)
    audio = fixture / 'offline.mp3'
    cover = fixture / 'cover.jpg'
    for source, options, output in [
        ('sine=frequency=440:duration=0.2', ['-c:a', 'libmp3lame'], audio),
        ('color=c=red:s=32x32', ['-frames:v', '1', '-threads', '1'], cover),
    ]:
        run(TOOLS / 'ffmpeg', '-hide_banner', '-loglevel', 'error', '-f', 'lavfi',
            '-i', source, *options, output)
    cover_id = sha256(cover.read_bytes()).hexdigest()
    artwork = data / 'review-library/artwork'
    artwork.mkdir(parents=True, exist_ok=True)
    (artwork / f'{cover_id}.jpg').write_bytes(cover.read_bytes())
    downloader = fixture / 'fixture-downloader'
    downloader.write_text('''#!/bin/sh
if [ "$1" = "--version" ]; then printf '2026.11.21\\n'; exit 0; fi
while [ "$#" -gt 1 ]; do
    if [ "$1" = "--output" ]; then
        output="$(dirname -- "$2")/source.mp3"
        cp -- "$JACKET_OFFLINE_RECORDING" "$output" || exit 1
        printf '%s\\n' "$output"
        exit 0
    fi
    shift
done
exit 1
''')
    downloader.chmod(0o700)
    write_json(data / 'downloader/preferences.json',
               {'mode': 'local', 'autoUpdate': False, 'localPath': str(downloader)})
    config = data / 'music-folder.json'
    write_json(config, {'root': str(old), 'rule': '{{artist}}/{{album}}/{{song}}'})
    url = 'https://www.youtube.com/watch?v=fixture0001'
    tags = dict(MEDIA_TAGS, albumArtist=MEDIA_TAGS['artist'], trackNumber=1,
                discNumber=1, releaseYear=2026)
    track = {'id': 'fixture0001', 'sourceUrl': url, 'duration': 0.2, 'uploader': '',
             'sourceMetadata': tags, 'metadata': tags}
    preferences = {'title': 'One-off filename', 'mode': 'collection', 'albumTitle': '',
                   'albumArtist': '', 'format': 'mp3', 'artworkMode': 'shared',
                   'sharedArtworkAsset': cover_id, 'outputKind': 'file'}
    identity = {'schemaVersion': 1, 'sessionId': 'offline-review', 'revision': 1}
    inspection = {'title': tags['title'], 'sourceUrl': url, 'suggestedMode': 'collection',
                  'tracks': [track], 'warnings': []}
    draft = dict(identity, url=url, inspection=inspection, selected=[0], preferences=preferences)
    review = data / 'review-library/draft/review.json'
    write_json(review, dict(identity, retiredSessions=[], draft=draft))
    return audio, old, new, config, review


def verify_export(destination):
    wait_for(str(destination), destination.is_file)
    probe = run(TOOLS / 'ffprobe', '-v', 'error', '-show_format', '-show_streams',
                '-of', 'json', destination, capture_output=True, text=True)
    metadata = json.loads(probe.stdout)
    tags = metadata['format']['tags']
    assert all(tags.get(key) == value for key, value in MEDIA_TAGS.items()), tags
    assert any(stream['disposition']['attached_pic'] and stream['width'] == 32
               and stream['height'] == 32 for stream in metadata['streams']), metadata
    print(f'Verified written file, tags and embedded artwork: {destination}', flush=True)


def smoke(evidence):
    recording, original, selected, config, review = prepare(Path.home())
    environment = dict(os.environ, ICED_BACKEND='tiny-skia', GDK_BACKEND='x11',
                       LIBGL_ALWAYS_SOFTWARE='1', RUST_LOG='warn',
                       JACKET_OFFLINE_RECORDING=str(recording))
    started_review = review.read_bytes()
    with (evidence / 'desktop.log').open('w') as log, process(['openbox'], log):
        wait_for('the window manager', lambda: subprocess.run(
            ['wmctrl', '-m'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0)
        with process([JACKET, '--compact'], log, failure_image=evidence / 'failure.png', env=environment) as app:
            jacket = window('Jacket', app)
            time.sleep(1)
            click(jacket, 552, 484)
            time.sleep(0.4)
            click(jacket, 489, 155)
            choose(window('Choose your music folder', app), selected)
            time.sleep(0.6)
            click(jacket, 514, 506)
            wait_for('saved music folder', lambda: json.loads(config.read_text())['root'] == str(selected))
            key(jacket, 'ctrl+shift+s')
            key(window('Save audio file', app), 'Escape')
            time.sleep(0.6)
            assert not list(selected.iterdir()) and not list(original.iterdir())
            key(jacket, 'ctrl+s')
            time.sleep(0.4)
            key(jacket, 'Return')
            destination = selected / 'The Paper Trails/Room to breathe/First light.mp3'
            verify_export(destination)
            wait_for('organized export completion', lambda: json.loads(review.read_text())['draft'] is None)
            # The file and cleared draft are committed before the UI's next
            # worker poll displays the receipt and releases the close guard.
            time.sleep(0.7)
            run('import', '-window', jacket, evidence / 'organized-save.png')
            run('wmctrl', '-ic', hex(int(jacket)))
            assert app.wait(timeout=15) == 0
        review.write_bytes(started_review)
        with process([JACKET, '--compact'], log, failure_image=evidence / 'failure.png', env=environment) as app:
            jacket = window('Jacket', app)
            time.sleep(1)
            key(jacket, 'ctrl+shift+s')
            one_off = recording.parent / 'one-off.mp3'
            choose(window('Save audio file', app), one_off)
            verify_export(one_off)
            wait_for('one-off export completion', lambda: json.loads(review.read_text())['draft'] is None)
            time.sleep(0.7)
            run('import', '-window', jacket, evidence / 'save-as.png')
            run('wmctrl', '-ic', hex(int(jacket)))
            assert app.wait(timeout=15) == 0


if __name__ == '__main__':
    smoke(Path(sys.argv[1]))
