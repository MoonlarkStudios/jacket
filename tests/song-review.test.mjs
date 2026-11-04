import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

// Render the real App and SongDetails together. Stub native boundaries and
// unrelated dialogs; focus and browser layout remain desktop acceptance work.
const result = await build({
  stdin: {
    contents:
      "export { default as App } from './src/App'; export { songExample } from './src/example'; export { inspectionPreferences } from './src/preferences'; export { ExportPreviewDialog } from './src/components/ExportPreviewDialog';",
    resolveDir: process.cwd(),
  },
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
  define: { 'import.meta.env.DEV': 'true' },
  plugins: [
    {
      name: 'song-review-fixture',
      setup(build) {
        build.onResolve(
          {
            filter:
              /^(react(\/jsx-runtime)?|@mantine\/core|@tabler\/icons-react|\.\/useWorkspace|\.\/useDesktopShell|\.\.?\/desktop|\.\/components\/(BulkTrackEditor|DownloadFeedback|ExportPreviewDialog|SourceField|WorkspaceDialogs))$/,
          },
          ({ path }) => ({ path, namespace: 'fixture' }),
        );
        build.onLoad({ filter: /.*/, namespace: 'fixture' }, ({ path }) => {
          const symbols = {
            '@mantine/core':
              'ActionIcon Alert Button Checkbox Group Image Loader Modal ScrollArea Stack Text TextInput Title Tooltip',
            '@tabler/icons-react':
              'IconArrowRight IconCheck IconChevronDown IconDownload IconEdit IconFolder IconLoader2 IconMusic IconPlus IconRotateClockwise IconSearch IconSettings IconVinyl IconX',
            './components/BulkTrackEditor': 'BulkTrackEditor',
            './components/DownloadFeedback':
              'DownloadFeedback SourceErrorMessage',
            './components/ExportPreviewDialog': 'ExportPreviewDialog',
            './components/SourceField': 'SourceField',
            './components/WorkspaceDialogs': 'WorkspaceDialogs TrackEditor',
          };
          const contents =
            path === 'react'
              ? 'export const useState = (initial) => [globalThis.songReviewFixture.previewStates?.length ? globalThis.songReviewFixture.previewStates.shift() : typeof initial === "function" ? initial() : initial, () => {}]; export const useRef = (current) => ({current}); export const useEffect = () => {};'
              : path === 'react/jsx-runtime'
                ? 'export const Fragment = "Fragment"; export const jsx = (type, props) => ({type, props}); export const jsxs = jsx;'
                : path === './useWorkspace'
                  ? 'export const useWorkspace = () => globalThis.songReviewFixture.workspace;'
                  : path === './useDesktopShell'
                    ? 'export const useDesktopShell = () => {};'
                    : path.endsWith('/desktop')
                      ? 'export const desktopAvailable = true; export const artworkAsset = () => Promise.resolve(null); export const previewExport = () => Promise.resolve(null); export const openPlaylist = (path) => { globalThis.songReviewFixture.played.push(path); return Promise.resolve(); }; export const revealArchive = (path) => { globalThis.songReviewFixture.revealed.push(path); return Promise.resolve(); };'
                      : symbols[path]
                          .split(' ')
                          .map((name) => `export const ${name} = '${name}';`)
                          .join('\n');
          return { contents };
        });
      },
    },
  ],
});
const { App, songExample, inspectionPreferences, ExportPreviewDialog } =
  await import(
    `data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`
  );

const song = structuredClone(songExample);
const metadata = song.tracks[0].metadata;
Object.assign(metadata, {
  title: 'The song title',
  artist: 'Recording artist',
  albumArtist: 'Different album artist',
});

function fixture({
  inspection = song,
  outputKind = inspection?.suggestedMode === 'collection' &&
  inspection.tracks.length === 1
    ? 'file'
    : 'folder',
  mode = 'collection',
  exampleMode = false,
  selected = new Set([0]),
  progress = null,
} = {}) {
  const loads = [];
  let exports = 0;
  globalThis.songReviewFixture = {
    revealed: [],
    played: [],
    workspace: {
      inspection,
      inspectionStatus: inspection ? 'complete' : 'idle',
      selected,
      chosen:
        inspection?.tracks.filter((_, index) => selected.has(index)) ?? [],
      preferences: {
        ...inspectionPreferences(inspection || song, 'compatible'),
        title: 'My custom export name',
        mode,
        outputKind,
      },
      exampleMode,
      progress,
      toolsReady: true,
      eventsReady: true,
      draftReady: true,
      loadInspection: (...args) => loads.push(args),
      exportZip: () => exports++,
    },
  };
  return {
    nodes: elements(App()),
    revealed: globalThis.songReviewFixture.revealed,
    played: globalThis.songReviewFixture.played,
    loads,
    get exports() {
      return exports;
    },
  };
}

function elements(node) {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!node || typeof node !== 'object') return [];
  return [
    node,
    ...elements(
      typeof node.type === 'function'
        ? node.type(node.props)
        : node.props?.children,
    ),
  ];
}
function text(node) {
  if (Array.isArray(node)) return node.map(text).join('');
  if (node && typeof node === 'object') return text(node.props?.children);
  return typeof node === 'string' || typeof node === 'number'
    ? String(node)
    : '';
}
function labelled(nodes, label) {
  return nodes.find((node) => node.props?.['aria-label'] === label);
}
function button(nodes, label) {
  const found = nodes.find(
    (node) => ['Button', 'button'].includes(node.type) && text(node) === label,
  );
  assert.ok(found, `${label} action exists`);
  return found.props;
}

