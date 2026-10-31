#!/usr/bin/env bash
# LibreWolf .zip -> Lunar Browser portable .zip and installer .exe.
# Runs in Git Bash on GitHub's Windows machines.
# usage: package-windows.sh <librewolf.zip> <x86_64|arm64> <version> <output dir>
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
branding="$here/../lunar/branding"
zip="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
arch="$2"
version="$3"
out="$(mkdir -p "$4" && cd "$4" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

unzip -q "$zip" -d "$work"
app="$work/lunar-browser"
mv "$work/librewolf" "$app"
python "$here/brand.py" "$app"

# Name and icon shown by Windows. rcedit is pinned to a release.
curl -sSL -o "$work/rcedit.exe" https://github.com/electron/rcedit/releases/download/v2.0.0/rcedit-x64.exe
for exe in librewolf.exe private_browsing.exe; do
  [ -f "$app/$exe" ] || continue
  "$work/rcedit.exe" "$app/$exe" \
    --set-icon "$(cygpath -w "$branding/icon.ico")" \
    --set-version-string ProductName "Lunar Browser" \
    --set-version-string FileDescription "Lunar Browser" \
    --set-version-string CompanyName "Lunar"
done
cp "$branding/icon.ico" "$app/lunar-browser.ico"

name="lunar-browser-$version-windows-$arch"
(cd "$work" && 7z a -tzip -bso0 "$out/$name-portable.zip" lunar-browser)

iscc="/c/Program Files (x86)/Inno Setup 6/ISCC.exe"
if [ ! -f "$iscc" ]; then choco install innosetup -y --no-progress >/dev/null; fi
inno_arch=$([ "$arch" = "x86_64" ] && echo x64compatible || echo arm64)
# Git Bash would turn the /D... switches into file paths; pass them as written.
MSYS2_ARG_CONV_EXCL='*' "$iscc" /Q \
  "/DSourceDir=$(cygpath -w "$app")" \
  "/DAppVersion=$version" \
  "/DArch=$inno_arch" \
  "/DOutputDir=$(cygpath -w "$out")" \
  "/DOutputName=$name-setup" \
  "$(cygpath -w "$here/../lunar/windows/lunar.iss")"
echo "made $out/$name-portable.zip and $out/$name-setup.exe"
