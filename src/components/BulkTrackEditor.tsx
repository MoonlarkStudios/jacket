import {
  Alert,
  Button,
  Group,
  Modal,
  NumberInput,
  ScrollArea,
  Select,
  Stack,
  Table,
  Text,
  TextInput,
} from '@mantine/core';
import { useState } from 'react';
import type { Track } from '../domain';
import {
  type BulkMetadataField,
  type BulkTrackEdits,
  type EditableMetadataField,
  previewBulkEdit,
} from '../trackEdits';

type EditAction = 'keep' | 'set' | 'clear';
const fields = [
  ['artist', 'Artist'],
  ['album', 'Album'],
  ['albumArtist', 'Album artist'],
  ['releaseYear', 'Year'],
] as const;
const fieldLabels: Record<EditableMetadataField, string> = {
  title: 'Title',
  artist: 'Artist',
  album: 'Album',
  albumArtist: 'Album artist',
  releaseYear: 'Year',
};
const actions = [
  { value: 'keep', label: 'Keep each track’s value' },
  { value: 'set', label: 'Set a new value' },
  { value: 'clear', label: 'Clear this field' },
];

function displayValue(value: string | number | null): string {
  return value === null || value === '' ? 'Not set' : String(value);
}

export function BulkTrackEditor({
  tracks,
  selected,
  inherited = {},
  busy = false,
  onApply,
  close,
}: {
  tracks: Track[];
  selected: ReadonlySet<number>;
  inherited?: Partial<{
    album: string;
    albumArtist: string;
    releaseYear: number;
  }>;
  busy?: boolean;
  onApply: (edits: BulkTrackEdits) => void;
  close: () => void;
}) {
  const [fieldActions, setFieldActions] = useState<
    Record<BulkMetadataField, EditAction>
  >({
    artist: 'keep',
    album: 'keep',
    albumArtist: 'keep',
    releaseYear: 'keep',
  });
  const [values, setValues] = useState({
    artist: '',
    album: '',
    albumArtist: '',
    releaseYear: '' as string | number,
  });
  const [prefix, setPrefix] = useState('');
  const [suffix, setSuffix] = useState('');
  const selectedTracks = tracks.filter((_, index) => selected.has(index));
  const edits: BulkTrackEdits = { metadata: {} };
  const invalidFields: string[] = [];
  const previewFields: EditableMetadataField[] = [];
  if (prefix || suffix) {
    edits.titleCleanup = { prefix, suffix };
    previewFields.push('title');
  }
  for (const [field, label] of fields) {
    const inheritedValue = field === 'artist' ? undefined : inherited[field];
    if (inheritedValue !== undefined || fieldActions[field] === 'keep')
      continue;
    previewFields.push(field);
    if (field === 'releaseYear') {
      const value = values.releaseYear;
      if (fieldActions[field] === 'clear') edits.metadata.releaseYear = null;
      else if (typeof value === 'number') edits.metadata.releaseYear = value;
      else invalidFields.push('Enter a year, or choose Clear.');
    } else if (fieldActions[field] === 'clear') edits.metadata[field] = '';
    else {
      edits.metadata[field] = values[field];
      if (!values[field].trim())
        invalidFields.push(
          `Enter a value for ${label.toLowerCase()}, or choose Clear.`,
        );
    }
  }
  const preview = previewBulkEdit(tracks, selected, edits);
  const errors = [...invalidFields, ...preview.errors];
  const canApply = !busy && errors.length === 0 && preview.changedCount > 0;

  return (
    <Modal
      returnFocus={false}
      opened
      onClose={close}
      title={`Edit ${selectedTracks.length} selected ${selectedTracks.length === 1 ? 'track' : 'tracks'}`}
      centered
      size="lg"
      scrollAreaComponent={ScrollArea.Autosize}
      closeButtonProps={{ disabled: busy }}
      closeOnEscape={!busy}
      closeOnClickOutside={!busy}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (!canApply) return;
          onApply(edits);
          close();
        }}
      >
        <Stack gap="lg">
          <Text size="sm" c="dimmed">
            Changes apply to checked tracks, including any hidden by search.
            Keep leaves each track’s own value. Track and disc numbers stay as
            they are.
          </Text>
          <Stack gap="md">
            {fields.map(([field, label], index) => {
              const inheritedValue =
                field === 'artist' ? undefined : inherited[field];
              const currentValues = new Set(
                selectedTracks.map((track) => track.metadata[field]),
              );
              const current =
                currentValues.size > 1
                  ? 'Mixed values'
                  : displayValue([...currentValues][0] ?? null);
              return (
                <div key={field}>
                  <Select
                    label={label}
                    data-autofocus={index === 0 || undefined}
                    value={fieldActions[field]}
                    data={actions}
                    allowDeselect={false}
                    disabled={busy || inheritedValue !== undefined}
                    description={
                      inheritedValue !== undefined
                        ? `From album details: ${displayValue(inheritedValue)}. Change it in Album details.`
                        : `Current: ${current}`
                    }
                    onChange={(value) =>
                      setFieldActions((previous) => ({
                        ...previous,
                        [field]: value as EditAction,
                      }))
                    }
                  />
                  {inheritedValue === undefined &&
                    fieldActions[field] === 'set' &&
                    (field === 'releaseYear' ? (
                      <NumberInput
                        mt={8}
                        label="New year"
                        value={values.releaseYear}
                        min={1000}
                        max={9999}
                        allowDecimal={false}
                        disabled={busy}
                        onChange={(value) =>
                          setValues((previous) => ({
                            ...previous,
                            releaseYear: value,
                          }))
                        }
                      />
                    ) : (
                      <TextInput
                        mt={8}
                        label={`New ${label.toLowerCase()}`}
                        value={values[field]}
                        disabled={busy}
                        onChange={(event) => {
                          const value = event.currentTarget.value;
                          setValues((previous) => ({
                            ...previous,
                            [field]: value,
                          }));
                        }}
                      />
                    ))}
                </div>
              );
            })}
          </Stack>
          <Stack gap="sm">
            <Text fw={500} size="sm">
              Clean up titles
            </Text>
            <Text size="sm" c="dimmed">
              Remove exact text from the start or end of each title. Matching is
              case sensitive. Unmatched titles stay as they are.
            </Text>
            <TextInput
              label="Remove prefix"
              placeholder="For example, Artist - "
              value={prefix}
              disabled={busy}
              onChange={(event) => setPrefix(event.currentTarget.value)}
            />
            <TextInput
              label="Remove suffix"
              placeholder="For example,  (Official Audio)"
              value={suffix}
              disabled={busy}
              onChange={(event) => setSuffix(event.currentTarget.value)}
            />
          </Stack>
          <div>
            <Text size="sm" fw={500} role="status" mb={8}>
              {preview.changedCount} of {selectedTracks.length} selected tracks
              will change
            </Text>
            {previewFields.length > 0 && (
              <ScrollArea.Autosize mah={260} type="auto">
                <Table
                  withTableBorder
                  styles={{
                    table: { tableLayout: 'fixed' },
                    td: { overflowWrap: 'anywhere', verticalAlign: 'top' },
                    th: { overflowWrap: 'anywhere' },
                  }}
                >
                  <Table.Caption>Preview of selected track edits</Table.Caption>
                  <Table.Thead>
                    <Table.Tr>
                      <Table.Th w="22%">Field</Table.Th>
                      <Table.Th>Before</Table.Th>
                      <Table.Th>After</Table.Th>
                    </Table.Tr>
                  </Table.Thead>
                  <Table.Tbody>
                    {preview.rows.map((row) => (
                      <TrackPreview
                        key={row.index}
                        row={row}
                        fields={previewFields}
                      />
                    ))}
                  </Table.Tbody>
                </Table>
              </ScrollArea.Autosize>
            )}
          </div>
          {errors.length > 0 && (
            <Alert color="red" title="Check these changes" role="alert">
              {errors.join(' ')}
            </Alert>
          )}
          <Group justify="flex-end" wrap="wrap">
            <Button variant="default" onClick={close} disabled={busy}>
              Cancel
            </Button>
            <Button type="submit" disabled={!canApply}>
              Apply to {preview.changedCount}{' '}
              {preview.changedCount === 1 ? 'track' : 'tracks'}
            </Button>
          </Group>
        </Stack>
      </form>
    </Modal>
  );
}

function TrackPreview({
  row,
  fields: previewFields,
}: {
  row: ReturnType<typeof previewBulkEdit>['rows'][number];
  fields: EditableMetadataField[];
}) {
  return (
    <>
      <Table.Tr>
        <Table.Th colSpan={3}>
          Track {row.index + 1}: {row.before.title}
        </Table.Th>
      </Table.Tr>
      {previewFields.map((field) => (
        <Table.Tr key={field}>
          <Table.Td>{fieldLabels[field]}</Table.Td>
          <Table.Td>{displayValue(row.before[field])}</Table.Td>
          <Table.Td>
            {displayValue(row.after[field])}
            {!row.changedFields.includes(field) && (
              <Text component="span" size="xs" c="dimmed">
                {' '}
                (unchanged)
              </Text>
            )}
          </Table.Td>
        </Table.Tr>
      ))}
    </>
  );
}
