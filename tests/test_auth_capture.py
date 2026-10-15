"""Cookie handoff tests use synthetic Cookie objects, never browser databases."""
import importlib.util
from http.cookiejar import Cookie
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    "session_capture", Path(__file__).resolve().parents[1] / "src-tauri/src/session_capture.py"
)
capture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)


def cookie(domain=".youtube.com", name="SAPISID", value="synthetic-fixture", **kwargs):
    return Cookie(version=0, name=name, value=value, port=None, port_specified=False,
                  domain=domain, domain_specified=True, domain_initial_dot=domain.startswith("."),
                  path="/", path_specified=True, secure=True, expires=kwargs.get("expires"),
                  discard=True, comment=None, comment_url=None, rest={"HttpOnly": None})


class CaptureTests(unittest.TestCase):
    def test_only_youtube_domains_survive_and_attributes_are_preserved(self):
        jar = capture.serialize([
            cookie(), cookie("music.youtube.com", "PREF"),
            cookie(".accounts.google.com", "GOOGLE_SESSION"),
            cookie("youtube.com.attacker.test", "LOOKALIKE"),
            cookie("..youtube.com", "MALFORMED"),
            cookie(".youtube.com", "EXPIRED", expires=1),
        ])
        self.assertIn("#HttpOnly_.youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID", jar)
        self.assertIn("#HttpOnly_music.youtube.com\tFALSE", jar)
        for name in ("GOOGLE_SESSION", "LOOKALIKE", "MALFORMED", "EXPIRED"):
            self.assertNotIn(name, jar)

    def test_control_characters_cannot_inject_cookie_lines(self):
        for value in ("bad\nvalue", "bad\tvalue", "bad\rvalue", "bad\x00value"):
            with self.assertRaises(ValueError):
                capture.serialize([cookie(value=value)])

    def test_unsigned_or_expired_browser_is_not_accepted_as_a_session(self):
        for cookies in ([], [cookie(name="PREF")], [cookie(expires=1)], [cookie(value="")]):
            with self.assertRaises(ValueError):
                capture.serialize(cookies)

    def test_expiry_and_host_only_domain_are_not_mutated(self):
        original = cookie("www.youtube.com", expires=4102444800)
        jar = capture.serialize([original])
        self.assertIn("www.youtube.com\tFALSE\t/\tTRUE\t4102444800", jar)
        self.assertEqual(original.expires, 4102444800)


if __name__ == "__main__":
    unittest.main()
