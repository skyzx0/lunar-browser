//! Lunar Search's crawler. It reads pages with Lunar's own network stack
//! and engine, follows their links, and adds them to the index.
//!
//! Polite by design: it obeys robots.txt and <meta name="robots">, sends
//! at most one request per second to any site, caps how much of a site it
//! takes, and says who it is in its user agent.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use url::Url;

use crate::engine::{doc, html::{self, Element, Node}};
use crate::net::dns::Resolver;
use crate::net::http::{self, CookieJar, Request};

use super::index::{Page, MAX_TEXT};

pub const CRAWLER_AGENT: &str = "LunarSearch";
const PER_SITE_DELAY: Duration = Duration::from_millis(1100);

/// Where the crawl starts: (address, pages to take from that site, link depth).
pub const SEEDS: &[(&str, usize, u8)] = &[
    ("https://en.wikipedia.org/wiki/Wikipedia:Vital_articles", 1200, 1),
    ("https://en.wikipedia.org/wiki/Main_Page", 150, 1),
    ("https://simple.wikipedia.org/wiki/Main_Page", 200, 2),
    ("https://en.wikinews.org/wiki/Main_Page", 120, 2),
    ("https://apnews.com/", 120, 2),
    ("https://www.bbc.com/news", 120, 2),
    ("https://www.theguardian.com/international", 120, 2),
    ("https://www.npr.org/sections/news/", 100, 2),
    ("https://arstechnica.com/", 100, 2),
    ("https://theconversation.com/global", 100, 2),
    ("https://www.gutenberg.org/browse/scores/top", 150, 1),
    ("https://doc.rust-lang.org/book/", 150, 2),
    ("https://docs.python.org/3/tutorial/index.html", 120, 2),
    ("https://go.dev/doc/", 80, 2),
    ("https://wiki.archlinux.org/title/Table_of_contents", 200, 2),
    ("https://developer.mozilla.org/en-US/docs/Web/HTML", 150, 2),
    ("https://news.ycombinator.com/", 60, 1),
    ("https://www.nasa.gov/", 80, 2),
    ("https://www.kernel.org/", 30, 2),
    ("https://www.openstreetmap.org/about", 30, 2),
];

const SKIP_EXTENSIONS: &[&str] = &[
    ".jpg", ".jpeg", ".png", ".gif", ".svg", ".webp", ".ico", ".pdf", ".zip", ".gz", ".xz", ".tar", ".mp3", ".mp4",
    ".webm", ".ogg", ".avi", ".mov", ".exe", ".dmg", ".iso", ".deb", ".rpm", ".css", ".js", ".json", ".xml", ".epub",
    ".mobi", ".apk", ".woff", ".woff2", ".ttf",
];

/// robots.txt rules for one site.
#[derive(Default)]
struct Robots {
    disallow: Vec<String>,
    allow: Vec<String>,
}

impl Robots {
    fn parse(text: &str) -> Robots {
        let mut rules = Robots::default();
        let mut ours = Robots::default();
        let mut group_agents: Vec<String> = Vec::new();
        let mut in_rules = false;
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once(':') else { continue };
            let (key, value) = (key.trim().to_ascii_lowercase(), value.trim().to_string());
            match key.as_str() {
                "user-agent" => {
                    if in_rules {
                        group_agents.clear();
                        in_rules = false;
                    }
                    group_agents.push(value.to_ascii_lowercase());
                }
                "disallow" | "allow" => {
                    in_rules = true;
                    let target = if group_agents.iter().any(|a| a == &CRAWLER_AGENT.to_ascii_lowercase()) {
                        &mut ours
                    } else if group_agents.iter().any(|a| a == "*") {
                        &mut rules
                    } else {
                        continue;
                    };
                    if value.is_empty() {
                        continue;
                    }
                    if key == "allow" { target.allow.push(value) } else { target.disallow.push(value) }
                }
                _ => {}
            }
        }
        // A group naming LunarSearch replaces the "*" rules.
        if !ours.disallow.is_empty() || !ours.allow.is_empty() { ours } else { rules }
    }

    fn allows(&self, path: &str) -> bool {
        let matches = |rule: &str| {
            let rule = rule.trim_end_matches('$');
            match rule.split_once('*') {
                Some((pre, post)) => path.starts_with(pre) && path[pre.len()..].contains(post.split('*').next().unwrap_or("")),
                None => path.starts_with(rule),
            }
        };
        let longest = |rules: &[String]| rules.iter().filter(|r| matches(r)).map(|r| r.len()).max();
        match (longest(&self.allow), longest(&self.disallow)) {
            (_, None) => true,
            (None, Some(_)) => false,
            (Some(a), Some(d)) => a >= d,
        }
    }
}

struct Site {
    queue: VecDeque<(String, u8)>,
    next: Instant,
    taken: usize,
    limit: usize,
    max_depth: u8,
}

/// What is left to crawl, per site.
#[derive(Default)]
pub struct Frontier {
    sites: HashMap<String, Site>,
    seen: HashSet<String>,
}

