import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

const result = await build({
  entryPoints: ['src/trackEdits.ts'],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
});
const { applyBulkEdit, previewBulkEdit, undoBulkEdit } = await import(
  `data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`
);

function track(title, metadata = {}) {
  const sourceMetadata = {
    title,
    artist: 'Original artist',
    album: 'Original album',
    albumArtist: 'Original album artist',
    releaseYear: 2026,
    trackNumber: 7,
    discNumber: 2,
    ...metadata,
  };
  return {
    id: 'repeated-video',
    sourceUrl: 'https://youtube.com/watch?v=repeated',
    uploader: 'Channel',
    duration: 120,
    artworkUrl: null,
    sourceMetadata,
    metadata: { ...sourceMetadata },
  };
}

test('bulk edits target selected occurrences and preserve source tags, ordering and numbers', () => {
  const tracks = [
    track('First', { artist: 'One' }),
    track('Second', { artist: 'Two' }),
    track('Third', { artist: 'Three' }),
  ];
  const before = structuredClone(tracks);
  const applied = applyBulkEdit(tracks, new Set([1, 2]), {
    metadata: { artist: ' New artist ', album: '', releaseYear: null },
  });
  assert.equal(applied.changedCount, 2);
  assert.deepEqual(tracks, before);
  assert.equal(applied.tracks[0], tracks[0]);
  assert.deepEqual(
    applied.tracks.map((item) => item.metadata.title),
    ['First', 'Second', 'Third'],
  );
  for (const index of [1, 2]) {
    const item = applied.tracks[index];
    assert.equal(item.metadata.artist, 'New artist');
    assert.equal(item.metadata.album, '');
    assert.equal(item.metadata.releaseYear, null);
    assert.equal(item.metadata.albumArtist, 'Original album artist');
    assert.equal(item.metadata.trackNumber, 7);
    assert.equal(item.metadata.discNumber, 2);
    assert.deepEqual(item.sourceMetadata, before[index].sourceMetadata);
  }
  assert.deepEqual(undoBulkEdit(applied.tracks, applied.undo), before);
});

test('preview includes unchanged occurrences and every edited field', () => {
  const tracks = [track('First'), track('Second', { artist: 'Different' })];
  const preview = previewBulkEdit(tracks, new Set([0, 1]), {
    metadata: { artist: 'Original artist', albumArtist: '' },
  });
  assert.equal(preview.changedCount, 2);
  assert.deepEqual(preview.errors, []);
  assert.deepEqual(preview.rows[0].changedFields, ['albumArtist']);
  assert.deepEqual(preview.rows[1].changedFields, ['artist', 'albumArtist']);
  assert.equal(preview.rows[0].before.albumArtist, 'Original album artist');
  assert.equal(preview.rows[0].after.albumArtist, '');
});

test('title cleanup removes one exact boundary match, trims the result and preserves nonmatches', () => {
  const tracks = [
    track('Artist - First (Official Audio)'),
    track('Artist - Artist - Second (Official Audio)'),
    track(' artist - Third '),
    track('Fourth (Official Audio) extra'),
    track('Artist - Fifth (Official Audio) (Official Audio)'),
  ];
  const applied = applyBulkEdit(tracks, new Set([0, 1, 2, 3, 4]), {
    metadata: {},
    titleCleanup: { prefix: 'Artist - ', suffix: ' (Official Audio)' },
  });
  assert.deepEqual(
    applied.tracks.map((item) => item.metadata.title),
    [
      'First',
      'Artist - Second',
      ' artist - Third ',
      'Fourth (Official Audio) extra',
      'Fifth (Official Audio)',
    ],
  );
  assert.equal(applied.changedCount, 3);
  assert.deepEqual(undoBulkEdit(applied.tracks, applied.undo), tracks);
});

test('cleanup treats metacharacters and Unicode as literal text', () => {
  const tracks = [track('[.*] 雨の歌 🎵'), track('.* 雨の歌 🎵')];
  const applied = applyBulkEdit(tracks, new Set([0, 1]), {
    metadata: {},
    titleCleanup: { prefix: '[.*] ', suffix: ' 🎵' },
  });
  assert.deepEqual(
    applied.tracks.map((item) => item.metadata.title),
    ['雨の歌', '.* 雨の歌'],
  );
});

