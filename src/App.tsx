import {
  ActionIcon,
  Alert,
  Button,
  Checkbox,
  Group,
  Image,
  Text,
  TextInput,
  Title,
  Tooltip,
} from '@mantine/core';
import {
  IconArrowRight,
  IconCheck,
  IconChevronDown,
  IconDownload,
  IconEdit,
  IconFolder,
  IconLoader2,
  IconPlus,
  IconRotateClockwise,
  IconSearch,
  IconSettings,
  IconVinyl,
  IconX,
} from '@tabler/icons-react';
import { useEffect, useRef, useState } from 'react';
import { BulkTrackEditor } from './components/BulkTrackEditor';
import {
  DownloadFeedback,
  SourceErrorMessage,
} from './components/DownloadFeedback';
import { ExportPreviewDialog } from './components/ExportPreviewDialog';
import { SourceField } from './components/SourceField';
import {
  type Dialog,
  TrackEditor,
  WorkspaceDialogs,
} from './components/WorkspaceDialogs';
import {
  artworkAsset,
  desktopAvailable,
  openPlaylist,
  revealArchive,
} from './desktop';
import { formatDuration, type Track } from './domain';
import { example } from './example';
import { archiveDirectory, archiveFilename } from './preferences';
import { useDesktopShell } from './useDesktopShell';
import { useWorkspace, type Workspace } from './useWorkspace';

function listeningTime(seconds: number) {
  const minutes = Math.round(seconds / 60);
  return minutes >= 60
    ? `${Math.floor(minutes / 60)} hr ${minutes % 60} min`
    : `${minutes} min`;
}
const formatNames = {
  compatible: 'M4A / MP3',
  preserve: 'Original codec',
  mp3: 'MP3',
};
const artworkNames = {
  perTrack: 'Track artwork',
  shared: 'Shared artwork',
  none: 'No artwork',
};

