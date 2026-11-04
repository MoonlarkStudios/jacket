"""Environment diagnostics use synthetic tool discovery, never host packages."""
from contextlib import redirect_stderr, redirect_stdout
import importlib.util
from io import StringIO
from pathlib import Path
import subprocess
import tomllib
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    'dev_environment', ROOT / 'scripts/check-dev-environment.py')
doctor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(doctor)


class DevelopmentEnvironmentTests(unittest.TestCase):
    def diagnose(self, *, missing=(), missing_modules=(), system='Linux'):
        config = tomllib.loads((ROOT / 'mise.toml').read_text())['tools']
        rust = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['channel']
        versions = {
            'node': f'v{config["node"]}',
            'rustc': f'rustc {rust}',
            'cargo': f'cargo {rust}',
            'python': f'Python {config["python"]}',
            'uv': f'uv {config["uv"]}',
            'yt-dlp': 'fixture downloader',
            'ffmpeg': 'ffmpeg version fixture',
            'ffprobe': 'ffprobe version fixture',
        }

        def which(name):
            return None if name in missing else f'/fixture/bin/{name}'

        def run(command, **_kwargs):
            name = Path(command[0]).name
            if name == 'pkg-config':
                return subprocess.CompletedProcess(
                    command, int(command[-1] in missing_modules), '', '')
            return subprocess.CompletedProcess(command, 0, versions[name], '')

        stdout, stderr = StringIO(), StringIO()
        with patch.object(doctor.platform, 'system', return_value=system), \
                patch.object(doctor.shutil, 'which', side_effect=which), \
                patch.object(doctor.subprocess, 'run', side_effect=run) as commands, \
                redirect_stdout(stdout), redirect_stderr(stderr):
            status = doctor.main()
        return status, stdout.getvalue(), stderr.getvalue(), commands.call_args_list

    def test_linux_requires_discoverable_native_development_files(self):
        status, stdout, stderr, _ = self.diagnose(missing_modules=('webkit2gtk-4.1',))
        self.assertEqual(status, 1)
        self.assertIn('WebKitGTK 4.1 development files', stderr)
        self.assertIn('docs/linux-preview.md#run-from-source', stderr)
        self.assertNotIn('Development environment is ready.', stdout)
        self.assertNotIn('GTK3 development files', stderr)

    def test_missing_pkg_config_has_actionable_guidance(self):
        status, _, stderr, commands = self.diagnose(missing=('pkg-config',))
        self.assertEqual(status, 1)
        self.assertIn('pkg-config is missing', stderr)
        self.assertIn('native development packages', stderr)
        self.assertFalse(any(Path(call.args[0][0]).name == 'pkg-config' for call in commands))

    def test_media_and_mise_runtime_guidance_are_distinct(self):
        for media in ('yt-dlp', 'ffmpeg', 'ffprobe'):
            with self.subTest(media=media):
                status, _, stderr, _ = self.diagnose(missing=(media,))
                self.assertEqual(status, 1)
                self.assertIn(f'{media} is missing', stderr)
                self.assertIn('platform package manager', stderr)
                self.assertNotIn('mise install', stderr)
        status, _, stderr, _ = self.diagnose(missing=('node',))
        self.assertEqual(status, 1)
        self.assertIn('node is missing; run mise install', stderr)

    def test_complete_linux_environment_passes_with_both_native_libraries_checked(self):
        status, stdout, stderr, commands = self.diagnose()
        self.assertEqual(status, 0)
        self.assertEqual(stderr, '')
        self.assertIn('Development environment is ready.', stdout)
        self.assertEqual(
            {call.args[0][-1] for call in commands if Path(call.args[0][0]).name == 'pkg-config'},
            {'gtk+-3.0', 'webkit2gtk-4.1'},
        )

    def test_non_linux_environment_does_not_require_linux_libraries(self):
        status, _, stderr, commands = self.diagnose(system='Windows', missing=('pkg-config',))
        self.assertEqual(status, 0)
        self.assertEqual(stderr, '')
        self.assertFalse(any(Path(call.args[0][0]).name == 'pkg-config' for call in commands))


if __name__ == '__main__':
    unittest.main()
