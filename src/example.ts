import type { Inspection, Metadata } from './domain';

const metadata = (title: string, trackNumber: number): Metadata => ({
  title,
  artist: 'Juniper',
  album: 'A Quiet Afternoon',
  albumArtist: 'Juniper',
  trackNumber,
  discNumber: null,
  releaseYear: 2026,
});

// Explicit development fixture for reviewing the interface. It never starts
// acquisition and does not impersonate a successfully inspected source.
export const example: Inspection = {
  title: 'A Quiet Afternoon',
  sourceUrl: 'https://music.youtube.com/browse/MP-example',
  suggestedMode: 'album',
  artworkUrl: '/example-cover.svg',
  warnings: [
    'This is example content for the browser preview. Open the desktop app to inspect and export real playlists.',
  ],
  tracks: [
    'Through the Window',
    'Garden Gate',
    'The Long Way Home',
    'Still Water',
    'Somewhere Familiar',
    'Late Afternoon',
    'Soft Landing',
    'Small Hours',
    'Beyond the Pines',
    'An Open Door',
    'The Things We Keep',
    'Everything in Its Place',
  ].map((title, index) => ({
    id: `example-${index}`,
    sourceUrl: 'https://www.youtube.com/watch?v=example',
    uploader: 'Juniper',
    duration: [218, 184, 256, 202, 193, 247, 214, 183, 259, 197, 228, 242][
      index
    ],
    artworkUrl: '/example-cover.svg',
    sourceMetadata: metadata(title, index + 1),
    metadata: metadata(title, index + 1),
  })),
};
