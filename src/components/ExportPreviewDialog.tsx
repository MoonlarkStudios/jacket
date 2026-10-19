import {
  Alert,
  Button,
  Group,
  Image,
  Loader,
  Modal,
  ScrollArea,
  Stack,
  Text,
} from '@mantine/core';
import { useEffect, useState } from 'react';
import { desktopAvailable, previewExport } from '../desktop';
import type { ExportPlan, ExportPreview } from '../domain';

export function ExportPreviewDialog({
  plan,
  cover,
  close,
}: {
  plan: ExportPlan;
  cover: string | null;
  close: () => void;
}) {
  const [preview, setPreview] = useState<ExportPreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Only output-affecting data trigger planning, never browser credentials.
  const serialized = JSON.stringify({ ...plan, auth: { browser: null } });
  useEffect(() => {
    if (!desktopAvailable) return;
    let disposed = false;
    setPreview(null);
    setError(null);
    void previewExport(JSON.parse(serialized) as ExportPlan)
      .then((value) => {
        if (!disposed) setPreview(value);
      })
      .catch((reason) => {
        if (!disposed) setError(String(reason));
      });
    return () => {
      disposed = true;
    };
  }, [serialized]);
  return (
    <Modal
      opened
      onClose={close}
      returnFocus={false}
      title="Preview your collection"
      centered
      size="lg"
      scrollAreaComponent={ScrollArea.Autosize}
    >
      <Stack gap="md">
        {!desktopAvailable ? (
          <Text c="dimmed">
            Open the desktop app to preview the files Jacket will save.
          </Text>
        ) : error ? (
          <Alert color="red" title="Preview unavailable">
            {error}
          </Alert>
        ) : !preview ? (
          <Group role="status">
            <Loader size="sm" />
            <Text>Preparing your preview…</Text>
          </Group>
        ) : (
          <>
            <Group wrap="nowrap" align="flex-start">
              {cover && (
                <Image
                  src={cover}
                  alt="Selected cover"
                  w={72}
                  h={72}
                  fit="contain"
                  style={{ flexShrink: 0 }}
                />
              )}
              <div style={{ minWidth: 0 }}>
                <Text fw={600} className="file-preview-name">
                  {preview.rootName}/
                </Text>
                <Text size="sm" c="dimmed">
                  {preview.tracks.length} tracks ·{' '}
                  {preview.outputKind === 'zip'
                    ? 'Inside your ZIP archive'
                    : 'Music folder'}
                </Text>
              </div>
            </Group>
            <ScrollArea.Autosize mah={340}>
              <Stack gap="sm">
                {preview.tracks.map((track) => (
                  <div className="file-preview-row" key={track.relativePath}>
                    <Text size="sm" fw={500} className="file-preview-name">
                      {track.relativePath}
                    </Text>
                    <Text size="xs">Title: {track.metadata.title}</Text>
                    <Text size="xs" c="dimmed">
                      {track.metadata.artist || 'Artist not set'} ·{' '}
                      {track.metadata.album || 'Album not set'}
                      {track.metadata.releaseYear
                        ? ` · ${track.metadata.releaseYear}`
                        : ''}
                    </Text>
                    <Text size="xs" c="dimmed">
                      {track.metadata.albumArtist
                        ? `Album artist: ${track.metadata.albumArtist} · `
                        : ''}
                      {track.metadata.trackNumber
                        ? `Track ${track.metadata.trackNumber}`
                        : 'Track number not set'}
                      {track.metadata.discNumber
                        ? ` · Disc ${track.metadata.discNumber}`
                        : ''}
                      {track.artworkPath ? ' · Cover included' : ' · No cover'}
                    </Text>
                    {track.extensionProvisional && (
                      <Text size="xs" c="dimmed">
                        File extension depends on the downloaded audio.
                      </Text>
                    )}
                  </div>
                ))}
                <Text size="xs" fw={600}>
                  Also included
                </Text>
                {preview.files
                  .filter(
                    (name) =>
                      !preview.tracks.some(
                        (track) => track.relativePath === name,
                      ),
                  )
                  .map((name) => (
                    <Text size="sm" className="file-preview-name" key={name}>
                      {name}
                    </Text>
                  ))}
              </Stack>
            </ScrollArea.Autosize>
            {preview.warnings.map((warning) => (
              <Text size="xs" c="dimmed" key={warning}>
                {warning}
              </Text>
            ))}
          </>
        )}
        <Button onClick={close}>Done</Button>
      </Stack>
    </Modal>
  );
}
