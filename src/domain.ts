export type CollectionMode = 'album' | 'collection';
export type AudioFormat = 'compatible' | 'preserve' | 'mp3';
export type ArtworkMode = 'perTrack' | 'shared' | 'none';
export type OutputKind = 'zip' | 'folder';
export type Browser = 'chrome' | 'firefox' | 'safari' | 'edge';

export interface Metadata {
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  trackNumber: number | null;
  discNumber: number | null;
  releaseYear: number | null;
}

export interface Track {
  id: string;
  sourceUrl: string;
  uploader: string;
  duration: number | null;
  artworkUrl: string | null;
  sourceMetadata: Metadata;
  metadata: Metadata;
}

export interface Inspection {
  title: string;
  sourceUrl: string;
  suggestedMode: CollectionMode;
  artworkUrl: string | null;
  albumReleaseYear?: number | null;
  tracks: Track[];
  warnings: string[];
}

export interface InspectionEvent {
  inspectionId: string;
  title: string;
  sourceUrl: string;
  suggestedMode: CollectionMode;
  artworkUrl: string | null;
  index: number;
  total: number | null;
  track: Track;
}

export interface ExportResult {
  outputPath: string;
  trackCount: number;
  warnings: string[];
  outputKind?: OutputKind;
  playlistPath?: string | null;
}

export interface JobEvent {
  jobId: string;
  stage: string;
  message: string;
  completed: number;
  total: number;
  result: ExportResult | null;
  error: string | null;
}

export interface ExportPlan {
  title: string;
  sourceUrl: string;
  mode: CollectionMode;
  format: AudioFormat;
  artworkMode: ArtworkMode;
  sharedArtworkUrl: string | null;
  sharedArtworkAsset?: string | null;
  outputKind?: OutputKind;
  auth: {
    browser: Browser | null;
    browserProfile?: string | null;
    managedSession?: string | null;
  };
  tracks: Track[];
}

export interface ReviewCompletionIdentity {
  sessionId: string;
  revision: number;
}

export interface ArtworkAsset {
  id: string;
  previewDataUrl: string;
  width: number;
  height: number;
}

export interface ArtworkCrop {
  x: number;
  y: number;
  size: number;
}

export interface ExportPreview {
  rootName: string;
  outputKind: OutputKind;
  tracks: {
    relativePath: string;
    metadata: Metadata;
    artworkPath: string | null;
    extensionProvisional: boolean;
  }[];
  files: string[];
  warnings: string[];
}

export interface PendingExport {
  jobId: string;
  plan: ExportPlan;
  outputPath: string;
  progress: {
    stage: string;
    message: string;
    completed: number;
    total: number;
  };
  error: string | null;
  reviewIdentity?: ReviewCompletionIdentity | null;
}

export type DownloaderChannel = 'stable' | 'nightly';
export type DownloaderMode = 'managed' | 'local';
export interface DownloaderPreferences {
  mode: DownloaderMode;
  autoUpdate: boolean;
  localPath: string | null;
}
export interface DownloaderStatus extends DownloaderPreferences {
  installedVersion: string;
  detectedLocalPath: string | null;
  pendingActivation: boolean;
  latestVersion: string | null;
  channel: DownloaderChannel;
  checkedAt: number | null;
  updateAvailable: boolean;
  downgrade: boolean;
  usingUpdatedCopy: boolean;
  error: string | null;
}

export function formatDuration(seconds: number | null): string {
  if (seconds === null) return '—';
  const rounded = Math.max(0, Math.round(seconds));
  return `${Math.floor(rounded / 60)}:${String(rounded % 60).padStart(2, '0')}`;
}

export function selectedTracks(
  tracks: Track[],
  selected: Set<number>,
  mode: CollectionMode,
  album: { title: string; artist: string; year: number | null },
): Track[] {
  return tracks
    .filter((_, index) => selected.has(index))
    .map((track) => ({
      ...track,
      metadata:
        mode === 'album'
          ? {
              ...track.metadata,
              album: album.title.trim() || track.metadata.album,
              albumArtist: album.artist.trim() || track.metadata.albumArtist,
              releaseYear: album.year ?? track.metadata.releaseYear,
            }
          : { ...track.metadata },
    }));
}
