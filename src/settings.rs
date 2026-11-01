use std::path::PathBuf;

pub const TEXT_SIZES: &[f32] = &[16.0, 20.0, 24.0, 28.0];

pub struct Settings {
    pub text_size: f32,
    pub crawl: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { text_size: 20.0, crawl: true }
    }
}

pub fn data_dir() -> Option<PathBuf> {
    let home = |var: &str| std::env::var_os(var).map(PathBuf::from);
    if cfg!(target_os = "macos") {
        Some(home("HOME")?.join("Library/Application Support/Lunar Browser"))
    } else if cfg!(windows) {
        Some(home("APPDATA")?.join("Lunar Browser"))
    } else {
        Some(home("XDG_DATA_HOME").or_else(|| home("HOME").map(|h| h.join(".local/share")))?.join("lunar-browser"))
    }
}

pub fn index_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("search-index.gz"))
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Settings::default();
        let Some(text) = data_dir().and_then(|d| std::fs::read_to_string(d.join("settings.txt")).ok()) else { return s };
        for line in text.lines() {
            match line.split_once('=') {
                Some(("text_size", v)) => {
                    if let Ok(n) = v.trim().parse::<f32>() {
                        if TEXT_SIZES.contains(&n) {
                            s.text_size = n;
                        }
                    }
                }
                Some(("crawl", v)) => s.crawl = v.trim() != "off",
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let Some(dir) = data_dir() else { return };
        let _ = std::fs::create_dir_all(&dir);
        let crawl = if self.crawl { "on" } else { "off" };
        let _ = std::fs::write(dir.join("settings.txt"), format!("text_size={}\ncrawl={crawl}\n", self.text_size));
    }
}

/// The address of a Lunar Search results page.
pub fn search_url(query: &str) -> String {
    let q: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    format!("lunar:search?q={q}")
}
