import type {
  AudioFormat,
  CollectionMode,
  ExportPlan,
  Inspection,
  Track,
} from './domain';

export interface ExportPreferences {
  title: string;
  mode: CollectionMode;
  albumTitle: string;
  albumArtist: string;
  year: number | null;
  format: AudioFormat;
  artworkMode: 'perTrack' | 'shared' | 'none';
  sharedArtwork: string | null;
}

export function readPreference(key: string): string | null {
  try {
    // Keep the original storage namespace so Jacket retains existing choices.
    return localStorage.getItem(`audio-dl.${key}`);
  } catch {
    return null;
  }
}

export function writePreference(key: string, value: string) {
  try {
    localStorage.setItem(`audio-dl.${key}`, value);
  } catch {
    // Storage can be unavailable; downloading still works for this session.
  }
}

export function audioFormat(value: string | null): AudioFormat {
  return value === 'mp3' || value === 'preserve' ? value : 'compatible';
}

// Only complete supported links trigger paste inspection. Typing remains
// explicit, and pasting text or a partial link leaves the form editable.
export function pastedSourceUrl(input: string): string | null {
  try {
    const url = new URL(input.trim());
    if (url.protocol !== 'https:' || url.username || url.password || url.port)
      return null;
    if (
      ![
        'youtube.com',
        'www.youtube.com',
        'music.youtube.com',
        'm.youtube.com',
        'youtu.be',
      ].includes(url.hostname)
    )
      return null;
    const video = url.searchParams.get('v');
    const playlist = url.searchParams.get('list');
    const complete =
      url.hostname === 'youtu.be'
        ? /^\/[\w-]{11}$/.test(url.pathname)
        : (url.pathname === '/watch' && !!video && /^[\w-]{11}$/.test(video)) ||
          ((url.pathname === '/playlist' || url.pathname === '/watch') &&
            !!playlist &&
            /^[\w-]{10,}$/.test(playlist)) ||
          (url.hostname === 'music.youtube.com' &&
            /^\/browse\/MP[\w-]+$/.test(url.pathname));
    return complete ? url.toString() : null;
  } catch {
    return null;
  }
}

export function recoveryPreferences(plan: ExportPlan): ExportPreferences {
  return {
    title: plan.title,
    mode: plan.mode,
    format: plan.format,
    albumTitle: shared(plan.tracks.map((track) => track.metadata.album)) || '',
    albumArtist:
      shared(plan.tracks.map((track) => track.metadata.albumArtist)) || '',
    year: shared(plan.tracks.map((track) => track.metadata.releaseYear)),
    artworkMode: plan.artworkMode,
    sharedArtwork: plan.sharedArtworkUrl,
  };
}

// A shared field is safe to prefill only when every track provides the same
// value. Unknown or conflicting values retain each track's individual tags.
function shared<T>(values: T[]): T | null {
  return values.length && values.every((value) => value === values[0])
    ? values[0]
    : null;
}

export function inspectionPreferences(
  value: Inspection,
  format: AudioFormat,
): ExportPreferences {
  const albumTitle = shared(
    value.tracks.map((track) => track.metadata.album.trim()),
  );
  return {
    title:
      value.suggestedMode === 'album' && albumTitle ? albumTitle : value.title,
    mode: value.suggestedMode,
    albumTitle:
      albumTitle || (value.suggestedMode === 'album' ? value.title : ''),
    albumArtist:
      shared(
        value.tracks.map((track) =>
          (track.metadata.albumArtist || track.metadata.artist).trim(),
        ),
      ) || '',
    year:
      (value.suggestedMode === 'album' ? value.albumReleaseYear : null) ??
      shared(value.tracks.map((track) => track.metadata.releaseYear)),
    format,
    artworkMode: value.suggestedMode === 'album' ? 'shared' : 'perTrack',
    sharedArtwork:
      value.artworkUrl ||
      value.tracks.find((track) => track.artworkUrl)?.artworkUrl ||
      null,
  };
}

export function updateAlbumPreferences(
  previous: ExportPreferences,
  patch: Partial<
    Pick<ExportPreferences, 'albumTitle' | 'albumArtist' | 'year'>
  >,
  sourceTitle: string,
): ExportPreferences {
  const followsAlbum =
    previous.title === (previous.albumTitle.trim() || sourceTitle) ||
    previous.title === sourceTitle;
  return {
    ...previous,
    ...patch,
    title:
      patch.albumTitle !== undefined && followsAlbum
        ? patch.albumTitle.trim() || sourceTitle
        : previous.title,
  };
}

export function archiveDirectory(path: string): string {
  return path.match(/^(.*[\\/])[^\\/]+$/)?.[1] || '';
}

// biome-ignore lint/suspicious/noControlCharactersInRegex: Archive filenames must remove filesystem-invalid control characters.
const invalidFilenameCharacters = /[<>:"/\\|?*\u0000-\u001f]/g;

export function archiveFilename(title: string): string {
  const name = title
    .replace(invalidFilenameCharacters, '_')
    .slice(0, 100)
    .trim();
  return `${name || 'Audio collection'}.zip`;
}

// Explicitly number all album entries in their current source order. Reset at
// each disc boundary and retain numbering gaps when tracks are deselected.
export function numberAlbumTracks(tracks: Track[]): Track[] {
  const nextByDisc = new Map<number, number>();
  return tracks.map((track) => {
    const disc = track.metadata.discNumber ?? 1;
    const number = (nextByDisc.get(disc) ?? 0) + 1;
    nextByDisc.set(disc, number);
    return { ...track, metadata: { ...track.metadata, trackNumber: number } };
  });
}

// Fill defaults before selection, so omitting a track from the download does
// not shift the remaining album numbers. Existing tags and source metadata
// stay intact; only missing numbers use the position within their disc.
export function withAlbumTrackNumbers(
  inspection: Inspection,
  mode: CollectionMode = inspection.suggestedMode,
): Inspection {
  if (mode !== 'album') return inspection;
  const ordered = numberAlbumTracks(inspection.tracks);
  return {
    ...inspection,
    tracks: inspection.tracks.map((track, index) =>
      track.metadata.trackNumber === null ? ordered[index] : track,
    ),
  };
}
