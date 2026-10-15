import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const outline = JSON.parse(
  await readFile(new URL('./wordmark-outline.json', import.meta.url), 'utf8'),
);
const colors = {
  ink: '#282725',
  paper: '#F4EFE5',
  copper: '#D47749',
  groove: '#595B58',
  muted: '#787369',
};
const sleeve =
  'M258 313H762Q774 313 774 325V463C710 485 678 537 678 600C678 663 710 715 774 737V861Q774 873 762 873H258Q246 873 246 861V325Q246 313 258 313Z';
const rectBand = '<rect x="366" y="310" width="85" height="566" rx="3"/>';
const wrap = (title, desc, body, box = '0 0 1254 1254', w = 1254, h = 1254) =>
  `<?xml version="1.0" encoding="UTF-8"?>\n<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="${box}" role="img" aria-labelledby="title desc"><title id="title">${title}</title><desc id="desc">${desc}</desc>${body}</svg>\n`;
const defs = `<defs>
 <linearGradient id="tile" x1="120" y1="100" x2="1100" y2="1170" gradientUnits="userSpaceOnUse"><stop stop-color="#3C3D3C"/><stop offset=".44" stop-color="#2C2D2C"/><stop offset="1" stop-color="#222322"/></linearGradient>
 <linearGradient id="vinyl" x1="650" y1="345" x2="1010" y2="860" gradientUnits="userSpaceOnUse"><stop stop-color="#363735"/><stop offset="1" stop-color="#1F201F"/></linearGradient>
 <linearGradient id="sleeve" x1="246" y1="313" x2="774" y2="873" gradientUnits="userSpaceOnUse"><stop stop-color="#E08955"/><stop offset=".5" stop-color="#D47749"/><stop offset="1" stop-color="#C96C3E"/></linearGradient>
 <linearGradient id="paper" x1="366" y1="310" x2="451" y2="876" gradientUnits="userSpaceOnUse"><stop stop-color="#FFF9EF"/><stop offset=".6" stop-color="#F4EFE5"/><stop offset="1" stop-color="#E9E2D5"/></linearGradient>
 <radialGradient id="label" cx=".35" cy=".25" r=".85"><stop stop-color="#FFFCF5"/><stop offset="1" stop-color="#EEE8DC"/></radialGradient>
 <filter id="depth" x="-15%" y="-15%" width="140%" height="150%" color-interpolation-filters="sRGB"><feDropShadow dx="0" dy="10" stdDeviation="11" flood-color="#000" flood-opacity=".25"/></filter>
 <filter id="band-depth" x="-35%" y="-10%" width="200%" height="130%" color-interpolation-filters="sRGB"><feDropShadow dx="4" dy="4" stdDeviation="4" flood-color="#000" flood-opacity=".23"/></filter>
 <clipPath id="sleeve-clip"><path d="${sleeve}"/></clipPath>
</defs>`;

function flatSymbol(mode = 'color') {
  if (mode === 'mono')
    return `<defs><mask id="knockout" maskUnits="userSpaceOnUse" x="206" y="286" width="850" height="614"><g fill="#fff"><circle cx="760" cy="600" r="266"/><path d="${sleeve}"/></g><g fill="#000">${rectBand}<circle cx="760" cy="600" r="86"/></g><circle cx="760" cy="600" r="16" fill="#fff"/><path d="M774 341V463M774 737V853" stroke="#000" stroke-width="9"/></mask></defs><rect x="206" y="286" width="850" height="614" fill="currentColor" mask="url(#knockout)"/>`;
  const dark = mode === 'dark';
  return `<circle cx="760" cy="600" r="266" fill="${dark ? '#4D504D' : colors.ink}"/><circle cx="760" cy="600" r="86" fill="${colors.paper}"/><circle cx="760" cy="600" r="16" fill="${colors.ink}"/><path d="${sleeve}" fill="${colors.copper}"/><g fill="${colors.paper}">${rectBand}</g>`;
}

function icon(small = false) {
  if (small)
    return `<rect x="80" y="86" width="1094" height="1070" rx="174" fill="${colors.ink}" stroke="#686B66" stroke-width="9"/><circle cx="760" cy="600" r="266" fill="#484B47" stroke="#70746D" stroke-width="11"/><circle cx="760" cy="600" r="91" fill="#FFF9EF"/><circle cx="760" cy="600" r="22" fill="${colors.ink}"/><path d="${sleeve}" fill="${colors.copper}"/><rect x="364" y="310" width="89" height="566" rx="3" fill="#FFF9EF"/>`;
  return `${defs}<rect x="80" y="86" width="1094" height="1070" rx="174" fill="url(#tile)" stroke="#4B4D49" stroke-width="3"/><g filter="url(#depth)"><circle cx="760" cy="600" r="266" fill="url(#vinyl)" stroke="#464843" stroke-width="4"/><circle cx="760" cy="600" r="197" fill="none" stroke="#74766C" stroke-opacity=".28" stroke-width="5"/><circle cx="760" cy="600" r="86" fill="url(#label)"/><circle cx="760" cy="600" r="16" fill="#232421"/><path d="${sleeve}" fill="url(#sleeve)" stroke="#EEA071" stroke-opacity=".48" stroke-width="2"/></g><g filter="url(#band-depth)"><rect x="366" y="310" width="85" height="566" rx="3" fill="url(#paper)" stroke="#FFFCF5" stroke-opacity=".55" stroke-width="2"/></g>`;
}

const markDesc =
  'An orange record sleeve, narrow ivory vertical packaging band, and a black record exposed at the right.';
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
    'Optically adjusted Jacket icon: wider band, brighter record edge, no groove or shadow.',
    icon(true),
  ),
);
for (const size of [16, 32, 64, 128, 256, 512, 1024]) {
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
        tile: { x: 80, y: 86, width: 1094, height: 1070, radius: 174 },
        sleeve: { x: 246, y: 313, width: 528, height: 560 },
        band: { x: 366, y: 310, width: 85, height: 566, widthRatio: 85 / 528 },
        record: {
          centerX: 760,
          centerY: 600,
          radius: 266,
          grooveRadius: 197,
          labelRadius: 86,
        },
        smallVariant: {
          maximumSize: 32,
          bandWidth: 89,
          labelRadius: 91,
          holeRadius: 22,
          grooves: 0,
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
  for (const size of [16, 32, 64, 128, 256, 512, 1024, 1254]) {
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
}
console.log('Jacket SVG identity built in', root);
