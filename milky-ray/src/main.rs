#![cfg_attr(windows, windows_subsystem = "windows")]

mod pages;
mod store;

use std::borrow::Cow;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lunar_browser::search::Engine;
use muda::{accelerator::Accelerator, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use serde_json::{json, Value};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::http::{header::CONTENT_TYPE, Request, Response};
use wry::{NewWindowResponse, PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder};

use store::Store;

enum UserEvent {
    Ipc(String),
    Title(u64, String),
    Loading(u64, bool, String),
    OpenTab(String),
    Navigate(u64, String),
    Menu(String),
    Downloaded(String),
}

struct Tab {
    id: u64,
    view: Option<WebView>,
    seen: Instant,
    title: String,
    url: String,
    loading: bool,
    zoom: f64,
}

struct App {
    window: Window,
    #[cfg(target_os = "linux")]
    fixed: gtk::Fixed,
    context: WebContext,
    ui: WebView,
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    content: (f64, f64, f64, f64),
    overlay: bool,
    backdrop: String,
    store: Store,
    search: Arc<Engine>,
    proxy: EventLoopProxy<UserEvent>,
}

fn internal(path: &str) -> String {
    if cfg!(windows) { format!("http://milky.localhost/{path}") } else { format!("milky://localhost/{path}") }
}

fn is_internal(url: &str) -> bool {
    url.starts_with("milky://") || url.starts_with("http://milky.localhost")
}

/// What the address bar shows for an address.
fn friendly(url: &str) -> String {
    if !is_internal(url) {
        return url.to_string();
    }
    let path = url.split_once("localhost/").map_or("", |(_, p)| p);
    if path.is_empty() || path.starts_with("newtab") { String::new() } else { format!("milky:{path}") }
}

fn respond(body: Vec<u8>, kind: &str) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .header(CONTENT_TYPE, kind)
        .header("Content-Security-Policy", "default-src 'self' 'unsafe-inline' data: milky: http://milky.localhost; img-src * data: blob:")
        .body(Cow::Owned(body))
        .unwrap()
}

fn not_found() -> Response<Cow<'static, [u8]>> {
    Response::builder().status(404).body(Cow::Borrowed(&b"not found"[..])).unwrap()
}

fn path_of(request: &Request<Vec<u8>>) -> (String, String) {
    let uri = request.uri();
    (uri.path().trim_start_matches('/').to_string(), uri.query().unwrap_or("").to_string())
}

const UI_HTML: &str = include_str!("../ui/index.html");
const UI_CSS: &str = include_str!("../ui/app.css");
const UI_JS: &str = include_str!("../ui/app.js");
const VT323: &[u8] = include_bytes!("../../assets/VT323-Regular.ttf");

fn ui_file(name: &str) -> Option<Vec<u8>> {
    if let Some(dir) = store::ui_dir() {
        if let Ok(bytes) = std::fs::read(dir.join(name)) {
            return Some(bytes);
        }
    }
    match name {
        "index.html" => Some(UI_HTML.as_bytes().to_vec()),
        "app.css" => Some(UI_CSS.as_bytes().to_vec()),
        "app.js" => Some(UI_JS.as_bytes().to_vec()),
        "vt323.ttf" => Some(VT323.to_vec()),
        _ => None,
    }
}

fn page_style(store: &Store) -> (Value, f64, Value) {
    (
        json!(store.get_str("siteCss")),
        store.get_f64(&["theme", "pageOpacity"], 100.0),
        json!(store.get_str_at(&["theme", "pageBackground"])),
    )
}

fn page_script(store: &Store) -> String {
    let (css, opacity, mode) = page_style(store);
    let js = store.get_str("siteJs");
    format!(
        r#"(function () {{
  function apply(css, opacity, mode) {{
    var extra = '';
    if (mode === 'transparent') extra += 'html, body {{ background: transparent !important; }}';
    if (mode === 'all') extra += 'html, body, body * {{ background-color: transparent !important; }} body *:not(img):not(video):not(picture):not(canvas):not(svg) {{ background-image: none !important; }}';
    if (opacity < 100) extra += 'html {{ opacity: ' + (opacity / 100) + ' !important; }}';
    var s = document.getElementById('__milky_style');
    if (!s) {{ s = document.createElement('style'); s.id = '__milky_style'; }}
    if (!s.isConnected) (document.head || document.documentElement).appendChild(s);
    s.textContent = css + ' ' + extra;
  }}
  window.__milkyApply = apply;
  var args = [{css}, {opacity}, {mode}];
  if (document.documentElement) apply.apply(null, args);
  document.addEventListener('DOMContentLoaded', function () {{ apply.apply(null, args); }});
}})();
try {{ {js} }} catch (e) {{ console.error('Milky Ray site script:', e); }}"#
    )
}

