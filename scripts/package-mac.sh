#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
arch="$1"
version="$2"
out="$(mkdir -p "$3" && cd "$3" && pwd)"
target="$arch-apple-darwin"
label=$([ "$arch" = aarch64 ] && echo arm64 || echo x86_64)
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cargo build --release --target "$target" --manifest-path "$root/Cargo.toml" --workspace

package() {
  local bin="$1" name="$2" icon="$3" id="$4" file="$5"
  local stage="$work/$bin"
  local app="$stage/$name.app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  cp "$root/target/$target/release/$bin" "$app/Contents/MacOS/$bin"
  cp "$icon" "$app/Contents/Resources/app.icns"
  cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$name</string>
  <key>CFBundleDisplayName</key><string>$name</string>
  <key>CFBundleIdentifier</key><string>$id</string>
  <key>CFBundleExecutable</key><string>$bin</string>
  <key>CFBundleIconFile</key><string>app</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
  <key>NSCameraUsageDescription</key><string>Websites you allow can use the camera.</string>
  <key>NSMicrophoneUsageDescription</key><string>Websites you allow can use the microphone.</string>
</dict>
</plist>
EOF
  codesign --force --sign - "$app"
  ln -s /Applications "$stage/Applications"
  hdiutil create -quiet -volname "$name" -srcfolder "$stage" -format UDZO -ov "$out/$file-$version-macos-$label.dmg"
  echo "made $out/$file-$version-macos-$label.dmg"
}

package lunar-browser "Lunar Browser" "$root/assets/icon.icns" os.lunar.browser lunar-browser
package milky-ray "Lunar Browser Milky Ray" "$root/milky-ray/assets/icon.icns" os.lunar.milkyray lunar-browser-milky-ray
