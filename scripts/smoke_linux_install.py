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
for source_url do :; done
case "$source_url" in
    *v=fixture0002)
        if [ -n "${JACKET_ALBUM_GATE:-}" ]; then
            touch "$JACKET_ALBUM_GATE.started"
            while [ ! -f "$JACKET_ALBUM_GATE" ]; do sleep 0.1; done
        fi
        ;;
esac
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


def verify_export(destination, expected_tags=MEDIA_TAGS):
    wait_for(str(destination), destination.is_file)
    probe = run(TOOLS / 'ffprobe', '-v', 'error', '-show_format', '-show_streams',
                '-of', 'json', destination, capture_output=True, text=True)
    metadata = json.loads(probe.stdout)
    tags = metadata['format']['tags']
    assert all(tags.get(key) == value for key, value in expected_tags.items()), tags
    assert any(stream['disposition']['attached_pic'] and stream['width'] == 32
               and stream['height'] == 32 for stream in metadata['streams']), metadata
    print(f'Verified written file, tags and embedded artwork: {destination}', flush=True)


def prepare_collection_review(review, started_review, *, album):
    saved = json.loads(started_review)
    draft = saved['draft']
    inspection = draft['inspection']
    first = inspection['tracks'][0]
    second_url = 'https://www.youtube.com/watch?v=fixture0002'
    second_tags = dict(first['metadata'], title='Second light', trackNumber=2)
    second = dict(first, id='fixture0002', sourceUrl=second_url,
                  sourceMetadata=second_tags, metadata=second_tags)
    mode = 'album' if album else 'collection'
    title = MEDIA_TAGS['album'] if album else 'Evening rotation'
    url = ('https://music.youtube.com/playlist?list=OLAK5uy_fixture' if album
           else 'https://www.youtube.com/playlist?list=PLSample123456')
    session = f'offline-{mode}-review'
    saved['sessionId'] = session
    draft.update(sessionId=session, url=url, selected=[0, 1])
    inspection.update(title=title, sourceUrl=url, suggestedMode=mode,
                      tracks=[first, second])
    draft['preferences'].update(title=title, mode=mode, albumTitle=MEDIA_TAGS['album'],
                                albumArtist=MEDIA_TAGS['artist'], outputKind='folder')
    write_json(review, saved)


def album_is_cancelled():
    cache = Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache'))
    for baseline in (cache / 'com.moonlark.audio-dl/jobs').glob('job-*/recovery.json'):
        record = json.loads(baseline.read_text())
        if not record.get('musicAlbumStems'):
            continue
        progress = baseline.with_name('recovery-progress.json')
        state = json.loads(progress.read_text()) if progress.is_file() else record
        if state['progress']['stage'] == 'cancelled':
            return True
    return False


def album_recovery(evidence, log, environment, recording, album, review, started_review):
    prepare_collection_review(review, started_review, album=True)
    first = album / 'First light.mp3'
    second = album / 'Second light.mp3'
    first_hash = sha256(first.read_bytes()).hexdigest()
    gate = recording.parent / 'allow-album-download'
    environment = dict(environment, JACKET_ALBUM_GATE=str(gate))
    failure = evidence / 'album-failure.png'
    with process([JACKET, '--compact'], log, failure_image=failure, env=environment) as app:
        jacket = window('Jacket', app)
        time.sleep(1)
        run('import', '-window', jacket, evidence / 'album-review.png')
        key(jacket, 'ctrl+s')
        wait_for('the missing album track to start', gate.with_suffix('.started').is_file)
        key(jacket, 'Escape')
        wait_for('the cancelled album checkpoint', album_is_cancelled)
        assert not second.exists() and not (album / 'playlist.m3u').exists()
        time.sleep(0.6)
        run('wmctrl', '-ic', hex(int(jacket)))
        assert app.wait(timeout=15) == 0

    gate.touch()
    with process([JACKET, '--compact'], log, failure_image=failure, env=environment) as app:
        jacket = window('Jacket', app)
        time.sleep(1)
        run('import', '-window', jacket, evidence / 'album-resume.png')
        click(jacket, 471, 146)
        verify_export(second, dict(MEDIA_TAGS, title='Second light'))
        wait_for('the recovered album to clear its review',
                 lambda: json.loads(review.read_text())['draft'] is None)
        assert sha256(first.read_bytes()).hexdigest() == first_hash
        playlist = (album / 'playlist.m3u').read_text().splitlines()
        assert [entry for entry in playlist if not entry.startswith('#')] == [first.name, second.name]
        manifest = json.loads((album / 'manifest.json').read_text())
        assert [track['relativePath'] for track in manifest['tracks']] == [first.name, second.name]
        time.sleep(0.7)
        run('import', '-window', jacket, evidence / 'album-save.png')
        run('wmctrl', '-ic', hex(int(jacket)))
        assert app.wait(timeout=15) == 0


def playlist_export(evidence, log, environment, recording, review, started_review):
    prepare_collection_review(review, started_review, album=False)
    chosen = recording.parent / 'playlists'
    chosen.mkdir()
    with process([JACKET, '--compact'], log, failure_image=evidence / 'playlist-failure.png', env=environment) as app:
        jacket = window('Jacket', app)
        time.sleep(1)
        key(jacket, 'ctrl+s')
        choose(window('Choose a parent music folder', app), chosen)
        output = chosen / 'Evening rotation'
        manifest = output / 'manifest.json'
        wait_for('the playlist manifest', manifest.is_file)
        tracks = json.loads(manifest.read_text())['tracks']
        assert len(tracks) == 2
        for track, title in zip(tracks, ('First light', 'Second light'), strict=True):
            verify_export(output / track['relativePath'], dict(MEDIA_TAGS, title=title))
        wait_for('playlist export completion', lambda: json.loads(review.read_text())['draft'] is None)
        time.sleep(0.7)
        run('import', '-window', jacket, evidence / 'playlist-saved.png')
        run('wmctrl', '-ic', hex(int(jacket)))
        assert app.wait(timeout=15) == 0


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
            click(jacket, 552, 507)
            choose(window('Choose your music folder', app), selected)
            wait_for('saved music folder', lambda: json.loads(config.read_text())['root'] == str(selected))
            key(jacket, 'ctrl+shift+s')
            key(window('Save audio file', app), 'Escape')
            time.sleep(0.6)
            assert not list(selected.iterdir()) and not list(original.iterdir())
            key(jacket, 'ctrl+s')
            destination = selected / 'The Paper Trails/Room to breathe/First light.mp3'
            verify_export(destination)
            wait_for('organized export completion', lambda: json.loads(review.read_text())['draft'] is None)
            # The file and cleared draft are committed before the UI's next
            # worker poll displays the receipt and releases the close guard.
            time.sleep(0.7)
            run('import', '-window', jacket, evidence / 'organized-save.png')
            run('wmctrl', '-ic', hex(int(jacket)))
            assert app.wait(timeout=15) == 0
        album_recovery(evidence, log, environment, recording, destination.parent, review,
                       started_review)
        playlist_export(evidence, log, environment, recording, review, started_review)
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
