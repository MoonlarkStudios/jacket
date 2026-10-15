import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

const result = await build({
  entryPoints: ['src/preferences.ts'],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
});
const p = await import(
  `data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`
);
const domainResult = await build({
  entryPoints: ['src/domain.ts'],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
});
const { selectedTracks } = await import(
  `data:text/javascript;base64,${Buffer.from(domainResult.outputFiles[0].text).toString('base64')}`
);
const track = (artist, year = 2026, number = null, disc = null) => ({
  id: 'repeated-id',
  metadata: {
    title: 'Song',
    artist,
    album: 'Album',
    albumArtist: artist,
    releaseYear: year,
    trackNumber: number,
    discNumber: disc,
  },
});
const inspection = (tracks) => ({
  title: 'Album',
  suggestedMode: 'album',
  tracks,
  artworkUrl: null,
});

test('shared defaults require every track to agree and preserve preferred format', () => {
  const agreed = p.inspectionPreferences(
    inspection([track('Artist'), track('Artist')]),
    'mp3',
  );
  assert.equal(agreed.albumArtist, 'Artist');
  assert.equal(agreed.year, 2026);
  assert.equal(agreed.format, 'mp3');
  const conflict = p.inspectionPreferences(
    inspection([track('One'), track('Two', 2025)]),
    'preserve',
  );
  assert.equal(conflict.albumArtist, '');
  assert.equal(conflict.year, null);
  const unknown = p.inspectionPreferences(
    inspection([track('Artist'), track('', null)]),
    'compatible',
  );
  assert.equal(unknown.albumArtist, '');
  assert.equal(unknown.year, null);
});

test('album renames follow the ZIP default but retain a custom filename', () => {
  const before = p.inspectionPreferences(
    inspection([track('Artist')]),
    'compatible',
  );
  const renamed = p.updateAlbumPreferences(
    before,
    { albumTitle: 'New album' },
    'Album',
  );
  assert.equal(renamed.title, 'New album');
  const custom = p.updateAlbumPreferences(
    { ...before, title: 'My archive' },
    { albumTitle: 'New album' },
    'Album',
  );
  assert.equal(custom.title, 'My archive');
  assert.equal(
    p.updateAlbumPreferences(renamed, { albumTitle: '' }, 'Album').title,
    'Album',
  );
});

test('verified album year overrides inconsistent track release years at export while source tags remain intact', () => {
  const album = {
    ...inspection([track('Jaco', null), track('Jaco', 2000)]),
    albumReleaseYear: 1976,
  };
  for (const item of album.tracks)
    item.sourceMetadata = structuredClone(item.metadata);
  const defaults = p.inspectionPreferences(album, 'compatible');
  assert.equal(defaults.year, 1976);
  const exported = selectedTracks(album.tracks, new Set([0, 1]), 'album', {
    title: defaults.albumTitle,
    artist: defaults.albumArtist,
    year: defaults.year,
  });
  assert.deepEqual(
    exported.map((t) => t.metadata.releaseYear),
    [1976, 1976],
  );
  assert.equal(exported[1].sourceMetadata.releaseYear, 2000);
  assert.equal(
    p.inspectionPreferences(
      { ...album, suggestedMode: 'collection' },
      'compatible',
    ).year,
    null,
  );
});

test('album downloads use the collection cover on every track and prefer the album name', () => {
  const album = {
    ...inspection([track('Artist'), track('Artist')]),
    title: 'Album - Artist',
    artworkUrl: 'https://i9.ytimg.com/s_p/album/sddefault.jpg?rs=signed',
  };
  album.tracks[0].artworkUrl = 'https://i.ytimg.com/vi/first/maxresdefault.jpg';
  album.tracks[1].artworkUrl =
    'https://i.ytimg.com/vi/second/maxresdefault.jpg';
  const preferences = p.inspectionPreferences(album, 'compatible');
  assert.equal(preferences.artworkMode, 'shared');
  assert.equal(preferences.sharedArtwork, album.artworkUrl);
  assert.equal(preferences.title, 'Album');
  const playlist = p.inspectionPreferences(
    { ...album, suggestedMode: 'collection' },
    'compatible',
  );
  assert.equal(playlist.artworkMode, 'perTrack');
  assert.equal(playlist.title, 'Album - Artist');
});

test('numbering restarts per disc, preserves occurrences, and keeps source metadata', () => {
  const original = [
    track('Artist', 2026, 7, 1),
    track('Artist', 2026, null, 1),
    track('Artist', 2026, null, 2),
  ];
  original[0].sourceMetadata = structuredClone(original[0].metadata);
  const numbered = p.numberAlbumTracks(original);
  assert.deepEqual(
    numbered.map((t) => t.metadata.trackNumber),
    [1, 2, 1],
  );
  assert.equal(numbered.length, 3);
  assert.equal(original[0].metadata.trackNumber, 7);
  assert.equal(numbered[0].sourceMetadata.trackNumber, 7);
});

