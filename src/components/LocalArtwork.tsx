import { Alert, Button, Group, Slider, Stack, Text } from '@mantine/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { useEffect, useRef, useState } from 'react';
import {
  artworkAsset,
  chooseArtwork,
  cropArtwork,
  desktopAvailable,
  importArtwork,
} from '../desktop';
import type { ArtworkAsset, ArtworkCrop } from '../domain';

function centerCrop(asset: ArtworkAsset): ArtworkCrop {
  const size = Math.min(asset.width, asset.height);
  return {
    size,
    x: Math.floor((asset.width - size) / 2),
    y: Math.floor((asset.height - size) / 2),
  };
}

function bounded(value: number, maximum: number): number {
  return Math.max(0, Math.min(Math.round(value), maximum));
}

type Pending = 'choosing' | 'importing' | 'cropping';

export function LocalArtwork({
  currentAssetId,
  busy,
  onSelect,
  onBusyChange,
}: {
  currentAssetId: string | null;
  busy: boolean;
  onSelect: (asset: ArtworkAsset) => void;
  onBusyChange?: (busy: boolean) => void;
}) {
  const [current, setCurrent] = useState<ArtworkAsset | null>(null);
  const [draft, setDraft] = useState<ArtworkAsset | null>(null);
  const [crop, setCrop] = useState<ArtworkCrop>({ x: 0, y: 0, size: 1 });
  const [pending, setPending] = useState<Pending | null>(null);
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [previewFailed, setPreviewFailed] = useState(false);
  const mounted = useRef(true);
  const active = useRef(false);
  const busyRef = useRef(busy);
  const notifyBusy = useRef(onBusyChange);
  const dropHandler = useRef<(paths: string[]) => void>(() => {});
  busyRef.current = busy;
  notifyBusy.current = onBusyChange;
  const disabled = busy || pending !== null;

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      if (active.current) notifyBusy.current?.(false);
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    setCurrent(null);
    if (!desktopAvailable || !currentAssetId) return;
    artworkAsset(currentAssetId)
      .then((asset) => {
        if (!disposed) setCurrent(asset);
      })
      .catch((error) => {
        if (!disposed)
          setError(
            `The saved cover could not be loaded. Choose it again. ${String(error)}`,
          );
      });
    return () => {
      disposed = true;
    };
  }, [currentAssetId]);

  useEffect(() => {
    if (!desktopAvailable) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent(({ payload }) => {
        if (disposed) return;
        if (payload.type === 'drop') {
          setDragging(false);
          dropHandler.current(payload.paths);
        } else if (payload.type === 'leave') setDragging(false);
        else if (!busyRef.current && !active.current) setDragging(true);
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {
        if (!disposed)
          setError('Drag and drop is unavailable. Use Choose cover image.');
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  function begin(operation: Pending): boolean {
    if (
      !desktopAvailable ||
      !mounted.current ||
      busyRef.current ||
      active.current
    )
      return false;
    active.current = true;
    setPending(operation);
    setDragging(false);
    setError(null);
    notifyBusy.current?.(true);
    return true;
  }

  function finish() {
    active.current = false;
    if (!mounted.current) return;
    setPending(null);
    notifyBusy.current?.(false);
  }

  async function importImage(
    operation: 'choosing' | 'importing',
    load: () => Promise<ArtworkAsset | null>,
  ) {
    if (!begin(operation)) return;
    try {
      const asset = await load();
      if (!mounted.current || !asset) return;
      setDraft(asset);
      setCrop(centerCrop(asset));
      setPreviewFailed(false);
    } catch (error) {
      if (mounted.current)
        setError(
          `Could not import this image. Try another JPEG or PNG. ${String(error)}`,
        );
    } finally {
      finish();
    }
  }

  dropHandler.current = (paths) => {
    if (busyRef.current || active.current) return;
    const path = paths.find((path) => /\.(?:jpe?g|png)$/i.test(path));
    if (!path) {
      setError('Drop a JPEG or PNG image, or use Choose cover image.');
      return;
    }
    void importImage('importing', () => importArtwork(path));
  };

  async function applyCrop() {
    if (!draft || previewFailed || !begin('cropping')) return;
    try {
      const asset = await cropArtwork(draft.id, crop);
      if (!mounted.current) return;
      setCurrent(asset);
      setDraft(null);
      onSelect(asset);
    } catch (error) {
      if (mounted.current)
        setError(
          `Could not save this cover. Try applying the crop again. ${String(error)}`,
        );
    } finally {
      finish();
    }
  }

  function resize(size: number) {
    if (!draft) return;
    const nextSize = Math.max(
      1,
      Math.min(Math.round(size), draft.width, draft.height),
    );
    setCrop((previous) => ({
      size: nextSize,
      x: bounded(
        previous.x + (previous.size - nextSize) / 2,
        draft.width - nextSize,
      ),
      y: bounded(
        previous.y + (previous.size - nextSize) / 2,
        draft.height - nextSize,
      ),
    }));
  }

  return (
    <Stack gap="md">
      <div>
        <Text size="sm" fw={500} mb={8}>
          Your cover image
        </Text>
        {!desktopAvailable ? (
          <Text size="sm" c="dimmed">
            Open Jacket’s desktop app to choose or drop a local cover image.
          </Text>
        ) : (
          <Text size="sm" c="dimmed">
            Choose or drop a JPEG or PNG up to 10 MiB. Jacket keeps a copy, so
            moving the original image will not affect your cover.
          </Text>
        )}
      </div>
      {current && !draft && (
        <img
          src={current.previewDataUrl}
          width={104}
          height={104}
          alt="Current imported cover"
          style={{ objectFit: 'contain', borderRadius: 4 }}
        />
      )}
      <div
        style={{
          padding: 16,
          border: `1px dashed ${dragging ? 'var(--mantine-primary-color-filled)' : 'var(--mantine-color-default-border)'}`,
          borderRadius: 8,
          background: dragging
            ? 'var(--mantine-primary-color-light)'
            : undefined,
        }}
      >
        <Button
          variant="default"
          fullWidth
          disabled={!desktopAvailable || disabled}
          loading={pending === 'choosing' || pending === 'importing'}
          onClick={() => void importImage('choosing', chooseArtwork)}
        >
          Choose cover image
        </Button>
        {desktopAvailable && (
          <Text size="xs" c="dimmed" mt={8} ta="center" role="status">
            {dragging
              ? 'Drop your image here'
              : 'Or drop an image while this dialog is open'}
          </Text>
        )}
      </div>
      {draft && (
        <Stack gap="md">
          <Text size="sm" fw={500}>
            Crop your cover
          </Text>
          <div
            style={{
              position: 'relative',
              width: 200,
              height: 200,
              maxWidth: '100%',
              overflow: 'hidden',
              borderRadius: 4,
              marginInline: 'auto',
              background: 'var(--mantine-color-default)',
            }}
          >
            <img
              src={draft.previewDataUrl}
              alt="Square cover crop preview"
              draggable={false}
              width={draft.width}
              height={draft.height}
              style={{
                position: 'absolute',
                maxWidth: 'none',
                width: `${(draft.width / crop.size) * 100}%`,
                height: `${(draft.height / crop.size) * 100}%`,
                left: `${-(crop.x / crop.size) * 100}%`,
                top: `${-(crop.y / crop.size) * 100}%`,
              }}
              onError={() => {
                setPreviewFailed(true);
                setError(
                  'This image preview could not be loaded. Choose the image again.',
                );
              }}
            />
          </div>
          <div>
            <Text size="sm" mb={8}>
              Crop size: {crop.size} px
            </Text>
            <Slider
              aria-label="Crop size"
              value={crop.size}
              min={1}
              max={Math.max(2, Math.min(draft.width, draft.height))}
              step={1}
              label={(value) => `${value} px`}
              disabled={disabled || Math.min(draft.width, draft.height) <= 1}
              onChange={resize}
            />
          </div>
          <div>
            <Text size="sm" mb={8}>
              Horizontal position: {crop.x} px
            </Text>
            <Slider
              aria-label="Horizontal crop position"
              value={crop.x}
              min={0}
              max={Math.max(1, draft.width - crop.size)}
              step={1}
              label={(value) => `${value} px`}
              disabled={disabled || draft.width === crop.size}
              onChange={(value) =>
                setCrop((previous) => ({
                  ...previous,
                  x: bounded(value, draft.width - previous.size),
                }))
              }
            />
          </div>
          <div>
            <Text size="sm" mb={8}>
              Vertical position: {crop.y} px
            </Text>
            <Slider
              aria-label="Vertical crop position"
              value={crop.y}
              min={0}
              max={Math.max(1, draft.height - crop.size)}
              step={1}
              label={(value) => `${value} px`}
              disabled={disabled || draft.height === crop.size}
              onChange={(value) =>
                setCrop((previous) => ({
                  ...previous,
                  y: bounded(value, draft.height - previous.size),
                }))
              }
            />
          </div>
          <Text size="xs" c="dimmed">
            Adjust the square with the sliders or their arrow keys. Your current
            cover stays selected until you apply this crop.
          </Text>
          <Group justify="flex-end" wrap="wrap">
            <Button
              variant="default"
              disabled={disabled}
              onClick={() => {
                setDraft(null);
                setError(null);
              }}
            >
              Cancel crop
            </Button>
            <Button
              disabled={disabled || previewFailed}
              loading={pending === 'cropping'}
              onClick={() => void applyCrop()}
            >
              Apply cover
            </Button>
          </Group>
        </Stack>
      )}
      {error && (
        <Alert color="red" title="Cover image" role="alert">
          {error}
        </Alert>
      )}
    </Stack>
  );
}
