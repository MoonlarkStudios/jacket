"""Build Jacket's own fictional album-cover fixture (optional Pillow export)."""

from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent
FONT = ROOT.parent / 'fonts' / 'Geist.ttf'
SIZE = 640
SCALE = 3
PALETTE = {
    'background': '#183b3f', 'paper': '#e2e6d7', 'sun': '#d8be76',
    'ridge': '#81998a', 'middle': '#4a776d', 'front': '#284f4f',
    'type': '#f4f1e5', 'muted': '#bdcfbd',
}


def render():
    svg = '''<svg xmlns="http://www.w3.org/2000/svg" width="640" height="640" viewBox="0 0 640 640">
  <title>Room to breathe — The Paper Trails</title>
  <desc>Original geometric landscape for Jacket's fictional album review example.</desc>
  <defs><clipPath id="window"><rect x="48" y="48" width="544" height="400" rx="2"/></clipPath></defs>
  <rect width="640" height="640" fill="#183b3f"/>
  <g clip-path="url(#window)">
    <rect x="48" y="48" width="544" height="400" fill="#e2e6d7"/>
    <circle cx="390" cy="196" r="102" fill="#d8be76"/>
    <path d="M48 328 206 202 357 334 466 242 592 334V448H48Z" fill="#81998a"/>
    <path d="M48 370 184 304 286 325 392 375 507 319 592 344V448H48Z" fill="#4a776d"/>
    <path d="M48 416 171 353 272 364 394 414 506 380 592 398V448H48Z" fill="#284f4f"/>
  </g>
  <text x="48" y="511" fill="#f4f1e5" font-family="Geist, sans-serif" font-size="44" font-weight="400" letter-spacing="-1.5">Room to breathe</text>
  <path d="M48 548H592" stroke="#71918a" stroke-width="1"/>
  <text x="48" y="591" fill="#bdcfbd" font-family="Geist, sans-serif" font-size="15" font-weight="400" letter-spacing="3">THE PAPER TRAILS</text>
</svg>
'''
    (ROOT / 'room-to-breathe.svg').write_text(svg)
    image = Image.new('RGB', (SIZE*SCALE, SIZE*SCALE), PALETTE['background'])
    draw = ImageDraw.Draw(image)
    def box(values):
        return tuple(round(value*SCALE) for value in values)
    def points(values):
        return [(round(x*SCALE),round(y*SCALE)) for x,y in values]
    draw.rectangle(box((48,48,592,448)), fill=PALETTE['paper'])
    draw.ellipse(box((288,94,492,298)), fill=PALETTE['sun'])
    draw.polygon(points([(48,328),(206,202),(357,334),(466,242),(592,334),(592,448),(48,448)]), fill=PALETTE['ridge'])
    draw.polygon(points([(48,370),(184,304),(286,325),(392,375),(507,319),(592,344),(592,448),(48,448)]), fill=PALETTE['middle'])
    draw.polygon(points([(48,416),(171,353),(272,364),(394,414),(506,380),(592,398),(592,448),(48,448)]), fill=PALETTE['front'])
    title_font = ImageFont.truetype(str(FONT),44*SCALE)
    draw.text((48*SCALE,511*SCALE), 'Room to breathe', fill=PALETTE['type'], font=title_font, anchor='ls')
    draw.line(box((48,548,592,548)), fill='#71918a', width=SCALE)
    artist_font=ImageFont.truetype(str(FONT),15*SCALE)
    x=48*SCALE
    for character in 'THE PAPER TRAILS':
        draw.text((x,591*SCALE),character,fill=PALETTE['muted'],font=artist_font,anchor='ls')
        x += draw.textlength(character,font=artist_font)+3*SCALE
    image.resize((SIZE,SIZE), Image.Resampling.LANCZOS).save(ROOT / 'room-to-breathe.png', optimize=True)


if __name__=='__main__':
    render()