test('invalid cleanup rejects the whole transaction without applying metadata', () => {
  const tracks = [track('Artist - '), track('Artist - Song')];
  const before = structuredClone(tracks);
  const edits = {
    metadata: { artist: 'Updated' },
    titleCleanup: { prefix: 'Artist - ', suffix: '' },
  };
  assert.deepEqual(previewBulkEdit(tracks, new Set([0, 1]), edits).errors, [
    'Track 1 would have an empty title.',
  ]);
  assert.throws(
    () => applyBulkEdit(tracks, new Set([0, 1]), edits),
    /Track 1 would have an empty title/,
  );
  assert.deepEqual(tracks, before);
  assert.doesNotThrow(() => applyBulkEdit(tracks, new Set([1]), edits));
});

test('bulk years accept supported whole years and reject invalid values atomically', () => {
  for (const year of [1000, 2026, 9999, null]) {
    const applied = applyBulkEdit([track('Song')], new Set([0]), {
      metadata: { releaseYear: year },
    });
    assert.equal(applied.tracks[0].metadata.releaseYear, year);
  }
  for (const year of [999, 10000, 2026.5, Number.NaN, Infinity]) {
    assert.throws(
      () =>
        applyBulkEdit([track('Song')], new Set([0]), {
          metadata: { releaseYear: year, artist: 'Must not apply' },
        }),
      /Enter a year/,
    );
  }
});

test('no-op edits, empty selection and invalid occurrence indices preserve array identity', () => {
  const tracks = [track('Song')];
  for (const selected of [new Set(), new Set([-1, 1, 10]), new Set([0])]) {
    const applied = applyBulkEdit(tracks, selected, { metadata: {} });
    assert.equal(applied.tracks, tracks);
    assert.equal(applied.changedCount, 0);
    assert.deepEqual(applied.undo, { changes: [] });
  }
  const same = applyBulkEdit(tracks, new Set([0]), {
    metadata: { artist: 'Original artist' },
    titleCleanup: { prefix: '', suffix: '' },
  });
  assert.equal(same.tracks, tracks);
});

test('undo preserves later changes to other fields and skips a subsequently edited field', () => {
  const tracks = [track('Prefix Song')];
  const applied = applyBulkEdit(tracks, new Set([0]), {
    metadata: { artist: 'Bulk artist', album: '' },
    titleCleanup: { prefix: 'Prefix ', suffix: '' },
  });
  const later = applied.tracks.map((item) => ({
    ...item,
    metadata: {
      ...item.metadata,
      artist: 'Later artist',
      trackNumber: 3,
      discNumber: 1,
    },
  }));
  const restored = undoBulkEdit(later, applied.undo);
  assert.equal(restored[0].metadata.title, 'Prefix Song');
  assert.equal(restored[0].metadata.artist, 'Later artist');
  assert.equal(restored[0].metadata.album, 'Original album');
  assert.equal(restored[0].metadata.trackNumber, 3);
  assert.equal(restored[0].metadata.discNumber, 1);
  assert.equal(restored[0].sourceMetadata, tracks[0].sourceMetadata);
});

test('undo guards against an occurrence replaced by a different source', () => {
  const applied = applyBulkEdit([track('Song')], new Set([0]), {
    metadata: { artist: 'Bulk artist' },
  });
  for (const replacement of [
    { ...applied.tracks[0], id: 'different' },
    {
      ...applied.tracks[0],
      sourceUrl: 'https://youtube.com/watch?v=different',
    },
  ]) {
    const tracks = [replacement];
    assert.equal(undoBulkEdit(tracks, applied.undo), tracks);
  }
});

test('undo snapshots do not alias the mutable output metadata', () => {
  const applied = applyBulkEdit([track('Song')], new Set([0]), {
    metadata: { artist: 'Bulk artist' },
  });
  applied.tracks[0].metadata.artist = 'Later direct edit';
  assert.equal(applied.undo.changes[0].after.artist, 'Bulk artist');
  assert.equal(
    undoBulkEdit(applied.tracks, applied.undo)[0].metadata.artist,
    'Later direct edit',
  );
});
