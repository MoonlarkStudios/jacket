import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

// Render the real App and SongDetails together. Stub native boundaries and
// unrelated dialogs; focus and browser layout remain desktop acceptance work.
const result = await build({
  stdin: {
    contents:
      "export { default as App } from './src/App'; export { songExample } from './src/example'; export { inspectionPreferences } from './src/preferences';",
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
              /^(react(\/jsx-runtime)?|@mantine\/core|@tabler\/icons-react|\.\/useWorkspace|\.\/useDesktopShell|\.\/desktop|\.\/components\/(BulkTrackEditor|DownloadFeedback|ExportPreviewDialog|SourceField|WorkspaceDialogs))$/,
          },
          ({ path }) => ({ path, namespace: 'fixture' }),
        );
        build.onLoad({ filter: /.*/, namespace: 'fixture' }, ({ path }) => {
          const symbols = {
            '@mantine/core':
              'ActionIcon Alert Button Checkbox Group Image Text TextInput Title Tooltip',
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
              ? 'export const useState = (initial) => [typeof initial === "function" ? initial() : initial, () => {}]; export const useRef = (current) => ({current}); export const useEffect = () => {};'
              : path === 'react/jsx-runtime'
                ? 'export const Fragment = "Fragment"; export const jsx = (type, props) => ({type, props}); export const jsxs = jsx;'
                : path === './useWorkspace'
                  ? 'export const useWorkspace = () => globalThis.songReviewFixture.workspace;'
                  : path === './useDesktopShell'
                    ? 'export const useDesktopShell = () => {};'
                    : path === './desktop'
                      ? 'export const desktopAvailable = true; export const artworkAsset = () => Promise.resolve(null); export const openPlaylist = () => Promise.resolve(); export const revealArchive = () => Promise.resolve();'
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
const { App, songExample, inspectionPreferences } = await import(
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
  outputKind = 'folder',
  mode = 'collection',
  exampleMode = false,
  selected = new Set([0]),
} = {}) {
  const loads = [];
  let exports = 0;
  globalThis.songReviewFixture = {
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
      toolsReady: true,
      eventsReady: true,
      draftReady: true,
      loadInspection: (...args) => loads.push(args),
      exportZip: () => exports++,
    },
  };
  return {
    nodes: elements(App()),
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

test('a completed song review shows song metadata, its edit action, and the matching folder or ZIP action', () => {
  for (const outputKind of ['folder', 'zip']) {
    const h = fixture({ outputKind });
    const nodes = h.nodes;
    assert.ok(labelled(nodes, 'Song details'));
    assert.equal(labelled(nodes, 'Collection tracks'), undefined);
    const action = button(
      nodes,
      outputKind === 'folder' ? 'Save song…' : 'Download song…',
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