fn apply_backdrop(window: &Window, mode: &str, light: bool) {
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, clear_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        let _ = clear_vibrancy(window);
        let material = match mode {
            "frosted" if light => Some(NSVisualEffectMaterial::Sidebar),
            "frosted" => Some(NSVisualEffectMaterial::UnderWindowBackground),
            "smoked" => Some(NSVisualEffectMaterial::HudWindow),
            _ => None,
        };
        if let Some(material) = material {
            let _ = apply_vibrancy(window, material, Some(NSVisualEffectState::Active), None);
        }
    }
    #[cfg(windows)]
    {
        use window_vibrancy::{apply_acrylic, apply_mica, clear_acrylic, clear_mica};
        let _ = clear_mica(window);
        let _ = clear_acrylic(window);
        match mode {
            "frosted" => {
                if apply_mica(window, Some(!light)).is_err() {
                    let _ = apply_acrylic(window, None);
                }
            }
            "smoked" => {
                let _ = apply_acrylic(window, Some((10, 10, 12, 140)));
            }
            _ => {}
        }
    }
    #[cfg(target_os = "linux")]
    let _ = (window, mode, light);
}

fn unique_path(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    (1..1000)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .unwrap_or(candidate)
}

fn reveal(path: &std::path::Path) {
    let program = if cfg!(target_os = "macos") { "open" } else if cfg!(windows) { "explorer" } else { "xdg-open" };
    let _ = std::process::Command::new(program).arg(path).spawn();
}

#[cfg(not(target_os = "linux"))]
fn build_child(builder: WebViewBuilder, window: &Window) -> wry::Result<WebView> {
    builder.build_as_child(window)
}

#[cfg(target_os = "linux")]
fn build_child(builder: WebViewBuilder, fixed: &gtk::Fixed) -> wry::Result<WebView> {
    use wry::WebViewBuilderExtUnix;
    builder.build_gtk(fixed)
}

impl App {

    fn apply_backdrop(&mut self) {
        let mode = self.store.get_str_at(&["theme", "backdrop"]);
        let light = self.store.get_bool_at(&["theme", "light"]);
        let key = format!("{mode}/{light}");
        if key != self.backdrop {
            apply_backdrop(&self.window, &mode, light);
            self.backdrop = key;
        }
    }

    fn window_size(&self) -> (f64, f64) {
        let size = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        (size.width, size.height)
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { position: LogicalPosition::new(x, y).into(), size: LogicalSize::new(w.max(1.0), h.max(1.0)).into() }
    }

    fn layout(&self) {
        let (w, h) = self.window_size();
        let _ = self.ui.set_bounds(Self::rect(0.0, 0.0, w, h));
        let (x, y, cw, ch) = self.content;
        for (i, tab) in self.tabs.iter().enumerate() {
            if let Some(view) = &tab.view {
                let _ = view.set_bounds(Self::rect(x, y, cw, ch));
                let _ = view.set_visible(i == self.active && !self.overlay);
            }
        }
    }

