import type { DownloaderStatus } from './domain';

// One attempt per known release and session. A new check or settings change
// explicitly permits another attempt; offline failures cannot create a loop.
export function automaticUpdateKey(status: DownloaderStatus | null) {
  return status?.mode === 'managed' &&
    status.autoUpdate &&
    status.updateAvailable &&
    !status.downgrade &&
    (!status.error || status.pendingActivation)
    ? `${status.channel}:${status.latestVersion}`
    : null;
}

export type DownloaderPhase = 'checking' | 'downloading' | 'verifying' | null;
export function downloaderPresentation(
  status: DownloaderStatus | null,
  phase: DownloaderPhase,
  error: string | null,
  cancelling = false,
) {
  if (cancelling)
    return { label: 'Stopping update…', tone: 'working' } as const;
  if (phase)
    return {
      label:
        phase === 'downloading'
          ? 'Updating in the background…'
          : phase === 'verifying'
            ? 'Verifying update…'
            : 'Checking yt-dlp…',
      tone: 'working',
    } as const;
  if (error || status?.error)
    return {
      label:
        status?.installedVersion === 'Unavailable'
          ? 'Downloader needs attention'
          : 'Using current version · needs attention',
      tone: 'attention',
    } as const;
  if (status?.mode === 'local')
    return { label: 'Using your local install', tone: 'ready' } as const;
  if (status?.pendingActivation)
    return {
      label: 'Update ready · waiting for your download',
      tone: 'working',
    } as const;
  if (status?.downgrade)
    return {
      label: 'Using a newer version than this channel',
      tone: 'ready',
    } as const;
  if (status?.updateAvailable)
    return {
      label: status.autoUpdate
        ? 'Update queued · waiting for your download'
        : 'Update available',
      tone: 'attention',
    } as const;
  if (!status)
    return { label: 'yt-dlp status unavailable', tone: 'muted' } as const;
  return {
    label: status.autoUpdate
      ? status.latestVersion
        ? 'Up to date · updates automatic'
        : 'Ready · updates automatic'
      : 'Ready · automatic updates paused',
    tone: 'ready',
  } as const;
}
