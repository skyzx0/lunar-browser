# Lunar Browser

An independent web browser with its own engine. Not a fork, reskin or
wrapper of Chromium, Firefox, WebKit or any other browser: Lunar parses
HTML, lays pages out and talks to the network with its own code, and draws
every page as text in the VT323 terminal font. For macOS, Windows and Linux.

    src/engine/html.rs      HTML parser: forgiving, drops scripts and styles
    src/engine/doc.rs       document model: blocks, links, form fields
    src/engine/layout.rs    lays a page out on a character grid
    src/net/http.rs         HTTP/1.1 client: HTTPS only, memory-only cookies
    src/net/dns.rs          DNS over HTTPS (Cloudflare, Quad9), no plaintext DNS
    src/app.rs              the window: tabs, toolbar, settings bar, page view
    src/pages.rs            Lunar's own pages (new tab, errors)
    scripts/                packaging for each system
    .github/workflows/      builds everything and publishes a release

Libraries do the jobs that aren't a browser's: drawing the window and text
(egui/eframe), TLS encryption (rustls, with Mozilla's root certificates),
URL parsing, gzip, and reading DNS answers (JSON).

## What it does, and doesn't

Works well on pages that are HTML first: Wikipedia, search results (Brave
Search, Ecosia), documentation, blogs, news sites, forums, plain-HTML
forms and logins.

Doesn't run JavaScript, apply CSS or load images, so web apps (YouTube,
Gmail, maps, chats) and pages that only show content through JavaScript
won't work. That's the trade for the protections below.

## Protections, always on

- **No JavaScript, ever**: no scripts to exploit, track you or fingerprint you
- **Only the page itself is fetched**: no images, scripts, fonts, ads,
  trackers or any third-party request
- **HTTPS only**: http:// is upgraded; sites without HTTPS aren't opened;
  certificate errors are final (localhost is the only exception)
- **Encrypted DNS** over HTTPS to Cloudflare or Quad9, reached by IP address,
  with no fallback to your network's DNS
- **Cookies in memory only**, sent back only to the exact host that set them,
  gone when you quit (or wipe them any time from the settings bar)
- Global Privacy Control and Do Not Track on every request, no Referer,
  a generic user agent
- Pages can't download files or open other apps

## Using it

- Address bar: type an address, or words to search
- Click links; ctrl/cmd-click or middle-click opens a new tab
- Click a form field to type; Enter submits; Tab moves to the next field
- Shortcuts (cmd on macOS, ctrl elsewhere): T new tab, W close tab,
  L address bar, R reload, `[` / `]` back / forward, `,` settings
- Settings bar (`[=]`): search engine (Brave Search, Ecosia, Wikipedia),
  text size, wipe cookies and the DNS cache

Print any page as text: `lunar-browser --dump <address or search>`.

## Building

On GitHub: Actions > Build Lunar Browser > Run workflow. It runs the tests,
builds all six versions, and publishes a release with SHA256SUMS:

    macOS    lunar-browser-<v>-macos-arm64.dmg / -x86_64.dmg
    Windows  lunar-browser-<v>-windows-x86_64-setup.exe / -portable.zip (and arm64)
    Linux    lunar-browser-<v>-linux-x86_64.deb / .tar.gz (and arm64)

Locally, with Rust installed:

    cargo run --release
    cargo test

The builds aren't signed by Apple or Microsoft: on macOS run
`xattr -cr "/Applications/Lunar Browser.app"` the first time; on Windows,
SmartScreen: More info > Run anyway.

## Credits

The VT323 font is under the SIL Open Font License (`assets/VT323-LICENSE.txt`).
