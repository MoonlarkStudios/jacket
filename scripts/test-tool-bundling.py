#!/usr/bin/env python3
"""Regression checks for native resource staging, without installed tools."""
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('tool_staging', Path(__file__).with_name('stage-macos-tools.py'))
staging = importlib.util.module_from_spec(spec)
spec.loader.exec_module(staging)


class MachODependencies(unittest.TestCase):
    def test_reads_imports_without_following_the_library_install_name(self):
        library = Path('_rust.abi3t.so')
        commands = '''Load command 0
          cmd LC_ID_DYLIB
      cmdsize 128
         name /opt/homebrew/lib/cryptography.hazmat.bindings._rust.abi3t.so (offset 24)
Load command 1
          cmd LC_LOAD_DYLIB
      cmdsize 56
         name /usr/lib/libSystem.B.dylib (offset 24)
Load command 2
          cmd LC_LOAD_WEAK_DYLIB
      cmdsize 56
         name @rpath/Optional Library.dylib (offset 24)
Load command 3
          cmd LC_REEXPORT_DYLIB
      cmdsize 64
         name @loader_path/libcrypto.dylib (offset 24)
'''
        with mock.patch.object(staging, 'run', return_value=commands) as otool:
            self.assertEqual(staging.dependencies(library), [
                '/usr/lib/libSystem.B.dylib',
                '@rpath/Optional Library.dylib',
                '@loader_path/libcrypto.dylib',
            ])
        otool.assert_called_once_with('/usr/bin/otool', '-l', library)


class ResourceModes(unittest.TestCase):
    def test_repeated_resource_copy_preserves_installed_inputs(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            installed = root / 'installed-package.py'
            installed.write_text('installed package bytes\n')
            installed.chmod(0o444)
            staged = root / 'staged'
            staged.mkdir()
            generated = staged / 'package.py'
            shutil.copy2(installed, generated)
            original_hash = hashlib.sha256(generated.read_bytes()).digest()
            staging.owner_writable_tree(staged)
            target = root / 'bundled-package.py'
            shutil.copy2(generated, target)
            shutil.copy2(generated, target)
            self.assertEqual(target.read_bytes(), installed.read_bytes())
            self.assertTrue(target.stat().st_mode & 0o200)
            self.assertFalse(installed.stat().st_mode & 0o200)
            self.assertEqual(hashlib.sha256(generated.read_bytes()).digest(), original_hash)

    def test_writable_generated_copy_keeps_bytes_and_execute_bit(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            executable = root / 'audio-tool'
            executable.write_bytes(b'executable payload')
            executable.chmod(0o555)
            before = executable.read_bytes()
            staging.owner_writable_tree(root)
            self.assertEqual(executable.read_bytes(), before)
            self.assertEqual(executable.stat().st_mode & 0o700, 0o700)


class ResourceIntegrity(unittest.TestCase):
    def test_changed_and_untracked_bytecode_files_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            payload = root / 'runtime.py'
            payload.write_bytes(b'original')
            manifest = {'files': {'runtime.py': hashlib.sha256(payload.read_bytes()).hexdigest()}}
            (root / 'manifest.json').write_text(json.dumps(manifest))
            staging.verify_manifest(root)
            payload.write_bytes(b'modified')
            with self.assertRaisesRegex(RuntimeError, 'Staged file changed'):
                staging.verify_manifest(root)
            payload.write_bytes(b'original')
            cache = root / '__pycache__'
            cache.mkdir()
            (cache / 'runtime.pyc').write_bytes(b'generated cache')
            with self.assertRaisesRegex(RuntimeError, 'Unexpected bundled file'):
                staging.verify_manifest(root)
            shutil.rmtree(cache)
            staging.verify_manifest(root)
            payload.unlink()
            with self.assertRaisesRegex(RuntimeError, 'Missing bundled file'):
                staging.verify_manifest(root)


if __name__ == '__main__':
    unittest.main()
