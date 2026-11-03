import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

const result = await build({
  entryPoints: ['src/sourceLinks.ts'],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
});
const { sourceLink, sourceForIntent } = await import(
  `data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`
);
const video = 'eQVkkG8Oz8c';
const playlist = 'OLAK5uy_example';

test('video links default to the song even when shared from an album or playlist', () => {
  for (const input of [
    `https://www.youtube.com/watch?v=${video}&list=${playlist}&index=7`,
    `https://youtu.be/${video}?list=${playlist}&si=share`,
    `https://m.youtube.com/watch?v=${video}&list=${playlist}`,
  ]) {
    const link = sourceLink(input);
    assert.equal(link.songUrl, `https://www.youtube.com/watch?v=${video}`);
    assert.equal(
      link.playlistUrl,
      `https://www.youtube.com/playlist?list=${playlist}`,
    );
    assert.equal(sourceForIntent(input), link.songUrl);
    assert.equal(sourceForIntent(input, 'playlist'), link.playlistUrl);
  }
});

test('YouTube Music song links preserve the Music host for either explicit target', () => {
  const input = `https://music.youtube.com/watch?v=${video}&list=${playlist}`;
  assert.equal(
    sourceForIntent(input),
    `https://music.youtube.com/watch?v=${video}`,
  );
  assert.equal(
    sourceForIntent(input, 'playlist'),
    `https://music.youtube.com/playlist?list=${playlist}`,
  );
});

test('plain songs, explicit playlists and album browse links remain unambiguous', () => {
  const song = sourceLink(`https://youtu.be/${video}?si=share`);
  assert.equal(song.songUrl, `https://www.youtube.com/watch?v=${video}`);
  assert.equal(song.playlistUrl, null);
  const explicit = `https://music.youtube.com/playlist?list=${playlist}`;
  assert.equal(sourceLink(explicit).songUrl, null);
  assert.equal(sourceForIntent(explicit), explicit);
  const browse = 'https://music.youtube.com/browse/MPREb_example';
  assert.equal(sourceLink(browse).songUrl, null);
  assert.equal(sourceForIntent(browse), browse);
});

test('invalid playlist context cannot turn a valid song into a collection', () => {
  const input = `https://youtube.com/watch?v=${video}&list=bad!list`;
  assert.equal(sourceLink(input).playlistUrl, null);
  assert.equal(
    sourceForIntent(input, 'playlist'),
    `https://www.youtube.com/watch?v=${video}`,
  );
});

test('normalization never accepts credentials, unsafe hosts, or malformed video links', () => {
  for (const input of [
    `https://youtube.com.evil.test/watch?v=${video}&list=${playlist}`,
    `https://user:secret@youtube.com/watch?v=${video}`,
    `http://youtube.com/watch?v=${video}`,
    `https://youtube.com:8443/watch?v=${video}`,
    'https://youtube.com/watch?v=short',
    'https://youtu.be/short',
    `https://youtu.be/${video}/extra`,
    `Here is a song https://youtube.com/watch?v=${video}`,
  ]) {
    assert.equal(sourceLink(input), null, input);
    assert.equal(sourceForIntent(input), null, input);
  }
});

test('watch links with malformed provided video IDs cannot resolve to the whole playlist', () => {
  for (const video of ['short', '', 'abcdefghijk-extra']) {
    const input = `https://youtube.com/watch?v=${video}&list=${playlist}`;
    assert.equal(sourceLink(input), null, input);
    assert.equal(sourceForIntent(input), null, input);
    assert.equal(sourceForIntent(input, 'playlist'), null, input);
  }
  assert.equal(
    sourceForIntent(`https://youtube.com/watch?list=${playlist}`),
    `https://www.youtube.com/playlist?list=${playlist}`,
  );
});

test('duplicate video or playlist parameters never choose one target implicitly', () => {
  for (const input of [
    `https://youtube.com/watch?v=${video}&v=abcdefghijk&list=${playlist}`,
    `https://youtube.com/watch?v=${video}&v=${video}`,
    `https://youtube.com/watch?v=${video}&list=${playlist}&list=PLother_playlist`,
    `https://youtube.com/playlist?list=${playlist}&list=${playlist}`,
    `https://youtu.be/${video}?list=${playlist}&list=PLother_playlist`,
  ]) {
    assert.equal(sourceLink(input), null, input);
    assert.equal(sourceForIntent(input, 'playlist'), null, input);
  }
});