export default function App() {
  const w = useWorkspace();
  const [dialog, setDialog] = useState<Dialog>(null);
  const [focusDownloader, setFocusDownloader] = useState(false);
  const [editing, setEditing] = useState<{
    index: number;
    track: Track;
  } | null>(null);
  const [query, setQuery] = useState('');
  const [failedCover, setFailedCover] = useState<string | null>(null);
  const [importedCover, setImportedCover] = useState<{
    id: string;
    url: string;
  } | null>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const sourceInput = useRef<HTMLInputElement>(null);
  const editorTrigger = useRef<HTMLButtonElement | null>(null);
  const dialogTrigger = useRef<HTMLElement | null>(null);
  const downloadButton = useRef<HTMLButtonElement | null>(null);
  const wasChoosing = useRef(false);
  const inspection = w.inspection;
  const reviewReady = w.inspectionStatus === 'complete';
  const p = w.preferences;
  const progress = w.progress;
  const success = progress?.stage === 'complete' ? progress.result : null;
  const exportNotes = [...new Set(success?.warnings ?? [])];
  const retry = ['failed', 'cancelled', 'interrupted'].includes(
    progress?.stage || '',
  );
  const canExport =
    desktopAvailable &&
    !w.exampleMode &&
    w.toolsReady &&
    w.eventsReady &&
    w.draftReady &&
    !!w.chosen.length &&
    !!p.title.trim() &&
    reviewReady &&
    !w.busy;
  const totalDuration = w.chosen.reduce(
    (total, track) => total + (track.duration ?? 0),
    0,
  );
  const visibleTracks =
    inspection?.tracks
      .map((track, index) => ({ track, index }))
      .filter(({ track }) =>
        `${track.metadata.title} ${track.metadata.artist} ${track.metadata.album}`
          .toLocaleLowerCase()
          .includes(query.trim().toLocaleLowerCase()),
      ) ?? [];
  const artistNames = [
    ...new Set(
      inspection?.tracks
        .map((track) => track.metadata.albumArtist || track.metadata.artist)
        .filter(Boolean),
    ),
  ];
  const displayArtist =
    p.mode === 'album' && p.albumArtist.trim()
      ? p.albumArtist
      : artistNames.length === 1
        ? artistNames[0]
        : null;
  const cover =
    p.artworkMode === 'none'
      ? null
      : p.artworkMode === 'shared'
        ? p.sharedArtworkAsset
          ? importedCover?.id === p.sharedArtworkAsset
            ? importedCover.url
            : null
          : p.sharedArtwork
        : inspection?.artworkUrl ||
          inspection?.tracks.find((track) => track.artworkUrl)?.artworkUrl;
  const mac = navigator.platform.toLowerCase().includes('mac');
  const unavailableNotes =
    inspection?.warnings.filter((note) =>
      /unavailable|incomplete entries/i.test(note),
    ) ?? [];
  const sourceNotes =
    inspection?.warnings.filter((note) => !unavailableNotes.includes(note)) ??
    [];
  const missingNumbers =
    inspection?.tracks.filter((track) => track.metadata.trackNumber === null)
      .length ?? 0;
  const missingArtists = w.chosen.filter(
    (track) => !track.metadata.artist.trim(),
  ).length;
  const saveFolder = w.savePath
    ? archiveDirectory(w.savePath)
        .replace(/[\\/]$/, '')
        .split(/[\\/]/)
        .pop()
    : null;

  const sourceUrl = inspection?.sourceUrl;
  useEffect(() => {
    const id = p.sharedArtworkAsset;
    if (!id || !desktopAvailable) {
      setImportedCover(null);
      return;
    }
    let disposed = false;
    void artworkAsset(id)
      .then((asset) => {
        if (!disposed) setImportedCover({ id, url: asset.previewDataUrl });
      })
      .catch((error) => {
        if (!disposed) w.setError(String(error));
      });
    return () => {
      disposed = true;
    };
  }, [p.sharedArtworkAsset, w.setError]);
  useEffect(() => {
    if (sourceUrl) heading.current?.focus();
    else sourceInput.current?.focus();
    setQuery('');
    setFailedCover(null);
  }, [sourceUrl]);

  useEffect(() => {
    if (!inspection && !w.loading && w.toolsReady && !dialog) {
      sourceInput.current?.focus();
    }
  }, [inspection, w.loading, w.toolsReady, dialog]);

  useEffect(() => {
    // The native save panel takes focus outside the webview. Return it to the
    // download action when a chooser closes without starting a download.
    if (wasChoosing.current && !w.choosing && !w.exporting) {
      requestAnimationFrame(() => {
        if (!downloadButton.current?.disabled) downloadButton.current?.focus();
      });
    }
    wasChoosing.current = w.choosing;
  }, [w.choosing, w.exporting]);

  useEffect(() => {
    function shortcut(event: KeyboardEvent) {
      if (
        (event.metaKey || event.ctrlKey) &&
        event.key === 'Enter' &&
        !dialog &&
        !editing &&
        canExport &&
        !success
      ) {
        event.preventDefault();
        void w.exportZip();
      }
    }
    document.addEventListener('keydown', shortcut);
    return () => document.removeEventListener('keydown', shortcut);
  }, [dialog, editing, canExport, success, w.exportZip]);

  useDesktopShell(
    w,
    () => openDialog('settings'),
    newPlaylist,
    !!editing ||
      dialog === 'details' ||
      dialog === 'artwork' ||
      dialog === 'export' ||
      dialog === 'bulk' ||
      dialog === 'preview',
    () => openDialog('authPrototype'),
  );

  async function newPlaylist() {
    if (!(await w.reset())) return false;
    setQuery('');
    setDialog(null);
    requestAnimationFrame(() => sourceInput.current?.focus());
    return true;
  }

  function undoNewDownload() {
    if (!w.undoReset()) return;
    setQuery('');
    requestAnimationFrame(() => heading.current?.focus());
  }

  function openDialog(next: Exclude<Dialog, null>, trigger?: HTMLElement) {
    if (!dialog) {
      if (next === 'settings') setFocusDownloader(false);
      const focused = document.activeElement;
      // WebKit does not focus buttons on click. Keep the actual trigger rather
      // than relying on activeElement, which can still be the page body.
      dialogTrigger.current =
        trigger ??
        (focused instanceof HTMLElement &&
        focused !== document.body &&
        focused !== document.documentElement
          ? focused
          : null);
    }
    setDialog(next);
  }

  function closeDialog() {
    setDialog(null);
    requestAnimationFrame(() => {
      const target = dialogTrigger.current;
      if (target?.isConnected && !target.matches(':disabled')) target.focus();
      else if (inspection) heading.current?.focus();
      else sourceInput.current?.focus();
    });
  }

  function closeEditor() {
    setEditing(null);
    requestAnimationFrame(() => editorTrigger.current?.focus());
  }

  return (
    <div className={`application ${inspection ? 'is-reviewing' : ''}`}>
      <header className="app-header">
        <img
          className="wordmark"
          src="/branding/jacket-logo.svg"
          alt="Jacket"
          width={180}
          height={60}
        />
        <Group gap={8}>
          {!desktopAvailable && (
            <Text size="xs" c="dimmed" className="preview-label">
              Browser preview
            </Text>
          )}
          {!inspection &&
            (!desktopAvailable ||
              import.meta.env.DEV ||
              import.meta.env.VITE_UI_PREVIEW === '1') && (
              <Button
                variant="subtle"
                color="gray"
                size="xs"
                onClick={() => w.loadInspection(example, true)}
                disabled={w.busy}
              >
                Try example
              </Button>
            )}
          {w.exampleMode && (
            <span className="example-tag">Example · downloads disabled</span>
          )}
          {inspection && (
            <Button
              variant="subtle"
              color="gray"
              size="compact-sm"
              leftSection={<IconPlus size={16} />}
              disabled={w.busy}
              onClick={() => void newPlaylist()}
            >
              New download
            </Button>
          )}
          <Tooltip label="Settings">
            <ActionIcon
              variant="subtle"
              color="gray"
              size="lg"
              aria-label="Settings"
              onClick={(event) => openDialog('settings', event.currentTarget)}
            >
              <IconSettings size={19} stroke={1.6} />
            </ActionIcon>
          </Tooltip>
        </Group>
      </header>

      <main
        className={`main-content ${inspection ? 'review-content' : 'start-content'}`}
      >
        {!inspection ? (
          <section className="start-panel">
            <div className="start-introduction">
              <Text className="start-eyebrow">FROM LINK TO NEATLY PACKED</Text>
              <Title order={1}>Download an album or playlist.</Title>
              <Text c="dimmed">
                Tagged audio and artwork, ready for your library.
              </Text>
            </div>
            <SourceField
              value={w.url}
              change={w.setUrl}
              submit={(source) => void w.inspect(source)}
              disabled={!desktopAvailable || !w.toolsReady || w.busy}
              loading={w.loading}
              inputRef={sourceInput}
            />
            <div className="source-context">
              <Text size="xs" c="dimmed">
                {w.browser
                  ? 'Using your browser session'
                  : 'Public playlists need no setup'}
              </Text>
              <button
                type="button"
                className="text-action"
                onClick={(event) => openDialog('settings', event.currentTarget)}
              >
                {w.browser ? 'Session settings' : 'Need private access?'}
              </button>
            </div>
            <ReviewUndo workspace={w} undo={undoNewDownload} />
            {w.loading && (
              <div className="inspection-status" role="status">
                <IconLoader2 className="spinner" size={18} />
                <div>
                  <Text size="sm" fw={500}>
                    Finding tracks and artwork
                  </Text>
                  <Text size="xs" c="dimmed">
                    Tracks will appear as their details arrive.
                  </Text>
                </div>
                <Button
                  variant="subtle"
                  size="xs"
                  color="gray"
                  disabled={w.cancelling}
                  onClick={() => void w.cancel()}
                >
                  {w.cancelling ? 'Stopping…' : 'Cancel'}
                </Button>
              </div>
            )}
            {!desktopAvailable && (
              <Text size="xs" c="dimmed" ta="center" mt={24}>
                Open the desktop app to download. Use the example to explore
                this preview.
              </Text>
            )}
            {w.draftStatus === 'failed' &&
              !w.inspection &&
              desktopAvailable && (
                <Button
                  variant="subtle"
                  color="gray"
                  disabled={w.busy}
                  onClick={() => void w.discardDraft()}
                >
                  Discard saved review
                </Button>
              )}
            {w.error && (
              <SourceErrorMessage
                error={w.error}
                dismiss={() => w.setError(null)}
                session={(trigger) => openDialog('settings', trigger)}
                onRetry={
                  w.url.trim() && !w.busy ? () => void w.inspect() : undefined
                }
              />
            )}
            {desktopAvailable && w.dependencies && !w.toolsReady && (
              <Alert
                color="orange"
                mt="lg"
                title="Install the audio tools to get started"
              >
                Missing{' '}
                {w.dependencies
                  .filter((tool) => !tool.available)
                  .map((tool) => tool.name)
                  .join(', ')}
                .{' '}
                <button
                  type="button"
                  className="text-action"
                  onClick={(event) =>
                    openDialog('settings', event.currentTarget)
                  }
                >
                  View tool status
                </button>
              </Alert>
            )}
          </section>
        ) : (
          <>
            <ReviewUndo workspace={w} undo={undoNewDownload} />
            {!w.exampleMode && !success && reviewReady && w.draftStatus && (
              <Group
                justify="space-between"
                gap="xs"
                className="review-persistence"
              >
                <Text size="xs" c="dimmed" role="status">
                  {w.draftStatus === 'saving'
                    ? 'Saving review…'
                    : w.draftStatus === 'saved'
                      ? 'Review saved on this device'
                      : 'Review could not be saved. Keep Jacket open to retain your edits.'}
                </Text>
                <button
                  type="button"
                  className="text-action"
                  disabled={w.busy}
                  onClick={() => void w.discardDraft()}
                >
                  Discard review
                </button>
              </Group>
            )}
            {w.savedReview && (
              <div className="review-undo">
                <div>
                  <Text size="sm" fw={500}>
                    You also have a saved review.
                  </Text>
                  <Text size="xs" c="dimmed">
                    {w.savedReview.title} · {w.savedReview.trackCount} tracks
                  </Text>
                </div>
                <Button
                  variant="default"
                  size="sm"
                  disabled={w.busy}
                  onClick={() => void w.continueSavedReview()}
                >
                  Continue saved review
                </Button>
              </div>
            )}
            <section className="collection-heading">
              <button
                type="button"
                className="collection-cover"
                aria-label="Choose artwork"
                disabled={w.busy || !reviewReady || !!success}
                onClick={(event) => openDialog('artwork', event.currentTarget)}
              >
                {cover && cover !== failedCover ? (
                  <Image
                    src={cover}
                    w="100%"
                    h="100%"
                    fit="cover"
                    onError={() => setFailedCover(cover)}
                    alt={
                      p.mode === 'album' ? 'Album artwork' : 'Playlist artwork'
                    }
                  />
                ) : (
                  <IconVinyl size={48} stroke={1} />
                )}
                {reviewReady && (
                  <span className="cover-edit">
                    <IconEdit size={13} /> Artwork
                  </span>
                )}
              </button>
              <div className="collection-copy">
                <Text className="collection-type">
                  {p.mode === 'album' ? 'ALBUM' : 'PLAYLIST'}
                </Text>
                <Title order={1} ref={heading} tabIndex={-1}>
                  {p.mode === 'album'
                    ? p.albumTitle || inspection.title
                    : p.title || inspection.title}
                </Title>
                <Text size="sm" c="dimmed">
                  {displayArtist && (
                    <>
                      {displayArtist}
                      <span className="meta-separator">·</span>
                    </>
                  )}
                  {p.mode === 'album' && p.year && (
                    <>
                      {p.year}
                      <span className="meta-separator">·</span>
                    </>
                  )}
                  {inspection.tracks.length}{' '}
                  {inspection.tracks.length === 1 ? 'track' : 'tracks'}
                  {w.inspectionStatus !== 'complete' ? (
                    ' found so far'
                  ) : (
                    <>
                      <span className="meta-separator">·</span>
                      {listeningTime(
                        inspection.tracks.reduce(
                          (sum, track) => sum + (track.duration ?? 0),
                          0,
                        ),
                      )}
                    </>
                  )}
                </Text>
              </div>
              <Button
                variant="default"
                size="xs"
                leftSection={<IconEdit size={14} />}
                disabled={w.busy || !reviewReady || !!success}
                onClick={(event) => openDialog('details', event.currentTarget)}
                className="details-button"
              >
                {p.mode === 'album' ? 'Edit album details' : 'Edit details'}
              </Button>
            </section>
            {reviewReady && unavailableNotes.length > 0 && (
              <Alert
                color="orange"
                className="review-notice"
                title="Some tracks aren't available"
              >
                {[...new Set(unavailableNotes)].map((note) => (
                  <Text size="xs" key={note}>
                    {note}
                  </Text>
                ))}
              </Alert>
            )}
            {reviewReady &&
              p.mode === 'album' &&
              (missingNumbers > 0 || !!w.numberingUndo) &&
              !success && (
                <div className="metadata-notice">
                  <Text size="xs" c="dimmed">
                    {w.numberingUndo
                      ? 'Track numbers now follow album order.'
                      : `${missingNumbers} ${missingNumbers === 1 ? 'track is' : 'tracks are'} missing album numbering.`}
                  </Text>
                  <button
                    type="button"
                    className="text-action"
                    disabled={w.busy}
                    onClick={w.numberingUndo ? w.undoNumbering : w.numberTracks}
                    title={
                      w.numberingUndo
                        ? 'Restore previous track numbers'
                        : 'Renumber every track in source order, restarting for each disc'
                    }
                  >
                    {w.numberingUndo
                      ? 'Undo numbering'
                      : 'Number by album order'}
                  </button>
                </div>
              )}
            {w.error && (
              <SourceErrorMessage
                error={w.error}
                dismiss={() => w.setError(null)}
                session={(trigger) => openDialog('settings', trigger)}
              />
            )}
            <section className="track-section" aria-label="Playlist tracks">
              <div className="track-toolbar">
                <Group gap={14}>
                  <Checkbox
                    aria-label={`Select all ${inspection.tracks.length} tracks`}
                    checked={w.selected.size === inspection.tracks.length}
                    indeterminate={
                      w.selected.size > 0 &&
                      w.selected.size < inspection.tracks.length
                    }
                    disabled={w.busy || !reviewReady || !!success}
                    onChange={() =>
                      w.setSelected(
                        w.selected.size === inspection.tracks.length
                          ? new Set()
                          : new Set(inspection.tracks.map((_, index) => index)),
                      )
                    }
                  />
                  <Text size="xs" fw={500} c="dimmed">
                    {!reviewReady
                      ? `${inspection.tracks.length} ${inspection.tracks.length === 1 ? 'track' : 'tracks'} found so far`
                      : w.selected.size === inspection.tracks.length
                        ? 'All tracks selected'
                        : `${w.selected.size} of ${inspection.tracks.length} selected`}
                  </Text>
                  {reviewReady && !success && (
                    <Button
                      size="compact-xs"
                      variant="subtle"
                      color="gray"
                      disabled={w.busy || w.selected.size === 0}
                      onClick={(event) =>
                        openDialog('bulk', event.currentTarget)
                      }
                    >
                      Edit selected tracks
                    </Button>
                  )}
                  {reviewReady && w.bulkUndo && !success && (
                    <button
                      type="button"
                      className="text-action"
                      disabled={w.busy}
                      onClick={w.undoBulk}
                    >
                      Undo edits
                    </button>
                  )}
                </Group>
                {inspection.tracks.length > 8 && (
                  <TextInput
                    placeholder="Find a track"
                    aria-label="Find a track"
                    leftSection={<IconSearch size={15} />}
                    value={query}
                    onChange={(event) => setQuery(event.currentTarget.value)}
                    rightSection={
                      query ? (
                        <ActionIcon
                          variant="subtle"
                          color="gray"
                          size="sm"
                          aria-label="Clear search"
                          onClick={() => setQuery('')}
                        >
                          <IconX size={12} />
                        </ActionIcon>
                      ) : null
                    }
                    variant="unstyled"
                    size="xs"
                    className="track-search"
                  />
                )}
              </div>
              <ol className="track-list">
                {visibleTracks.map(({ track, index }) => (
                  <li
                    key={`${track.id}-${index}`}
                    className={`track-row ${w.selected.has(index) ? '' : 'is-unselected'}`}
                  >
                    <Checkbox
                      aria-label={`Select ${track.metadata.title}`}
                      checked={w.selected.has(index)}
                      disabled={w.busy || !reviewReady || !!success}
                      onChange={() => w.toggle(index)}
                    />
                    <span
                      className="track-number"
                      title={
                        p.mode === 'album'
                          ? 'Album track number'
                          : 'Playlist position'
                      }
                    >
                      {p.mode === 'album'
                        ? track.metadata.trackNumber === null
                          ? '—'
                          : `${track.metadata.discNumber && track.metadata.discNumber > 1 ? `${track.metadata.discNumber}.` : ''}${String(track.metadata.trackNumber).padStart(2, '0')}`
                        : String(index + 1).padStart(2, '0')}
                    </span>
                    <div className="track-copy">
                      <Text size="sm" fw={500} lineClamp={1}>
                        {track.metadata.title}
                      </Text>
                      <Text size="xs" c="dimmed" lineClamp={1}>
                        {track.metadata.artist || 'Add track artist'}
                        {track.metadata.album && p.mode === 'collection'
                          ? ` · ${track.metadata.album}`
                          : ''}
                      </Text>
                    </div>
                    <Text size="xs" c="dimmed" className="track-duration">
                      {formatDuration(track.duration)}
                    </Text>
                    <Tooltip label="Edit metadata">
                      <ActionIcon
                        className="track-edit"
                        variant="subtle"
                        color="gray"
                        size="md"
                        aria-label={`Edit ${track.metadata.title}`}
                        disabled={w.busy || !reviewReady || !!success}
                        onClick={(event) => {
                          editorTrigger.current = event.currentTarget;
                          setEditing({ index, track });
                        }}
                      >
                        <IconEdit size={16} stroke={1.6} />
                      </ActionIcon>
                    </Tooltip>
                  </li>
                ))}
                {!visibleTracks.length && (
                  <li className="no-results">
                    <Text size="sm" c="dimmed">
                      No tracks match “{query}”.
                    </Text>
                    <Button
                      variant="subtle"
                      size="xs"
                      onClick={() => setQuery('')}
                    >
                      Clear search
                    </Button>
                  </li>
                )}
              </ol>
            </section>
            <footer className="export-footer">
              {w.inspectionStatus !== 'complete' ? (
                <InspectionFeedback workspace={w} />
              ) : success ? (
                <div className="success-panel" role="status">
                  <div className="success-top">
                    <span className="success-icon">
                      <IconCheck size={20} />
                    </span>
                    <div className="success-copy">
                      <Text fw={600} size="sm">
                        {success.outputKind === 'folder'
                          ? 'Your music folder is ready'
                          : 'Your archive is ready'}
                      </Text>
                      <Text size="xs" c="dimmed" className="output-path">
                        {success.outputPath}
                      </Text>
                    </div>
                    <Button
                      variant="default"
                      size="sm"
                      leftSection={<IconFolder size={16} />}
                      onClick={() =>
                        void revealArchive(success.outputPath).catch((error) =>
                          w.setError(String(error)),
                        )
                      }
                    >
                      {success.outputKind === 'folder'
                        ? 'Open folder'
                        : mac
                          ? 'Show in Finder'
                          : 'Show in folder'}
                    </Button>
                  </div>
                  {exportNotes.length > 0 && (
                    <details className="source-notes">
                      <summary>
                        {exportNotes.length} export{' '}
                        {exportNotes.length === 1 ? 'note' : 'notes'}
                      </summary>
                      {exportNotes.map((warning) => (
                        <Text size="xs" key={warning} mt={6}>
                          {warning}
                        </Text>
                      ))}
                    </details>
                  )}
                  <Group gap="lg" className="success-next">
                    {success.outputKind === 'folder' && (
                      <Button
                        size="xs"
                        variant="default"
                        onClick={() =>
                          void openPlaylist(success.outputPath).catch((error) =>
                            w.setError(String(error)),
                          )
                        }
                      >
                        Open playlist
                      </Button>
                    )}
                    <Button
                      variant="subtle"
                      size="xs"
                      onClick={newPlaylist}
                      rightSection={<IconArrowRight size={14} />}
                    >
                      Download another{' '}
                      {p.mode === 'album' ? 'album' : 'playlist'}
                    </Button>
                    <button
                      type="button"
                      className="text-action"
                      onClick={w.prepareAnotherCopy}
                    >
                      Save another copy
                    </button>
                  </Group>
                </div>
              ) : (
                <>
                  <DownloadFeedback
                    progress={progress}
                    exporting={w.exporting}
                    cancelling={w.cancelling}
                    destinationPath={w.destinationPath}
                    onOpenSettings={(trigger) =>
                      openDialog('settings', trigger)
                    }
                    onChooseDestination={() =>
                      void w.chooseAnotherDestination()
                    }
                  />
                  <div className="export-actions">
                    <div className="export-summary">
                      <Text size="sm" fw={600}>
                        {w.selected.size
                          ? `${w.selected.size} ${w.selected.size === 1 ? 'track' : 'tracks'}${totalDuration ? ` · ${listeningTime(totalDuration)}` : ''}`
                          : 'Select tracks to download'}
                      </Text>
                      <button
                        type="button"
                        className="export-filename"
                        onClick={(event) =>
                          openDialog('details', event.currentTarget)
                        }
                        disabled={w.busy}
                        aria-label={`Rename ${p.title}`}
                      >
                        <span>
                          {w.savePath?.split(/[\\/]/).pop() ||
                            (p.outputKind === 'folder'
                              ? `${p.title}/`
                              : archiveFilename(p.title))}
                        </span>
                        <IconEdit size={14} aria-hidden="true" />
                      </button>
                      <button
                        type="button"
                        className="export-options"
                        onClick={(event) =>
                          openDialog('export', event.currentTarget)
                        }
                        disabled={w.busy}
                        aria-label={`Download options: ${formatNames[p.format]}, ${artworkNames[p.artworkMode]}`}
                      >
                        <span className="export-options-label">
                          Download options
                        </span>
                        <IconChevronDown size={13} aria-hidden="true" />
                        <span className="export-options-summary">
                          {formatNames[p.format]} ·{' '}
                          {artworkNames[p.artworkMode]} ·{' '}
                          {p.outputKind === 'folder' ? 'Folder' : 'ZIP'}
                        </span>
                      </button>
                      <button
                        type="button"
                        className="text-action preview-files"
                        disabled={w.busy || !w.chosen.length}
                        onClick={(event) =>
                          openDialog('preview', event.currentTarget)
                        }
                      >
                        Preview files
                      </button>
                      {reviewReady && !w.exampleMode && desktopAvailable && (
                        <Text
                          size="xs"
                          c="dimmed"
                          title={w.savePath || undefined}
                        >
                          {saveFolder
                            ? w.exporting
                              ? `Saving to ${saveFolder}`
                              : retry && w.destinationPath
                                ? `Retry saves to ${saveFolder}`
                                : `Choose where to save · ${saveFolder}`
                            : 'Choose where to save when downloading'}
                        </Text>
                      )}
                    </div>
                    <div className="download-controls">
                      {w.exporting ? (
                        <Button
                          variant="default"
                          size="md"
                          disabled={w.cancelling}
                          onClick={() => void w.cancel()}
                        >
                          {w.cancelling ? 'Stopping…' : 'Cancel download'}
                        </Button>
                      ) : (
                        <Tooltip
                          label={
                            w.exampleMode || !desktopAvailable
                              ? 'Open an album or playlist in the desktop app to download'
                              : !w.toolsReady
                                ? 'Check the audio tools in Settings'
                                : !w.eventsReady
                                  ? 'Connecting to the audio tools…'
                                  : !w.draftReady
                                    ? 'Restoring unfinished work…'
                                    : w.selected.size === 0
                                      ? 'Select at least one track'
                                      : !p.title.trim()
                                        ? `Add a collection name in ${p.mode === 'album' ? 'Edit album details' : 'Edit details'}`
                                        : `${mac ? '⌘' : 'Ctrl'} + Enter`
                          }
                          events={{ hover: true, focus: true, touch: false }}
                        >
                          <span
                            className="download-button-target"
                            tabIndex={!canExport ? 0 : undefined}
                          >
                            <Button
                              ref={downloadButton}
                              className="save-button"
                              size="md"
                              leftSection={<IconDownload size={17} />}
                              loading={w.choosing}
                              disabled={!canExport}
                              onClick={() => void w.exportZip()}
                            >
                              {progress?.stage === 'interrupted'
                                ? 'Resume download'
                                : retry
                                  ? 'Retry download'
                                  : p.outputKind === 'folder'
                                    ? `Save ${p.mode === 'album' ? 'album' : 'playlist'}…`
                                    : p.mode === 'album'
                                      ? 'Download album…'
                                      : 'Download playlist…'}
                            </Button>
                          </span>
                        </Tooltip>
                      )}
                    </div>
                  </div>
                  {!w.exporting && missingArtists > 0 && (
                    <Text className="tag-reminder" size="xs" c="dimmed">
                      {missingArtists} selected{' '}
                      {missingArtists === 1 ? 'track has' : 'tracks have'} no
                      artist tag. Use the pencil beside a track to add one.
                    </Text>
                  )}
                  {!w.exporting && sourceNotes.length > 0 && (
                    <details className="source-notes">
                      <summary>
                        {sourceNotes.length} source{' '}
                        {sourceNotes.length === 1 ? 'note' : 'notes'}
                      </summary>
                      {[...new Set(sourceNotes)].map((warning) => (
                        <Text size="xs" key={warning} mt={6}>
                          {warning}
                        </Text>
                      ))}
                    </details>
                  )}
                </>
              )}
            </footer>
          </>
        )}
      </main>
      <WorkspaceDialogs
        workspace={w}
        dialog={dialog}
        focusDownloader={focusDownloader}
        openDownloader={() => openDialog('downloader')}
        openSettings={() => {
          setFocusDownloader(true);
          openDialog('settings');
        }}
        close={closeDialog}
      />
      {dialog === 'bulk' && inspection && (
        <BulkTrackEditor
          tracks={inspection.tracks}
          selected={w.selected}
          inherited={
            p.mode === 'album'
              ? {
                  album: p.albumTitle.trim() || undefined,
                  albumArtist: p.albumArtist.trim() || undefined,
                  releaseYear: p.year ?? undefined,
                }
              : undefined
          }
          busy={w.busy}
          onApply={w.applyBulk}
          close={closeDialog}
        />
      )}
      {dialog === 'preview' && w.exportPlan && (
        <ExportPreviewDialog
          plan={w.exportPlan}
          cover={cover ?? null}
          close={closeDialog}
        />
      )}
      {editing && (
        <TrackEditor
          key={editing.index}
          editing={editing}
          album={
            p.mode === 'album'
              ? { title: p.albumTitle, artist: p.albumArtist, year: p.year }
              : null
          }
          hasNext={editing.index < (inspection?.tracks.length ?? 0) - 1}
          close={closeEditor}
          openAlbumDetails={(track) => {
            w.updateTrack(editing.index, track);
            setEditing(null);
            dialogTrigger.current = editorTrigger.current;
            setDialog('details');
          }}
          save={(index, track, advance) => {
            w.updateTrack(index, track);
            const next = inspection?.tracks[index + 1];
            if (advance && next) setEditing({ index: index + 1, track: next });
            else closeEditor();
          }}
        />
      )}
    </div>
  );
}

