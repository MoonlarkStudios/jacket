import type { Metadata, Track } from './domain';

export type BulkMetadataField =
  | 'artist'
  | 'album'
  | 'albumArtist'
  | 'releaseYear';
export type EditableMetadataField = BulkMetadataField | 'title';

export interface BulkTrackEdits {
  // Absent fields retain each occurrence's own value. Empty strings and null
  // explicitly clear a field; track and disc numbers are never bulk edited.
  metadata: Partial<Pick<Metadata, BulkMetadataField>>;
  titleCleanup?: { prefix: string; suffix: string };
}

export interface BulkEditPreview {
  index: number;
  before: Metadata;
  after: Metadata;
  changedFields: EditableMetadataField[];
}

interface BulkEditChange extends BulkEditPreview {
  id: string;
  sourceUrl: string;
}

export interface BulkEditUndo {
  changes: BulkEditChange[];
}

const metadataFields: BulkMetadataField[] = [
  'artist',
  'album',
  'albumArtist',
  'releaseYear',
];
const editableFields: EditableMetadataField[] = ['title', ...metadataFields];

export function previewBulkEdit(
  tracks: readonly Track[],
  selected: ReadonlySet<number>,
  edits: BulkTrackEdits,
): { rows: BulkEditPreview[]; changedCount: number; errors: string[] } {
  const errors: string[] = [];
  const year = edits.metadata.releaseYear;
  if (
    year !== undefined &&
    year !== null &&
    (!Number.isInteger(year) || year < 1000 || year > 9999)
  )
    errors.push('Enter a year from 1000 to 9999, or choose Clear.');

  const rows = tracks.flatMap((track, index): BulkEditPreview[] => {
    if (!selected.has(index)) return [];
    const before = track.metadata;
    const after = { ...before };
    for (const field of metadataFields) {
      const value = edits.metadata[field];
      if (value === undefined) continue;
      if (field === 'releaseYear') after.releaseYear = year ?? null;
      else if (typeof value === 'string') after[field] = value.trim();
    }
    const cleanup = edits.titleCleanup;
    if (cleanup) {
      let title = before.title;
      if (cleanup.prefix && title.startsWith(cleanup.prefix))
        title = title.slice(cleanup.prefix.length);
      if (cleanup.suffix && title.endsWith(cleanup.suffix))
        title = title.slice(0, -cleanup.suffix.length);
      // A nonmatching cleanup never silently changes unrelated whitespace.
      if (title !== before.title) {
        after.title = title.trim();
        if (!after.title)
          errors.push(`Track ${index + 1} would have an empty title.`);
      }
    }
    return [
      {
        index,
        before: { ...before },
        after,
        changedFields: editableFields.filter(
          (field) => after[field] !== before[field],
        ),
      },
    ];
  });
  return {
    rows,
    changedCount: rows.filter((row) => row.changedFields.length > 0).length,
    errors,
  };
}

export function applyBulkEdit(
  tracks: Track[],
  selected: ReadonlySet<number>,
  edits: BulkTrackEdits,
): { tracks: Track[]; undo: BulkEditUndo; changedCount: number } {
  const preview = previewBulkEdit(tracks, selected, edits);
  if (preview.errors.length) throw new Error(preview.errors.join(' '));
  const changes = preview.rows
    .filter((row) => row.changedFields.length > 0)
    .map((row) => ({
      ...row,
      id: tracks[row.index].id,
      sourceUrl: tracks[row.index].sourceUrl,
    }));
  const byIndex = new Map(changes.map((change) => [change.index, change]));
  return {
    tracks: changes.length
      ? tracks.map((track, index) => {
          const change = byIndex.get(index);
          return change ? { ...track, metadata: { ...change.after } } : track;
        })
      : tracks,
    undo: { changes },
    changedCount: changes.length,
  };
}

export function undoBulkEdit(tracks: Track[], undo: BulkEditUndo): Track[] {
  const byIndex = new Map(undo.changes.map((change) => [change.index, change]));
  let reverted = false;
  const restored = tracks.map((track, index) => {
    const change = byIndex.get(index);
    if (
      !change ||
      track.id !== change.id ||
      track.sourceUrl !== change.sourceUrl
    )
      return track;
    const metadata = { ...track.metadata };
    let changed = false;
    for (const field of change.changedFields) {
      // Preserve subsequent edits, including a later change to this field.
      if (metadata[field] !== change.after[field]) continue;
      if (field === 'releaseYear')
        metadata.releaseYear = change.before.releaseYear;
      else metadata[field] = change.before[field];
      changed = true;
    }
    if (!changed) return track;
    reverted = true;
    return { ...track, metadata };
  });
  return reverted ? restored : tracks;
}