test('a completed song review offers audio-file and folder saves plus ZIP downloads', () => {
  for (const outputKind of ['file', 'folder', 'zip']) {
    const h = fixture({ outputKind });
    const nodes = h.nodes;
    assert.ok(labelled(nodes, 'Song details'));
    assert.equal(labelled(nodes, 'Collection tracks'), undefined);
    const action = button(
      nodes,
      outputKind === 'zip' ? 'Download song…' : 'Save song…',
    );
    assert.equal(action.disabled, false);
    action.onClick();
    assert.equal(h.exports, 1);
    assert.equal(button(nodes, 'Edit song details').disabled, false);
  }
});

test('song heading displays recording title and artist independently of album artist and export name', () => {
  const nodes = fixture().nodes;
  const heading = nodes.find(
    (node) => node.type === 'Title' && node.props.order === 1,
  );
  assert.equal(text(heading), metadata.title);
  const copy = nodes.find(
    (node) => node.props?.className === 'collection-copy',
  );
  assert.match(text(copy), /Recording artist/);
  assert.doesNotMatch(text(copy), /Different album artist/);
  assert.doesNotMatch(text(copy), /My custom export name/);
  assert.ok(labelled(nodes, 'Rename My custom export name'));
});

test('an album with only one selected track retains the album review and download action', () => {
  const album = {
    ...song,
    suggestedMode: 'album',
    tracks: [song.tracks[0], { ...song.tracks[0], id: 'second' }],
  };
  const nodes = fixture({
    inspection: album,
    mode: 'album',
    selected: new Set([1]),
  }).nodes;
  assert.ok(labelled(nodes, 'Collection tracks'));
  assert.equal(labelled(nodes, 'Song details'), undefined);
  assert.equal(button(nodes, 'Save album…').disabled, false);
  button(nodes, 'Edit album details');
});

test('start examples expose both song and album paths and never enable example downloads', () => {
  const h = fixture({ inspection: null });
  const nodes = h.nodes;
  const examples = nodes.filter(
    (node) =>
      node.type === 'button' && /Try a song|Explore an album/.test(text(node)),
  );
  assert.equal(examples.length, 2);
  for (const example of examples) example.props.onClick();
  assert.equal(h.loads[0][0].tracks.length, 1);
  assert.equal(h.loads[0][0].suggestedMode, 'collection');
  assert.ok(h.loads[1][0].tracks.length > 1);
  assert.equal(h.loads[1][0].suggestedMode, 'album');
  assert.ok(h.loads.every(([, isExample]) => isExample === true));
  assert.equal(
    button(fixture({ exampleMode: true }).nodes, 'Save song…').disabled,
    true,
  );
});

test('single-audio output has a clear summary and cannot save or preview zero or multiple selections', () => {
  const nodes = fixture().nodes;
  const summary = nodes.find(
    (node) => node.props?.className === 'export-options-summary',
  );
  assert.match(text(summary), /Audio file/);
  assert.doesNotMatch(text(summary), /Folder|ZIP/);
  const album = {
    ...song,
    suggestedMode: 'album',
    tracks: [song.tracks[0], { ...song.tracks[0], id: 'second' }],
  };
  for (const selected of [new Set(), new Set([0, 1])]) {
    const nodes = fixture({
      inspection: album,
      mode: 'album',
      outputKind: 'file',
      selected,
    }).nodes;
    assert.equal(
      nodes.find((node) => node.props?.className === 'save-button').props
        .disabled,
      true,
    );
    assert.equal(button(nodes, 'Preview files').disabled, true);
  }
  const single = fixture({
    inspection: album,
    mode: 'album',
    outputKind: 'file',
    selected: new Set([1]),
  }).nodes;
  assert.equal(
    single.find((node) => node.props?.className === 'save-button').props
      .disabled,
    false,
  );
});

test('a completed audio file reports the song ready and reveals the actual file without playlist playback', async () => {
  const outputPath = '/Music/Actual song (2).m4a';
  const h = fixture({
    progress: {
      stage: 'complete',
      result: {
        outputKind: 'file',
        outputPath,
        trackCount: 1,
        warnings: [],
        playlistPath: null,
      },
    },
  });
  const success = h.nodes.find(
    (node) => node.props?.className === 'success-copy',
  );
  assert.match(text(success), /Your song is ready/);
  assert.match(text(success), /Actual song \(2\).m4a/);
  const reveal = h.nodes.find(
    (node) =>
      node.type === 'Button' &&
      /Show in Finder|Show in folder/.test(text(node)),
  );
  assert.ok(reveal);
  await reveal.props.onClick();
  assert.deepEqual(h.revealed, [outputPath]);
  assert.deepEqual(h.played, []);
  assert.equal(
    h.nodes.some((node) => text(node) === 'Open playlist'),
    false,
  );
});

test('audio-file preview shows the file without a directory slash or empty companion-file section', () => {
  const filename = 'Artist - Song.mp3';
  globalThis.songReviewFixture = {
    previewStates: [
      {
        rootName: filename,
        outputKind: 'file',
        tracks: [
          {
            relativePath: filename,
            metadata,
            artworkPath: null,
            extensionProvisional: false,
          },
        ],
        files: [filename],
        warnings: [],
      },
      null,
    ],
  };
  const nodes = elements(
    ExportPreviewDialog({
      plan: {
        title: 'Artist - Song',
        outputKind: 'file',
        tracks: song.tracks,
        auth: { browser: null },
      },
      cover: null,
      close() {},
    }),
  );
  const heading = nodes.find(
    (node) => node.props?.className === 'file-preview-name',
  );
  assert.equal(text(heading), filename);
  assert.equal(
    nodes.some((node) => text(node) === 'Also included'),
    false,
  );
  assert.ok(nodes.some((node) => /Audio file/.test(text(node))));
});
