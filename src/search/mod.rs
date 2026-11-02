pub mod crawl;
pub mod index;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use url::Url;

use crawl::{Crawler, Frontier, Step, SEEDS};
use index::{Hit, Index};

const WORKERS: usize = 12;
const STARTER_INDEX: &str = "https://github.com/skyzx0/lunar-browser/releases/latest/download/lunar-search-index.gz";
const SAVE_EVERY: usize = 50;

pub struct Engine {
    index: RwLock<Index>,
    frontier: Mutex<Frontier>,
    path: Option<PathBuf>,
    loaded: AtomicBool,
    crawl: AtomicBool,
    active: AtomicUsize,
    unsaved: AtomicUsize,
}

pub struct Results {
    pub hits: Vec<Hit>,
    pub matched: usize,
    pub pages: usize,
    pub crawling: bool,
    pub loaded: bool,
}

impl Engine {
    fn new(path: Option<PathBuf>, crawl: bool) -> Engine {
        Engine {
            index: RwLock::new(Index::default()),
            frontier: Mutex::new(Frontier::default()),
            path,
            loaded: AtomicBool::new(false),
            crawl: AtomicBool::new(crawl),
            active: AtomicUsize::new(0),
            unsaved: AtomicUsize::new(0),
        }
    }

    pub fn open(path: Option<PathBuf>, crawl: bool) -> Arc<Engine> {
        let engine = Arc::new(Engine::new(path, crawl));
        let e = engine.clone();
        std::thread::spawn(move || {
            e.load();
            for _ in 0..WORKERS {
                let e = e.clone();
                std::thread::spawn(move || e.work());
            }
        });
        engine
    }

    pub fn open_now(path: Option<PathBuf>) -> Arc<Engine> {
        let engine = Arc::new(Engine::new(path, false));
        engine.load();
        engine
    }

    fn load(&self) {
        if let Some(path) = &self.path {
            *self.index.write().unwrap() = Index::load(path);
            let marker = path.with_extension("starter");
            if self.crawl.load(Ordering::SeqCst) && !marker.exists() && self.fetch_starter(path) {
                let _ = std::fs::write(marker, STARTER_INDEX);
            }
        }
        let index = self.index.read().unwrap();
        let mut per_host: HashMap<String, usize> = HashMap::new();
        let mut frontier = self.frontier.lock().unwrap();
        for url in index.urls() {
            frontier.mark_seen(url);
            if let Some(host) = Url::parse(url).ok().and_then(|u| u.host_str().map(String::from)) {
                *per_host.entry(host).or_default() += 1;
            }
        }
        for (start, limit, depth) in SEEDS {
            let host = Url::parse(start).ok().and_then(|u| u.host_str().map(String::from)).unwrap_or_default();
            let done = per_host.remove(&host).unwrap_or(0);
            if done < *limit {
                frontier.add_site(start, limit - done, *depth);
            }
        }
        drop(frontier);
        drop(index);
        self.loaded.store(true, Ordering::SeqCst);
    }

    fn fetch_starter(&self, path: &std::path::Path) -> bool {
        let Ok(url) = Url::parse(STARTER_INDEX) else { return false };
        let net = (crate::net::dns::Resolver::new(), Mutex::new(crate::net::http::CookieJar::default()));
        let request = crate::net::http::Request { url, method: "GET".into(), body: None };
        let Ok(response) = crate::net::http::fetch_limit(request, &net.1, &net.0, crate::net::http::USER_AGENT, 400 << 20) else { return false };
        if response.status != 200 {
            return false;
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("download");
        if std::fs::write(&tmp, &response.body).is_err() {
            return false;
        }
        let mut starter = Index::load(&tmp);
        let _ = std::fs::remove_file(&tmp);
        if starter.len() == 0 {
            return false;
        }
        let mut index = self.index.write().unwrap();
        for page in index.pages() {
            starter.add(page.clone());
        }
        *index = starter;
        drop(index);
        self.save();
        true
    }

    fn work(&self) {
        let crawler = Crawler::new();
        loop {
            if !self.crawl.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            self.active.fetch_add(1, Ordering::SeqCst);
            let step = crawler.step(&self.frontier);
            self.active.fetch_sub(1, Ordering::SeqCst);
            match step {
                Step::Indexed(page) => {
                    self.index.write().unwrap().add(page);
                    if self.unsaved.fetch_add(1, Ordering::SeqCst) + 1 >= SAVE_EVERY {
                        self.save();
                    }
                }
                Step::Skipped => {}
                Step::Wait => std::thread::sleep(Duration::from_millis(150)),
                Step::Done => {
                    if self.unsaved.load(Ordering::SeqCst) > 0 {
                        self.save();
                    }
                    std::thread::sleep(Duration::from_secs(2));
                }
            }
        }
    }

    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        self.unsaved.store(0, Ordering::SeqCst);
        let _ = self.index.read().unwrap().save(path);
    }

    pub fn search(&self, query: &str) -> Results {
        let index = self.index.read().unwrap();
        let (hits, matched) = index.search(query, 30);
        Results { hits, matched, pages: index.len(), crawling: self.crawling(), loaded: self.loaded.load(Ordering::SeqCst) }
    }

    pub fn loaded(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    pub fn pages(&self) -> usize {
        self.index.read().unwrap().len()
    }

    /// True while the crawler has more to do.
    pub fn crawling(&self) -> bool {
        self.crawl.load(Ordering::SeqCst) && (self.frontier.lock().unwrap().pending() > 0 || self.active.load(Ordering::SeqCst) > 0)
    }

    pub fn set_crawl(&self, on: bool) {
        self.crawl.store(on, Ordering::SeqCst);
        if !on {
            self.save();
        }
    }

    pub fn add_site(&self, url: &Url) -> bool {
        let Some(host) = url.host_str() else { return false };
        if !matches!(url.scheme(), "https" | "http") {
            return false;
        }
        let mut f = self.frontier.lock().unwrap();
        f.add_site(url.as_str(), 300, 3);
        f.add_site(&format!("https://{host}/"), 0, 3);
        true
    }
}
