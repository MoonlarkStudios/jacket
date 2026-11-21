# Example album artwork

`room-to-breathe.svg` and its PNG export are original Jacket assets for the
fictional album **Room to breathe** by **The Paper Trails**. They demonstrate the
review, artwork and file-preview interfaces without making a network request.
They are covered by the repository's GPL-3.0-or-later license.

The geometric landscape is authored in `build_cover.py`. The SVG is independently
editable; the companion PNG supports the same native decoding path used for real
covers. The optional export script uses Pillow and the repository's Geist font:

```sh
python assets/examples/build_cover.py
```

These assets do not identify real recordings and are never enabled for download.
