use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, CursorIcon, FontId, Key, KeyboardShortcut, Layout, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};
use url::Url;

use crate::engine::doc::{self, Doc, FieldKind, Style};
use crate::engine::html;
use crate::engine::layout::{self, Action, Line};
use crate::net::dns::Resolver;
use crate::net::http::{self, CookieJar, Request};
use crate::pages;
use crate::search::Engine;
use crate::settings::{self, Settings, TEXT_SIZES};

const NEW_TAB: &str = "lunar:newtab";
const SEARCH: &str = "lunar:search";
const MAX_COLS: usize = 110;

pub mod color {
    use eframe::egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(0x05, 0x07, 0x0d);
    pub const PANEL: Color32 = Color32::from_rgb(0x0a, 0x0e, 0x18);
    pub const LINE: Color32 = Color32::from_rgb(0x1c, 0x24, 0x38);
    pub const TEXT: Color32 = Color32::from_rgb(0xc9, 0xd2, 0xe3);
    pub const BRIGHT: Color32 = Color32::from_rgb(0xf2, 0xf5, 0xfa);
    pub const DIM: Color32 = Color32::from_rgb(0x5f, 0x6a, 0x85);
    pub const MOON: Color32 = Color32::from_rgb(0xe9, 0xe4, 0xb8);
    pub const ACCENT: Color32 = Color32::from_rgb(0x8f, 0xd8, 0xff);
    pub const OK: Color32 = Color32::from_rgb(0x7e, 0xe2, 0xa0);
    pub const BAD: Color32 = Color32::from_rgb(0xff, 0x7b, 0x72);
    pub const FIELD: Color32 = Color32::from_rgb(0x12, 0x1a, 0x2c);
}

struct Net {
    resolver: Resolver,
    jar: Mutex<CookieJar>,
}

#[derive(Clone)]
enum Target {
    Get(String),
    Post(String, Vec<u8>),
}

impl Target {
    fn url(&self) -> &str {
        match self {
            Target::Get(u) | Target::Post(u, _) => u,
        }
    }
}

struct Page {
    /// What the address bar shows.
    url: String,
    /// What relative links resolve against.
    base: Option<Url>,
    doc: Doc,
    lines: Vec<Line>,
    laid_out: Option<(usize, Option<usize>, u64)>,
    focus: Option<usize>,
    edits: u64,
    secure: bool,
    error: bool,
}

impl Page {
    fn new(url: String, base: Option<Url>, doc: Doc, secure: bool, error: bool) -> Self {
        Page { url, base, doc, lines: Vec::new(), laid_out: None, focus: None, edits: 0, secure, error }
    }
}

struct Tab {
    id: u64,
    history: Vec<Target>,
    pos: usize,
    page: Option<Page>,
    loading: Option<u64>,
    scroll_to_top: bool,
}

impl Tab {
    fn title(&self) -> String {
        match &self.page {
            Some(p) if !p.doc.title.trim().is_empty() => p.doc.title.trim().to_string(),
            Some(p) if !p.url.is_empty() => p.url.clone(),
            _ if self.loading.is_some() => "loading".into(),
            _ => "New tab".into(),
        }
    }
}

struct Loaded {
    tab: u64,
    load: u64,
    page: Page,
}

pub struct LunarApp {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    address: String,
    address_has_focus: bool,
    focus_address: bool,
    settings_open: bool,
    settings: Settings,
    search: Arc<Engine>,
    net: Arc<Net>,
    tx: Sender<Loaded>,
    rx: Receiver<Loaded>,
    hovered_link: Option<String>,
    notice: Option<(String, Instant)>,
    spinner: usize,
    index_ready: bool,
    screenshot: Option<(String, u32)>,
}

fn display_url(target: &str) -> String {
    if target == NEW_TAB { String::new() } else { target.to_string() }
}

fn page_from_html(url: &str, base: Option<Url>, source: &str, secure: bool, error: bool) -> Page {
    let doc = doc::build(&html::parse(source));
    Page::new(display_url(url), base, doc, secure, error)
}

