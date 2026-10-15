import {
  Accordion,
  Button,
  Group,
  Image,
  Modal,
  NumberInput,
  SegmentedControl,
  Select,
  Stack,
  Text,
  TextInput,
} from '@mantine/core';
import { IconCheck } from '@tabler/icons-react';
import { useState } from 'react';
import type { Track } from '../domain';
import { updateAlbumPreferences } from '../preferences';
import type { Workspace } from '../useWorkspace';
import { BrowserSessionSettings } from './BrowserSessionSettings';
import {
  DownloaderSettings,
  DownloaderStatusButton,
} from './DownloaderSettings';
import { OwnedSessionSettings } from './OwnedSessionSettings';

export type Dialog =
  | 'settings'
  | 'downloader'
  | 'details'
  | 'export'
  | 'artwork'
  | 'authPrototype'
  | null;
export function WorkspaceDialogs({
  workspace: w,
  dialog,
  close,
  openDownloader,
  openSettings,
  focusDownloader = false,
}: {
  workspace: Workspace;
  dialog: Dialog;
  close: () => void;
  openDownloader: () => void;
  openSettings: () => void;
  focusDownloader?: boolean;
}) {
  const p = w.preferences;
  const patch = (value: Partial<typeof p>) =>
    w.setPreferences((previous) => ({ ...previous, ...value }));
  return (
    <>
      <Modal
        returnFocus={false}
        opened={dialog === 'settings'}
        onClose={() => {
          if (!w.browserSession.testing) close();
        }}
        closeButtonProps={{ disabled: w.browserSession.testing }}
        closeOnEscape={!w.browserSession.testing}
        closeOnClickOutside={!w.browserSession.testing}
        title="Settings"
        centered
      >
        <Stack gap="lg">
          {dialog === 'settings' && <BrowserSessionSettings workspace={w} />}
          <Text size="xs" c="dimmed">
            {w.dependencies?.every((tool) => tool.bundled)
              ? 'Audio tools are included.'
              : 'This development build uses locally installed audio tools.'}
          </Text>
          <DownloaderStatusButton
            workspace={w}
            open={openDownloader}
            autoFocus={focusDownloader}
          />
          <Button onClick={close} fullWidth disabled={w.browserSession.testing}>
            Done
          </Button>
        </Stack>
      </Modal>
      <Modal
        returnFocus={false}
        opened={dialog === 'authPrototype'}
        onClose={close}
        title="Developer session prototype"
        centered
      >
        {dialog === 'authPrototype' && <OwnedSessionSettings workspace={w} />}
      </Modal>
      <DownloaderSettings
        workspace={w}
        opened={dialog === 'downloader'}
        close={close}
        back={openSettings}
      />
      {dialog === 'details' && <DetailsEditor workspace={w} close={close} />}
      <Modal
        returnFocus={false}
        opened={dialog === 'export'}
        onClose={close}
        title="Download options"
        centered
      >
        <Stack gap="lg">
          <div>
            <Select
              label="Audio format"
              value={p.format}
              onChange={(value) => patch({ format: value as typeof p.format })}
              allowDeselect={false}
              data={[
                { value: 'compatible', label: 'M4A / MP3 · recommended' },
                { value: 'preserve', label: 'Original codec' },
                { value: 'mp3', label: 'MP3' },
              ]}
            />
            <Text size="sm" c="dimmed" mt={8}>
              {p.format === 'preserve'
                ? 'Keep the downloaded codec without an extra lossy conversion. WAV supports fewer tags; any omitted fields are listed after download.'
                : p.format === 'compatible'
                  ? 'Keep AAC and ALAC as M4A. Use MP3 for other sources.'
                  : 'Convert every track to MP3.'}
            </Text>
          </div>
          <ArtworkOptions workspace={w} />
          <Button onClick={close} fullWidth>
            Done
          </Button>
        </Stack>
      </Modal>
      <Modal
        returnFocus={false}
        opened={dialog === 'artwork'}
        onClose={close}
        title="Artwork"
        centered
      >
        <Stack gap="lg">
          <ArtworkOptions workspace={w} />
          <Button onClick={close} fullWidth>
            Done
          </Button>
        </Stack>
      </Modal>
    </>
  );
}

