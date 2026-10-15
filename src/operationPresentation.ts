import type { JobEvent } from './domain';

export type OperationErrorKind =
  | 'session'
  | 'destination'
  | 'storage'
  | 'network'
  | 'unavailable'
  | 'tools'
  | 'source'
  | 'unknown';

export interface OperationErrorPresentation {
  kind: OperationErrorKind;
  title: string;
  guidance: string;
}

/** Only offer a browser session for errors that actually mention account access. */
export function presentOperationError(
  error: string | null,
): OperationErrorPresentation {
  const detail = error ?? '';
  if (
    /valid YouTube.*(?:URL|link)|HTTPS YouTube|unsupported (?:URL|link)/i.test(
      detail,
    )
  ) {
    return {
      kind: 'source',
      title: 'Check the YouTube link',
      guidance:
        'Paste the full HTTPS link to a YouTube or YouTube Music album, playlist, or video, then try again.',
    };
  }
  if (
    /sign[ -]?in|log[ -]?in|authentication|cookies?|keychain|private (?:video|playlist)|age[ -]restrict|members[ -]only|session (?:expired|unavailable)|(?:invalid|expired|reconnect).*session|session.*(?:invalid|expired|reconnect)/i.test(
      detail,
    )
  ) {
    return {
      kind: 'session',
      title: 'YouTube needs account access',
      guidance:
        'Check your browser session in Settings. Use an account that can open this link, then try again.',
    };
  }
  if (
    /no space left|disk (?:is )?full|not enough (?:disk )?space|enospc|os error 28/i.test(
      detail,
    )
  ) {
    return {
      kind: 'storage',
      title: 'There isn’t enough disk space',
      guidance:
        'Free up space on this computer and the destination drive, then try again.',
    };
  }
  if (
    /(?:ffmpeg|ffprobe|yt-dlp|downloader).*(?:missing|not installed|no such file)|(?:ffmpeg|ffprobe|yt-dlp|downloader) (?:is )?(?:not found|unavailable)|(?:missing|cannot find|could not find).*(?:ffmpeg|ffprobe|yt-dlp|downloader)|requested format is not available/i.test(
      detail,
    )
  ) {
    return {
      kind: 'tools',
      title: 'The audio downloader needs attention',
      guidance: 'Check the audio downloader in Settings, then try again.',
    };
  }
  if (
    /permission denied|access (?:is )?denied|read-only file system|os error (?:13|30)\b|cannot (?:create|write)|could not (?:create|write|save).*?(?:file|folder|archive)|destination.*(?:missing|unavailable|not found)|no such file or directory/i.test(
      detail,
    )
  ) {
    return {
      kind: 'destination',
      title: 'Jacket couldn’t access a file or folder',
      guidance:
        'Check that the destination folder is available and writable, then try again. You can also choose another folder.',
    };
  }
  if (
    /(?:video|playlist) (?:is )?(?:unavailable|not available)|removed by|deleted|not available in your country|not available in your region|geo.?restrict|http (?:error )?404\b/i.test(
      detail,
    )
  ) {
    return {
      kind: 'unavailable',
      title: 'A source is no longer available',
      guidance:
        'Check that the link opens on YouTube. Choose an available link, or remove unavailable tracks from your selection and try again.',
    };
  }
  if (
    /timed? ?out|timeout|network|connection|dns|unable to resolve|temporary failure|http (?:error )?(?:403|408|429|5\d\d)\b|too many requests|remote end closed|urlopen error/i.test(
      detail,
    )
  ) {
    return {
      kind: 'network',
      title: 'The connection was interrupted',
      guidance:
        'Check your internet connection, then try again. If YouTube is limiting requests, wait a little before retrying.',
    };
  }
  return {
    kind: 'unknown',
    title: 'Jacket couldn’t continue',
    guidance:
      'Try again. If the problem continues, open the details below for more information.',
  };
}

export interface DownloadPresentation {
  title: string;
  currentTrack: string | null;
  completed: number;
  total: number;
  percentage: number;
  indeterminate: boolean;
  countLabel: string;
}

export function getDownloadPresentation(
  progress: JobEvent,
): DownloadPresentation {
  const total = Number.isFinite(progress.total)
    ? Math.max(0, Math.floor(progress.total))
    : 0;
  const completed = Number.isFinite(progress.completed)
    ? Math.min(total, Math.max(0, Math.floor(progress.completed)))
    : 0;
  const next = Math.min(completed + 1, total);
  const countLabel = `${completed} of ${total} ${total === 1 ? 'track' : 'tracks'} prepared`;
  let title = 'Starting download…';
  let currentTrack: string | null = null;
  if (progress.stage === 'packaging') {
    title = 'Creating ZIP…';
  } else if (progress.stage === 'downloading') {
    title = total
      ? `Downloading track ${next} of ${total}`
      : 'Downloading audio…';
    currentTrack = progress.message.startsWith('Retrying ')
      ? progress.message
          .replace(/^Retrying /, '')
          .replace(/ after a connection problem$/, '')
      : progress.message.replace(/^Downloading /, '');
  } else if (progress.stage === 'preparing') {
    title = progress.message.startsWith('Prepared ')
      ? completed === total
        ? 'All tracks prepared'
        : `Prepared track ${completed} of ${total}`
      : `Preparing track ${next} of ${total}`;
    currentTrack = progress.message.replace(/^(?:Tagging|Prepared) /, '');
  }
  return {
    title,
    currentTrack: currentTrack || null,
    completed,
    total,
    percentage: total ? (completed / total) * 100 : 0,
    indeterminate:
      progress.stage === 'packaging' || progress.stage === 'queued',
    countLabel,
  };
}
