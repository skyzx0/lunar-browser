//! The few things you can change, saved as a small text file. Nothing about
//! your browsing is ever written there.

use std::path::PathBuf;

pub struct Engine {
    pub id: &'static str,
    pub name: &'static str,
    /// Search page and the name of its query parameter.
    pub url: &'static str,
    pub param: &'static str,
}

/// Search engines whose result pages work without JavaScript. Brave Search
/// runs its own index, independent of Google and Bing.
pub const ENGINES: &[Engine] = &[
    Engine { id: "brave", name: "Brave Search", url: "https://search.brave.com/search", param: "q" },
    Engine { id: "ecosia", name: "Ecosia", url: "https://www.ecosia.org/search", param: "q" },
    Engine { id: "wikipedia", name: "Wikipedia", url: "https://en.wikipedia.org/w/index.php", param: "search" },
];

pub const TEXT_SIZES: &[f32] = &[16.0, 20.0, 24.0, 28.0];

pub struct Settings {
    pub search: &'static Engine,
    pub text_size: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { search: &ENGINES[0], text_size: 20.0 }
    }
}

fn path() -> Option<PathBuf> {
    let home = |var: &str| std::env::var_os(var).map(PathBuf::from);
    let dir = if cfg!(target_os = "macos") {
        home("HOME")?.join("Library/Application Support/Lunar Browser")
    } else if cfg!(windows) {
        home("APPDATA")?.join("Lunar Browser")
    } else {
        home("XDG_CONFIG_HOME").or_else(|| home("HOME").map(|h| h.join(".config")))?.join("lunar-browser")
    };
    Some(dir.join("settings.txt"))
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Settings::default();
        let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else { return s };
        for line in text.lines() {
            match line.split_once('=') {
                Some(("search", v)) => {
                    if let Some(e) = ENGINES.iter().find(|e| e.id == v.trim()) {
                        s.search = e;
                    }
                }
                Some(("text_size", v)) => {
                    if let Ok(n) = v.trim().parse::<f32>() {
                        if TEXT_SIZES.contains(&n) {
                            s.text_size = n;
                        }
                    }
                }
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, format!("search={}\ntext_size={}\n", self.search.id, self.text_size));
    }

    pub fn search_url(&self, query: &str) -> String {
        let q: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
        format!("{}?{}={q}", self.search.url, self.search.param)
    }
}
