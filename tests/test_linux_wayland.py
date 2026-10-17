"""Protect the real-desktop check from silently accepting headless workarounds."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    'wayland_check', Path(__file__).resolve().parents[1] / 'scripts/check-linux-wayland.py')
wayland = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wayland)


class WaylandAcceptance(unittest.TestCase):
    def test_a_real_session_with_production_rendering_is_required(self):
        with self.assertRaisesRegex(ValueError, 'real Wayland'):
            wayland.check_environment({})
        wayland.check_environment({'WAYLAND_DISPLAY': 'wayland-0'})
        for key in wayland.RENDERING_OVERRIDES:
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, key):
                wayland.check_environment({'WAYLAND_DISPLAY': 'wayland-0', key: '1'})

    def test_a_live_process_with_a_protocol_or_buffer_error_is_not_a_pass(self):
        for error in [
            'wl_display#1.error(wp_linux_drm_syncobj_surface_v1#54, 2, "invalid buffer")',
            'Error 71 (Protocol error) dispatching to Wayland display.',
            'Failed to create GBM buffer of size 900x720: Invalid argument',
        ]:
            with self.subTest(error=error), self.assertRaises(ValueError):
                wayland.check_trace('wl_surface#34.commit()\n' + error)

    def test_nvidia_acceptance_requires_both_the_fix_and_explicit_sync(self):
        frame = 'wl_surface#34.commit()\n'
        prepared = 'Jacket: prepared GTK OpenGL painting for NVIDIA Wayland\n'
        acquire = 'wp_linux_drm_syncobj_surface_v1#54.set_acquire_point(1, 0, 1)\n'
        wayland.check_trace(frame)
        for trace in (frame, frame + prepared, frame + acquire):
            with self.assertRaisesRegex(ValueError, 'not both observed'):
                wayland.check_trace(trace, require_nvidia_sync=True)
        wayland.check_trace(frame + prepared + acquire, require_nvidia_sync=True)

    def test_xwayland_cannot_satisfy_native_wayland_acceptance(self):
        with self.assertRaisesRegex(ValueError, 'No native Wayland surface'):
            wayland.check_trace('Jacket window opened')


if __name__ == '__main__':
    unittest.main()
