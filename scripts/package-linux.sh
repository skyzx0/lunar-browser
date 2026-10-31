#!/usr/bin/env bash
# Builds Lunar Browser for Linux: a .tar.gz and a .deb.
# usage: package-linux.sh <x86_64|aarch64> <version> <output dir>
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
arch="$1"
version="$2"
out="$(mkdir -p "$3" && cd "$3" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cargo build --release --manifest-path "$root/Cargo.toml"
name="lunar-browser-$version-linux-$([ "$arch" = aarch64 ] && echo arm64 || echo x86_64)"

dir="$work/lunar-browser"
mkdir -p "$dir"
cp "$root/target/release/lunar-browser" "$dir/"
cp "$root/assets/icon.png" "$dir/lunar-browser.png"
cp "$root/assets/VT323-LICENSE.txt" "$root/README.md" "$dir/"
cat > "$dir/lunar-browser.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Lunar Browser
GenericName=Web Browser
Comment=Independent web browser with its own engine, an ASCII look and no JavaScript
Exec=lunar-browser %u
Icon=lunar-browser
Terminal=false
Categories=Network;WebBrowser;
MimeType=text/html;x-scheme-handler/http;x-scheme-handler/https;
StartupWMClass=lunar-browser
EOF
tar -czf "$out/$name.tar.gz" -C "$work" lunar-browser

deb="$work/deb"
mkdir -p "$deb/DEBIAN" "$deb/usr/bin" "$deb/usr/share/applications" "$deb/usr/share/icons/hicolor/512x512/apps" "$deb/usr/share/doc/lunar-browser"
cp "$dir/lunar-browser" "$deb/usr/bin/"
cp "$dir/lunar-browser.desktop" "$deb/usr/share/applications/"
cp "$root/assets/icon.png" "$deb/usr/share/icons/hicolor/512x512/apps/lunar-browser.png"
cp "$root/assets/VT323-LICENSE.txt" "$deb/usr/share/doc/lunar-browser/"
cat > "$deb/DEBIAN/control" <<EOF
Package: lunar-browser
Version: $version
Architecture: $([ "$arch" = aarch64 ] && echo arm64 || echo amd64)
Maintainer: Lunar <lunar@localhost>
Depends: libgl1, libxkbcommon0, libc6
Recommends: libwayland-client0, libx11-6, libxcursor1, libxrandr2, libxi6
Section: web
Priority: optional
Description: Independent web browser with an ASCII look
 Lunar Browser has its own engine: it is not a fork of any browser. Pages
 are drawn as text in a terminal font. No JavaScript, HTTPS only, encrypted
 DNS, no trackers or third-party requests, cookies in memory only.
EOF
dpkg-deb --build --root-owner-group "$deb" "$out/$name.deb" >/dev/null
echo "made $out/$name.tar.gz and $out/$name.deb"
