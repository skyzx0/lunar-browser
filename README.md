# Lunar Browser

LibreWolf, with Lunar's protections locked on, LibreWolf's own privacy search
engines (without DuckDuckGo) and an ASCII look. For macOS, Windows and Linux.

    lunar/lunar.cfg        Lunar's settings, added after LibreWolf's own
    lunar/policies.json    Lunar's policies, merged over LibreWolf's
    lunar/theme/           the ASCII theme: browser UI, new tab page, art
    lunar/branding/        icons and the VT323 font
    lunar/windows/         the Windows installer
    scripts/brand.py       LibreWolf install -> Lunar (the same on every OS)
    scripts/package-*.sh   unpack LibreWolf, brand it, package it per OS
    .github/workflows/     builds all of it and publishes a release

## Building

On GitHub: Actions > Build Lunar Browser > Run workflow. It takes LibreWolf's
latest official release (or the version you type in), checks it against
LibreWolf's checksum, and publishes a release with:

    macOS    lunar-browser-<v>-macos-arm64.dmg / -x86_64.dmg
    Windows  lunar-browser-<v>-windows-x86_64-setup.exe / -portable.zip (and arm64)
    Linux    lunar-browser-<v>-linux-x86_64.deb / .tar.xz (and arm64)

On a Mac, from a LibreWolf .dmg:

    ./scripts/package-mac.sh librewolf-<v>-macos-arm64-package.dmg out/lunar.dmg

## Protections, always on

Locked, so they can't be turned off in Settings or about:config:

- HTTPS only, everywhere, private windows included
- encrypted DNS (DNS-over-HTTPS, Quad9) with no plaintext fallback
- fingerprinting resistance (LibreWolf's resistFingerprinting)
- strict tracker blocking, plus uBlock Origin (installed by LibreWolf)
- third-party cookies isolated per site; cross-site referrers trimmed
- Global Privacy Control and Do Not Track sent
- WebRTC can't reveal your local or real IP address
- cookies, cache, history, form data and downloads wiped when you quit;
  no disk cache
- notifications, push, MIDI, gamepads, VR and motion sensors off;
  camera, microphone and location always ask
- TLS 1.2 or newer, certificate pinning enforced, no telemetry

Firefox's HTTPS-only error page still lets you open a site over HTTP for one
visit; Lunar can't remove that button without rebuilding LibreWolf.

## Search

LibreWolf's built-in engines without DuckDuckGo: **Mojeek** (default),
Startpage and Wikipedia. Change it in Settings (the gear on the toolbar) >
Search.

## How it differs from plain LibreWolf

- `general.config.sandbox_enabled` is off, so `lunar.cfg` can load the theme
  from inside the app on every start. That file sits in the install folder
  and could already lock any setting, so this doesn't open anything new.
- Name, icon and window titles say Lunar Browser. The program file is still
  `librewolf` / `librewolf.exe`, and the profile folder is LibreWolf's: if
  LibreWolf is installed too, the two share profiles.
- The macOS app is re-signed ad hoc (no Apple Developer ID), so it isn't
  notarized; Windows builds aren't code-signed.

## Credits

Built on [LibreWolf](https://librewolf.net) (MPL 2.0), itself built on
Firefox. The VT323 font is under the SIL Open Font License
(`lunar/branding/VT323-LICENSE.txt`).
