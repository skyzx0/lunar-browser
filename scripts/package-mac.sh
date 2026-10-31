#!/usr/bin/env bash
# LibreWolf .dmg -> "Lunar Browser.app" in a new .dmg.
# usage: package-mac.sh <librewolf.dmg> <output.dmg>
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
branding="$here/../lunar/branding"
dmg="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
out="$2"
work="$(mktemp -d)"
trap 'hdiutil detach -quiet "$work/mnt" 2>/dev/null || true; rm -rf "$work"' EXIT

hdiutil attach -nobrowse -readonly -mountpoint "$work/mnt" "$dmg" >/dev/null
mkdir "$work/stage"
ditto "$work/mnt/LibreWolf.app" "$work/stage/Lunar Browser.app"
hdiutil detach -quiet "$work/mnt"

app="$work/stage/Lunar Browser.app"
python3 "$here/brand.py" "$app/Contents/Resources"

# Name and icon shown by macOS.
plist="$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleName Lunar Browser" "$plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleDisplayName Lunar Browser" "$plist" 2>/dev/null ||
  /usr/libexec/PlistBuddy -c "Add :CFBundleDisplayName string Lunar Browser" "$plist"
icon="$(/usr/libexec/PlistBuddy -c "Print :CFBundleIconFile" "$plist")"
cp "$branding/icon.icns" "$app/Contents/Resources/${icon%.icns}.icns"

# Our changes break LibreWolf's signature: sign it again (ad hoc, no Apple ID).
xattr -cr "$app"
codesign --force --deep --sign - "$app"
codesign --verify --deep "$app"

ln -s /Applications "$work/stage/Applications"
hdiutil create -quiet -volname "Lunar Browser" -srcfolder "$work/stage" -format UDZO -ov "$out"
echo "made $out"