fn load(net: &Net, target: &Target) -> Page {
    let url = match Url::parse(target.url()) {
        Ok(u) if matches!(u.scheme(), "http" | "https") => u,
        _ => return page_from_html(target.url(), None, &pages::error(target.url(), "ERR_UNSUPPORTED: not a web address"), false, true),
    };
    let (method, body) = match target {
        Target::Get(_) => ("GET", None),
        Target::Post(_, b) => ("POST", Some(b.clone())),
    };
    let request = Request { url, method: method.into(), body };
    match http::fetch(request, &net.jar, &net.resolver) {
        Ok(response) => {
            let final_url = response.url.to_string();
            let secure = response.url.scheme() == "https";
            let kind = response.header("content-type").unwrap_or("text/html").to_ascii_lowercase();
            if kind.contains("html") || kind.contains("xml") {
                let text = http::decode_text(&response);
                page_from_html(&final_url, Some(response.url.clone()), &text, secure, false)
            } else if kind.starts_with("text/") || kind.contains("json") {
                let text = http::decode_text(&response);
                let mut doc = Doc::default();
                doc.title = final_url.rsplit('/').next().unwrap_or("").to_string();
                doc.blocks.push(doc::Block::Pre(vec![doc::Inline::Text { text, style: Style::Code, link: None }]));
                Page::new(final_url, Some(response.url), doc, secure, false)
            } else {
                let error = format!("ERR_UNSUPPORTED: {kind}");
                page_from_html(&final_url, None, &pages::error(&final_url, &error), secure, true)
            }
        }
        Err(e) => page_from_html(target.url(), None, &pages::error(target.url(), &e), false, true),
    }
}

fn internal_page(search: &Engine, target: &str) -> Option<Page> {
    if target == NEW_TAB {
        return Some(page_from_html(NEW_TAB, None, &pages::new_tab(search.pages()), true, false));
    }
    let query = target.strip_prefix(SEARCH)?;
    let q = url::form_urlencoded::parse(query.trim_start_matches('?').as_bytes())
        .find(|(k, _)| k == "q")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default();
    let html = pages::search(&q, &search.search(&q));
    Some(page_from_html(target, None, &html, true, false))
}

fn target_for(input: &str) -> Option<Target> {
    let text = input.trim();
    if text.is_empty() {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    if lower == NEW_TAB || lower.starts_with(SEARCH) {
        return Some(Target::Get(text.into()));
    }
    if lower.starts_with("https://") || lower.starts_with("http://") {
        return Some(Target::Get(text.into()));
    }
    let is_local = lower.starts_with("localhost") || lower.starts_with("127.0.0.1");
    let looks_like_host = !text.contains(char::is_whitespace)
        && text.split(['/', '?', '#']).next().map_or(false, |h| h.contains('.') && !h.starts_with('.') && !h.ends_with('.'));
    if is_local {
        return Some(Target::Get(format!("http://{text}")));
    }
    if looks_like_host {
        return Some(Target::Get(format!("https://{text}")));
    }
    Some(Target::Get(settings::search_url(text)))
}

pub fn dump(input: &str, cols: usize) -> bool {
    let Some(target) = target_for(input) else { return false };
    let page = if target.url() == NEW_TAB || target.url().starts_with(SEARCH) {
        let search = Engine::open_now(settings::index_path());
        internal_page(&search, target.url()).unwrap()
    } else {
        let net = Net { resolver: Resolver::new(), jar: Mutex::new(CookieJar::default()) };
        load(&net, &target)
    };
    eprintln!("{} | {}", page.url, page.doc.title);
    for line in layout::layout(&page.doc, cols, None) {
        println!("{}", line.runs.iter().map(|r| r.text.as_str()).collect::<String>());
    }
    !page.error
}

pub fn crawl(pages: usize, minutes: u64) {
    let search = Engine::open(settings::index_path(), true);
    let start = Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(5));
        let n = search.pages();
        eprintln!("{:>5}s  {n} pages", start.elapsed().as_secs());
        let out_of_time = start.elapsed() > Duration::from_secs(minutes * 60);
        if n >= pages || out_of_time || (start.elapsed() > Duration::from_secs(20) && !search.crawling()) {
            break;
        }
    }
    search.set_crawl(false);
    eprintln!("saved {} pages", search.pages());
}