    /// Where the address bar's text goes.
    fn target_for(&self, text: &str) -> Option<String> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let lower = text.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("milky:") {
            return Some(internal(rest));
        }
        if is_internal(text) || lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("about:") {
            return Some(text.to_string());
        }
        if lower.starts_with("localhost") || lower.starts_with("127.0.0.1") {
            return Some(format!("http://{text}"));
        }
        let host = text.split(['/', '?', '#']).next().unwrap_or("");
        if !text.contains(char::is_whitespace) && host.contains('.') && !host.starts_with('.') && !host.ends_with('.') {
            return Some(format!("https://{text}"));
        }
        Some(self.store.search_url(text, &internal("search")))
    }

    fn new_tab(&mut self, url: Option<String>) {
        let id = self.next_id;
        self.next_id += 1;
        let url = url.unwrap_or_else(|| self.store.home_url(&internal("newtab")));
        let zoom = (self.store.get_f64(&["defaultZoom"], 100.0) / 100.0).clamp(0.3, 3.0);
        match self.build_view(id, &url, zoom) {
            Ok(view) => {
                self.tabs.push(Tab { id, view: Some(view), seen: Instant::now(), title: "New tab".into(), url, loading: true, zoom });
                self.active = self.tabs.len() - 1;
                self.save_memory();
                self.layout();
                self.send_state();
                if self.tabs[self.active].url.contains("newtab") {
                    self.focus_address();
                } else {
                    self.focus_active();
                }
            }
            Err(e) => self.toast(&format!("couldn't open a tab: {e}")),
        }
    }

    fn focus_active(&self) {
        if let Some(v) = self.active_view() {
            let _ = v.focus();
        }
    }

    fn wake(&mut self, index: usize) {
        let Some(tab) = self.tabs.get(index) else { return };
        if tab.view.is_some() {
            return;
        }
        let (id, url, zoom) = (tab.id, tab.url.clone(), tab.zoom);
        match self.build_view(id, &url, zoom) {
            Ok(view) => {
                let tab = &mut self.tabs[index];
                tab.view = Some(view);
                tab.loading = true;
            }
            Err(e) => self.toast(&format!("couldn't wake the tab: {e}")),
        }
    }

    fn save_memory(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.seen = Instant::now();
        }
        if !self.store.get_bool("memorySaver", true) {
            return;
        }
        let keep = self.store.get_f64(&["liveTabs"], 4.0).max(1.0) as usize;
        let idle = Duration::from_secs_f64(self.store.get_f64(&["sleepMinutes"], 10.0).max(1.0) * 60.0);
        let mut live: Vec<usize> = (0..self.tabs.len()).filter(|&i| i != self.active && self.tabs[i].view.is_some()).collect();
        live.sort_by_key(|&i| std::cmp::Reverse(self.tabs[i].seen));
        let mut changed = false;
        for (rank, i) in live.into_iter().enumerate() {
            let tab = &mut self.tabs[i];
            if rank + 1 >= keep || tab.seen.elapsed() > idle {
                tab.view = None;
                tab.loading = false;
                changed = true;
            }
        }
        if changed {
            self.send_state();
        }
    }

    fn build_view(&mut self, id: u64, url: &str, zoom: f64) -> wry::Result<WebView> {
        let private = self.store.get_bool("private", false);
        let user_agent = self.store.get_str("userAgent");
        let script = page_script(&self.store);
        let https_only = self.store.get_bool("httpsOnly", true);
        let search = self.search.clone();
        let store = self.store.clone_values();

        let (p1, p2, p3, p4, p5) = (self.proxy.clone(), self.proxy.clone(), self.proxy.clone(), self.proxy.clone(), self.proxy.clone());
        let mut builder = WebViewBuilder::new_with_web_context(&mut self.context)
            .with_bounds(Self::rect(self.content.0, self.content.1, self.content.2, self.content.3))
            .with_url(url)
            .with_transparent(true)
            .with_initialization_script(script)
            .with_incognito(private)
            .with_devtools(true)
            .with_hotkeys_zoom(true)
            .with_back_forward_navigation_gestures(true)
            .with_clipboard(true)
            .with_custom_protocol("milky".into(), move |_id, request| {
                let (path, query) = path_of(&request);
                match path.as_str() {
                    "" | "newtab" => respond(pages::new_tab(&store, search.pages()).into_bytes(), "text/html; charset=utf-8"),
                    "search" => {
                        let q = url::form_urlencoded::parse(query.as_bytes())
                            .find(|(k, _)| k == "q")
                            .map(|(_, v)| v.into_owned())
                            .unwrap_or_default();
                        respond(pages::search(&store, &q, &search.search(&q), &internal("search")).into_bytes(), "text/html; charset=utf-8")
                    }
                    "vt323.ttf" => respond(VT323.to_vec(), "font/ttf"),
                    _ => not_found(),
                }
            })
            .with_document_title_changed_handler(move |title| {
                let _ = p1.send_event(UserEvent::Title(id, title));
            })
            .with_on_page_load_handler(move |event, url| {
                let _ = p2.send_event(UserEvent::Loading(id, matches!(event, PageLoadEvent::Started), url));
            })
            .with_navigation_handler(move |url| {
                if https_only && url.starts_with("http://") && !is_internal(&url) {
                    let host = url.trim_start_matches("http://").split(['/', ':', '?']).next().unwrap_or("");
                    if host != "localhost" && host != "127.0.0.1" && !host.ends_with(".localhost") {
                        let _ = p3.send_event(UserEvent::Navigate(id, url.replacen("http://", "https://", 1)));
                        return false;
                    }
                }
                true
            })
            .with_new_window_req_handler(move |url, _features| {
                let _ = p4.send_event(UserEvent::OpenTab(url));
                NewWindowResponse::Deny
            })
            .with_download_started_handler(move |url, path| {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| url.rsplit('/').next().unwrap_or("download").split('?').next().unwrap_or("download").to_string());
                if let Some(dir) = store::downloads_dir() {
                    *path = unique_path(&dir, &name);
                }
                let _ = p5.send_event(UserEvent::Downloaded(name));
                true
            });
        if !user_agent.is_empty() {
            builder = builder.with_user_agent(user_agent);
        }
        #[cfg(not(target_os = "linux"))]
        let view = build_child(builder, &self.window)?;
        #[cfg(target_os = "linux")]
        let view = build_child(builder, &self.fixed)?;
        if (zoom - 1.0).abs() > 0.01 {
            let _ = view.zoom(zoom);
        }
        Ok(view)
    }

    fn close_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.new_tab(None);
            return;
        }
        if self.active >= self.tabs.len() || self.active > index {
            self.active = self.active.saturating_sub(1).min(self.tabs.len() - 1);
        }
        self.layout();
        self.send_state();
    }

    fn select(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = index;
            self.wake(index);
            self.save_memory();
            self.layout();
            self.send_state();
            self.focus_active();
        }
    }

    fn tab_mut(&mut self, id: u64) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    fn go(&mut self, text: &str) {
        let Some(url) = self.target_for(text) else { return };
        if let Some(v) = self.active_view() {
            let _ = v.load_url(&url);
            let _ = v.focus();
        }
    }

    fn active_view(&self) -> Option<&WebView> {
        self.tabs.get(self.active).and_then(|t| t.view.as_ref())
    }

    fn zoom(&mut self, change: Option<f64>) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.zoom = match change {
                Some(step) => (tab.zoom + step).clamp(0.3, 3.0),
                None => 1.0,
            };
            if let Some(v) = &tab.view {
                let _ = v.zoom(tab.zoom);
            }
        }
    }

    fn focus_address(&self) {
        let _ = self.ui.focus();
        let _ = self.ui.evaluate_script("window.milky && milky.focusAddress()");
    }

    fn toast(&self, text: &str) {
        let _ = self.ui.evaluate_script(&format!("window.milky && milky.toast({})", json!(text)));
    }

    fn send_state(&self) {
        let tabs: Vec<Value> = self
            .tabs
            .iter()
            .map(|t| json!({ "id": t.id, "title": t.title, "url": friendly(&t.url), "loading": t.loading, "secure": t.url.starts_with("https://") || is_internal(&t.url), "asleep": t.view.is_none() }))
            .collect();
        let state = json!({
            "tabs": tabs,
            "active": self.tabs.get(self.active).map(|t| t.id),
            "searchPages": self.search.pages(),
            "crawling": self.search.crawling(),
        });
        let _ = self.ui.evaluate_script(&format!("window.milky && milky.update({state})"));
    }

    fn handle_ipc(&mut self, message: &str) {
        let Ok(msg) = serde_json::from_str::<Value>(message) else { return };
        let num = |k: &str| msg.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let id = msg.get("id").and_then(Value::as_u64);
        let index = id.and_then(|id| self.tabs.iter().position(|t| t.id == id));
        match msg.get("cmd").and_then(Value::as_str).unwrap_or("") {
            "ready" => self.send_state(),
            "drag" => {
                let _ = self.window.drag_window();
            }
            "maximize" => self.window.set_maximized(!self.window.is_maximized()),
            "layout" => {
                self.content = (num("x"), num("y"), num("w"), num("h"));
                self.layout();
            }
            "overlay" => {
                self.overlay = msg.get("open").and_then(Value::as_bool).unwrap_or(false);
                self.layout();
                if !self.overlay {
                    if let Some(v) = self.active_view() {
                        let _ = v.focus();
                    }
                }
            }
            "newTab" => self.new_tab(msg.get("url").and_then(Value::as_str).map(String::from)),
            "closeTab" => {
                if let Some(i) = index {
                    self.close_tab(i)
                }
            }
            "selectTab" => {
                if let Some(i) = index {
                    self.select(i)
                }
            }
            "moveTab" => {
                if let (Some(from), Some(to)) = (index, msg.get("to").and_then(Value::as_u64)) {
                    let to = (to as usize).min(self.tabs.len() - 1);
                    let active_id = self.tabs[self.active].id;
                    let tab = self.tabs.remove(from);
                    self.tabs.insert(to, tab);
                    self.active = self.tabs.iter().position(|t| t.id == active_id).unwrap_or(0);
                    self.send_state();
                }
            }
            "navigate" => {
                if let Some(text) = msg.get("text").and_then(Value::as_str) {
                    let text = text.to_string();
                    self.go(&text);
                }
            }
            "back" => {
                if let Some(v) = self.active_view() {
                    let _ = v.evaluate_script("history.back()");
                }
            }
            "forward" => {
                if let Some(v) = self.active_view() {
                    let _ = v.evaluate_script("history.forward()");
                }
            }
            "reload" => {
                if let Some(v) = self.active_view() {
                    let _ = v.reload();
                }
            }
            "stop" => {
                if let Some(v) = self.active_view() {
                    let _ = v.evaluate_script("window.stop()");
                }
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.loading = false;
                }
                self.send_state();
            }
            "home" => {
                let home = self.store.home_url(&internal("newtab"));
                if let Some(v) = self.active_view() {
                    let _ = v.load_url(&home);
                }
            }
            "zoomIn" => self.zoom(Some(0.1)),
            "zoomOut" => self.zoom(Some(-0.1)),
            "zoomReset" => self.zoom(None),
            "devtools" => {
                if let Some(v) = self.active_view() {
                    v.open_devtools();
                }
            }
            "print" => {
                if let Some(v) = self.active_view() {
                    let _ = v.print();
                }
            }
            "clearData" => {
                if let Some(v) = self.active_view() {
                    let _ = v.clear_all_browsing_data();
                }
                self.toast("cookies, cache and site data cleared");
            }
            "saveSettings" => {
                if let Some(settings) = msg.get("settings") {
                    self.store.replace(settings.clone());
                    self.search.set_crawl(self.store.get_bool("crawl", true));
                    let (css, opacity, mode) = page_style(&self.store);
                    let call = format!("window.__milkyApply && window.__milkyApply({css}, {opacity}, {mode})");
                    for view in self.tabs.iter().filter_map(|t| t.view.as_ref()) {
                        let _ = view.evaluate_script(&call);
                    }
                    self.apply_backdrop();
                    self.save_memory();
                }
            }
            "uiFiles" => match store::ui_dir() {
                Some(dir) => {
                    let _ = std::fs::create_dir_all(&dir);
                    for (name, body) in [("index.html", UI_HTML), ("app.css", UI_CSS), ("app.js", UI_JS)] {
                        if !dir.join(name).exists() {
                            let _ = std::fs::write(dir.join(name), body);
                        }
                    }
                    reveal(&dir);
                    self.toast("Edit the files in this folder, then restart Milky Ray");
                }
                None => self.toast("couldn't find a folder for the interface files"),
            },
            "uiReset" => {
                if let Some(dir) = store::ui_dir().filter(|d| d.exists()) {
                    let backup = dir.with_file_name("ui-backup");
                    let _ = std::fs::remove_dir_all(&backup);
                    let _ = std::fs::rename(&dir, &backup);
                    self.toast("Built-in interface restored after restart; your files are in ui-backup");
                } else {
                    self.toast("Already using the built-in interface");
                }
            }
            "indexSite" => {
                let url = self.tabs.get(self.active).map(|t| t.url.clone()).unwrap_or_default();
                match url::Url::parse(&url) {
                    Ok(u) if !is_internal(&url) && self.search.add_site(&u) => {
                        self.search.set_crawl(true);
                        self.toast(&format!("Lunar Search is indexing {}", u.host_str().unwrap_or("")));
                    }
                    _ => self.toast("open a website first, then index it"),
                }
            }
            _ => {}
        }
    }

    fn handle(&mut self, event: UserEvent) {
        match event {
            UserEvent::Ipc(message) => self.handle_ipc(&message),
            UserEvent::Title(id, title) => {
                if let Some(tab) = self.tab_mut(id) {
                    tab.title = title;
                }
                self.send_state();
            }
            UserEvent::Loading(id, loading, url) => {
                if let Some(tab) = self.tab_mut(id) {
                    tab.loading = loading;
                    if !url.is_empty() {
                        tab.url = url;
                    }
                }
                self.send_state();
            }
            UserEvent::OpenTab(url) => self.new_tab(Some(url)),
            UserEvent::Navigate(id, url) => {
                if let Some(view) = self.tabs.iter().find(|t| t.id == id).and_then(|t| t.view.as_ref()) {
                    let _ = view.load_url(&url);
                }
            }
            UserEvent::Downloaded(name) => self.toast(&format!("downloading {name} to Downloads")),
            UserEvent::Menu(id) => match id.as_str() {
                "new-tab" => self.new_tab(None),
                "close-tab" => self.close_tab(self.active),
                "address" => self.focus_address(),
                "reload" => self.handle_ipc(r#"{"cmd":"reload"}"#),
                "back" => self.handle_ipc(r#"{"cmd":"back"}"#),
                "forward" => self.handle_ipc(r#"{"cmd":"forward"}"#),
                "zoom-in" => self.zoom(Some(0.1)),
                "zoom-out" => self.zoom(Some(-0.1)),
                "zoom-reset" => self.zoom(None),
                "devtools" => self.handle_ipc(r#"{"cmd":"devtools"}"#),
                "next-tab" if !self.tabs.is_empty() => self.select((self.active + 1) % self.tabs.len()),
                "prev-tab" if !self.tabs.is_empty() => self.select((self.active + self.tabs.len() - 1) % self.tabs.len()),
                "settings" => {
                    let _ = self.ui.evaluate_script("window.milky && milky.togglePanel('appearance')");
                }
                "sidebar" => {
                    let _ = self.ui.evaluate_script("window.milky && milky.toggleSidebar()");
                }
                "print" => self.handle_ipc(r#"{"cmd":"print"}"#),
                "selftest-open" => {
                    self.new_tab(Some("https://example.com/".into()));
                    self.new_tab(Some(internal("search?q=moon")));
                }
                "selftest-style" => {
                    let mut settings: Value = serde_json::from_str(&self.store.json()).unwrap_or(json!({}));
                    settings["siteCss"] = json!("body { outline: 3px solid rgb(255, 0, 0) }");
                    settings["theme"]["pageBackground"] = json!("all");
                    settings["theme"]["backdrop"] = json!("frosted");
                    self.handle_ipc(&json!({ "cmd": "saveSettings", "settings": settings }).to_string());
                    self.handle_ipc(r#"{"cmd":"stop"}"#);
                }
                "selftest-report" => {
                    for t in &self.tabs {
                        eprintln!("tab {} | loading={} | asleep={} | {} | {}", t.id, t.loading, t.view.is_none(), friendly(&t.url), t.title);
                    }
                    eprintln!("lunar search pages: {}", self.search.pages());
                    if let Some(view) = self.tabs.iter().find(|t| t.url.contains("example.com")).and_then(|t| t.view.as_ref()) {
                        let _ = view.evaluate_script_with_callback(
                            "JSON.stringify({ style: (document.getElementById('__milky_style') || {}).textContent, outline: getComputedStyle(document.body).outlineColor, bodyBackground: getComputedStyle(document.body).backgroundColor })",
                            |result| {
                                eprintln!("page check: {result}");
                                std::process::exit(0);
                            },
                        );
                    } else {
                        std::process::exit(0);
                    }
                }
                _ => {}
            },
        }
    }
}

fn build_menu() -> Menu {
    let item = |id: &str, label: &str, key: Option<&str>| {
        MenuItem::with_id(id, label, true, key.and_then(|k| k.parse::<Accelerator>().ok()))
    };
    let menu = Menu::new();
    #[cfg(target_os = "macos")]
    {
        let app = Submenu::new("Milky Ray", true);
        let _ = app.append_items(&[
            &PredefinedMenuItem::about(None, None),
            &PredefinedMenuItem::separator(),
            &item("settings", "Settings…", Some("CmdOrCtrl+Comma")),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ]);
        let _ = menu.append(&app);
    }
    let file = Submenu::new("File", true);
    let _ = file.append_items(&[
        &item("new-tab", "New Tab", Some("CmdOrCtrl+T")),
        &item("close-tab", "Close Tab", Some("CmdOrCtrl+W")),
        &item("address", "Open Location", Some("CmdOrCtrl+L")),
        &PredefinedMenuItem::separator(),
        &item("print", "Print…", Some("CmdOrCtrl+P")),
    ]);
    #[cfg(not(target_os = "macos"))]
    let _ = file.append_items(&[&item("settings", "Settings", Some("CmdOrCtrl+Comma")), &PredefinedMenuItem::quit(None)]);
    let edit = Submenu::new("Edit", true);
    let _ = edit.append_items(&[
        &PredefinedMenuItem::undo(None),
        &PredefinedMenuItem::redo(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::cut(None),
        &PredefinedMenuItem::copy(None),
        &PredefinedMenuItem::paste(None),
        &PredefinedMenuItem::select_all(None),
    ]);
    let view = Submenu::new("View", true);
    let _ = view.append_items(&[
        &item("reload", "Reload", Some("CmdOrCtrl+R")),
        &item("zoom-in", "Zoom In", Some("CmdOrCtrl+Equal")),
        &item("zoom-out", "Zoom Out", Some("CmdOrCtrl+Minus")),
        &item("zoom-reset", "Actual Size", Some("CmdOrCtrl+Digit0")),
        &PredefinedMenuItem::separator(),
        &item("sidebar", "Toggle Sidebar", Some("CmdOrCtrl+Shift+S")),
        &item("devtools", "Developer Tools", Some("CmdOrCtrl+Alt+I")),
        &PredefinedMenuItem::fullscreen(None),
    ]);
    let history = Submenu::new("History", true);
    let _ = history.append_items(&[
        &item("back", "Back", Some("CmdOrCtrl+BracketLeft")),
        &item("forward", "Forward", Some("CmdOrCtrl+BracketRight")),
    ]);
    let tabs = Submenu::new("Tabs", true);
    let _ = tabs.append_items(&[
        &item("next-tab", "Next Tab", Some("Ctrl+Tab")),
        &item("prev-tab", "Previous Tab", Some("Ctrl+Shift+Tab")),
    ]);
    let _ = menu.append_items(&[&file, &edit, &view, &history, &tabs]);
    #[cfg(target_os = "macos")]
    {
        let window = Submenu::new("Window", true);
        let _ = window.append_items(&[&PredefinedMenuItem::minimize(None), &PredefinedMenuItem::maximize(None)]);
        let _ = menu.append(&window);
    }
    menu
}

fn main() -> wry::Result<()> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let mut builder = WindowBuilder::new()
        .with_title("Lunar Browser Milky Ray")
        .with_transparent(true)
        .with_inner_size(tao::dpi::LogicalSize::new(1320.0, 860.0))
        .with_min_inner_size(tao::dpi::LogicalSize::new(640.0, 420.0));
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::WindowBuilderExtMacOS;
        builder = builder.with_titlebar_transparent(true).with_fullsize_content_view(true).with_title_hidden(true);
    }
    let window = builder.build(&event_loop).expect("window");

    let menu = build_menu();
    #[cfg(target_os = "macos")]
    menu.init_for_nsapp();
    #[cfg(target_os = "windows")]
    {
        use tao::platform::windows::WindowExtWindows;
        unsafe {
            let _ = menu.init_for_hwnd(window.hwnd() as _);
            let _ = menu.hide_for_hwnd(window.hwnd() as _);
        }
    }
    #[cfg(target_os = "linux")]
    let fixed = {
        use gtk::prelude::*;
        use tao::platform::unix::WindowExtUnix;
        let vbox = window.default_vbox().expect("gtk box");
        let _ = menu.init_for_gtk_window(window.gtk_window(), Some(vbox));
        let _ = menu.hide_for_gtk_window(window.gtk_window());
        let fixed = gtk::Fixed::new();
        vbox.pack_start(&fixed, true, true, 0);
        fixed.show_all();
        fixed
    };
    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = menu_proxy.send_event(UserEvent::Menu(event.id().0.clone()));
    }));

    let store = Store::load();
    let search = Engine::open(store::index_path(), store.get_bool("crawl", true));
    let mut context = WebContext::new(store::data_dir().map(|d| d.join("webdata")));

    let settings_json = store.json();
    let platform = std::env::consts::OS;
    let ipc_proxy = proxy.clone();
    let ui_builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(internal("ui/"))
        .with_initialization_script(format!("window.__MILKY__ = {{ settings: {settings_json}, platform: {platform:?} }};"))
        .with_transparent(true)
        .with_accept_first_mouse(true)
        .with_devtools(true)
        .with_custom_protocol("milky".into(), move |_id, request| {
            let (path, _) = path_of(&request);
            let name = path.trim_start_matches("ui").trim_start_matches('/');
            let name = if name.is_empty() { "index.html" } else { name };
            let kind = match name.rsplit('.').next() {
                Some("html") => "text/html; charset=utf-8",
                Some("css") => "text/css; charset=utf-8",
                Some("js") => "text/javascript; charset=utf-8",
                Some("ttf") => "font/ttf",
                Some("png") => "image/png",
                Some("svg") => "image/svg+xml",
                _ => "application/octet-stream",
            };
            if name.contains("..") || name.contains('/') || name.contains('\\') {
                return not_found();
            }
            match ui_file(name) {
                Some(body) => respond(body, kind),
                None => not_found(),
            }
        })
        .with_ipc_handler(move |request: Request<String>| {
            let _ = ipc_proxy.send_event(UserEvent::Ipc(request.body().clone()));
        });
    #[cfg(target_os = "linux")]
    let ui = build_child(ui_builder, &fixed)?;
    #[cfg(not(target_os = "linux"))]
    let ui = build_child(ui_builder, &window)?;

    let mut app = App {
        window,
        #[cfg(target_os = "linux")]
        fixed,
        context,
        ui,
        tabs: Vec::new(),
        active: 0,
        next_id: 1,
        content: (0.0, 96.0, 1320.0, 764.0),
        overlay: false,
        backdrop: String::new(),
        store,
        search,
        proxy,
    };
    app.layout();
    app.apply_backdrop();
    app.new_tab(None);
    let _menu = menu;

    if std::env::var("MILKY_SELFTEST").is_ok() {
        let p = app.proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let _ = p.send_event(UserEvent::Menu("selftest-open".into()));
            std::thread::sleep(std::time::Duration::from_secs(8));
            let _ = p.send_event(UserEvent::Menu("selftest-style".into()));
            let wait = std::env::var("MILKY_SELFTEST").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(4);
            std::thread::sleep(std::time::Duration::from_secs(wait));
            let _ = p.send_event(UserEvent::Menu("selftest-report".into()));
        });
    }
    let mut last_pages = 0;
    let mut last_check = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(std::time::Instant::now() + std::time::Duration::from_secs(2));
        match event {
            Event::UserEvent(e) => app.handle(e),
            Event::WindowEvent { event: WindowEvent::Resized(_), .. } => app.layout(),
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                app.search.save();
                *control_flow = ControlFlow::Exit;
            }
            Event::NewEvents(tao::event::StartCause::ResumeTimeReached { .. }) => {
                if last_check.elapsed() > Duration::from_secs(30) {
                    last_check = Instant::now();
                    app.save_memory();
                }
                let pages = app.search.pages();
                if pages != last_pages {
                    last_pages = pages;
                    app.send_state();
                }
            }
            _ => {}
        }
    });
}

