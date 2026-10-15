import assert from 'node:assert/strict';
import { test } from 'node:test';
import { DEFAULT_THEME, defaultVariantColorsResolver } from '@mantine/core';
import { build } from 'esbuild';

const result = await build({
  entryPoints: ['src/theme.ts'],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
});
const { jacketTheme, jacketVariantColors } = await import(
  `data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`
);
const theme = {
  ...DEFAULT_THEME,
  ...jacketTheme,
  colors: { ...DEFAULT_THEME.colors, ...jacketTheme.colors },
};
function luminance(hex) {
  const rgb = hex
    .slice(1)
    .match(/../g)
    .map((part) => Number.parseInt(part, 16) / 255);
  const linear = rgb.map((value) =>
    value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4,
  );
  return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
}
function contrast(a, b) {
  const values = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (values[0] + 0.05) / (values[1] + 0.05);
}

test('secondary copper actions retain readable text on both Jacket surfaces', () => {
  for (const variant of ['subtle', 'transparent']) {
    const colors = jacketVariantColors({ theme, color: 'copper', variant });
    for (const surface of ['#f4efe5', '#fffcf7']) {
      assert.ok(
        contrast(colors.color, surface) >= 4.5,
        `${variant} text on ${surface}`,
      );
      assert.ok(
        contrast(colors.hoverColor, surface) >= 4.5,
        `${variant} hovered text on ${surface}`,
      );
    }
  }
});

test('filled copper actions keep readable ink labels and other colors retain their resolver', () => {
  const filled = jacketVariantColors({
    theme,
    color: 'copper',
    variant: 'filled',
  });
  assert.ok(contrast(filled.color, filled.background) >= 4.5);
  assert.ok(contrast(filled.color, filled.hover) >= 4.5);
  const gray = { theme, color: 'gray', variant: 'subtle' };
  assert.deepEqual(
    jacketVariantColors(gray),
    defaultVariantColorsResolver(gray),
  );
});
