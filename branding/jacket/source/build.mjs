import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const outline = JSON.parse(
  await readFile(new URL('./wordmark-outline.json', import.meta.url), 'utf8'),
);
const colors = {
  ink: '#20221E',
  paper: '#F3EFE5',
  copper: '#D47746',
  groove: '#B4C5A6',
  tile: '#282B25',
  muted: '#787369',
};
const sleeve =
  'M276 344H718Q732 344 732 358V503Q732 510 726 514C682 537 657 575 657 618C657 661 682 699 726 722Q732 726 732 733V862Q732 876 718 876H276Q262 876 262 862V358Q262 344 276 344Z';
const rectBand = '<rect x="367" y="344" width="44" height="532"/>';
const grooves = (stroke = colors.groove, width = 24) =>
  `<g fill="none" stroke="${stroke}" stroke-width="${width}" stroke-linecap="round"><path d="M780 433A186 186 0 0 1 780 803"/><path d="M780 499A120 120 0 0 1 780 737"/></g>`;
const wrap = (title, desc, body, box = '0 0 1254 1254', w = 1254, h = 1254) =>
  `<?xml version="1.0" encoding="UTF-8"?>\n<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="${box}" role="img" aria-labelledby="title desc"><title id="title">${title}</title><desc id="desc">${desc}</desc>${body}</svg>\n`;
function flatSymbol(mode = 'color') {
  if (mode === 'mono')
    return `<defs><mask id="knockout" maskUnits="userSpaceOnUse" x="206" y="286" width="850" height="614"><g fill="#fff"><circle cx="760" cy="618" r="250"/><path d="${sleeve}"/></g><g fill="#000">${rectBand}<circle cx="760" cy="618" r="18"/></g>${grooves('#000')}<path d="M732 368V503M732 733V868" stroke="#000" stroke-width="9"/></mask></defs><rect x="206" y="286" width="850" height="614" fill="currentColor" mask="url(#knockout)"/>`;
  const edge = mode === 'dark' ? '#59614F' : colors.ink;
  return `<circle cx="760" cy="618" r="250" fill="${colors.ink}" stroke="${edge}" stroke-width="3"/>${grooves()}<circle cx="760" cy="618" r="18" fill="${colors.paper}"/><path d="${sleeve}" fill="${colors.copper}"/><g fill="${colors.paper}">${rectBand}</g>`;
}

function icon(small = false) {
  const band = small
    ? '<rect x="365" y="344" width="48" height="532"/>'
    : rectBand;
  return `<rect x="130" y="132" width="996" height="990" rx="178" fill="${colors.tile}" stroke="#4A4F42" stroke-width="${small ? 6 : 2}"/><circle cx="760" cy="618" r="250" fill="${colors.ink}" stroke="#414839" stroke-width="${small ? 6 : 2}"/>${grooves(colors.groove, small ? 36 : 24)}<circle cx="760" cy="618" r="${small ? 22 : 18}" fill="${colors.paper}"/><path d="${sleeve}" fill="${colors.copper}"/><g fill="${colors.paper}">${band}</g>`;
}

const markDesc =
  'A copper record sleeve with a narrow cream band, a more exposed charcoal record, two broad sage grooves, and a cream center dot.';
