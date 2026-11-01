#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
arch="$1"
version="$2"
out="$(mkdir -p "$3" && cd "$3" && pwd)"
target="$arch-pc-windows-msvc"
label=$([ "$arch" = aarch64 ] && echo arm64 || echo x86_64)
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cargo build --release --target "$target" --manifest-path "$root/Cargo.toml" --workspace

iscc="/c/Program Files (x86)/Inno Setup 6/ISCC.exe"
if [ ! -f "$iscc" ]; then choco install innosetup -y --no-progress >/dev/null; fi
inno_arch=$([ "$arch" = "x86_64" ] && echo x64compatible || echo arm64)

package() {
  local bin="$1" name="$2" icon="$3" guid="$4" comments="$5" file="$6-$version-windows-$label"
  local dir="$work/$bin"
  mkdir -p "$dir"
  cp "$root/target/$target/release/$bin.exe" "$dir/"
  cp "$icon" "$dir/app.ico"
  cp "$root/assets/VT323-LICENSE.txt" "$root/README.md" "$dir/"
  (cd "$work" && 7z a -tzip -bso0 "$out/$file-portable.zip" "$bin")
  MSYS2_ARG_CONV_EXCL='*' "$iscc" /Q \
    "/DAppName=$name" \
    "/DAppExe=$bin.exe" \
    "/DAppGuid=$guid" \
    "/DAppComments=$comments" \
    "/DSourceDir=$(cygpath -w "$dir")" \
    "/DAppVersion=$version" \
    "/DArch=$inno_arch" \
    "/DOutputDir=$(cygpath -w "$out")" \
    "/DOutputName=$file-setup" \
    "$(cygpath -w "$root/scripts/installer.iss")"
  echo "made $out/$file-portable.zip and $out/$file-setup.exe"
}

package lunar-browser "Lunar Browser" "$root/assets/icon.ico" 6C7D3E1A-4F0B-4C55-9E2B-1A5E3B7C9D20 \
  "Independent web browser with its own engine and an ASCII look" lunar-browser
package milky-ray "Lunar Browser Milky Ray" "$root/milky-ray/assets/icon.ico" 9A4E2B71-3C5D-4F8A-B6E1-7D2C0F5A9B34 \
  "Fully customizable web browser with Lunar Search" lunar-browser-milky-ray
