import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

// Exercise draft actions without replacing the production UI's dependencies.
// Focus, scrolling and native controls still require the desktop UI check.
const result = await build({
  entryPoints: ['src/components/BulkTrackEditor.tsx'],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
  plugins: [
    {
      name: 'bulk-editor-fixture',
      setup(build) {
        build.onResolve(
          { filter: /^(react|react\/jsx-runtime|@mantine\/core)$/ },
          ({ path }) => ({ path, namespace: 'fixture' }),
        );
        build.onLoad({ filter: /.*/, namespace: 'fixture' }, ({ path }) => ({
          contents:
            path === 'react'
              ? 'export const useState = (...args) => globalThis.bulkEditorFixture.state(...args);'
              : path === 'react/jsx-runtime'
                ? 'export const Fragment = "Fragment"; export const jsx = (type, props, key) => ({ type, props, key }); export const jsxs = jsx;'
                : `
                  ${['Alert', 'Button', 'Group', 'Modal', 'NumberInput', 'Select', 'Stack', 'Text', 'TextInput'].map((name) => `export const ${name} = '${name}';`).join('\n')}
                  export const ScrollArea = { Autosize: 'ScrollArea.Autosize' };
                  export const Table = Object.assign('Table', Object.fromEntries(['Caption', 'Thead', 'Tr', 'Th', 'Tbody', 'Td'].map(name => [name, 'Table.' + name])));
                `,
        }));
      },
    },
  ],
});
const { BulkTrackEditor } = await import(
  `data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`
);

const tracks = ['Prefix First', 'Prefix Second', 'Hidden third'].map(
  (title, index) => ({
    id: 'repeated-video',
    sourceUrl: 'https://youtube.com/watch?v=repeated',
    sourceMetadata: {},
    metadata: {
      title,
      artist: `Artist ${index + 1}`,
      album: 'Album',
      albumArtist: 'Artist',
      releaseYear: 2026,
      trackNumber: index + 1,
      discNumber: null,
    },
  }),
);

function fixture(overrides = {}) {
  const hooks = [];
  let cursor = 0;
  let tree;
  const applied = [];
  let closes = 0;
  const props = {
    tracks,
    selected: new Set([0, 1]),
    onApply: (edits) => applied.push(edits),
    close: () => closes++,
    ...overrides,
  };
  globalThis.bulkEditorFixture = {
    state(initial) {
      const index = cursor++;
      if (!(index in hooks))
        hooks[index] = typeof initial === 'function' ? initial() : initial;
      return [
        hooks[index],
        (next) => {
          hooks[index] = typeof next === 'function' ? next(hooks[index]) : next;
        },
      ];
    },
  };
  function elements(node) {
    if (Array.isArray(node)) return node.flatMap((child) => elements(child));
    if (!node || typeof node !== 'object') return [];
    if (typeof node.type === 'function') return elements(node.type(node.props));
    return [node, ...elements(node.props?.children)];
  }
  function render() {
    cursor = 0;
    tree = BulkTrackEditor(props);
  }
  function find(type, label) {
    const found = elements(tree).find(
      (node) => node.type === type && (!label || node.props.label === label),
    );
    assert.ok(found, `${type} ${label ?? ''} exists`);
    return found.props;
  }
  function submit() {
    find('form').onSubmit({ preventDefault() {} });
  }
  render();
  return {
    render,
    find,
    submit,
    applied,
    elements: () => elements(tree),
    get closes() {
      return closes;
    },
  };
}

test('bulk editor starts with mixed values kept and cannot submit a no-op', () => {
  const h = fixture();
  assert.equal(h.find('Modal').title, 'Edit 2 selected tracks');
  assert.equal(h.find('Select', 'Artist').description, 'Current: Mixed values');
  assert.equal(h.find('Select', 'Artist').value, 'keep');
  h.submit();
  assert.equal(h.applied.length, 0);
  assert.equal(h.closes, 0);
});

test('set and clear submit exactly one transaction, keeping untouched fields absent', () => {
  const h = fixture();
  h.find('Select', 'Artist').onChange('set');
  h.find('Select', 'Album artist').onChange('clear');
  h.render();
  h.find('TextInput', 'New artist').onChange({
    currentTarget: { value: 'New artist' },
  });
  h.find('TextInput', 'Remove prefix').onChange({
    currentTarget: { value: 'Prefix ' },
  });
  h.render();
  h.submit();
  assert.deepEqual(h.applied, [
    {
      metadata: { artist: 'New artist', albumArtist: '' },
      titleCleanup: { prefix: 'Prefix ', suffix: '' },
    },
  ]);
  assert.equal(h.closes, 1);
  assert.equal(
    h.elements().filter((node) => node.type === 'Table.Td').length,
    18,
    'all three edited fields preview both selected occurrences',
  );
});

test('inherited metadata fields are disabled and cannot mask shared album values', () => {
  const h = fixture({
    inherited: {
      album: 'Shared album',
      albumArtist: 'Shared artist',
      releaseYear: 1999,
    },
  });
  for (const label of ['Album', 'Album artist', 'Year']) {
    const control = h.find('Select', label);
    assert.equal(control.disabled, true);
    assert.match(control.description, /From album details/);
    // Even a stale queued control event must not emit an inherited edit.
    control.onChange('clear');
  }
  h.find('Select', 'Artist').onChange('clear');
  h.render();
  h.submit();
  assert.deepEqual(h.applied, [{ metadata: { artist: '' } }]);
});

test('empty set values and title-erasing cleanup prevent the complete transaction', () => {
  const h = fixture();
  h.find('Select', 'Artist').onChange('set');
  h.render();
  h.submit();
  assert.equal(h.applied.length, 0);
  assert.equal(h.find('Alert').title, 'Check these changes');
  h.find('TextInput', 'New artist').onChange({
    currentTarget: { value: 'Corrected' },
  });
  h.find('TextInput', 'Remove prefix').onChange({
    currentTarget: { value: 'Prefix First' },
  });
  h.render();
  h.submit();
  assert.equal(h.applied.length, 0);
  assert.equal(h.closes, 0);
});

test('busy state blocks a pending submit and closes retain no metadata edits', () => {
  const h = fixture({ busy: true });
  h.find('Select', 'Artist').onChange('clear');
  h.render();
  h.submit();
  assert.equal(h.applied.length, 0);
  assert.equal(h.find('Modal').closeOnEscape, false);
  assert.equal(h.find('Select', 'Artist').disabled, true);
  const cancel = h
    .elements()
    .find((node) => node.type === 'Button' && node.props.children === 'Cancel');
  assert.equal(cancel.props.disabled, true);

  const editable = fixture();
  editable.find('Select', 'Artist').onChange('clear');
  editable.render();
  const cancelEditable = editable
    .elements()
    .find((node) => node.type === 'Button' && node.props.children === 'Cancel');
  cancelEditable.props.onClick();
  assert.equal(editable.applied.length, 0);
  assert.equal(editable.closes, 1);
});