function DetailsEditor({
  workspace: w,
  close,
}: {
  workspace: Workspace;
  close: () => void;
}) {
  const [draft, setDraft] = useState(() => ({ ...w.preferences }));
  const sourceTitle = w.inspection?.title || 'Audio collection';
  const albumPatch = (
    value: Partial<Pick<typeof draft, 'albumTitle' | 'albumArtist' | 'year'>>,
  ) =>
    setDraft((previous) =>
      updateAlbumPreferences(previous, value, sourceTitle),
    );
  return (
    <Modal
      returnFocus={false}
      opened
      onClose={close}
      title={draft.mode === 'album' ? 'Album details' : 'Playlist options'}
      centered
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (w.busy || !draft.title.trim()) return;
          if (draft.mode !== w.preferences.mode) w.setMode(draft.mode);
          w.setPreferences((previous) => ({
            ...previous,
            title: draft.title.trim(),
            mode: draft.mode,
            albumTitle: draft.albumTitle,
            albumArtist: draft.albumArtist,
            year: draft.year,
            artworkMode: draft.artworkMode,
          }));
          close();
        }}
      >
        <Stack gap="lg">
          <div>
            <Text size="sm" fw={500} mb={8} id="organize-label">
              Organize as
            </Text>
            <SegmentedControl
              fullWidth
              aria-labelledby="organize-label"
              value={draft.mode}
              disabled={w.busy}
              onChange={(value) => {
                const mode = value as typeof draft.mode;
                setDraft((previous) => ({
                  ...previous,
                  mode,
                  albumTitle:
                    mode === 'album'
                      ? previous.albumTitle || sourceTitle
                      : previous.albumTitle,
                  artworkMode:
                    previous.artworkMode === 'none'
                      ? 'none'
                      : mode === 'album'
                        ? 'shared'
                        : 'perTrack',
                }));
              }}
              data={[
                { value: 'collection', label: 'Playlist' },
                { value: 'album', label: 'Album' },
              ]}
            />
            <Text size="sm" c="dimmed" mt={10}>
              {draft.mode === 'album'
                ? 'Apply shared album details. Each track keeps its own artist and numbering.'
                : 'Keep each track’s own metadata, with files in playlist order.'}
            </Text>
          </div>
          {draft.mode === 'album' && (
            <>
              <TextInput
                label="Album title"
                placeholder="Keep track album titles"
                value={draft.albumTitle}
                disabled={w.busy}
                onChange={(event) =>
                  albumPatch({ albumTitle: event.currentTarget.value })
                }
                data-autofocus
              />
              <TextInput
                label="Album artist"
                placeholder="Keep track album artists"
                value={draft.albumArtist}
                disabled={w.busy}
                onChange={(event) =>
                  albumPatch({ albumArtist: event.currentTarget.value })
                }
              />
              <NumberInput
                label="Release year"
                placeholder="Keep track years"
                value={draft.year ?? ''}
                disabled={w.busy}
                min={1000}
                max={9999}
                allowDecimal={false}
                onChange={(value) =>
                  albumPatch({ year: typeof value === 'number' ? value : null })
                }
              />
            </>
          )}
          <TextInput
            label="ZIP filename"
            description="The .zip extension is added automatically."
            value={draft.title}
            disabled={w.busy}
            onChange={(event) => {
              const title = event.currentTarget.value;
              setDraft((previous) => ({ ...previous, title }));
            }}
            data-autofocus={draft.mode === 'collection' || undefined}
            required
          />
          <Group justify="flex-end">
            <Button variant="default" onClick={close}>
              Cancel
            </Button>
            <Button type="submit" disabled={w.busy || !draft.title.trim()}>
              Save changes
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
}

function ArtworkOptions({ workspace: w }: { workspace: Workspace }) {
  const p = w.preferences;
  const [unavailableImages, setUnavailableImages] = useState<Set<string>>(
    () => new Set(),
  );
  const patch = (value: Partial<typeof p>) =>
    w.setPreferences((previous) => ({ ...previous, ...value }));
  const candidates = [
    ...new Set(
      [
        w.inspection?.artworkUrl,
        ...(w.inspection?.tracks.map((track) => track.artworkUrl) ?? []),
        p.sharedArtwork,
      ].filter((value): value is string => Boolean(value)),
    ),
  ];
  return (
    <>
      <div>
        <Select
          label="Artwork"
          value={p.artworkMode}
          onChange={(value) =>
            patch({ artworkMode: value as typeof p.artworkMode })
          }
          allowDeselect={false}
          data={[
            { value: 'perTrack', label: 'Original image for each track' },
            { value: 'shared', label: 'One image for every track' },
            { value: 'none', label: 'No artwork' },
          ]}
        />
        <Text size="sm" c="dimmed" mt={8}>
          Images are embedded where supported and included in the ZIP.
        </Text>
      </div>
      {p.artworkMode === 'shared' && candidates.length > 0 && (
        <div>
          <Text size="sm" fw={500} mb={10}>
            Choose an image
          </Text>
          <div className="artwork-grid">
            {candidates.map((candidate, index) => (
              <button
                key={candidate}
                type="button"
                className={`artwork-choice ${candidate === p.sharedArtwork ? 'is-selected' : ''}`}
                aria-label={
                  unavailableImages.has(candidate)
                    ? `Artwork option ${index + 1}: image unavailable`
                    : `Use artwork ${index + 1}`
                }
                aria-pressed={candidate === p.sharedArtwork}
                disabled={unavailableImages.has(candidate)}
                onClick={() => patch({ sharedArtwork: candidate })}
              >
                {unavailableImages.has(candidate) ? (
                  <Text
                    component="span"
                    size="xs"
                    c="dimmed"
                    style={{
                      height: 76,
                      display: 'grid',
                      placeItems: 'center',
                    }}
                  >
                    Image unavailable
                  </Text>
                ) : (
                  <Image
                    src={candidate}
                    h={76}
                    w="100%"
                    fit="contain"
                    alt={`Artwork option ${index + 1}`}
                    onError={() =>
                      setUnavailableImages((previous) =>
                        new Set(previous).add(candidate),
                      )
                    }
                  />
                )}
                {candidate === p.sharedArtwork && (
                  <span className="artwork-check">
                    <IconCheck size={12} />
                  </span>
                )}
              </button>
            ))}
          </div>
          {p.sharedArtwork && unavailableImages.has(p.sharedArtwork) && (
            <Text size="sm" c="dimmed" mt={10} role="status">
              The selected image is unavailable. Choose another image or select
              No artwork.
            </Text>
          )}
          <Text size="xs" c="dimmed" mt={10}>
            Source images may be video thumbnails rather than album covers.
          </Text>
        </div>
      )}
      {p.artworkMode !== 'none' && candidates.length === 0 && (
        <Text size="sm" c="dimmed">
          No source images are available. Audio will download without artwork.
        </Text>
      )}
    </>
  );
}