const out = async (name, body) => writeFile(path.join(root, 'svg', name), body);
await Promise.all(
  ['svg', 'png', 'previews'].map((d) =>
    mkdir(path.join(root, d), { recursive: true }),
  ),
);
await out('jacket-app-icon.svg', wrap('Jacket app icon', markDesc, icon()));
await out(
  'jacket-app-icon-small.svg',
  wrap(
    'Jacket small app icon',
    'Optically adjusted Jacket icon: two stronger sage grooves, a narrow cream band, and a larger center dot.',
    icon(true),
  ),
);
for (const size of [16, 24, 32, 64, 128, 256, 512, 1024]) {
  await out(
    `jacket-icon-${size}.svg`,
    wrap(
      `Jacket ${size} pixel app icon`,
      size <= 32 ? 'Optically adjusted small-size version.' : markDesc,
      icon(size <= 32),
      '0 0 1254 1254',
      size,
      size,
    ),
  );
}
await out(
  'jacket-mark.svg',
  wrap('Jacket symbol', markDesc, flatSymbol(), '206 286 850 614', 850, 614),
);
await out(
  'jacket-mark-dark.svg',
  wrap(
    'Jacket symbol for dark backgrounds',
    markDesc,
    flatSymbol('dark'),
    '206 286 850 614',
    850,
    614,
  ),
);
for (const [variant, color] of [
  ['ink', colors.ink],
  ['paper', colors.paper],
]) {
  await out(
    `jacket-mark-mono-${variant}.svg`,
    wrap(
      'Jacket single-color symbol',
      'Record sleeve and packaging band expressed in a single color with transparent cutouts.',
      `<g style="color:${color}">${flatSymbol('mono')}</g>`,
      '206 286 850 614',
      850,
      614,
    ),
  );
  await out(
    `jacket-wordmark-${variant}.svg`,
    wrap(
      'Jacket wordmark',
      'Jacket wordmark converted to vector outlines, with no font dependency.',
      `<path d="${outline.path}" fill="${color}"/>`,
      `0 0 ${outline.width} ${outline.height}`,
      outline.width,
      outline.height,
    ),
  );
  const mark = `<g transform="translate(-50 -50) scale(.42)" style="color:${color}">${flatSymbol('mono')}</g>`;
  const word = `<path d="${outline.path}" transform="translate(440 105) scale(${190 / outline.height})" fill="${color}"/>`;
  await out(
    `jacket-logo-mono-${variant}.svg`,
    wrap(
      'Jacket single-color logo',
      'Outlined wordmark and transparent single-color symbol.',
      mark + word,
      '0 0 1200 400',
      1200,
      400,
    ),
  );
}
for (const [variant, mode, wordColor] of [
  ['light', 'color', colors.ink],
  ['dark', 'dark', colors.paper],
]) {
  const mark = `<g transform="translate(-50 -50) scale(.42)">${flatSymbol(mode)}</g>`;
  const word = `<path d="${outline.path}" transform="translate(440 105) scale(${190 / outline.height})" fill="${wordColor}"/>`;
  await out(
    `jacket-logo-${variant}.svg`,
    wrap(
      'Jacket horizontal logo',
      'Jacket symbol paired with an outlined wordmark.',
      mark + word,
      '0 0 1200 400',
      1200,
      400,
    ),
  );
  const stackedMark = `<g transform="translate(-80 -155) scale(.66)">${flatSymbol(mode)}</g>`;
  const stackedWord = `<path d="${outline.path}" transform="translate(67 505) scale(${526 / outline.width})" fill="${wordColor}"/>`;
  await out(
    `jacket-logo-stacked-${variant}.svg`,
    wrap(
      'Jacket stacked logo',
      'Jacket sleeve symbol above an outlined wordmark.',
      stackedMark + stackedWord,
      '0 0 660 690',
      660,
      690,
    ),
  );
}

await writeFile(
  path.join(root, 'brand-tokens.json'),
  `${JSON.stringify(
    {
      name: 'Jacket',
      descriptor: 'Download and package albums and playlists.',
      tagline: 'From link to neatly packed.',
      colors,
      typography: {
        wordmark:
          'Outlined Avenir Next Demi Bold; supplied SVG is font independent.',
        ui: 'System sans-serif (-apple-system, BlinkMacSystemFont, Segoe UI, sans-serif)',
      },
      icon: {
        canvas: { width: 1254, height: 1254 },
        tile: { x: 130, y: 132, width: 996, height: 990, radius: 178 },
        sleeve: { x: 262, y: 344, width: 470, height: 532 },
        band: { x: 367, y: 344, width: 44, height: 532, widthRatio: 44 / 470 },
        record: {
          centerX: 760,
          centerY: 618,
          radius: 250,
          exposedDiameterRatio: (760 + 250 - 732) / 500,
          grooveRadii: { outer: 186, inner: 120 },
          grooveWidth: 24,
          centerDotRadius: 18,
        },
        smallVariant: {
          maximumSize: 32,
          bandWidth: 48,
          centerDotRadius: 22,
          grooves: 2,
          grooveWidth: 36,
        },
      },
      usage: {
        clearspace: 'At least one band width around the visible logo artwork.',
        minimumLogoWidth: 180,
        minimumSymbolWidth: 32,
      },
    },
    null,
    2,
  )}\n`,
);

