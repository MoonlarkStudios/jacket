"""Emit only the public music metadata needed by Jacket's native app."""

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


def artist_hit(item, artist_id=None):
    # A featured artist is nested in artists, while ordinary search results
    # expose the artist and browseId directly.
    lead = next(iter(item.get("artists") or []), {})
    return {
        "id": artist_id or item.get("browseId") or lead.get("id") or "",
        "name": item.get("artist") or item.get("name") or lead.get("name") or "",
        "artwork_url": artwork(item, 160),
    }


def song_hit(item):
    album = item.get("album") or {}
    duration = item.get("duration_seconds")
    if duration is None:
        parts = (item.get("length") or "").split(":")
        if parts and all(part.isascii() and part.isdecimal() for part in parts):
            duration = sum(int(part) * 60 ** index for index, part in enumerate(reversed(parts)))
    return {
        "id": item.get("videoId") or "",
        "title": item.get("title") or "",
        "artist": artists(item),
        "album": album.get("name") or "",
        "album_id": album.get("id"),
        "duration": duration,
        "artwork_url": artwork(item, 160),
        "available": item.get("isAvailable", True),
    }


def song_detail(client, video_id):
    bundle = client.get_watch_playlist(videoId=video_id, limit=1)
    return next(
        (
            song_hit(track | {"thumbnails": track.get("thumbnail")})
            for track in bundle.get("tracks", [])
            if track.get("videoId") == video_id
        ),
        None,
    )


def search_hits(client, query, category):
    readers = {"artist": artist_hit, "album": album_hit, "song": song_hit}
    results = client.search(query, filter=category, limit=24)
    hits = []
    for item in results:
        kind = item.get("resultType")
        if kind in readers:
            hits.append({
                "result_type": kind,
                "featured": item.get("category") == "Top result",
                **readers[kind](item),
            })
    return hits[:24]


def artist_detail(client, artist_id):
    item = client.get_artist(artist_id)
    hits = []
    for key, kind, read_hit in (
        ("albums", "album", album_hit),
        ("singles", "album", album_hit),
        ("songs", "song", song_hit),
    ):
        for result in (item.get(key) or {}).get("results", [])[:12]:
            hit = read_hit(result)
            hit["artist"] = hit["artist"] or item.get("name") or ""
            hits.append({"result_type": kind, "featured": False, **hit})
    return {"artist": artist_hit(item, artist_id), "hits": hits}


def album_detail(client, album_id):
    item = client.get_album(album_id)
    return album_hit(item, 512) | {
        "artists": [
            {"name": artist["name"], "id": artist.get("id")}
            for artist in item.get("artists") or [] if artist.get("name")
        ],
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

    client = YTMusic(language="en")
    if sys.argv[1:] == ["--version"]:
        from importlib.metadata import version

        print(version("ytmusicapi"))
        return
    operation, value, category = sys.argv[1:]
    if operation == "search":
        result = search_hits(client, value, category or None)
    elif operation == "album":
        result = album_detail(client, value)
    elif operation == "artist":
        result = artist_detail(client, value)
    elif operation == "song":
        result = song_detail(client, value)
    else:
        raise ValueError("Unknown YouTube Music operation")
    print(json.dumps(result))


if __name__ == "__main__":
    try:
        main()
    except ModuleNotFoundError:
        sys.exit("YouTube Music could not load its search libraries.")
    except Exception:
        sys.exit("YouTube Music did not complete the request. Try again or open it in your browser.")
