//! Milky Ray's settings: one JSON file, written by the appearance editor and
//! settings panels in the interface. Shared, so pages like the new tab page
//! always use the current look.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde_json::{json, Value};

pub type Shared = Arc<RwLock<Value>>;

pub struct Store {
    values: Shared,
}

pub fn data_dir() -> Option<PathBuf> {
    let home = |var: &str| std::env::var_os(var).map(PathBuf::from);
    if cfg!(target_os = "macos") {
        Some(home("HOME")?.join("Library/Application Support/Lunar Browser Milky Ray"))
    } else if cfg!(windows) {
        Some(home("APPDATA")?.join("Lunar Browser Milky Ray"))
    } else {
        Some(home("XDG_DATA_HOME").or_else(|| home("HOME").map(|h| h.join(".local/share")))?.join("milky-ray"))
    }
}

/// Where your own copy of the interface files lives, if you made one.
pub fn ui_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("ui"))
}

pub fn index_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("search-index.gz"))
}

pub fn downloads_dir() -> Option<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)?;
    let dir = home.join("Downloads");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn settings_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("settings.json"))
}

impl Store {
    pub fn load() -> Store {
        let values = settings_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        Store { values: Arc::new(RwLock::new(values)) }
    }

    pub fn json(&self) -> String {
        self.values.read().unwrap().to_string()
    }

    pub fn clone_values(&self) -> Shared {
        self.values.clone()
    }

    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        self.values.read().unwrap().get(key).and_then(Value::as_bool).unwrap_or(default)
    }

    pub fn get_f64(&self, path: &[&str], default: f64) -> f64 {
        let v = self.values.read().unwrap();
        path.iter().try_fold(&*v, |v, k| v.get(*k)).and_then(Value::as_f64).unwrap_or(default)
    }

    pub fn get_str(&self, key: &str) -> String {
        self.values.read().unwrap().get(key).and_then(Value::as_str).unwrap_or("").trim().to_string()
    }

    /// Replaces all settings (sent by the interface) and saves them.
    pub fn replace(&self, values: Value) {
        if !values.is_object() {
            return;
        }
        let text = serde_json::to_string_pretty(&values).unwrap_or_default();
        *self.values.write().unwrap() = values;
        if let Some(path) = settings_path() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, text);
        }
    }

    /// Lunar Search by default; a custom search address (with %s for the
    /// words) if one is set.
    pub fn search_url(&self, text: &str, lunar_search: &str) -> String {
        let q: String = url::form_urlencoded::byte_serialize(text.as_bytes()).collect();
        let custom = self.get_str("customSearch");
        if self.get_str("searchEngine") == "custom" && custom.contains("%s") && custom.starts_with("https://") {
            custom.replace("%s", &q)
        } else {
            format!("{lunar_search}?q={q}")
        }
    }

    pub fn home_url(&self, new_tab: &str) -> String {
        let home = self.get_str("homePage");
        if home.starts_with("https://") || home.starts_with("http://") { home } else { new_tab.to_string() }
    }
}
