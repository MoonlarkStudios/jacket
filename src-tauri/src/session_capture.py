"""Native-only prototype handoff. stdout is secret IPC, never diagnostics."""
import contextlib
import io
import os
from pathlib import Path
import re
import sys
import time


def youtube_domain(domain):
    return isinstance(domain, str) and re.fullmatch(
        r"(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)*youtube\.com",
        domain.lower().removeprefix("."),
    ) is not None


def serialize(cookies):
    lines = ["# Netscape HTTP Cookie File", "# Jacket local session"]
    authenticated = False
    for cookie in cookies:
        if not youtube_domain(cookie.domain) or cookie.is_expired(time.time()):
            continue
        fields = [cookie.domain, "TRUE" if cookie.domain.startswith(".") else "FALSE",
                  cookie.path, "TRUE" if cookie.secure else "FALSE",
                  str(cookie.expires or 0), cookie.name, cookie.value]
        if any(not isinstance(value, str) or re.search(r"[\x00-\x1f\x7f]", value)
               for value in fields):
            raise ValueError("Invalid cookie attributes")
        if cookie.has_nonstandard_attr("HttpOnly"):
            fields[0] = "#HttpOnly_" + fields[0]
        lines.append("\t".join(fields))
        # Sanity check only; an authenticated restricted-link test is separate.
        authenticated |= cookie.name in ("SAPISID", "__Secure-3PAPISID", "__Secure-1PAPISID") and bool(cookie.value)
    if not authenticated:
        raise ValueError("No account session")
    result = "\n".join(lines) + "\n"
    if len(result.encode()) > 1024 * 1024:
        raise ValueError("Session too large")
    return result


def capture(library, profile):
    profile = Path(profile)
    if not profile.is_absolute() or not profile.is_dir() or profile.is_symlink():
        raise ValueError("Missing isolated profile")
    # Never let yt-dlp's recursive database discovery follow a profile symlink.
    for directory, subdirs, files in os.walk(profile, followlinks=False):
        if any((Path(directory) / name).is_symlink() for name in subdirs + files):
            raise ValueError("Profile contains a symlink")
    sys.path.insert(0, library)
    from yt_dlp.cookies import extract_cookies_from_browser

    class QuietLogger:
        def __getattr__(self, _name):
            return lambda *_args, **_kwargs: None

    # This process reads only the exact owned profile. All other-domain cookies
    # are discarded in memory; no all-sites plaintext export is written.
    with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
        cookies = extract_cookies_from_browser("chrome", str(profile), QuietLogger())
    return serialize(cookies)


if __name__ == "__main__":
    try:
        sys.stdout.write(capture(*sys.argv[1:]))
    except Exception:
        # Exception text can contain database values or paths; keep it private.
        sys.stderr.write("Could not read the isolated YouTube session. Sign in again, or allow the browser Keychain request.\n")
        sys.exit(1)
