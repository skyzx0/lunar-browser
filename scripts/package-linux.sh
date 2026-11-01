#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
arch="$1"
version="$2"
out="$(mkdir -p "$3" && cd "$3" && pwd)"
label=$([ "$arch" = aarch64 ] && echo arm64 || echo x86_64)
deb_arch=$([ "$arch" = aarch64 ] && echo arm64 || echo amd64)
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cargo build --release --manifest-path "$root/Cargo.toml" --workspace

package() {
  local bin="$1" name="$2" icon="$3" comment="$4" depends="$5" description="$6"
  local file="$bin-$version-linux-$label"
  [ "$bin" = milky-ray ] && file="lunar-browser-milky-ray-$version-linux-$label"
  local dir="$work/$bin/$bin"
  mkdir -p "$dir"
  cp "$root/target/release/$bin" "$dir/"
  cp "$icon" "$dir/$bin.png"
  cp "$root/assets/VT323-LICENSE.txt" "$root/README.md" "$dir/"
  cat > "$dir/$bin.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=$name
GenericName=Web Browser
Comment=$comment
Exec=$bin %u
Icon=$bin
Terminal=false
Categories=Network;WebBrowser;
MimeType=text/html;x-scheme-handler/http;x-scheme-handler/https;
StartupWMClass=$bin
EOF
  tar -czf "$out/$file.tar.gz" -C "$work/$bin" "$bin"

  local deb="$work/$bin-deb"
  mkdir -p "$deb/DEBIAN" "$deb/usr/bin" "$deb/usr/share/applications" "$deb/usr/share/icons/hicolor/512x512/apps" "$deb/usr/share/doc/$bin"
  cp "$dir/$bin" "$deb/usr/bin/"
  cp "$dir/$bin.desktop" "$deb/usr/share/applications/"
  cp "$icon" "$deb/usr/share/icons/hicolor/512x512/apps/$bin.png"
  cp "$root/assets/VT323-LICENSE.txt" "$deb/usr/share/doc/$bin/"
  cat > "$deb/DEBIAN/control" <<EOF
Package: $bin
Version: $version
Architecture: $deb_arch
Maintainer: Lunar <lunar@localhost>
Depends: $depends
Section: web
Priority: optional
Description: $comment
 $description
EOF
  dpkg-deb --build --root-owner-group "$deb" "$out/$file.deb" >/dev/null
  echo "made $out/$file.tar.gz and $out/$file.deb"
}

package lunar-browser "Lunar Browser" "$root/assets/icon.png" \
  "Independent web browser with its own engine, an ASCII look and no JavaScript" \
  "libgl1, libxkbcommon0, libc6" \
  "Lunar Browser has its own engine and its own search engine, Lunar Search. Pages are drawn as text in a terminal font. No JavaScript, HTTPS only, encrypted DNS, no trackers."

package milky-ray "Lunar Browser Milky Ray" "$root/milky-ray/assets/icon.png" \
  "Fully customizable web browser with Lunar Search" \
  "libwebkit2gtk-4.1-0, libgtk-3-0 | libgtk-3-0t64, libc6" \
  "Milky Ray runs every website with the system web engine (WebKitGTK), with an interface you can restyle completely and Lunar Search, Lunar's own search engine."