pub fn normalize(url: &Url) -> Option<String> {
    if !matches!(url.scheme(), "https" | "http") {
        return None;
    }
    let mut url = http::upgrade(url.clone());
    url.set_fragment(None);
    let path = url.path().to_ascii_lowercase();
    if SKIP_EXTENSIONS.iter().any(|e| path.ends_with(e)) {
        return None;
    }
    if url.query().map_or(false, |q| q.len() > 80) {
        return None;
    }
    Some(url.to_string())
}

impl Frontier {
    pub fn add_site(&mut self, start: &str, limit: usize, depth: u8) {
        let Some(url) = Url::parse(start).ok().as_ref().and_then(normalize) else { return };
        let Some(host) = Url::parse(&url).ok().and_then(|u| u.host_str().map(String::from)) else { return };
        let site = self.sites.entry(host).or_insert_with(|| Site {
            queue: VecDeque::new(),
            next: Instant::now(),
            taken: 0,
            limit: 0,
            max_depth: depth,
        });
        site.limit += limit;
        site.max_depth = site.max_depth.max(depth);
        self.seen.insert(url.clone());
        site.queue.push_front((url, 0));
    }

    pub fn mark_seen(&mut self, url: &str) {
        self.seen.insert(url.to_string());
    }

    fn push(&mut self, url: String, depth: u8) {
        let Some(host) = Url::parse(&url).ok().and_then(|u| u.host_str().map(String::from)) else { return };
        // Only sites the crawl was pointed at: no wandering off across the web.
        let Some(site) = self.sites.get_mut(&host) else { return };
        if depth > site.max_depth || site.queue.len() + site.taken >= site.limit * 3 || !self.seen.insert(url.clone()) {
            return;
        }
        site.queue.push_back((url, depth));
    }

    /// The next address whose site may be visited now.
    fn pop(&mut self) -> Option<(String, String, u8)> {
        let now = Instant::now();
        let host = self
            .sites
            .iter()
            .filter(|(_, s)| !s.queue.is_empty() && s.taken < s.limit && s.next <= now)
            .min_by_key(|(_, s)| s.taken)
            .map(|(h, _)| h.clone())?;
        let site = self.sites.get_mut(&host)?;
        site.next = now + PER_SITE_DELAY;
        site.taken += 1;
        let (url, depth) = site.queue.pop_front()?;
        Some((host, url, depth))
    }

    pub fn pending(&self) -> usize {
        self.sites.values().filter(|s| s.taken < s.limit).map(|s| s.queue.len().min(s.limit - s.taken)).sum()
    }
}

/// One crawler worker's tools.
pub struct Crawler {
    resolver: Resolver,
    jar: Mutex<CookieJar>,
    robots: Mutex<HashMap<String, Robots>>,
}

pub enum Step {
    Indexed(Page),
    Skipped,
    Wait,
    Done,
}

fn links(el: &Element, out: &mut Vec<String>) {
    for child in &el.children {
        if let Node::Element(e) = child {
            if e.tag == "a" {
                let nofollow = e.attr("rel").map_or(false, |r| r.contains("nofollow"));
                if let (Some(href), false) = (e.attr("href"), nofollow) {
                    out.push(href.trim().to_string());
                }
            }
            links(e, out);
        }
    }
}

fn meta_robots(root: &Element) -> (bool, bool) {
    let mut noindex = false;
    let mut nofollow = false;
    fn walk(el: &Element, noindex: &mut bool, nofollow: &mut bool) {
        for child in &el.children {
            if let Node::Element(e) = child {
                if e.tag == "meta" {
                    let name = e.attr("name").unwrap_or("").to_ascii_lowercase();
                    if name == "robots" || name == CRAWLER_AGENT.to_ascii_lowercase() {
                        let content = e.attr("content").unwrap_or("").to_ascii_lowercase();
                        *noindex |= content.contains("noindex") || content.contains("none");
                        *nofollow |= content.contains("nofollow") || content.contains("none");
                    }
                }
                walk(e, noindex, nofollow);
            }
        }
    }
    walk(root, &mut noindex, &mut nofollow);
    (noindex, nofollow)
}

/// Drops the parts of a page that repeat on every page of a site (menus,
/// headers, footers, sidebars, forms), so they don't drown out its content.
fn prune(el: &mut Element) {
    el.children.retain(|child| match child {
        Node::Element(e) => {
            let role = e.attr("role").unwrap_or("");
            !matches!(e.tag.as_str(), "nav" | "header" | "footer" | "aside" | "form" | "menu")
                && !matches!(role, "navigation" | "banner" | "contentinfo" | "search" | "complementary")
        }
        Node::Text(_) => true,
    });
    for child in &mut el.children {
        if let Node::Element(e) = child {
            prune(e);
        }
    }
}

