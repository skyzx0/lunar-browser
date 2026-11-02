(() => {
  const $ = (s, el = document) => el.querySelector(s);
  const $$ = (s, el = document) => [...el.querySelectorAll(s)];
  const boot = window.__MILKY__ || { settings: {}, platform: 'preview' };
  const send = (cmd, data = {}) => {
    try { window.ipc.postMessage(JSON.stringify({ cmd, ...data })); } catch (_) { /* preview without the app */ }
  };
  const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const icon = (name) => `<svg class="i"><use href="#i-${name}"/></svg>`;

  // ---------- themes ----------
  const PRESETS = {
    'Match system': { system: true },
    Graphite: { bg1: '#161719', surface: '#1f2023', text: '#e4e2dd', muted: '#8b8984', accent: '#d9a05b', font: 'system', radius: 8 },
    Porcelain: { bg1: '#f2f1ee', surface: '#ffffff', text: '#1c1b19', muted: '#74716b', accent: '#2563c9', font: 'system', radius: 8, light: true },
    'Gruvbox Dark': { bg1: '#282828', surface: '#3c3836', text: '#ebdbb2', muted: '#a89984', accent: '#fe8019', font: 'mono', radius: 3 },
    'Gruvbox Light': { bg1: '#fbf1c7', surface: '#f2e5bc', text: '#3c3836', muted: '#7c6f64', accent: '#af3a03', font: 'mono', radius: 3, light: true },
    Nord: { bg1: '#2e3440', surface: '#3b4252', text: '#e5e9f0', muted: '#9099ab', accent: '#88c0d0', font: 'system', radius: 6 },
    'Solarized Dark': { bg1: '#002b36', surface: '#073642', text: '#93a1a1', muted: '#657b83', accent: '#b58900', font: 'mono', radius: 4 },
    'Solarized Light': { bg1: '#fdf6e3', surface: '#eee8d5', text: '#586e75', muted: '#93a1a1', accent: '#268bd2', font: 'mono', radius: 4, light: true },
    'Rosé Pine': { bg1: '#191724', surface: '#1f1d2e', text: '#e0def4', muted: '#908caa', accent: '#ebbcba', font: 'rounded', radius: 10 },
    'Rosé Pine Dawn': { bg1: '#faf4ed', surface: '#fffaf3', text: '#575279', muted: '#797593', accent: '#d7827e', font: 'rounded', radius: 10, light: true },
    Everforest: { bg1: '#2d353b', surface: '#343f44', text: '#d3c6aa', muted: '#859289', accent: '#a7c080', font: 'system', radius: 6 },
    Kanagawa: { bg1: '#1f1f28', surface: '#2a2a37', text: '#dcd7ba', muted: '#727169', accent: '#e6c384', font: 'serif', radius: 4 },
    Monokai: { bg1: '#272822', surface: '#3e3d32', text: '#f8f8f2', muted: '#75715e', accent: '#a6e22e', font: 'mono', radius: 2 },
    Zenburn: { bg1: '#3f3f3f', surface: '#4f4f4f', text: '#dcdccc', muted: '#9f9f8f', accent: '#f0dfaf', font: 'mono', radius: 0 },
    'Lunar Terminal': { bg1: '#05070d', surface: '#0a0e18', text: '#c9d2e3', muted: '#5f6a85', accent: '#8fd8ff', font: 'vt323', radius: 0 },
  };
  const systemDark = window.matchMedia('(prefers-color-scheme: dark)');
  const resolvePreset = (name) => {
    const p = PRESETS[name];
    if (p && p.system) return systemDark.matches ? PRESETS.Graphite : PRESETS.Porcelain;
    return p;
  };
  const FONTS = {
    system: 'Inter, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif',
    rounded: 'ui-rounded, "SF Pro Rounded", Nunito, system-ui, sans-serif',
    serif: 'ui-serif, "New York", Charter, Georgia, serif',
    mono: 'ui-monospace, "SF Mono", Menlo, Consolas, monospace',
    vt323: '"VT323", ui-monospace, monospace',
  };
  const DEFAULTS = {
    theme: {
      preset: 'Match system', ...PRESETS.Graphite, bg2: PRESETS.Graphite.bg1, wallpaper: 'solid', angle: 180, size: 13, pageSize: 15, image: '', light: false, customFont: '',
      windowOpacity: 100, tabsTint: 0, toolbarTint: 0, sidebarTint: 0, frameOpacity: 100, panelOpacity: 97,
      blur: 0, backdrop: 'off', pageOpacity: 100, pageBackground: 'normal',
    },
    layout: {
      tabs: 'top', sideRight: false, sidebarHidden: false, tabStyle: 'rounded', toolbar: 'top', density: 'normal',
      bookmarksBar: false, statusBar: false, edge: false, addressCenter: false,
      buttons: { back: true, forward: true, reload: true, home: false, star: true, appearance: true },
    },
    newTab: { clock: true, greeting: '', search: true },
    uiCss: '',
    newTabCss: '',
    siteCss: '',
    siteJs: '',
    userAgent: '',
    defaultZoom: 100,
    bookmarks: [],
    searchEngine: 'lunar',
    customSearch: '',
    homePage: '',
    httpsOnly: true,
    private: false,
    crawl: true,
  };
  const merge = (base, over) => {
    const out = Array.isArray(base) ? [...base] : { ...base };
    for (const [k, v] of Object.entries(over || {})) {
      out[k] = v && typeof v === 'object' && !Array.isArray(v) && base[k] && typeof base[k] === 'object' ? merge(base[k], v) : v;
    }
    return out;
  };
  let settings = merge(DEFAULTS, boot.settings);
  let state = { tabs: [], active: null, searchPages: 0, crawling: false };
  let panel = null;
  let editingAddress = false;

  const getPath = (path) => path.split('.').reduce((o, k) => (o == null ? o : o[k]), settings);
  const setPath = (path, value) => {
    const keys = path.split('.');
    let o = settings;
    keys.slice(0, -1).forEach((k) => { o[k] = o[k] && typeof o[k] === 'object' ? o[k] : {}; o = o[k]; });
    o[keys.at(-1)] = value;
  };

  let saveTimer;
  const save = () => {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => send('saveSettings', { settings }), 250);
  };

  function applyTheme() {
    const live = resolvePreset(settings.theme.preset);
    const t = live && PRESETS[settings.theme.preset].system ? { ...settings.theme, ...live, bg2: live.bg1, light: !!live.light } : settings.theme;
    const l = settings.layout;
    const root = document.documentElement.style;
    for (const k of ['bg1', 'bg2', 'surface', 'text', 'muted', 'accent']) root.setProperty('--' + k, t[k]);
    const pct = (v, d) => Math.max(0, Math.min(100, v ?? d));
    root.setProperty('--window-opacity', pct(t.windowOpacity, 100) / 100);
    root.setProperty('--tabs-tint', pct(t.tabsTint, 0) + '%');
    root.setProperty('--toolbar-tint', pct(t.toolbarTint, 0) + '%');
    root.setProperty('--sidebar-tint', pct(t.sidebarTint, 0) + '%');
    root.setProperty('--frame-alpha', pct(t.frameOpacity, 100) + '%');
    root.setProperty('--panel-alpha', pct(t.panelOpacity, 97) + '%');
    root.setProperty('--angle', (t.angle ?? 180) + 'deg');
    root.setProperty('--radius', (t.radius ?? 8) + 'px');
    root.setProperty('--blur', (t.blur ?? 0) + 'px');
    root.setProperty('--size', (t.size ?? 13) + 'px');
    root.setProperty('--font', t.customFont ? `${t.customFont}, ${FONTS.system}` : (FONTS[t.font] || FONTS.system));
    root.setProperty('--pad', { compact: '5px', normal: '8px', roomy: '12px' }[l.density] || '8px');
    root.setProperty('--wallpaper',
      t.wallpaper === 'gradient' || t.wallpaper === 'stars' ? 'linear-gradient(var(--angle), var(--bg1), var(--bg2))'
        : t.wallpaper === 'image' && t.image ? `url("${t.image}") center / cover no-repeat, var(--bg1)` : 'var(--bg1)');
    const b = document.body;
    b.className = '';
    b.classList.add('platform-' + boot.platform, 'tabs-' + l.tabs, 'tab-' + l.tabStyle);
    const flags = {
      stars: t.wallpaper === 'stars', light: t.light, glass: (t.blur ?? 0) > 0, frameless: pct(t.frameOpacity, 100) === 0,
      'toolbar-bottom': l.toolbar === 'bottom', right: l.sideRight, 'sidebar-hidden': l.sidebarHidden,
      'show-bookmarks': l.bookmarksBar, 'show-status': l.statusBar, edge: l.edge, 'address-center': l.addressCenter,
    };
    for (const [name, on] of Object.entries(flags)) if (on) b.classList.add(name);
    for (const [name, on] of Object.entries(l.buttons || {})) if (!on) b.classList.add('hide-' + name);
    $('#user-css').textContent = settings.uiCss || '';
    requestAnimationFrame(reportLayout);
  }

  let lastRect = '';
  function reportLayout() {
    const r = $('#content').getBoundingClientRect();
    const rect = { x: Math.round(r.left) + 1, y: Math.round(r.top) + 1, w: Math.round(r.width) - 2, h: Math.round(r.height) - 2 };
    const key = JSON.stringify(rect);
    if (key !== lastRect) {
      lastRect = key;
      send('layout', rect);
    }
    if (panel) placePanel();
  }
  new ResizeObserver(reportLayout).observe($('#content'));
  systemDark.addEventListener('change', () => { if (PRESETS[settings.theme.preset]?.system) { applyTheme(); send('saveSettings', { settings }); } });
  window.addEventListener('resize', reportLayout);

  // ---------- tabs ----------
  function tabEl(tab) {
    const el = document.createElement('div');
    el.className = 'tab' + (tab.id === state.active ? ' active' : '') + (tab.loading ? ' loading' : '');
    el.draggable = true;
    el.title = tab.title || tab.url;
    el.innerHTML = `<span class="mark"></span><span class="title">${esc(tab.title || tab.url || 'New tab')}</span><button class="close" title="Close tab">${icon('close')}</button>`;
    el.addEventListener('mousedown', (e) => { if (e.button === 1) { e.preventDefault(); send('closeTab', { id: tab.id }); } });
    el.addEventListener('click', (e) => {
      if (e.target.closest('.close')) send('closeTab', { id: tab.id });
      else send('selectTab', { id: tab.id });
    });
    el.addEventListener('dragstart', (e) => e.dataTransfer.setData('text/milky-tab', String(tab.id)));
    el.addEventListener('dragover', (e) => { e.preventDefault(); el.classList.add('drop-before'); });
    el.addEventListener('dragleave', () => el.classList.remove('drop-before'));
    el.addEventListener('drop', (e) => {
      e.preventDefault();
      el.classList.remove('drop-before');
      const id = Number(e.dataTransfer.getData('text/milky-tab'));
      const to = state.tabs.findIndex((t) => t.id === tab.id);
      if (id && id !== tab.id) send('moveTab', { id, to });
    });
    return el;
  }

  function renderTabs() {
    for (const list of [$('#top-tabs'), $('#side-tabs')]) list.replaceChildren(...state.tabs.map(tabEl));
  }

  function bookmarkLinks(container, empty) {
    const items = settings.bookmarks.map((b) => {
      const a = document.createElement('a');
      a.href = '#';
      a.textContent = b.title || b.url;
      a.title = b.url;
      a.addEventListener('click', (e) => { e.preventDefault(); (e.metaKey || e.ctrlKey) ? send('newTab', { url: b.url }) : send('navigate', { text: b.url }); });
      return a;
    });
    if (!items.length && empty) {
      const s = document.createElement('span');
      s.className = 'empty';
      s.textContent = empty;
      items.push(s);
    }
    container.replaceChildren(...items);
  }

  const activeTab = () => state.tabs.find((t) => t.id === state.active);

  function renderChrome() {
    renderTabs();
    bookmarkLinks($('#bookmarksbar'), 'No bookmarks yet');
    bookmarkLinks($('#side-bookmarks'));
    const tab = activeTab();
    if (tab && !editingAddress) $('#address').value = tab.url || '';
    const lock = $('#lock');
    const kind = !tab || !tab.url || editingAddress ? 'search' : tab.secure ? 'lock' : 'warn';
    lock.innerHTML = `<use href="#i-${kind}"/>`;
    lock.setAttribute('class', 'i lock' + (kind === 'warn' ? ' insecure' : ''));
    $('#reload').innerHTML = icon(tab && tab.loading ? 'stop' : 'reload');
    $('#reload').dataset.cmd = tab && tab.loading ? 'stop' : 'reload';
    $('#reload').title = tab && tab.loading ? 'Stop' : 'Reload';
    const starred = tab && tab.url && settings.bookmarks.some((b) => b.url === tab.url);
    $('#star').classList.toggle('on', !!starred);
    $('#status-left').textContent = `Lunar Search: ${state.searchPages.toLocaleString()} pages${state.crawling ? ', crawling' : ''}`;
    $('#status-right').textContent = tab ? (tab.loading ? 'Loading' : tab.secure ? 'Encrypted' : 'Not encrypted') : '';
    document.title = tab ? `${tab.title || 'New tab'} - Milky Ray` : 'Milky Ray';
  }

  // ---------- toolbar ----------
  document.addEventListener('click', (e) => {
    const cmd = e.target.closest('[data-cmd]');
    if (cmd) { send(cmd.dataset.cmd); return; }
    const p = e.target.closest('[data-panel]');
    if (p) togglePanel(p.dataset.panel, p.closest('.panel-tabs') !== null);
  });
  $('#address-form').addEventListener('submit', (e) => {
    e.preventDefault();
    const text = $('#address').value.trim();
    if (text) send('navigate', { text });
    editingAddress = false;
    $('#address').blur();
  });
  $('#address').addEventListener('focus', () => { editingAddress = true; renderChrome(); setTimeout(() => $('#address').select(), 0); });
  $('#address').addEventListener('blur', () => { editingAddress = false; renderChrome(); });
  $('#address').addEventListener('keydown', (e) => { if (e.key === 'Escape') { editingAddress = false; $('#address').blur(); } });
  $('#star').addEventListener('click', () => {
    const tab = activeTab();
    if (!tab || !tab.url || tab.url.startsWith('milky:')) return toast('Open a website to bookmark it');
    const i = settings.bookmarks.findIndex((b) => b.url === tab.url);
    if (i >= 0) { settings.bookmarks.splice(i, 1); toast('Bookmark removed'); }
    else { settings.bookmarks.push({ title: tab.title || tab.url, url: tab.url }); toast('Bookmarked'); }
    save();
    renderChrome();
  });

  document.addEventListener('mousedown', (e) => {
    if (e.button === 0 && e.target.classList.contains('drag')) send('drag');
  });
  document.addEventListener('dblclick', (e) => {
    if (e.target.id === 'top-tabs' || e.target.id === 'tabstrip') send('newTab');
    else if (e.target.classList.contains('drag')) send('maximize');
  });

  // ---------- panels ----------
  function placePanel() {
    const r = $('#content').getBoundingClientRect();
    Object.assign($('#panel').style, { left: r.left + 'px', top: r.top + 'px', width: r.width + 'px', height: r.height + 'px' });
  }

  function togglePanel(name, switching = false) {
    if (panel === name && !switching) return closePanel();
    panel = name;
    $('#panel').hidden = false;
    placePanel();
    $$('.panel-tabs button').forEach((b) => b.classList.toggle('on', b.dataset.panel === name));
    renderPanel();
    send('overlay', { open: true });
  }

  function closePanel() {
    panel = null;
    $('#panel').hidden = true;
    send('overlay', { open: false });
  }
  $('#panel-close').addEventListener('click', closePanel);
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && panel) closePanel(); });

  const hint = (h) => (h ? `<span class="hint">${h}</span>` : '');
  const sw = (key, label, h = '') => `<div class="row"><label>${label}${hint(h)}</label><input type="checkbox" class="switch" data-k="${key}" ${getPath(key) ? 'checked' : ''}></div>`;
  const color = (key, label) => `<div class="row"><label>${label}</label><input type="color" data-k="${key}" value="${toHex(getPath(key))}"></div>`;
  const range = (key, label, min, max, step = 1, unit = '', h = '') => `<div class="row"><label>${label} <span class="hint" style="display:inline" data-show="${key}">${getPath(key)}${unit}</span>${hint(h)}</label><input type="range" data-k="${key}" data-num data-unit="${unit}" min="${min}" max="${max}" step="${step}" value="${getPath(key)}"></div>`;
  const seg = (key, label, options) => `<div class="row"><label>${label}</label><div class="seg" data-seg="${key}">${options.map(([v, t]) => `<button data-v="${v}" class="${getPath(key) === v ? 'on' : ''}">${t}</button>`).join('')}</div></div>`;
  const text = (key, label, placeholder = '', h = '') => `<div class="row"><label>${label}${hint(h)}</label><input type="text" data-k="${key}" value="${esc(getPath(key))}" placeholder="${esc(placeholder)}" spellcheck="false"></div>`;
  const area = (key, placeholder, tall = false) => `<textarea data-k="${key}" spellcheck="false" class="${tall ? 'tall' : ''}" placeholder="${esc(placeholder)}">${esc(getPath(key))}</textarea>`;

  function toHex(c) {
    if (/^#[0-9a-f]{6}$/i.test(c || '')) return c;
    const m = String(c || '').match(/rgba?\(([^)]+)\)/);
    if (!m) return '#888888';
    return '#' + m[1].split(',').slice(0, 3).map((n) => Math.round(parseFloat(n)).toString(16).padStart(2, '0')).join('');
  }

  function renderPanel() {
    const body = $('#panel-body');
    const t = settings.theme;
    if (panel === 'appearance') {
      body.innerHTML = `
        <div class="group"><h3>Theme</h3>
          <p>Classic color schemes, each with the colors its author chose. Any of them can be changed below.</p>
          <div class="presets">${Object.entries(PRESETS).map(([name, p]) => `
          <button class="preset ${t.preset === name ? 'on' : ''}" data-preset="${esc(name)}" style="${p.system ? '' : `--p-bg:${p.bg1};--p-surface:${p.surface};--p-text:${p.text};--p-muted:${p.muted};--p-accent:${p.accent};--p-radius:${p.radius}px;--p-font:${FONTS[p.font]}`}">
            <div class="mini${p.system ? ' split' : ''}"><div class="mini-tabs"><i class="on"></i><i></i><i></i></div><div class="mini-bar"></div><div class="mini-page"><b></b><s></s><s></s><s class="link"></s></div></div>
            <span class="name">${esc(name)}<em>${p.system ? 'auto' : 'Aa'}</em></span></button>`).join('')}</div></div>
        <div class="group"><h3>Transparency</h3>
          <p>Every part of the window can be see-through. At 0 a part shows whatever is behind it: the window background, or your desktop.</p>
          <div class="btn-row"><button class="btn" id="see-through">Make everything see-through</button><button class="btn" id="solid">Make everything solid</button></div>
          <div class="subhead">Window</div>
          ${range('theme.windowOpacity', 'Window background', 0, 100, 1, '%', 'The color, gradient or picture behind everything')}
          ${seg('theme.backdrop', 'Blur the desktop behind', [['off', 'Off'], ['frosted', 'Frosted'], ['smoked', 'Smoked']])}
          ${boot.platform === 'linux' ? '<p class="note">Desktop blur isn’t available on Linux; the window can still be see-through.</p>' : ''}
          <div class="subhead">Parts of the window</div>
          ${range('theme.tabsTint', 'Tab bar', 0, 100, 1, '%')}
          ${range('theme.toolbarTint', 'Toolbar and bookmarks', 0, 100, 1, '%')}
          ${range('theme.sidebarTint', 'Sidebar', 0, 100, 1, '%')}
          ${range('theme.frameOpacity', 'Frame behind pages', 0, 100, 1, '%')}
          ${range('theme.panelOpacity', 'Panels (like this one)', 30, 100, 1, '%')}
          ${range('theme.blur', 'Blur behind bars and panels', 0, 40, 1, 'px')}
          <div class="subhead">Web pages</div>
          ${seg('theme.pageBackground', 'Page backgrounds', [['normal', 'Normal'], ['transparent', 'Transparent'], ['all', 'Everything clear']])}
          <p class="note">Transparent removes a page's own background color; Everything clear removes every background on the page, so only text and pictures stay. Text keeps full strength either way.</p>
          ${range('theme.pageOpacity', 'Whole page opacity', 20, 100, 1, '%', 'Fades text and pictures too')}</div>
        <div class="group"><h3>Color</h3>
          ${color('theme.bg1', 'Window')}${color('theme.surface', 'Surfaces')}${color('theme.accent', 'Accent')}
          ${color('theme.text', 'Text')}${color('theme.muted', 'Secondary text')}
          ${sw('theme.light', 'Light appearance', 'Scrollbars and form controls in light colors')}</div>
        <div class="group"><h3>Background</h3>
          ${seg('theme.wallpaper', 'Style', [['solid', 'Solid'], ['gradient', 'Gradient'], ['stars', 'Stars'], ['image', 'Picture']])}
          ${color('theme.bg2', 'Gradient end')}
          ${range('theme.angle', 'Gradient angle', 0, 360, 5, '°')}
          <div class="row"><label>Picture${hint('Used when the style is Picture')}</label><input type="file" id="wallpaper-file" accept="image/*"></div></div>
        <div class="group"><h3>Shape and type</h3>
          ${range('theme.radius', 'Corner radius', 0, 20, 1, 'px')}
          ${seg('layout.density', 'Density', [['compact', 'Compact'], ['normal', 'Normal'], ['roomy', 'Roomy']])}
          ${sw('layout.edge', 'Edge-to-edge pages', 'No margin or frame around pages')}
          ${seg('theme.font', 'Font', [['system', 'Sans'], ['rounded', 'Rounded'], ['serif', 'Serif'], ['mono', 'Mono'], ['vt323', 'VT323']])}
          ${text('theme.customFont', 'Any installed font', 'e.g. "IBM Plex Sans"', 'Overrides the choice above')}
          ${range('theme.size', 'Interface text', 11, 18, 0.5, 'px')}
          ${range('theme.pageSize', 'New tab and search text', 12, 22, 1, 'px')}</div>
        <div class="group"><h3>Layout</h3>
          ${seg('layout.tabs', 'Tabs', [['top', 'Top'], ['side', 'Sidebar']])}
          ${sw('layout.sideRight', 'Sidebar on the right')}
          ${seg('layout.tabStyle', 'Tab style', [['rounded', 'Rounded'], ['pill', 'Pill'], ['box', 'Folder'], ['underline', 'Underline'], ['minimal', 'Plain']])}
          ${seg('layout.toolbar', 'Toolbar', [['top', 'Top'], ['bottom', 'Bottom']])}
          ${sw('layout.addressCenter', 'Center the address')}
          ${sw('layout.bookmarksBar', 'Bookmarks bar')}${sw('layout.statusBar', 'Status bar')}</div>
        <div class="group"><h3>Toolbar buttons</h3>
          ${sw('layout.buttons.back', 'Back')}${sw('layout.buttons.forward', 'Forward')}${sw('layout.buttons.reload', 'Reload')}
          ${sw('layout.buttons.home', 'Home')}${sw('layout.buttons.star', 'Bookmark')}${sw('layout.buttons.appearance', 'Appearance', 'Also in the menu: View > Settings')}</div>
        <div class="group"><h3>New tab page</h3>
          ${sw('newTab.clock', 'Clock')}${sw('newTab.search', 'Search field')}
          ${text('newTab.greeting', 'Heading', 'None')}</div>
        <div class="group"><h3>Interface CSS</h3><p>Applied to tabs, toolbar and panels. Every color is a variable: --bg1, --surface, --text, --muted, --accent, --radius, --font.</p>
          ${area('uiCss', '#toolbar { gap: 6px; }')}</div>
        <div class="group"><h3>New tab and search page CSS</h3>${area('newTabCss', '.clock { font-weight: 600; }')}</div>
        <div class="group"><h3>Share</h3>
          <div class="btn-row"><button class="btn" id="export">Copy theme</button><button class="btn" id="import">Paste theme</button><button class="btn" id="reset">Reset appearance</button></div></div>`;
    } else if (panel === 'settings') {
      body.innerHTML = `
        <div class="group"><h3>Search</h3>
          ${seg('searchEngine', 'Search with', [['lunar', 'Lunar Search'], ['custom', 'Custom']])}
          ${text('customSearch', 'Custom search address', 'https://example.com/search?q=%s', '%s is replaced by your words')}
          <div class="row"><label>Lunar Search${hint(`${state.searchPages.toLocaleString()} pages, indexed on this computer${state.crawling ? ', still crawling' : ''}`)}</label><button class="btn" id="index-site">Index this site</button></div>
          ${sw('crawl', 'Keep crawling', 'Lunar Search grows its index in the background')}</div>
        <div class="group"><h3>Browsing</h3>
          ${text('homePage', 'Home page', 'New tab page')}
          ${range('defaultZoom', 'Default zoom', 50, 200, 5, '%', 'For new tabs')}
          ${text('userAgent', 'User agent', 'Your system browser’s', 'For new tabs. Leave empty for the default')}</div>
        <div class="group"><h3>Privacy and security</h3>
          ${sw('httpsOnly', 'HTTPS only', 'Upgrade http:// pages to https://')}
          ${sw('private', 'Private tabs', 'New tabs keep no cookies or history after they close')}
          <div class="btn-row"><button class="btn" id="clear-data">Clear cookies and site data</button></div></div>
        <div class="group"><h3>CSS for every website</h3><p>Added to all pages you open. Applies right away.</p>
          ${area('siteCss', 'img { filter: grayscale(1); }')}</div>
        <div class="group"><h3>JavaScript for every website</h3><p>Runs at the start of every page, like a userscript. Applies to pages loaded after you change it. Only paste code you trust.</p>
          ${area('siteJs', 'document.addEventListener("DOMContentLoaded", () => { /* … */ });')}</div>
        <div class="group"><h3>Interface files</h3>
          <p>Milky Ray's whole interface is three files: index.html, app.css and app.js. Copy them to a folder, change anything, and restart Milky Ray to use your version.</p>
          <div class="btn-row"><button class="btn" id="ui-files">Edit interface files</button><button class="btn" id="ui-reset">Use built-in interface</button><button class="btn" data-cmd="devtools">Developer tools</button></div></div>
        <div class="group"><h3>All settings</h3><p>Every setting as JSON. Edit and apply.</p>
          <textarea id="raw-settings" class="tall" spellcheck="false">${esc(JSON.stringify({ ...settings, theme: { ...settings.theme, image: settings.theme.image ? '(picture)' : '' } }, null, 2))}</textarea>
          <div class="btn-row"><button class="btn primary" id="raw-apply">Apply</button></div></div>
        <div class="group"><h3>About</h3><p>Lunar Browser Milky Ray. Pages are drawn by your system's web engine, so every site works. The interface, tabs, settings and Lunar Search are Milky Ray's own.</p></div>`;
    } else {
      body.innerHTML = `<div class="group"><h3>Bookmarks</h3><div class="bm-list">${settings.bookmarks.map((b, i) => `
        <div class="bm"><span>${esc(b.title)}<small>${esc(b.url)}</small></span><button class="btn" data-open="${i}">Open</button><button class="btn" data-remove="${i}">Remove</button></div>`).join('') || '<p class="note">No bookmarks yet. Use the star in the toolbar to add the page you are on.</p>'}
        </div></div>
        <div class="group"><h3>Add a bookmark</h3>${text('_bmTitle', 'Name', '')}${text('_bmUrl', 'Address', 'https://')}<div class="btn-row"><button class="btn primary" id="bm-add">Add</button></div></div>`;
    }
    bindPanel(body);
  }

  function changed(key) {
    const looks = ['theme.bg1', 'theme.bg2', 'theme.surface', 'theme.text', 'theme.muted', 'theme.accent', 'theme.light', 'theme.font', 'theme.radius'];
    if (looks.includes(key)) settings.theme.preset = 'Custom';
    applyTheme();
    renderChrome();
    save();
  }

  function bindPanel(body) {
    $$('[data-k]', body).forEach((el) => {
      const key = el.dataset.k;
      if (key.startsWith('_')) return;
      el.addEventListener('input', () => {
        let v = el.type === 'checkbox' ? el.checked : el.value;
        if ('num' in el.dataset) {
          v = Number(v);
          const show = $(`[data-show="${key}"]`, body);
          if (show) show.textContent = v + el.dataset.unit;
        }
        setPath(key, v);
        changed(key);
      });
    });
    $$('[data-seg]', body).forEach((group) => group.addEventListener('click', (e) => {
      const b = e.target.closest('button');
      if (!b) return;
      setPath(group.dataset.seg, b.dataset.v);
      $$('button', group).forEach((x) => x.classList.toggle('on', x === b));
      changed(group.dataset.seg);
    }));
    $$('[data-preset]', body).forEach((b) => b.addEventListener('click', () => {
      const p = PRESETS[b.dataset.preset];
      const colors = p.system ? {} : { ...p, bg2: p.bg1, light: !!p.light };
      settings.theme = { ...settings.theme, ...colors, preset: b.dataset.preset };
      applyTheme(); renderChrome(); save(); renderPanel();
    }));
    const file = $('#wallpaper-file', body);
    if (file) file.addEventListener('change', () => {
      const f = file.files[0];
      if (!f) return;
      if (f.size > 6 * 1024 * 1024) return toast('Pick a picture under 6 MB');
      const reader = new FileReader();
      reader.onload = () => { settings.theme.image = reader.result; settings.theme.wallpaper = 'image'; changed('theme.image'); renderPanel(); };
      reader.readAsDataURL(f);
    });
    const on = (id, fn) => { const el = $('#' + id, body); if (el) el.addEventListener('click', fn); };
    on('export', async () => {
      const theme = JSON.stringify({ theme: { ...settings.theme, image: '' }, layout: settings.layout, uiCss: settings.uiCss, newTabCss: settings.newTabCss });
      try { await navigator.clipboard.writeText(theme); toast('Theme copied'); } catch (_) { prompt('Copy your theme:', theme); }
    });
    on('import', async () => {
      let raw = '';
      try { raw = await navigator.clipboard.readText(); } catch (_) { raw = prompt('Paste a Milky Ray theme:') || ''; }
      try {
        const t = JSON.parse(raw);
        settings = merge(settings, { theme: t.theme || {}, layout: t.layout || {}, uiCss: t.uiCss ?? settings.uiCss, newTabCss: t.newTabCss ?? settings.newTabCss });
        applyTheme(); renderChrome(); save(); renderPanel(); toast('Theme applied');
      } catch (_) { toast('That isn’t a Milky Ray theme'); }
    });
    on('reset', () => {
      settings = merge(settings, { theme: DEFAULTS.theme, layout: DEFAULTS.layout, uiCss: '', newTabCss: '' });
      applyTheme(); renderChrome(); save(); renderPanel();
    });
    on('see-through', () => {
      Object.assign(settings.theme, { windowOpacity: 0, tabsTint: 0, toolbarTint: 0, sidebarTint: 0, frameOpacity: 0, panelOpacity: 70, blur: 12, backdrop: boot.platform === 'linux' ? 'off' : 'frosted', pageBackground: 'all' });
      applyTheme(); renderChrome(); save(); renderPanel(); toast('Everything is see-through');
    });
    on('solid', () => {
      Object.assign(settings.theme, { windowOpacity: 100, tabsTint: 0, toolbarTint: 0, sidebarTint: 0, frameOpacity: 100, panelOpacity: 97, blur: 0, backdrop: 'off', pageBackground: 'normal', pageOpacity: 100 });
      applyTheme(); renderChrome(); save(); renderPanel();
    });
    on('raw-apply', () => {
      try {
        const next = JSON.parse($('#raw-settings', body).value);
        if (next.theme && next.theme.image === '(picture)') next.theme.image = settings.theme.image;
        settings = merge(DEFAULTS, next);
        applyTheme(); renderChrome(); save(); renderPanel(); toast('Settings applied');
      } catch (e) { toast('Not valid JSON: ' + e.message); }
    });
    on('index-site', () => send('indexSite'));
    on('clear-data', () => send('clearData'));
    on('ui-files', () => send('uiFiles'));
    on('ui-reset', () => send('uiReset'));
    on('bm-add', () => {
      const title = $('[data-k="_bmTitle"]', body).value.trim();
      let url = $('[data-k="_bmUrl"]', body).value.trim();
      if (!/^https?:\/\//.test(url)) url = 'https://' + url;
      if (url.length < 12) return toast('Type an address');
      settings.bookmarks.push({ title: title || url, url });
      save(); renderChrome(); renderPanel();
    });
    $$('[data-open]', body).forEach((b) => b.addEventListener('click', () => { send('navigate', { text: settings.bookmarks[b.dataset.open].url }); closePanel(); }));
    $$('[data-remove]', body).forEach((b) => b.addEventListener('click', () => { settings.bookmarks.splice(Number(b.dataset.remove), 1); save(); renderChrome(); renderPanel(); }));
  }

  function toast(message) {
    const el = document.createElement('div');
    el.className = 'toast';
    el.textContent = message;
    $('#toasts').append(el);
    setTimeout(() => el.remove(), 3000);
  }

  // ---------- the app calls these ----------
  window.milky = {
    update(next) {
      state = next;
      renderChrome();
    },
    focusAddress() { const a = $('#address'); a.focus(); a.select(); },
    toast,
    togglePanel,
    toggleSidebar() {
      if (settings.layout.tabs !== 'side') { settings.layout.tabs = 'side'; settings.layout.sidebarHidden = false; }
      else settings.layout.sidebarHidden = !settings.layout.sidebarHidden;
      applyTheme(); save();
    },
  };

  applyTheme();
  renderChrome();
  send('ready');
})();
