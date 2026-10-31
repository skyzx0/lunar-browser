#!/usr/bin/env bash
# Builds Lunar Browser for Windows: a portable .zip and an installer .exe.
# Runs in Git Bash on GitHub's Windows machines.
# usage: package-windows.sh <x86_64|aarch64> <version> <output dir>
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
arch="$1"
version="$2"
out="$(mkdir -p "$3" && cd "$3" && pwd)"
target="$arch-pc-windows-msvc"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cargo build --release --target "$target" --manifest-path "$root/Cargo.toml"
name="lunar-browser-$version-windows-$([ "$arch" = aarch64 ] && echo arm64 || echo x86_64)"

dir="$work/lunar-browser"
mkdir -p "$dir"
cp "$root/target/$target/release/lunar-browser.exe" "$dir/"
cp "$root/assets/icon.ico" "$dir/lunar-browser.ico"
cp "$root/assets/VT323-LICENSE.txt" "$root/README.md" "$dir/"
(cd "$work" && 7z a -tzip -bso0 "$out/$name-portable.zip" lunar-browser)

iscc="/c/Program Files (x86)/Inno Setup 6/ISCC.exe"
if [ ! -f "$iscc" ]; then choco install innosetup -y --no-progress >/dev/null; fi
inno_arch=$([ "$arch" = "x86_64" ] && echo x64compatible || echo arm64)
# Git Bash would turn the /D... switches into file paths; pass them as written.
MSYS2_ARG_CONV_EXCL='*' "$iscc" /Q \
  "/DSourceDir=$(cygpath -w "$dir")" \
  "/DAppVersion=$version" \
  "/DArch=$inno_arch" \
  "/DOutputDir=$(cygpath -w "$out")" \
  "/DOutputName=$name-setup" \
  "$(cygpath -w "$root/scripts/lunar.iss")"
echo "made $out/$name-portable.zip and $out/$name-setup.exe"