/// The readable text of a page, in reading order.
fn page_text(d: &doc::Doc) -> String {
    fn inlines(list: &[doc::Inline], out: &mut String) {
        for i in list {
            if let doc::Inline::Text { text, .. } = i {
                out.push_str(text);
                out.push(' ');
            }
        }
    }
    fn blocks(list: &[doc::Block], out: &mut String) {
        for b in list {
            match b {
                doc::Block::Para(i) | doc::Block::Lines(i) | doc::Block::Heading(_, i) | doc::Block::Pre(i) => inlines(i, out),
                doc::Block::List { items, .. } => items.iter().for_each(|item| blocks(item, out)),
                doc::Block::Quote(bs) => blocks(bs, out),
                doc::Block::Table(rows) => rows.iter().flatten().for_each(|cell| inlines(cell, out)),
                doc::Block::Rule | doc::Block::Gap => {}
            }
            if out.len() > MAX_TEXT * 2 {
                return;
            }
        }
    }
    let mut out = String::new();
    blocks(&d.blocks, &mut out);
    let text = out.split_whitespace().collect::<Vec<_>>().join(" ");
    text.chars().take(MAX_TEXT).collect()
}

impl Crawler {
    pub fn new() -> Self {
        Crawler { resolver: Resolver::new(), jar: Mutex::new(CookieJar::default()), robots: Mutex::new(HashMap::new()) }
    }

    fn fetch(&self, url: Url) -> Result<http::Response, String> {
        let request = Request { url, method: "GET".into(), body: None };
        http::fetch_as(request, &self.jar, &self.resolver, &format!("{CRAWLER_AGENT}/1.0 (+https://github.com/skyzx0/lunar-browser)"))
    }

    fn allowed(&self, host: &str, url: &Url) -> bool {
        if !self.robots.lock().unwrap().contains_key(host) {
            let robots = Url::parse(&format!("https://{host}/robots.txt"))
                .ok()
                .and_then(|u| self.fetch(u).ok())
                .filter(|r| r.status == 200)
                .map(|r| Robots::parse(&http::decode_text(&r)))
                .unwrap_or_default();
            self.robots.lock().unwrap().insert(host.to_string(), robots);
        }
        let mut path = url.path().to_string();
        if let Some(q) = url.query() {
            path.push('?');
            path.push_str(q);
        }
        self.robots.lock().unwrap().get(host).map_or(true, |r| r.allows(&path))
    }

    /// Crawls one page, if one is ready.
    pub fn step(&self, frontier: &Mutex<Frontier>) -> Step {
        let next = frontier.lock().unwrap().pop();
        let Some((host, address, depth)) = next else {
            return if frontier.lock().unwrap().pending() == 0 { Step::Done } else { Step::Wait };
        };
        let Ok(url) = Url::parse(&address) else { return Step::Skipped };
        if !self.allowed(&host, &url) {
            return Step::Skipped;
        }
        let Ok(response) = self.fetch(url) else { return Step::Skipped };
        let kind = response.header("content-type").unwrap_or("text/html").to_ascii_lowercase();
        if response.status != 200 || !kind.contains("html") {
            return Step::Skipped;
        }
        let final_url = response.url.clone();
        let mut root = html::parse(&http::decode_text(&response));
        let (noindex, nofollow) = meta_robots(&root);
        if !nofollow {
            let mut found = Vec::new();
            links(&root, &mut found);
            let mut f = frontier.lock().unwrap();
            for href in found {
                if let Some(next) = final_url.join(&href).ok().as_ref().and_then(normalize) {
                    f.push(next, depth + 1);
                }
            }
        }
        if noindex {
            return Step::Skipped;
        }
        let title = root.find("title").map(|t| t.text()).unwrap_or_default();
        prune(&mut root);
        let mut d = doc::build(&root);
        d.title = title;
        let text = page_text(&d);
        if text.len() < 200 {
            return Step::Skipped;
        }
        let title = if d.title.trim().is_empty() { final_url.to_string() } else { d.title.trim().to_string() };
        Step::Indexed(Page { url: normalize(&final_url).unwrap_or(address), title, text })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robots_rules() {
        let r = Robots::parse("User-agent: *\nDisallow: /private\nAllow: /private/ok\nDisallow: /*?action=\n\nUser-agent: Other\nDisallow: /");
        assert!(r.allows("/"));
        assert!(!r.allows("/private/x"));
        assert!(r.allows("/private/ok/page"));
        assert!(!r.allows("/w/index.php?action=edit"));
        let ours = Robots::parse("User-agent: *\nDisallow: /\n\nUser-agent: LunarSearch\nDisallow: /secret");
        assert!(ours.allows("/page"));
        assert!(!ours.allows("/secret"));
    }

    #[test]
    fn frontier_stays_on_seed_sites() {
        let mut f = Frontier::default();
        f.add_site("https://a.example/", 2, 1);
        f.push("https://a.example/x".into(), 1);
        f.push("https://elsewhere.example/".into(), 1);
        f.push("https://a.example/too-deep".into(), 2);
        assert_eq!(f.pending(), 2);
    }
}
