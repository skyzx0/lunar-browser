#!/usr/bin/env bash
# LibreWolf .tar.xz -> Lunar Browser .tar.xz and .deb.
# usage: package-linux.sh <librewolf.tar.xz> <x86_64|arm64> <version> <output dir>
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
branding="$here/../lunar/branding"
tarball="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
arch="$2"
version="$3"
out="$(mkdir -p "$4" && cd "$4" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

tar -xJf "$tarball" -C "$work"
app="$work/lunar-browser"
mv "$work/librewolf" "$app"
python3 "$here/brand.py" "$app"

for size in 16 32 48 64 128; do
  cp "$branding/default$size.png" "$app/browser/chrome/icons/default/default$size.png"
done
cp "$branding/icon.png" "$app/lunar-browser.png"
cat > "$app/lunar-browser" <<'EOF'
#!/bin/sh
exec "$(dirname "$(readlink -f "$0")")/librewolf" "$@"
EOF
chmod +x "$app/lunar-browser"
cat > "$app/lunar-browser.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Lunar Browser
GenericName=Web Browser
Comment=Private, locked-down web browser, based on LibreWolf
Exec=lunar-browser %u
Icon=lunar-browser
Terminal=false
Categories=Network;WebBrowser;
MimeType=text/html;x-scheme-handler/http;x-scheme-handler/https;
StartupWMClass=librewolf
EOF

name="lunar-browser-$version-linux-$arch"
tar -cJf "$out/$name.tar.xz" -C "$work" lunar-browser

# .deb: installed in /opt/lunar-browser, started with "lunar-browser".
deb_arch=$([ "$arch" = "x86_64" ] && echo amd64 || echo arm64)
root="$work/deb"
mkdir -p "$root/DEBIAN" "$root/opt" "$root/usr/bin" "$root/usr/share/applications"
cp -a "$app" "$root/opt/lunar-browser"
ln -s /opt/lunar-browser/lunar-browser "$root/usr/bin/lunar-browser"
cp "$app/lunar-browser.desktop" "$root/usr/share/applications/"
for size in 16 32 48 64 128 256; do
  mkdir -p "$root/usr/share/icons/hicolor/${size}x${size}/apps"
  cp "$branding/default$size.png" "$root/usr/share/icons/hicolor/${size}x${size}/apps/lunar-browser.png"
done
cat > "$root/DEBIAN/control" <<EOF
Package: lunar-browser
Version: ${version//_/-}
Architecture: $deb_arch
Maintainer: Lunar <lunar@localhost>
Depends: libgtk-3-0 | libgtk-3-0t64, libasound2 | libasound2t64, libx11-xcb1, libdbus-1-3
Section: web
Priority: optional
Homepage: https://librewolf.net
Description: Private, locked-down web browser with an ASCII look
 Lunar Browser is LibreWolf with Lunar's protections locked on:
 HTTPS only, encrypted DNS, fingerprint resistance, tracker blocking,
 and everything wiped when you quit.
EOF
dpkg-deb --build --root-owner-group "$root" "$out/$name.deb" >/dev/null
echo "made $out/$name.tar.xz and $out/$name.deb"