impl LunarApp {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let settings = Settings::load();
        let search = Engine::open(settings::index_path(), settings.crawl);
        let mut app = LunarApp {
            tabs: Vec::new(),
            active: 0,
            next_id: 1,
            address: String::new(),
            address_has_focus: false,
            focus_address: true,
            settings_open: false,
            settings,
            search,
            net: Arc::new(Net { resolver: Resolver::new(), jar: Mutex::new(CookieJar::default()) }),
            tx,
            rx,
            hovered_link: None,
            notice: None,
            spinner: 0,
            index_ready: false,
            screenshot: std::env::var("LUNAR_SCREENSHOT").ok().map(|p| (p, 0)),
        };
        app.new_tab(None);
        if let Ok(open) = std::env::var("LUNAR_OPEN") {
            if let Some(target) = target_for(&open) {
                app.go(0, target, true);
                app.focus_address = false;
            }
        }
        app.settings_open = std::env::var("LUNAR_SETTINGS").is_ok();
        app
    }

    fn notice(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), Instant::now()));
    }

    fn new_tab(&mut self, target: Option<Target>) {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab { id, history: Vec::new(), pos: 0, page: None, loading: None, scroll_to_top: true });
        self.active = self.tabs.len() - 1;
        let target = target.unwrap_or(Target::Get(NEW_TAB.into()));
        let is_new_tab = target.url() == NEW_TAB;
        self.go(self.active, target, true);
        self.focus_address = is_new_tab;
    }

    fn close_tab(&mut self, index: usize, ctx: &egui::Context) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.active >= self.tabs.len() || self.active > index {
            self.active = self.active.saturating_sub(1).min(self.tabs.len() - 1);
        }
        self.sync_address();
    }

    fn sync_address(&mut self) {
        if let Some(tab) = self.tabs.get(self.active) {
            self.address = match (&tab.page, tab.history.get(tab.pos)) {
                (Some(p), _) => p.url.clone(),
                (None, Some(t)) => display_url(t.url()),
                _ => String::new(),
            };
        }
    }

    fn go(&mut self, index: usize, target: Target, push: bool) {
        let internal = internal_page(&self.search, target.url());
        let Some(tab) = self.tabs.get_mut(index) else { return };
        if push {
            tab.history.truncate(if tab.history.is_empty() { 0 } else { tab.pos + 1 });
            tab.history.push(target.clone());
            tab.pos = tab.history.len() - 1;
        }
        tab.scroll_to_top = true;
        if let Some(page) = internal {
            tab.loading = None;
            tab.page = Some(page);
        } else {
            let load_id = self.next_id;
            self.next_id += 1;
            tab.loading = Some(load_id);
            let (tx, net, tab_id, target) = (self.tx.clone(), self.net.clone(), tab.id, target.clone());
            std::thread::spawn(move || {
                let page = load(&net, &target);
                let _ = tx.send(Loaded { tab: tab_id, load: load_id, page });
            });
        }
        if index == self.active {
            self.sync_address();
            if target.url() != NEW_TAB {
                self.address = target.url().to_string();
            }
        }
    }

    fn back(&mut self) {
        let i = self.active;
        let tab = &mut self.tabs[i];
        if tab.pos > 0 {
            tab.pos -= 1;
            let t = tab.history[tab.pos].clone();
            self.go(i, t, false);
        }
    }

    fn forward(&mut self) {
        let i = self.active;
        let tab = &mut self.tabs[i];
        if tab.pos + 1 < tab.history.len() {
            tab.pos += 1;
            let t = tab.history[tab.pos].clone();
            self.go(i, t, false);
        }
    }

    fn reload(&mut self) {
        let i = self.active;
        if let Some(t) = self.tabs[i].history.get(self.tabs[i].pos).cloned() {
            self.go(i, t, false);
        }
    }

    fn refresh_when_index_loads(&mut self, ctx: &egui::Context) {
        if self.index_ready {
            return;
        }
        if !self.search.loaded() {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        self.index_ready = true;
        for tab in &mut self.tabs {
            let Some(target) = tab.history.get(tab.pos) else { continue };
            if let Some(page) = internal_page(&self.search, target.url()) {
                tab.page = Some(page);
            }
        }
    }

    fn receive(&mut self) {
        while let Ok(loaded) = self.rx.try_recv() {
            if let Some(index) = self.tabs.iter().position(|t| t.id == loaded.tab) {
                let tab = &mut self.tabs[index];
                if tab.loading != Some(loaded.load) {
                    continue; // replaced by a newer load
                }
                tab.loading = None;
                tab.scroll_to_top = true;
                tab.page = Some(loaded.page);
                if index == self.active && !self.address_has_focus {
                    self.sync_address();
                }
            }
        }
    }

    /// Follows a link or acts on a form field.
    fn activate(&mut self, action: Action, new_tab: bool) {
        let i = self.active;
        let Some(page) = self.tabs[i].page.as_mut() else { return };
        match action {
            Action::Link(href) => {
                let url = match &page.base {
                    Some(base) => base.join(&href).ok(),
                    None => Url::parse(&href).ok(),
                };
                let Some(url) = url else { return self.notice(format!("bad link: {href}")) };
                if !matches!(url.scheme(), "http" | "https") {
                    return self.notice(format!("Lunar doesn't open {}: links", url.scheme()));
                }
                // Links within the same page: nothing to load.
                if let Some(base) = &page.base {
                    let mut a = url.clone();
                    let mut b = base.clone();
                    a.set_fragment(None);
                    b.set_fragment(None);
                    if a == b && url.fragment().is_some() && !new_tab {
                        return;
                    }
                }
                if new_tab {
                    self.new_tab(Some(Target::Get(url.to_string())));
                    self.notice("opened in a new tab");
                } else {
                    self.go(i, Target::Get(url.to_string()), true);
                }
            }
            Action::Field(f) => {
                let kind = page.doc.fields[f].kind.clone();
                match kind {
                    FieldKind::Text | FieldKind::Password => {
                        page.focus = Some(f);
                    }
                    FieldKind::Checkbox => {
                        let field = &mut page.doc.fields[f];
                        field.checked = !field.checked;
                    }
                    FieldKind::Radio => {
                        let (name, form) = (page.doc.fields[f].name.clone(), page.doc.fields[f].form);
                        for other in page.doc.fields.iter_mut() {
                            if other.kind == FieldKind::Radio && other.name == name && other.form == form {
                                other.checked = false;
                            }
                        }
                        page.doc.fields[f].checked = true;
                    }
                    FieldKind::Select(options) => {
                        let field = &mut page.doc.fields[f];
                        let at = options.iter().position(|(v, _)| *v == field.value).unwrap_or(0);
                        field.value = options[(at + 1) % options.len()].0.clone();
                    }
                    FieldKind::Submit => {
                        let form = page.doc.fields[f].form;
                        self.submit(form, Some(f));
                        return;
                    }
                    FieldKind::Hidden => {}
                }
                if let Some(page) = self.tabs[i].page.as_mut() {
                    page.edits += 1;
                }
            }
        }
    }

    fn submit(&mut self, form: Option<usize>, button: Option<usize>) {
        let i = self.active;
        let Some(page) = self.tabs[i].page.as_ref() else { return };
        let Some(form_index) = form else { return };
        let form = &page.doc.forms[form_index];
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        for (n, field) in page.doc.fields.iter().enumerate() {
            if field.form != Some(form_index) || field.name.is_empty() {
                continue;
            }
            match field.kind {
                FieldKind::Checkbox | FieldKind::Radio if !field.checked => continue,
                FieldKind::Submit if button != Some(n) => continue,
                _ => {}
            }
            query.append_pair(&field.name, &field.value);
        }
        let encoded = query.finish();
        let action = form.action.trim();
        let url = match &page.base {
            Some(base) => base.join(action),
            None => Url::parse(action),
        };
        let Ok(mut url) = url else { return self.notice("this form has nowhere to go") };
        if url.as_str().starts_with(SEARCH) {
            return self.go(i, Target::Get(format!("{SEARCH}?{encoded}")), true);
        }
        if !matches!(url.scheme(), "http" | "https") {
            return self.notice("this form doesn't send to a web address");
        }
        let target = if form.method == "post" {
            Target::Post(url.to_string(), encoded.into_bytes())
        } else {
            url.set_query(Some(&encoded));
            url.set_fragment(None);
            Target::Get(url.to_string())
        };
        self.go(i, target, true);
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let pressed = |ctx: &egui::Context, m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)));
        if pressed(ctx, Modifiers::COMMAND, Key::T) {
            self.new_tab(None);
        }
        if pressed(ctx, Modifiers::COMMAND, Key::W) {
            self.close_tab(self.active, ctx);
        }
        if pressed(ctx, Modifiers::COMMAND, Key::L) {
            self.focus_address = true;
        }
        if pressed(ctx, Modifiers::COMMAND, Key::R) {
            self.reload();
        }
        if pressed(ctx, Modifiers::COMMAND, Key::Comma) {
            self.settings_open = !self.settings_open;
        }
        if pressed(ctx, Modifiers::COMMAND, Key::OpenBracket) || pressed(ctx, Modifiers::ALT, Key::ArrowLeft) {
            self.back();
        }
        if pressed(ctx, Modifiers::COMMAND, Key::CloseBracket) || pressed(ctx, Modifiers::ALT, Key::ArrowRight) {
            self.forward();
        }
        if pressed(ctx, Modifiers::CTRL, Key::Tab) && !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
            self.sync_address();
        }
    }

    fn type_into_field(&mut self, ctx: &egui::Context) {
        if self.address_has_focus {
            return;
        }
        let i = self.active;
        let Some(page) = self.tabs.get_mut(i).and_then(|t| t.page.as_mut()) else { return };
        let Some(f) = page.focus else { return };
        let events = ctx.input(|inp| inp.events.clone());
        let mut submit = false;
        for event in events {
            match event {
                egui::Event::Text(text) => {
                    page.doc.fields[f].value.push_str(&text);
                    page.edits += 1;
                }
                egui::Event::Paste(text) => {
                    page.doc.fields[f].value.push_str(text.lines().next().unwrap_or(""));
                    page.edits += 1;
                }
                egui::Event::Key { key: Key::Backspace, pressed: true, modifiers, .. } => {
                    let value = &mut page.doc.fields[f].value;
                    if modifiers.command || modifiers.alt {
                        let trimmed = value.trim_end();
                        let cut = trimmed.rfind(' ').map_or(0, |p| p + 1);
                        value.truncate(cut);
                    } else {
                        value.pop();
                    }
                    page.edits += 1;
                }
                egui::Event::Key { key: Key::Enter, pressed: true, .. } => submit = true,
                egui::Event::Key { key: Key::Escape, pressed: true, .. } => {
                    page.focus = None;
                    page.edits += 1;
                }
                egui::Event::Key { key: Key::Tab, pressed: true, .. } => {
                    let next = page.doc.fields.iter().enumerate().skip(f + 1)
                        .find(|(_, fl)| matches!(fl.kind, FieldKind::Text | FieldKind::Password))
                        .map(|(n, _)| n);
                    page.focus = next.or(Some(f));
                    page.edits += 1;
                }
                _ => {}
            }
        }
        if submit {
            let form = page.doc.fields[f].form;
            page.focus = None;
            self.submit(form, None);
        }
    }

    fn tab_strip(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            if cfg!(target_os = "macos") {
                ui.add_space(70.0);
            }
            ui.label(RichText::new(">LUNAR_").color(color::MOON));
            ui.add_space(8.0);
            let mut select = None;
            let mut close = None;
            let width = ((ui.available_width() - 60.0) / self.tabs.len().max(1) as f32).clamp(60.0, 220.0);
            for (i, tab) in self.tabs.iter().enumerate() {
                let active = i == self.active;
                let spin = ["|", "/", "-", "\\"][self.spinner % 4];
                let mark = match &tab.page {
                    _ if tab.loading.is_some() => spin,
                    Some(p) if p.error => "x",
                    Some(p) if !p.secure => "!",
                    _ => "*",
                };
                let max_chars = ((width - 40.0) / 9.0).max(4.0) as usize;
                let mut title: String = tab.title().chars().take(max_chars).collect();
                if tab.title().chars().count() > max_chars {
                    title.push('…');
                }
                let text = RichText::new(format!("[{mark} {title}")).color(if active { color::BRIGHT } else { color::DIM });
                let resp = ui.add_sized([width - 24.0, 26.0], egui::Button::new(text).frame(active).selected(active));
                if resp.clicked() {
                    select = Some(i);
                }
                if resp.middle_clicked() {
                    close = Some(i);
                }
                let x = ui.add(egui::Button::new(RichText::new("x]").color(color::DIM)).frame(false));
                if x.clicked() {
                    close = Some(i);
                }
            }
            if ui.add(egui::Button::new("[+]").frame(false)).on_hover_text("New tab (ctrl/cmd+T)").clicked() {
                self.new_tab(None);
            }
            if let Some(i) = select {
                self.active = i;
                self.sync_address();
            }
            if let Some(i) = close {
                self.close_tab(i, ctx);
            }
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (can_back, can_fwd, loading) = {
                let t = &self.tabs[self.active];
                (t.pos > 0, t.pos + 1 < t.history.len(), t.loading.is_some())
            };
            if ui.add_enabled(can_back, egui::Button::new("[<]").frame(false)).on_hover_text("Back").clicked() {
                self.back();
            }
            if ui.add_enabled(can_fwd, egui::Button::new("[>]").frame(false)).on_hover_text("Forward").clicked() {
                self.forward();
            }
            let (label, tip) = if loading { ("[x]", "Stop") } else { ("[@]", "Reload") };
            if ui.add(egui::Button::new(label).frame(false)).on_hover_text(tip).clicked() {
                if loading {
                    self.tabs[self.active].loading = None;
                } else {
                    self.reload();
                }
            }
            if ui.add(egui::Button::new("[~]").frame(false)).on_hover_text("New tab page").clicked() {
                let i = self.active;
                self.go(i, Target::Get(NEW_TAB.into()), true);
            }
            let secure = self.tabs[self.active].page.as_ref().map_or(true, |p| p.secure);
            let lock = if self.address.is_empty() { " " } else if secure { "#" } else { "!" };
            ui.label(RichText::new(format!(">{lock}")).color(if secure { color::OK } else { color::BAD }));
            let settings_w = 40.0;
            let edit = egui::TextEdit::singleline(&mut self.address)
                .hint_text("search or type an address...")
                .desired_width(ui.available_width() - settings_w);
            let resp = ui.add(edit);
            if self.focus_address {
                resp.request_focus();
                self.focus_address = false;
            }
            self.address_has_focus = resp.has_focus();
            if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                if let Some(target) = target_for(&self.address.clone()) {
                    let i = self.active;
                    self.go(i, target, true);
                }
            }
            let gear = if self.settings_open { RichText::new("[=]").color(color::ACCENT) } else { RichText::new("[=]") };
            if ui.add(egui::Button::new(gear).frame(false)).on_hover_text("Settings (ctrl/cmd+,)").clicked() {
                self.settings_open = !self.settings_open;
            }
        });
    }

    fn settings_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("settings:").color(color::MOON));
            ui.add_space(10.0);
            let mut changed = false;
            ui.label(RichText::new(format!("lunar search: {} pages", self.search.pages())).color(color::DIM));
            let can_index = self.tabs[self.active].page.as_ref().and_then(|p| p.base.clone());
            if ui.add_enabled(can_index.is_some(), egui::Button::new("[ index this site ]").frame(false))
                .on_hover_text("Crawl the site you're on (up to 300 pages) and add it to Lunar Search")
                .clicked()
            {
                if let Some(url) = can_index {
                    if self.search.add_site(&url) {
                        self.settings.crawl = true;
                        self.search.set_crawl(true);
                        changed = true;
                        self.notice(format!("indexing {}", url.host_str().unwrap_or("")));
                    }
                }
            }
            let crawl_label = if self.settings.crawl { "[ crawling: on ]" } else { "[ crawling: off ]" };
            if ui.add(egui::Button::new(crawl_label).frame(false))
                .on_hover_text("Lunar Search grows its index by crawling in the background")
                .clicked()
            {
                self.settings.crawl = !self.settings.crawl;
                self.search.set_crawl(self.settings.crawl);
                changed = true;
            }
            ui.add_space(10.0);
            ui.label(RichText::new("text").color(color::DIM));
            egui::ComboBox::from_id_salt("size")
                .selected_text(format!("{}", self.settings.text_size))
                .show_ui(ui, |ui| {
                    for size in TEXT_SIZES {
                        if ui.selectable_label(self.settings.text_size == *size, format!("{size}")).clicked() {
                            self.settings.text_size = *size;
                            changed = true;
                        }
                    }
                });
            if changed {
                self.settings.save();
                let fresh = pages::new_tab(self.search.pages());
                for tab in &mut self.tabs {
                    if tab.history.get(tab.pos).map_or(false, |t| t.url() == NEW_TAB) {
                        tab.page = Some(page_from_html(NEW_TAB, None, &fresh, true, false));
                    }
                }
            }
            ui.add_space(10.0);
            if ui.add(egui::Button::new("[ wipe cookies + dns cache now ]").frame(false)).clicked() {
                let n = self.net.jar.lock().unwrap().count();
                self.net.jar.lock().unwrap().clear();
                self.net.resolver.clear();
                self.notice(format!("wiped {n} cookies and the dns cache"));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Button::new("[x]").frame(false)).clicked() {
                    self.settings_open = false;
                }
                ui.label(RichText::new("protections: [locked on]").color(color::OK))
                    .on_hover_text("Lunar's protections are always on and can't be turned off.");
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let small = |t: &str| RichText::new(t).size(15.0).color(color::OK);
            for item in ["+OWN-ENGINE", "+OWN-SEARCH", "+NO-JAVASCRIPT", "+HTTPS-ONLY", "+DNS:ENCRYPTED", "+NO-TRACKERS", "+NO-3RD-PARTIES", "+COOKIES:RAM-ONLY"] {
                ui.label(small(item));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let text = if let Some((n, at)) = &self.notice {
                    if at.elapsed() < Duration::from_secs(4) { Some(n.clone()) } else { None }
                } else {
                    None
                };
                let text = text
                    .or_else(|| self.hovered_link.clone())
                    .or_else(|| self.tabs[self.active].loading.map(|_| format!("loading {}", ["|", "/", "-", "\\"][self.spinner % 4])))
                    .or_else(|| self.search.crawling().then(|| format!("lunar search: indexing, {} pages", self.search.pages())))
                    .unwrap_or_else(|| "ready".into());
                ui.label(RichText::new(text).size(15.0).color(color::TEXT));
            });
        });
    }

    fn page_view(&mut self, ui: &mut egui::Ui) {
        let size = self.settings.text_size;
        let font = FontId::monospace(size);
        let char_w = ui.fonts_mut(|f| f.glyph_width(&font, 'M'));
        let line_h = (size * 1.12).round();
        let margin = 24.0;
        let cols = (((ui.available_width() - margin * 2.0 - 16.0) / char_w).floor() as usize).clamp(20, MAX_COLS);

        let i = self.active;
        let tab = &mut self.tabs[i];
        let tab_id = tab.id;
        let scroll_to_top = std::mem::take(&mut tab.scroll_to_top);
        let Some(page) = tab.page.as_mut() else {
            ui.centered_and_justified(|ui| ui.label(RichText::new("loading...").color(color::DIM)));
            return;
        };
        let key = (cols, page.focus, page.edits);
        if page.laid_out != Some(key) {
            page.lines = layout::layout(&page.doc, cols, page.focus);
            page.laid_out = Some(key);
        }

        let mut clicked: Option<(Action, bool)> = None;
        let mut hovered: Option<String> = None;
        let mut clicked_nothing = false;
        let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt(tab_id);
        if scroll_to_top {
            area = area.vertical_scroll_offset(0.0);
        }
        let lines = &page.lines;
        let focus = page.focus;
        area.show_rows(ui, line_h, lines.len() + 2, |ui, rows| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for row in rows {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), line_h), Sense::click());
                let Some(line) = lines.get(row) else { continue };
                let painter = ui.painter_at(rect);
                let origin = Pos2::new(rect.left() + margin, rect.top());
                let pointer_col = resp.hover_pos().map(|p| ((p.x - origin.x) / char_w).floor()).filter(|c| *c >= 0.0).map(|c| c as usize);
                let hover_action = pointer_col.and_then(|c| line.run_at(c)).and_then(|r| r.action.clone());
                let mut col = 0usize;
                for run in &line.runs {
                    let w = run.text.chars().count();
                    let x = origin.x + col as f32 * char_w;
                    let run_rect = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w as f32 * char_w, line_h));
                    let is_hover = run.action.is_some() && run.action == hover_action;
                    let focused_field = matches!(&run.action, Some(Action::Field(f)) if Some(*f) == focus);
                    let (fg, bg) = match run.style {
                        Style::Text => (color::TEXT, None),
                        Style::Bold => (color::BRIGHT, None),
                        Style::Italic => (Color32::from_rgb(0xb5, 0xc3, 0xdd), None),
                        Style::Code => (color::MOON, None),
                        Style::Heading => (color::MOON, None),
                        Style::Link => (color::ACCENT, None),
                        Style::Dim => (color::DIM, None),
                        Style::Field => (color::TEXT, Some(if focused_field { color::LINE } else { color::FIELD })),
                        Style::Button => (color::OK, Some(color::FIELD)),
                    };
                    if let Some(bg) = bg {
                        painter.rect_filled(run_rect, 0.0, bg);
                    }
                    if is_hover && run.style != Style::Field {
                        painter.rect_filled(run_rect, 0.0, Color32::from_rgba_unmultiplied(0x8f, 0xd8, 0xff, 0x18));
                    }
                    let text_y = rect.top() + (line_h - size) / 2.0;
                    painter.text(Pos2::new(x, text_y), egui::Align2::LEFT_TOP, &run.text, font.clone(), fg);
                    if run.style == Style::Link && is_hover {
                        let y = rect.bottom() - 2.0;
                        painter.line_segment([Pos2::new(x, y), Pos2::new(x + w as f32 * char_w, y)], Stroke::new(1.0, fg));
                    }
                    col += w;
                }
                if let Some(action) = &hover_action {
                    ui.ctx().set_cursor_icon(match action {
                        Action::Field(_) => CursorIcon::Text,
                        Action::Link(_) => CursorIcon::PointingHand,
                    });
                    if let Action::Link(href) = action {
                        hovered = Some(href.clone());
                    }
                }
                if resp.clicked() || resp.middle_clicked() {
                    match hover_action {
                        Some(a) => {
                            let new_tab = resp.middle_clicked() || ui.input(|i| i.modifiers.command);
                            clicked = Some((a, new_tab));
                        }
                        None => clicked_nothing = true,
                    }
                }
            }
        });
        // Resolve hovered link for the status bar.
        self.hovered_link = hovered.map(|h| match &page.base {
            Some(b) => b.join(&h).map(|u| u.to_string()).unwrap_or(h),
            None => h,
        });
        if clicked_nothing && page.focus.is_some() {
            page.focus = None;
            page.edits += 1;
        }
        if let Some((action, new_tab)) = clicked {
            if !matches!(action, Action::Field(_)) {
                if let Some(page) = self.tabs[i].page.as_mut() {
                    page.focus = None;
                }
            }
            self.activate(action, new_tab);
        }
    }
}