if (process.env.SHARP_MODULE) {
  const require = createRequire(import.meta.url);
  const sharp = require(process.env.SHARP_MODULE);
  for (const size of [16, 24, 32, 64, 128, 256, 512, 1024, 1254]) {
    const file = path.join(
      root,
      'svg',
      size <= 32 ? 'jacket-app-icon-small.svg' : 'jacket-app-icon.svg',
    );
    await sharp(file, { density: 144 })
      .resize(size, size)
      .png()
      .toFile(path.join(root, 'png', `jacket-icon-${size}.png`));
  }
  await sharp(path.join(root, 'svg', 'jacket-logo-light.svg'), { density: 144 })
    .resize(1200, 400)
    .png()
    .toFile(path.join(root, 'png', 'jacket-logo-light.png'));

  const preview = (width, height, body) =>
    Buffer.from(
      `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">${body}</svg>`,
    );
  const label = (x, y, text, fill = colors.paper, size = 22) =>
    `<text x="${x}" y="${y}" fill="${fill}" font-family="sans-serif" font-size="${size}">${text}</text>`;
  const wordmark = (x, y, width, fill) =>
    `<path d="${outline.path}" transform="translate(${x} ${y}) scale(${width / outline.width})" fill="${fill}"/>`;
  await sharp(
    preview(
      1200,
      560,
      `<rect width="1200" height="560" fill="${colors.ink}"/><g transform="translate(8 16) scale(.42)">${icon()}</g>${wordmark(605, 190, 430, colors.paper)}${label(607, 365, 'Copper sleeve · Sage grooves', colors.paper, 24)}`,
    ),
  )
    .png()
    .toFile(path.join(root, 'previews', 'jacket-identity.png'));
  await sharp(
    preview(
      1200,
      800,
      `<rect width="1200" height="800" fill="${colors.paper}"/>${label(56, 60, 'Jacket / Brand identity', colors.ink)}<g transform="translate(44 80) scale(.43)">${icon()}</g><g transform="translate(540 145) scale(.4)">${flatSymbol()}</g>${wordmark(654, 535, 390, colors.ink)}${label(56, 724, 'Charcoal · Cream · Copper · Sage', colors.ink)}${[colors.ink, colors.paper, colors.copper, colors.groove].map((color, index) => `<rect x="${650 + index * 100}" y="665" width="78" height="78" rx="12" fill="${color}" stroke="${colors.muted}" stroke-width="1"/>`).join('')}`,
    ),
  )
    .png()
    .toFile(path.join(root, 'previews', 'brand-identity.png'));
  const proof = [];
  for (const [index, size] of [16, 24, 32].entries()) {
    const y = 140 + index * 160;
    const file = path.join(root, 'png', `jacket-icon-${size}.png`);
    proof.push(
      { input: await readFile(file), left: 220 - size / 2, top: y - size / 2 },
      {
        input: await sharp(file)
          .resize(size * 4, size * 4, { kernel: 'nearest' })
          .png()
          .toBuffer(),
        left: 410 - size * 2,
        top: y - size * 2,
      },
    );
  }
  await sharp(
    preview(
      600,
      580,
      `<rect width="600" height="580" fill="${colors.ink}"/>${label(32, 38, 'Jacket / optical icon proof')}${label(184, 76, 'Actual size', colors.paper, 16)}${label(370, 76, '4× pixels', colors.paper, 16)}${[16, 24, 32].map((size, index) => label(32, 146 + index * 160, `${size} px`)).join('')}`,
    ),
  )
    .composite(proof)
    .png()
    .toFile(path.join(root, 'previews', 'jacket-small-sizes.png'));
}
console.log('Jacket SVG identity built in', root);
