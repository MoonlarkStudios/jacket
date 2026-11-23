"""Emit only the public album metadata needed by Jacket's native app."""

import json
import sys


def artists(item):
    return " & ".join(
        entry["name"] for entry in item.get("artists") or [] if entry.get("name")
    )


def artwork(item, min_width):
    covers = item.get("thumbnails") or []
    return next(
        (
            cover["url"]
            for cover in covers
            if cover.get("url") and (cover.get("width") or 0) >= min_width
        ),
        next((cover["url"] for cover in reversed(covers) if cover.get("url")), None),
    )


def year(item):
    value = str(item.get("year", ""))
    return int(value) if len(value) == 4 and value.isascii() and value.isdecimal() else None


def album_hit(item, min_width=160):
    return {
        "id": item.get("browseId") or "",
        "title": item.get("title") or "",
        "artist": artists(item),
        "year": year(item),
        "kind": item.get("type") or "Album",
        "artwork_url": artwork(item, min_width),
    }


def album_detail(client, album_id):
    item = client.get_album(album_id)
    return album_hit(item, 512) | {
        "playlist_id": item.get("audioPlaylistId"),
        "track_count": item.get("trackCount"),
        "tracks": [
            {
                "id": track.get("videoId"),
                "title": track.get("title") or "",
                "artist": artists(track),
                "duration": track.get("duration_seconds"),
                "available": track.get("isAvailable", True),
            }
            for track in item.get("tracks") or []
        ],
    }


def main():
    from ytmusicapi import YTMusic

    operation, value = sys.argv[1:]
    client = YTMusic(language="en")
    if operation == "search":
        hits = client.search(value, filter="albums", limit=16)
        result = [album_hit(item) for item in hits[:16]]
    elif operation == "album":
        result = album_detail(client, value)
    else:
        raise ValueError("Unknown YouTube Music operation")
    print(json.dumps(result))


if __name__ == "__main__":
    try:
        main()
    except ModuleNotFoundError:
        sys.exit("Set up YouTube Music with mise run setup:youtube-music, then try again.")
    except Exception:
        sys.exit("YouTube Music did not complete the request. Try again or open it in your browser.")
