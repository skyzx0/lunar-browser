#!/usr/bin/env bash
# Builds "Lunar Browser.app" and a .dmg.
# usage: package-mac.sh <aarch64|x86_64> <version> <output dir>
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
arch="$1"
version="$2"
out="$(mkdir -p "$3" && cd "$3" && pwd)"
target="$arch-apple-darwin"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cargo build --release --target "$target" --manifest-path "$root/Cargo.toml"

app="$work/stage/Lunar Browser.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$root/target/$target/release/lunar-browser" "$app/Contents/MacOS/lunar-browser"
cp "$root/assets/icon.icns" "$app/Contents/Resources/lunar.icns"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Lunar Browser</string>
  <key>CFBundleDisplayName</key><string>Lunar Browser</string>
  <key>CFBundleIdentifier</key><string>os.lunar.browser</string>
  <key>CFBundleExecutable</key><string>lunar-browser</string>
  <key>CFBundleIconFile</key><string>lunar</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
</dict>
</plist>
EOF
# No Apple Developer ID: sign ad hoc so it runs on Apple Silicon.
codesign --force --sign - "$app"

ln -s /Applications "$work/stage/Applications"
name="lunar-browser-$version-macos-$([ "$arch" = aarch64 ] && echo arm64 || echo x86_64)"
hdiutil create -quiet -volname "Lunar Browser" -srcfolder "$work/stage" -format UDZO -ov "$out/$name.dmg"
echo "made $out/$name.dmg"
