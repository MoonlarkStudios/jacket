import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from 'esbuild';

const result = await build({
  entryPoints: ['src/components/SourceField.tsx'],
  bundle: true,
  write: false,
  platform: 'node',
  format: 'esm',
  loader: { '.css': 'empty' },
  plugins: [
    {
      name: 'source-field-fixture',
      setup(build) {
        build.onResolve(
          {
            filter:
              /^(react(\/jsx-runtime)?|@mantine\/core|@tabler\/icons-react)$/,
          },
          ({ path }) => ({ path, namespace: 'fixture' }),
        );
        build.onLoad({ filter: /.*/, namespace: 'fixture' }, ({ path }) => ({
          contents:
            path === 'react'
              ? 'export const useId = () => "fixture-id"; export const useState = (initial) => globalThis.sourceFieldFixture.state(initial);'
              : path === 'react/jsx-runtime'
                ? 'export const jsx = (type, props) => ({ type, props }); export const jsxs = jsx;'
                : (path === '@mantine/core'
                    ? ['Button', 'SegmentedControl', 'TextInput', 'Tooltip']
                    : ['IconArrowRight', 'IconLink', 'IconLoader2', 'IconX']
                  )
                    .map((name) => `export const ${name} = '${name}';`)
                    .join('\n'),
        }));
      },
    },
  ],
});
const { SourceField } = await import(
  `data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`
);

function fixture() {
  let choice = null;
  let value = '';
  const submitted = [];
  globalThis.sourceFieldFixture = {
    state: () => [choice, (next) => (choice = next)],
  };
  const render = (props = {}) =>
    SourceField({
      value,
      change: (next) => (value = next),
      submit: (source) => submitted.push(source),
      inputRef: { current: null },
      ...props,
    });
  const paste = (source, props) => {
    let prevented = false;
    find(render(props), 'TextInput').props.onPaste({
      preventDefault: () => (prevented = true),
      clipboardData: { getData: () => source },
    });
    return prevented;
  };
  return { render, paste, submitted };
}

function find(element, type) {
  if (element?.type === type) return element;
  const children = [element?.props?.children].flat();
  for (const child of children) {
    const found = findChild(child, type);
    if (found) return found;
  }
  return null;
}
function findChild(child, type) {
  return child && typeof child === 'object' ? find(child, type) : null;
}

test('ambiguous paste waits for an explicit submit and defaults to this song', () => {
  const f = fixture();
  assert.equal(
    f.paste(
      'https://music.youtube.com/watch?v=eQVkkG8Oz8c&list=OLAK5uy_example',
    ),
    true,
  );
  assert.deepEqual(f.submitted, []);
  const form = f.render();
  assert.equal(find(form, 'SegmentedControl').props.value, 'song');
  form.props.onSubmit({ preventDefault() {} });
  assert.deepEqual(f.submitted, [
    'https://music.youtube.com/watch?v=eQVkkG8Oz8c',
  ]);
});

test('selecting entire playlist submits its explicit URL and a new link defaults to song', () => {
  const f = fixture();
  f.paste('https://youtu.be/eQVkkG8Oz8c?list=OLAK5uy_example');
  find(f.render(), 'SegmentedControl').props.onChange('playlist');
  f.render().props.onSubmit({ preventDefault() {} });
  assert.deepEqual(f.submitted, [
    'https://www.youtube.com/playlist?list=OLAK5uy_example',
  ]);
  find(f.render(), 'TextInput').props.onChange({
    currentTarget: {
      value: 'https://youtu.be/abcdefghijk?list=OLAK5uy_another',
    },
  });
  assert.equal(find(f.render(), 'SegmentedControl').props.value, 'song');
});

test('plain song paste starts immediately while disabled or loading fields do not submit', () => {
  const f = fixture();
  f.paste('https://youtu.be/eQVkkG8Oz8c?si=share');
  assert.deepEqual(f.submitted, [
    'https://www.youtube.com/watch?v=eQVkkG8Oz8c',
  ]);
  assert.equal(
    f.paste('https://youtu.be/abcdefghijk', { disabled: true }),
    false,
  );
  assert.equal(
    f.paste('https://youtu.be/abcdefghijk', { loading: true }),
    false,
  );
  f.render({ loading: true }).props.onSubmit({ preventDefault() {} });
  assert.equal(f.submitted.length, 1);
});

test('malformed video-plus-playlist and repeated targets remain editable without starting a collection', () => {
  for (const source of [
    'https://youtube.com/watch?v=short&list=PLvalid_playlist',
    'https://youtube.com/watch?v=&list=PLvalid_playlist',
    'https://youtube.com/watch?v=eQVkkG8Oz8c&v=abcdefghijk&list=PLvalid_playlist',
    'https://youtube.com/watch?v=eQVkkG8Oz8c&list=PLvalid_playlist&list=PLother_playlist',
  ]) {
    const f = fixture();
    assert.equal(f.paste(source), false, source);
    assert.deepEqual(f.submitted, []);
    assert.equal(find(f.render(), 'SegmentedControl'), null);
  }
});