impl eframe::App for LunarApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.search.save();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        self.receive();
        self.refresh_when_index_loads(ctx);
        self.shortcuts(ctx);
        self.type_into_field(ctx);
        let loading = self.tabs.iter().any(|t| t.loading.is_some());
        if loading {
            self.spinner = self.spinner.wrapping_add(1);
            ctx.request_repaint_after(Duration::from_millis(120));
        } else if self.notice.as_ref().map_or(false, |(_, at)| at.elapsed() < Duration::from_secs(4)) {
            ctx.request_repaint_after(Duration::from_millis(500));
        } else if self.search.crawling() {
            ctx.request_repaint_after(Duration::from_secs(2));
        }
        if let Some((path, frames)) = self.screenshot.as_mut() {
            *frames += 1;
            ctx.request_repaint();
            if *frames == 20 && !loading {
                let path = path.clone();
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |image| {
                    let _ = std::fs::write(&path, crate::devshot::png(&image));
                    std::process::exit(0);
                })));
            } else if loading && *frames == 20 {
                *frames = 10;
            }
        }
        let title = format!("{} - Lunar Browser", self.tabs[self.active].title());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        let bar = |fill: Color32| egui::Frame::new().fill(fill).inner_margin(egui::Margin::symmetric(8, 4));
        egui::Panel::top("tabs").frame(bar(color::PANEL)).show(ui, |ui| self.tab_strip(ui, ctx));
        egui::Panel::top("toolbar").frame(bar(color::BG)).show(ui, |ui| self.toolbar(ui));
        if self.settings_open {
            egui::Panel::top("settings").frame(bar(color::PANEL)).show(ui, |ui| self.settings_bar(ui));
        }
        egui::Panel::bottom("status").frame(bar(color::PANEL)).show(ui, |ui| self.status_bar(ui));
        egui::CentralPanel::default().frame(egui::Frame::new().fill(color::BG)).show(ui, |ui| self.page_view(ui));
    }
}