export function TrackEditor({
  editing,
  album,
  hasNext,
  close,
  save,
  openAlbumDetails,
}: {
  editing: { index: number; track: Track };
  album: { title: string; artist: string; year: number | null } | null;
  hasNext: boolean;
  close: () => void;
  save: (index: number, track: Track, advance?: boolean) => void;
  openAlbumDetails: (track: Track) => void;
}) {
  const [track, setTrack] = useState(() => structuredClone(editing.track));
  function patch(value: Partial<Track['metadata']>) {
    setTrack((previous) => ({
      ...previous,
      metadata: { ...previous.metadata, ...value },
    }));
  }
  return (
    <Modal
      returnFocus={false}
      opened
      onClose={close}
      title={`Edit track ${editing.index + 1}`}
      centered
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (track.metadata.title.trim()) save(editing.index, track);
        }}
      >
        <Stack gap="md">
          <TextInput
            label="Title"
            value={track.metadata.title}
            onChange={(event) => patch({ title: event.currentTarget.value })}
            data-autofocus
            required
          />
          <TextInput
            label="Artist"
            placeholder="Not provided"
            value={track.metadata.artist}
            onChange={(event) => patch({ artist: event.currentTarget.value })}
          />
          <TextInput
            label="Album"
            placeholder="Not provided"
            value={album?.title.trim() || track.metadata.album}
            disabled={!!album?.title.trim()}
            description={album?.title.trim() ? 'From album details' : undefined}
            onChange={(event) => patch({ album: event.currentTarget.value })}
          />
          <Accordion variant="default">
            <Accordion.Item value="more">
              <Accordion.Control>More metadata</Accordion.Control>
              <Accordion.Panel>
                <Stack gap="md">
                  <TextInput
                    label="Album artist"
                    value={album?.artist.trim() || track.metadata.albumArtist}
                    disabled={!!album?.artist.trim()}
                    description={
                      album?.artist.trim() ? 'From album details' : undefined
                    }
                    onChange={(event) =>
                      patch({ albumArtist: event.currentTarget.value })
                    }
                  />
                  <Group grow align="start">
                    {(
                      [
                        ['trackNumber', 'Track'],
                        ['discNumber', 'Disc'],
                        ['releaseYear', 'Year'],
                      ] as const
                    ).map(([key, label]) => (
                      <NumberInput
                        key={key}
                        label={label}
                        value={
                          (key === 'releaseYear'
                            ? (album?.year ?? track.metadata[key])
                            : track.metadata[key]) ?? ''
                        }
                        disabled={key === 'releaseYear' && album?.year != null}
                        description={
                          key === 'releaseYear' && album?.year != null
                            ? 'From album details'
                            : undefined
                        }
                        min={key === 'releaseYear' ? 1000 : 1}
                        max={key === 'releaseYear' ? 9999 : 999}
                        allowDecimal={false}
                        onChange={(value) =>
                          patch({
                            [key]: typeof value === 'number' ? value : null,
                          })
                        }
                      />
                    ))}
                  </Group>
                  <Text size="xs" c="dimmed">
                    Uploaded by {track.uploader || 'an unknown channel'}. Upload
                    information is kept separate from music tags.
                  </Text>
                </Stack>
              </Accordion.Panel>
            </Accordion.Item>
          </Accordion>
          {album && (
            <div>
              <Text size="sm" c="dimmed">
                Album details apply to all tracks. Each track keeps its own
                artist.
              </Text>
              <Button
                variant="subtle"
                size="sm"
                px={0}
                mt={4}
                disabled={!track.metadata.title.trim()}
                onClick={() => openAlbumDetails(track)}
              >
                Save track & edit album details
              </Button>
            </div>
          )}
          <Group justify="flex-end" mt="sm">
            <Button variant="default" onClick={close}>
              Cancel
            </Button>
            {hasNext && (
              <Button
                variant="default"
                disabled={!track.metadata.title.trim()}
                onClick={() => save(editing.index, track, true)}
              >
                Save & next
              </Button>
            )}
            <Button type="submit" disabled={!track.metadata.title.trim()}>
              Save changes
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
}