function InspectionFeedback({ workspace: w }: { workspace: Workspace }) {
  const count = w.inspection?.tracks.length ?? 0;
  return (
    <div className="inspection-feedback">
      <div
        className="inspection-feedback-copy"
        role="status"
        aria-live="polite"
      >
        <Group gap={8}>
          {w.loading && <IconLoader2 className="spinner" size={16} />}
          <Text size="sm" fw={600}>
            {w.loading
              ? w.cancelling
                ? 'Stopping inspection…'
                : 'Reading tracks'
              : w.inspectionStatus === 'cancelled'
                ? 'Inspection stopped'
                : 'Inspection couldn’t finish'}
          </Text>
        </Group>
        <Text size="xs" c="dimmed" mt={6}>
          {w.loading
            ? `${w.inspectionTotal ? `${count} of ${w.inspectionTotal} tracks` : `${count} ${count === 1 ? 'track' : 'tracks'}`} found · You can browse while the rest arrive.`
            : `${count} ${count === 1 ? 'track' : 'tracks'} found. Inspect again to finish before saving.`}
        </Text>
      </div>
      <Button
        variant="default"
        disabled={w.cancelling}
        onClick={() => void (w.loading ? w.cancel() : w.inspect())}
      >
        {w.loading
          ? w.cancelling
            ? 'Stopping…'
            : 'Cancel inspection'
          : 'Inspect again'}
      </Button>
    </div>
  );
}

function ReviewUndo({
  workspace: w,
  undo,
}: {
  workspace: Workspace;
  undo: () => void;
}) {
  if (!w.previousReview) return null;
  return (
    <div className="review-undo" role="status">
      <div>
        <Text size="sm" fw={500}>
          Your previous review is still here.
        </Text>
        <Text size="xs" c="dimmed" lineClamp={1} title={w.previousReview.title}>
          {w.previousReview.title} · {w.previousReview.trackCount}{' '}
          {w.previousReview.trackCount === 1 ? 'track' : 'tracks'}
        </Text>
      </div>
      <Button
        variant="default"
        size="sm"
        leftSection={<IconRotateClockwise size={16} />}
        disabled={!w.canUndoReset}
        onClick={undo}
      >
        Undo new download
      </Button>
    </div>
  );
}
