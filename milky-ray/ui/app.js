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
    Graphite: { bg1: '#141518', bg2: '#141518', surface: '#1d1f23', text: '#e6e4df', muted: '#8b8984', accent: '#e0a458', wallpaper: 'solid', font: 'system', radius: 8 },
    Porcelain: { bg1: '#f3f2ef', bg2: '#f3f2ef', surface: '#ffffff', text: '#1d1c1a', muted: '#76736d', accent: '#2f6fdf', wallpaper: 'solid', font: 'system', radius: 8, light: true },
    Midnight: { bg1: '#0e1116', bg2: '#0e1116', surface: '#161b22', text: '#d6dde6', muted: '#7d8794', accent: '#58a6ff', wallpaper: 'solid', font: 'system', radius: 6 },
    Sand: { bg1: '#ece6db', bg2: '#ece6db', surface: '#f7f3ea', text: '#2b2620', muted: '#7c7264', accent: '#b4532a', wallpaper: 'solid', font: 'serif', radius: 4, light: true },
    Moss: { bg1: '#121612', bg2: '#121612', surface: '#1a201a', text: '#dfe6d8', muted: '#8a9684', accent: '#a3c46c', wallpaper: 'solid', font: 'system', radius: 8 },
    Ink: { bg1: '#0a0a0a', bg2: '#0a0a0a', surface: '#151515', text: '#f0f0f0', muted: '#8a8a8a', accent: '#f0f0f0', wallpaper: 'solid', font: 'mono', radius: 2 },
    'Milky Way': { bg1: '#0f0d1a', bg2: '#251a40', surface: '#1b1829', text: '#ece8f6', muted: '#9a93b3', accent: '#c4b5fd', wallpaper: 'stars', font: 'system', radius: 10, angle: 160 },
    'Lunar Terminal': { bg1: '#05070d', bg2: '#05070d', surface: '#0a0e18', text: '#c9d2e3', muted: '#5f6a85', accent: '#8fd8ff', wallpaper: 'solid', font: 'vt323', radius: 0 },
  };
  const FONTS = {
    system: 'Inter, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif',
    rounded: 'ui-rounded, "SF Pro Rounded", Nunito, system-ui, sans-serif',
    serif: 'ui-serif, "New York", Charter, Georgia, serif',
    mono: 'ui-monospace, "SF Mono", Menlo, Consolas, monospace',
    vt323: '"VT323", ui-monospace, monospace',
  };
  const DEFAULTS = {
    theme: { preset: 'Graphite', ...PRESETS.Graphite, angle: 180, blur: 0, size: 13, pageSize: 15, image: '', light: false, windowOpacity: 100, pageOpacity: 100, customFont: '' },
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
    const t = settings.theme;
    const l = settings.layout;
    const root = document.documentElement.style;
    for (const k of ['bg1', 'bg2', 'surface', 'text', 'muted', 'accent']) root.setProperty('--' + k, t[k]);
    root.setProperty('--angle', (t.angle ?? 180) + 'deg');
    root.setProperty('--radius', (t.radius ?? 8) + 'px');
    root.setProperty('--blur', (t.blur ?? 0) + 'px');
    root.setProperty('--size', (t.size ?? 13) + 'px');
    root.setProperty('--window-alpha', (t.windowOpacity ?? 100) + '%');
    root.setProperty('--font', t.customFont ? `${t.customFont}, ${FONTS.system}` : (FONTS[t.font] || FONTS.system));
    root.setProperty('--pad', { compact: '5px', normal: '8px', roomy: '12px' }[l.density] || '8px');
    root.setProperty('--wallpaper',
      t.wallpaper === 'gradient' || t.wallpaper === 'stars'
        ? 'linear-gradient(var(--angle), color-mix(in srgb, var(--bg1) var(--window-alpha), transparent), color-mix(in srgb, var(--bg2) var(--window-alpha), transparent))'
        : t.wallpaper === 'image' && t.image ? `url("${t.image}") center / cover no-repeat, var(--chrome)` : 'var(--chrome)');
    const b = document.body;
    b.className = '';
    b.classList.add('platform-' + boot.platform, 'tabs-' + l.tabs, 'tab-' + l.tabStyle);
    const flags = {
      stars: t.wallpaper === 'stars', light: t.light, glass: (t.blur ?? 0) > 0,
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
        <div class="group"><h3>Theme</h3><div class="presets">${Object.entries(PRESETS).map(([name, p]) => `
          <button class="preset ${t.preset === name ? 'on' : ''}" data-preset="${esc(name)}">
            <div class="swatch"><span style="background:${p.bg1}"></span><span style="background:${p.surface}"></span><span style="background:${p.accent}"></span></div>
            <span class="name">${esc(name)}</span></button>`).join('')}</div></div>
        <div class="group"><h3>Color</h3>
          ${color('theme.bg1', 'Window')}${color('theme.surface', 'Surfaces')}${color('theme.accent', 'Accent')}
          ${color('theme.text', 'Text')}${color('theme.muted', 'Secondary text')}
          ${sw('theme.light', 'Light appearance', 'Scrollbars and form controls in light colors')}</div>
        <div class="group"><h3>Transparency</h3>
          <p>Lower these to see your desktop through Milky Ray. Pages that paint their own background stay solid at 100%.</p>
          ${range('theme.windowOpacity', 'Window', 0, 100, 1, '%')}
          ${range('theme.pageOpacity', 'Pages', 20, 100, 1, '%', 'Makes whole web pages see-through')}
          ${range('theme.blur', 'Blur behind bars', 0, 40, 1, 'px')}</div>
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
    if (key.startsWith('theme.') && !['theme.preset', 'theme.windowOpacity', 'theme.pageOpacity', 'theme.blur'].includes(key)) settings.theme.preset = 'Custom';
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
      settings.theme = { ...settings.theme, light: false, angle: 180, ...p, preset: b.dataset.preset };
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
