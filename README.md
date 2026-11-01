# Lunar Browser

An independent web browser with its own engine and its own search engine.
Not a fork, reskin or wrapper of Chromium, Firefox, WebKit or any other
browser, and no Google, Bing, Brave or DuckDuckGo behind its search: Lunar
parses HTML, lays pages out, talks to the network, crawls, indexes and
ranks with its own code, and draws every page as text in the VT323
terminal font. For macOS, Windows and Linux.

    src/engine/html.rs      HTML parser: forgiving, drops scripts and styles
    src/engine/doc.rs       document model: blocks, links, form fields
    src/engine/layout.rs    lays a page out on a character grid
    src/net/http.rs         HTTP/1.1 client: HTTPS only, memory-only cookies
    src/net/dns.rs          DNS over HTTPS (Cloudflare, Quad9), no plaintext DNS
    src/search/crawl.rs     Lunar Search's crawler: robots.txt, one request/s per site
    src/search/index.rs     Lunar Search's index: BM25 ranking, snippets
    src/app.rs              the window: tabs, toolbar, settings bar, page view
    src/pages.rs            Lunar's own pages (new tab, errors)
    milky-ray/              Lunar Browser Milky Ray (see below)
    scripts/                packaging for each system
    .github/workflows/      builds everything and publishes a release

Libraries do the jobs that aren't a browser's: drawing the window and text
(egui/eframe), TLS encryption (rustls, with Mozilla's root certificates),
URL parsing, gzip, and reading DNS answers (JSON).

## Lunar Browser Milky Ray

The second browser in the family: everything Lunar leaves out. Every
website and all JavaScript work, because pages are drawn by the web engine
built into your system (WebKit on macOS, WebView2 on Windows, WebKitGTK on
Linux). It isn't a fork and doesn't bundle an engine, so it's a few MB.
Everything around the page is Milky Ray's own, and all of it can be
restyled:

- **10 themes**: Milky Ray, Nebula, Aurora, Ocean, Forest, Solar Flare,
  Sakura, Paper, OLED Midnight, Lunar Terminal
- **Every color**: background gradient (two colors and an angle), accent,
  text, secondary text, light or dark
- **Wallpaper**: drifting starfield, gradient, solid, or your own picture
- **Glass and shape**: blur strength, corner roundness, density, pages
  framed or edge to edge
- **Text**: system, rounded, serif, mono or VT323; interface and page size
- **Layout**: tabs on top or in a sidebar (left or right); five tab styles;
  toolbar on top or bottom; bookmarks bar, status bar and home button on
  or off; centered address
- **New tab page**: clock, greeting, search box, bookmark tiles, all themed
- **Custom CSS** for the interface and for its pages, and themes you can
  copy and paste to share
- Tabs you can drag to reorder, bookmarks, zoom, developer tools, printing,
  downloads to your Downloads folder
- **Lunar Search** as its search engine (or any search address you set),
  HTTPS-only upgrading, private tabs, clearing site data

    milky-ray/src/main.rs   window, tabs, menus, settings, Lunar Search pages
    milky-ray/ui/           the interface (HTML, CSS, JS): restyle anything

## Lunar Search

Lunar's own search engine. It crawls the web with Lunar's own network
stack, keeps its index on your computer, and ranks results there, so no
search company ever sees what you search for.

- It starts from about 110 sites: Wikipedia's 10,000 level-4 vital
  articles, other Wikimedia projects and Wikipedias (Italian, German,
  French, Spanish), Britannica, the Stanford Encyclopedia of Philosophy,
  world news (AP, BBC, the Guardian, NPR, DW, France 24, ABC, CBC, PBS...),
  science (NASA, ESA, Quanta, ScienceDaily, phys.org...), health (NHS,
  MedlinePlus, CDC), government (gov.uk, usa.gov), programming docs (MDN,
  Rust, Python, Go, C++, PostgreSQL, Kubernetes, Arch Wiki, man pages...),
  blogs, books, recipes and more
- It discovers sites those pages link to and takes a few pages from each,
  up to about 2,000 more sites; social networks, app stores and login-only
  sites are skipped
- **Ready-made index:** every release, GitHub crawls about 30,000 pages
  with Lunar's own crawler and publishes the index with the release.
  Lunar downloads it on first launch (from this project's own releases),
  then keeps growing its copy locally
- Any site you're on can be added: settings bar > `[ index this site ]`
- It obeys robots.txt and `<meta name="robots">`, sends at most one request
  per second to any site, and identifies itself as `LunarSearch/1.0`
- Your browsing is never indexed unless you choose to index a site
- Crawling can be switched off in the settings bar; the index is a file
  in Lunar's data folder (`search-index.gz`)

It knows tens of thousands of pages, not billions: a big web search engine
needs data centers. What it has, it searches privately and instantly.

## What it does, and doesn't

Works well on pages that are HTML first: Wikipedia, documentation, blogs,
news sites, forums, plain-HTML forms and logins.

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
- Settings bar (`[=]`): Lunar Search's page count, index this site,
  crawling on/off, text size, wipe cookies and the DNS cache

Print any page or search as text: `lunar-browser --dump <address or search>`.
Grow the index without opening a window: `lunar-browser --crawl <pages>`.
