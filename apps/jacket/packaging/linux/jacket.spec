# The shared payload is already built and hash-verified on Ubuntu 24.04.
# Keep its executable bytes intact and retain RPM's automatic ELF dependencies.
%global debug_package %{nil}
%global __os_install_post %{nil}
%global _binary_payload w9.zstdio
%global source_date_epoch_from_changelog 0
Name: jacket
Version: %{jacket_version}
Release: %{jacket_revision}
Summary: Prepare tagged audio for your music library
License: GPL-3.0-or-later
URL: https://github.com/MoonlarkStudios/jacket
ExclusiveArch: x86_64
Requires: ca-certificates, python3 >= 3.10, xdg-utils, xdg-desktop-portal, zenity
Requires: libwayland-client.so.0()(64bit), libwayland-cursor.so.0()(64bit), libwayland-egl.so.1()(64bit)
Requires: libX11, libXcursor, libXi, libXrandr, libxkbcommon, libxkbcommon-x11, libGLESv2.so.2()(64bit), mesa-libGL, mesa-dri-drivers

%description
Review songs, albums and playlists, edit their tags and artwork, and save
organized audio files with the bundled media tools.

%install
mkdir -p %{buildroot}
cp -a "%{jacket_payload}/usr" %{buildroot}/

%files
/usr/bin/jacket
/usr/libexec/jacket
/usr/share/applications/com.moonlark.audio-dl.desktop
%doc /usr/share/doc/jacket/
/usr/share/icons/hicolor/*/apps/com.moonlark.audio-dl.png
/usr/share/metainfo/com.moonlark.audio-dl.metainfo.xml
%license /usr/share/licenses/jacket/