test('album defaults fill only missing numbers in disc order without changing source tags', () => {
  const original = [
    track('Artist', 2026, null, 1),
    track('Artist', 2026, 2, 1),
    track('Artist', 2026, null, 1),
    track('Artist', 2026, null, 2),
    track('Artist', 2026, 8, 2),
  ];
  for (const item of original)
    item.sourceMetadata = structuredClone(item.metadata);
  const prepared = p.withAlbumTrackNumbers(inspection(original));
  assert.deepEqual(
    prepared.tracks.map((t) => t.metadata.trackNumber),
    [1, 2, 3, 1, 8],
  );
  assert.deepEqual(
    original.map((t) => t.metadata.trackNumber),
    [null, 2, null, null, 8],
  );
  assert.equal(prepared.tracks[0].sourceMetadata.trackNumber, null);
  assert.equal(prepared.tracks.length, original.length);
  assert.deepEqual(
    selectedTracks(prepared.tracks, new Set([0, 2, 3, 4]), 'album', {
      title: 'Album',
      artist: 'Artist',
      year: null,
    }).map((t) => t.metadata.trackNumber),
    [1, 3, 1, 8],
  );
});

test('playlist defaults remain unchanged and switching to album fills numbers once', () => {
  const playlist = {
    ...inspection([track('Artist'), track('Artist')]),
    suggestedMode: 'collection',
  };
  assert.equal(p.withAlbumTrackNumbers(playlist), playlist);
  const album = p.withAlbumTrackNumbers(playlist, 'album');
  assert.deepEqual(
    album.tracks.map((t) => t.metadata.trackNumber),
    [1, 2],
  );
  assert.deepEqual(
    p
      .withAlbumTrackNumbers(album, 'album')
      .tracks.map((t) => t.metadata.trackNumber),
    [1, 2],
  );
});

test('archive paths work on macOS and Windows and filenames are sanitized', () => {
  assert.equal(
    p.archiveDirectory('/Users/me/Music/Album.zip'),
    '/Users/me/Music/',
  );
  assert.equal(p.archiveDirectory('C:\\Music\\Album.zip'), 'C:\\Music\\');
  assert.equal(p.archiveDirectory('Album.zip'), '');
  assert.equal(p.archiveFilename('Album / live'), 'Album _ live.zip');
  assert.equal(p.archiveFilename('  '), 'Audio collection.zip');
});

test('unrecognized formats and unavailable storage fall back safely', () => {
  assert.equal(p.audioFormat('invalid'), 'compatible');
  assert.equal(p.audioFormat(null), 'compatible');
  assert.equal(p.readPreference('format'), null);
  assert.doesNotThrow(() => p.writePreference('format', 'mp3'));
});

test('pasting starts inspection only for complete supported source links', () => {
  const album =
    'https://music.youtube.com/playlist?list=OLAK5uy_mUJg8SRBv0XUvkn4To7sIQs1TBjAhJYE4';
  assert.equal(p.pastedSourceUrl(`  ${album}\n`), album);
  assert.equal(
    p.pastedSourceUrl('https://youtu.be/eQVkkG8Oz8c?si=example'),
    'https://youtu.be/eQVkkG8Oz8c?si=example',
  );
  assert.equal(
    p.pastedSourceUrl('https://music.youtube.com/browse/MPREb_example'),
    'https://music.youtube.com/browse/MPREb_example',
  );
  for (const input of [
    'https://music.youtube.com/',
    'https://youtube.com/watch?v=short',
    'https://youtube.com/playlist?list=PL',
    'https://youtube.com.evil.test/watch?v=eQVkkG8Oz8c',
    'https://user:password@youtube.com/watch?v=eQVkkG8Oz8c',
    'http://youtube.com/watch?v=eQVkkG8Oz8c',
    'Here is an album https://youtube.com/watch?v=eQVkkG8Oz8c',
  ]) {
    assert.equal(p.pastedSourceUrl(input), null, input);
  }
});

test('restart recovery preserves a custom destination name, cleared fields, numbering gaps and mixed track metadata', () => {
  const tracks = [track('One', null, 3), track('Two', 2000, null)];
  tracks[0].metadata.album = 'First album';
  tracks[1].metadata.album = 'Second album';
  for (const item of tracks) {
    item.metadata.albumArtist = '';
    item.sourceMetadata = structuredClone(item.metadata);
  }
  const plan = {
    title: 'My custom archive',
    mode: 'album',
    format: 'preserve',
    artworkMode: 'none',
    sharedArtworkUrl: null,
    tracks,
  };
  const preferences = p.recoveryPreferences(plan);
  assert.equal(preferences.title, 'My custom archive');
  assert.equal(preferences.format, 'preserve');
  assert.equal(preferences.albumTitle, '');
  assert.equal(preferences.albumArtist, '');
  assert.equal(preferences.year, null);
  assert.deepEqual(
    selectedTracks(tracks, new Set([0, 1]), preferences.mode, {
      title: preferences.albumTitle,
      artist: preferences.albumArtist,
      year: preferences.year,
    }).map((t) => t.metadata),
    tracks.map((t) => t.metadata),
  );
});
