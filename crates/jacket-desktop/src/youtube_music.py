"""Emit only the public music metadata needed by Jacket's native app."""

import json
import re
import sys
import time


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


def album_hit(item, min_width=160, *, default_kind="Album"):
    return {
        "id": item.get("browseId") or "",
        "title": item.get("title") or "",
        "artist": artists(item),
        "year": year(item),
        "kind": item.get("type") or default_kind,
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


def artist_detail(client, artist_id, *, full_releases=False):
    item = client.get_artist(artist_id)
    hits = []
    name = item.get("name") or ""
    has_more_releases = False
    for key, default_kind in (("albums", "Album"), ("singles", "Single")):
        shelf = item.get(key) or {}
        results = shelf.get("results") or []
        browse_id, params = shelf.get("browseId"), shelf.get("params")
        if browse_id and params:
            has_more_releases = True
            if full_releases:
                results = client.get_artist_albums(browse_id, params, limit=None) or results
        for result in results:
            hit = album_hit(result, default_kind=default_kind)
            hit["artist"] = hit["artist"] or name
            hits.append({"result_type": "album", "featured": False, **hit})
    for result in ((item.get("songs") or {}).get("results") or [])[:12]:
        hit = song_hit(result)
        hit["artist"] = hit["artist"] or name
        hits.append({"result_type": "song", "featured": False, **hit})
    return {
        "artist": artist_hit(item, artist_id),
        "hits": hits,
        "has_more_releases": has_more_releases,
    }


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


EDITION_SUFFIX = re.compile(
    r"\s*[\[(](?:(?:\d{4} )?remaster(?:ed)?(?: \d{4})?|"
    r"(?:legacy|deluxe|super deluxe|expanded|anniversary|special|standard|\d+(?:st|nd|rd|th) anniversary) edition)"
    r"[\])]\s*$",
    re.IGNORECASE,
)


def title_key(title):
    return " ".join(re.findall(r"[^\W_]+", title.casefold()))


def artist_ids(item):
    credits = item.get("artists") or []
    if all(credit.get("id") for credit in credits):
        return {credit["id"].removeprefix("MPLA") for credit in credits}
    return set()


def same_album(source, candidate):
    ids = artist_ids(source)
    kind = source.get("type")
    return (
        ids
        and ids == artist_ids(candidate)
        and kind
        and kind == candidate.get("type")
        and title_key(EDITION_SUFFIX.sub("", source.get("title") or ""))
        == title_key(EDITION_SUFFIX.sub("", candidate.get("title") or ""))
    )


def shared_track_list(source, candidate):
    def track_titles(album):
        return {
            title_key(EDITION_SUFFIX.sub("", track["title"]))
            for track in album.get("tracks") or [] if track.get("title")
        }
    left = track_titles(source)
    right = track_titles(candidate)
    return len(left & right) * 2 > min(len(left), len(right))


def edition_summary(item, browse_id):
    count = item.get("trackCount")
    return {
        "id": browse_id,
        "title": item.get("title") or "",
        "year": year(item),
        "track_count": count if type(count) is int and count > 0 else None,
        "artwork_url": artwork(item, 160),
    }


def edition_candidates(items, album_id):
    seen = {album_id}
    for item in items:
        browse_id = item.get("browseId") or ""
        if browse_id not in seen and re.fullmatch(r"MPRE[A-Za-z0-9_-]{1,96}", browse_id):
            seen.add(browse_id)
            yield item, browse_id


def album_editions(client, album_id, lookup_errors):
    deadline = time.monotonic() + 8
    source = client.get_album(album_id)
    candidates = list(edition_candidates(source.get("other_versions") or [], album_id))
    linked = bool(candidates)
    if not linked:
        artist = next((credit["name"] for credit in source.get("artists") or [] if credit.get("name")), "")
        if not artist or not artist_ids(source) or not source.get("title"):
            return []
        title = EDITION_SUFFIX.sub("", source["title"])
        hits = client.search(f"{title} {artist}", filter="albums", limit=12)
        candidates = edition_candidates((hit for hit in hits if same_album(source, hit)), album_id)

    editions = []
    for item, browse_id in candidates:
        summary = edition_summary(item, browse_id)
        if time.monotonic() < deadline and (not linked or None in (summary["year"], summary["track_count"])):
            try:
                detail = client.get_album(browse_id)
            except lookup_errors:
                detail = None
            if detail is not None:
                if not linked and not (same_album(source, detail) and shared_track_list(source, detail)):
                    continue
                values = edition_summary(detail, browse_id)
                summary.update({key: value for key, value in values.items() if value not in (None, "")})
            elif not linked:
                continue
        elif not linked:
            break
        if summary["title"]:
            editions.append(summary)
    return editions


def main():
    from ytmusicapi import YTMusic

    if sys.argv[1:] == ["--version"]:
        from importlib.metadata import version

        YTMusic(language="en")
        print(version("ytmusicapi"))
        return
    operation, value, category = sys.argv[1:]
    if operation == "editions":
        from functools import partial
        from requests import RequestException, Session
        from ytmusicapi.exceptions import YTMusicError

        with Session() as session:
            session.request = partial(session.request, timeout=2.5)
            client = YTMusic(language="en", requests_session=session)
            result = album_editions(client, value, (YTMusicError, RequestException))
        print(json.dumps(result))
        return
    client = YTMusic(language="en")
    if operation == "search":
        result = search_hits(client, value, category or None)
    elif operation == "album":
        result = album_detail(client, value)
    elif operation in ("artist", "artist-releases"):
        result = artist_detail(client, value, full_releases=operation == "artist-releases")
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
