import { pastedSourceUrl } from './preferences';

export type SourceIntent = 'song' | 'playlist';

export interface SourceLink {
  url: string;
  songUrl: string | null;
  playlistUrl: string | null;
}

// A video shared from a playlist still describes a song. Keep both targets
// available so the UI can ask before starting a potentially large inspection.
export function sourceLink(input: string): SourceLink | null {
  const supported = pastedSourceUrl(input);
  if (!supported) return null;
  const url = new URL(supported);
  const video =
    url.hostname === 'youtu.be'
      ? url.pathname.slice(1)
      : url.pathname === '/watch'
        ? url.searchParams.get('v')
        : null;
  const validVideo = video && /^[\w-]{11}$/.test(video) ? video : null;
  const list = url.searchParams.get('list');
  const validList = list && /^[\w-]{10,}$/.test(list) ? list : null;
  const origin =
    url.hostname === 'music.youtube.com'
      ? 'https://music.youtube.com'
      : 'https://www.youtube.com';
  return {
    url: supported,
    songUrl: validVideo ? `${origin}/watch?v=${validVideo}` : null,
    playlistUrl: validList
      ? `${origin}/playlist?list=${validList}`
      : validVideo
        ? null
        : supported,
  };
}

export function sourceForIntent(
  input: string,
  intent: SourceIntent = 'song',
): string | null {
  const link = sourceLink(input);
  if (!link) return null;
  return intent === 'playlist'
    ? link.playlistUrl || link.songUrl
    : link.songUrl || link.playlistUrl;
}
