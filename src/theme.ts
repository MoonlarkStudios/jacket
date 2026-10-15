import {
  Button,
  createTheme,
  defaultVariantColorsResolver,
  Modal,
  Tooltip,
  type VariantColorsResolver,
} from '@mantine/core';

export const jacketVariantColors: VariantColorsResolver = (input) => {
  const colors = defaultVariantColorsResolver(input);
  if ((input.color || input.theme.primaryColor) !== 'copper') return colors;
  if (input.variant === 'filled') {
    return {
      ...colors,
      background: '#D47749',
      hover: '#DF8A5D',
      color: '#282725',
    };
  }
  if (input.variant === 'subtle' || input.variant === 'transparent') {
    return {
      ...colors,
      color: '#914323',
      hoverColor: '#70331c',
    };
  }
  return colors;
};

export const jacketTheme = createTheme({
  primaryColor: 'copper',
  primaryShade: 8,
  black: '#282725',
  white: '#fffcf7',
  variantColorResolver: jacketVariantColors,
  fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
  headings: { fontFamily: 'inherit', fontWeight: '600' },
  defaultRadius: 8,
  components: {
    Button: Button.extend({ defaultProps: { radius: 8, variant: 'filled' } }),
    Modal: Modal.extend({
      defaultProps: {
        radius: 14,
        padding: 24,
        overlayProps: { backgroundOpacity: 0.2, blur: 3 },
      },
    }),
    Tooltip: Tooltip.extend({ defaultProps: { openDelay: 350 } }),
  },
  colors: {
    gray: [
      '#faf7f0',
      '#f1eade',
      '#e5ddd0',
      '#d6cbbb',
      '#b9aea0',
      '#9a8f80',
      '#716b62',
      '#595349',
      '#403c35',
      '#282725',
    ],
    copper: [
      '#fff5ee',
      '#fae8db',
      '#f2cfb8',
      '#eab293',
      '#e19871',
      '#df8a5d',
      '#d47749',
      '#b15d35',
      '#914323',
      '#70331c',
    ],
  },
});
