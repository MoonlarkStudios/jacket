"""Read physical tags with Mutagen, independently of FFmpeg and Lofty."""
import base64
import hashlib
import json
from pathlib import Path
import struct
import sys

import mutagen
from mutagen.flac import Picture

path = Path(sys.argv[1])
audio = mutagen.File(path)
result = {"tags": {}, "pictures": []}

def picture(data, kind, mime, description, **extra):
    return {"type": int(kind), "mime": mime, "description": description,
            "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(), **extra}

if path.suffix == '.mp3':
    result['id3Version'] = list(audio.tags.version)
    for key, frame in audio.tags.items():
        if frame.FrameID == 'APIC':
            result['pictures'].append(picture(frame.data, frame.type, frame.mime, frame.desc))
        elif frame.FrameID == 'COMM':
            result['tags'].setdefault('COMM', []).append({'text': frame.text, 'language': frame.lang, 'description': frame.desc})
        elif hasattr(frame, 'text'):
            result['tags'][key] = [str(value) for value in frame.text]
elif path.suffix == '.m4a':
    for key, values in audio.tags.items():
        if key == 'covr':
            for image in values:
                result['pictures'].append(picture(image, 3, 'image/jpeg' if image.imageformat == image.FORMAT_JPEG else 'image/png', ''))
        else:
            result['tags'][key] = values
elif path.suffix == '.wav':
    # Mutagen's WAV reader targets ID3. Parse the actual RIFF INFO chunks to
    # verify our intentionally limited WAV profile, including omitted fields.
    data = path.read_bytes()
    pos = 12
    while pos + 8 <= len(data):
        kind = data[pos:pos + 4]
        size = struct.unpack_from('<I', data, pos + 4)[0]
        chunk = data[pos + 8:pos + 8 + size]
        pos += 8 + size + (size % 2)
        if kind != b'LIST' or chunk[:4] != b'INFO':
            continue
        item = 4
        while item + 8 <= len(chunk):
            key = chunk[item:item + 4].decode('ascii')
            length = struct.unpack_from('<I', chunk, item + 4)[0]
            result['tags'][key] = [chunk[item + 8:item + 8 + length].rstrip(b'\0').decode('utf-8')]
            item += 8 + length + (length % 2)
else:
    for key, values in audio.tags.items():
        if key.lower() == 'metadata_block_picture':
            for value in values:
                image = Picture(base64.b64decode(value, validate=True))
                result['pictures'].append(picture(image.data, image.type, image.mime, image.desc, width=image.width, height=image.height))
        else:
            result['tags'][key.upper()] = values
    if path.suffix == '.flac':
        for image in audio.pictures:
            result['pictures'].append(picture(image.data, image.type, image.mime, image.desc, width=image.width, height=image.height))

print(json.dumps(result, ensure_ascii=False))
