import { Button, Checkbox, Text } from '@mantine/core';
import { IconEdit } from '@tabler/icons-react';
import type { Track } from '../domain';

export function SongDetails({
  track,
  included,
  disabled,
  edit,
  include,
}: {
  track: Track;
  included: boolean;
  disabled: boolean;
  edit: (trigger: HTMLButtonElement) => void;
  include: () => void;
}) {
  const m = track.metadata;
  const fields: [string, string | number | null][] = [
    ['Artist', m.artist],
    ['Album', m.album],
    ['Album artist', m.albumArtist],
    ['Year', m.releaseYear],
    ...(m.trackNumber !== null
      ? [['Track', m.trackNumber] as [string, number]]
      : []),
    ...(m.discNumber !== null
      ? [['Disc', m.discNumber] as [string, number]]
      : []),
  ];
  return (
    <section className="song-details" aria-label="Song details">
      <div className="song-details-heading">
        <div>
          <Text size="sm" fw={600}>
            Song details
          </Text>
          <Text size="xs" c="dimmed">
            These tags travel with your audio.
          </Text>
        </div>
        <Button
          variant="default"
          size="xs"
          leftSection={<IconEdit size={14} />}
          disabled={disabled}
          onClick={(event) => edit(event.currentTarget)}
        >
          Edit song details
        </Button>
      </div>
      <dl className="song-metadata">
        {fields.map(([label, value]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd className={value ? undefined : 'is-missing'}>
              {value || 'Not provided'}
            </dd>
          </div>
        ))}
      </dl>
      {!included && (
        <Checkbox
          label="Include this song in the download"
          checked={false}
          disabled={disabled}
          onChange={include}
        />
      )}
    </section>
  );
}
