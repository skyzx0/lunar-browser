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
const DISCOVER_PAGES: usize = 12;
const DISCOVER_SITES: usize = 2000;
const NEVER: &[&str] = &[
    "facebook.com", "fb.com", "twitter.com", "x.com", "t.co", "instagram.com", "threads.net", "tiktok.com",
    "youtube.com", "youtu.be", "linkedin.com", "pinterest.com", "reddit.com", "redd.it", "snapchat.com",
    "whatsapp.com", "telegram.org", "t.me", "discord.com", "discord.gg", "google.com", "goo.gl", "googleapis.com",
    "gstatic.com", "doubleclick.net", "amazon.com", "amzn.to", "apps.apple.com", "play.google.com", "bit.ly",
    "tinyurl.com", "ow.ly", "buff.ly", "medium.com", "paypal.com", "patreon.com", "archive.org", "web.archive.org",
    "wikidata.org", "wikimedia.org", "mediawiki.org", "doi.org", "github.com", "githubusercontent.com",
    "bsky.app", "mastodon.social", "flickr.com", "vimeo.com", "spotify.com", "soundcloud.com", "twitch.tv",
];

pub const SEEDS: &[(&str, usize, u8)] = &[
    ("https://en.wikipedia.org/wiki/Wikipedia:Vital_articles/Level/4/People", 700, 1),
    ("https://en.wikipedia.org/wiki/Wikipedia:Vital_articles/Level/4/History", 500, 1),
    ("https://en.wikipedia.org/wiki/Wikipedia:Vital_articles/Level/4/Geography", 600, 1),
    ("https://en.wikipedia.org/wiki/Wikipedia:Vital_articles/Level/4/Arts", 400, 1),
    ("https://en.wikipedia.org/wiki/Wikipedia:Vital_articles/Level/4/Philosophy_and_religion", 250, 1),
    ("https://en.wikipedia.org/wiki/Wikipedia:Vital_articles/Level/4/Everyday_life", 300, 1),
    ("https://en.wikipedia.org/wiki/Wikipedia:Vital_articles/Level/4/Society_and_social_sciences", 450, 1),
    ("https://en.wikipedia.org/wiki/Wikipedia:Vital_articles/Level/4/Biology_and_health_sciences", 550, 1),
    ("https://en.wikipedia.org/wiki/Wikipedia:Vital_articles/Level/4/Physical_sciences", 500, 1),
    ("https://en.wikipedia.org/wiki/Wikipedia:Vital_articles/Level/4/Technology", 400, 1),
    ("https://en.wikipedia.org/wiki/Wikipedia:Vital_articles/Level/4/Mathematics", 250, 1),
    ("https://simple.wikipedia.org/wiki/Main_Page", 600, 2),
    ("https://en.wikivoyage.org/wiki/Main_Page", 400, 2),
    ("https://en.wikibooks.org/wiki/Main_Page", 250, 2),
    ("https://en.wikiquote.org/wiki/Main_Page", 150, 2),
    ("https://en.wikinews.org/wiki/Main_Page", 250, 2),
    ("https://it.wikipedia.org/wiki/Pagina_principale", 300, 2),
    ("https://de.wikipedia.org/wiki/Wikipedia:Hauptseite", 250, 2),
    ("https://fr.wikipedia.org/wiki/Wikip%C3%A9dia:Accueil_principal", 250, 2),
    ("https://es.wikipedia.org/wiki/Wikipedia:Portada", 250, 2),
    ("https://www.britannica.com/", 400, 2),
    ("https://plato.stanford.edu/contents.html", 400, 1),
    ("https://iep.utm.edu/", 200, 2),
    ("https://mathworld.wolfram.com/", 250, 2),
    ("https://hyperphysics.phy-astr.gsu.edu/hbase/hframe.html", 200, 2),
    ("https://ourworldindata.org/", 200, 2),
    ("https://www.gutenberg.org/browse/scores/top", 250, 1),
    ("https://standardebooks.org/ebooks", 150, 1),
    ("https://www.poetryfoundation.org/", 150, 2),
    ("https://www.loc.gov/", 150, 2),
    // News
    ("https://apnews.com/", 300, 2),
    ("https://www.bbc.com/news", 300, 2),
    ("https://www.theguardian.com/international", 300, 2),
    ("https://www.npr.org/sections/news/", 250, 2),
    ("https://www.dw.com/en/", 200, 2),
    ("https://www.france24.com/en/", 200, 2),
    ("https://www.abc.net.au/news", 200, 2),
    ("https://www.cbc.ca/news", 200, 2),
    ("https://www.euronews.com/", 150, 2),
    ("https://www.pbs.org/newshour/", 150, 2),
    ("https://www.propublica.org/", 150, 2),
    ("https://www.independent.co.uk/", 150, 2),
    ("https://theconversation.com/global", 250, 2),
    ("https://www.smithsonianmag.com/", 200, 2),
    // Science
    ("https://www.sciencedaily.com/", 250, 2),
    ("https://phys.org/", 250, 2),
    ("https://www.quantamagazine.org/", 200, 2),
    ("https://www.space.com/", 150, 2),
    ("https://www.nasa.gov/", 250, 2),
    ("https://www.esa.int/", 150, 2),
    ("https://www.noaa.gov/", 150, 2),
    ("https://www.usgs.gov/", 150, 2),
    ("https://arxiv.org/list/cs/recent", 150, 1),
    // Health and government
    ("https://www.nhs.uk/conditions/", 400, 1),
    ("https://medlineplus.gov/healthtopics.html", 400, 1),
    ("https://www.cdc.gov/", 200, 2),
    ("https://www.gov.uk/browse", 250, 2),
    ("https://www.usa.gov/", 150, 2),
    ("https://www.nps.gov/index.htm", 150, 2),
    ("https://www.archives.gov/", 100, 2),
    // Technology and programming
    ("https://arstechnica.com/", 250, 2),
    ("https://lwn.net/", 150, 2),
    ("https://techcrunch.com/", 150, 2),
    ("https://news.ycombinator.com/", 100, 1),
    ("https://developer.mozilla.org/en-US/docs/Web", 500, 2),
    ("https://doc.rust-lang.org/book/", 250, 2),
    ("https://doc.rust-lang.org/std/", 150, 1),
    ("https://docs.python.org/3/", 400, 2),
    ("https://go.dev/doc/", 200, 2),
    ("https://en.cppreference.com/w/", 250, 2),
    ("https://www.php.net/manual/en/", 200, 2),
    ("https://kotlinlang.org/docs/home.html", 150, 2),
    ("https://www.typescriptlang.org/docs/", 120, 2),
    ("https://react.dev/learn", 120, 2),
    ("https://vuejs.org/guide/introduction.html", 100, 2),
    ("https://docs.djangoproject.com/en/stable/", 150, 2),
    ("https://nodejs.org/en/learn", 100, 2),
    ("https://git-scm.com/docs", 150, 1),
    ("https://www.postgresql.org/docs/current/", 250, 2),
    ("https://www.sqlite.org/docs.html", 150, 1),
    ("https://nginx.org/en/docs/", 120, 2),
    ("https://docs.docker.com/get-started/", 150, 2),
    ("https://kubernetes.io/docs/home/", 200, 2),
    ("https://wiki.archlinux.org/title/Table_of_contents", 400, 2),
    ("https://wiki.debian.org/FrontPage", 150, 2),
    ("https://docs.freebsd.org/en/books/handbook/", 150, 2),
    ("https://www.gnu.org/manual/manual.html", 150, 2),
    ("https://man7.org/linux/man-pages/dir_all_alphabetic.html", 300, 1),
    ("https://www.kernel.org/", 50, 2),
    ("https://www.w3.org/", 100, 2),
    // Blogs and essays
    ("https://paulgraham.com/articles.html", 150, 1),
    ("https://danluu.com/", 120, 1),
    ("https://jvns.ca/", 120, 1),
    ("https://martinfowler.com/", 120, 2),
    ("https://www.joelonsoftware.com/", 80, 2),
    ("https://blog.codinghorror.com/", 80, 2),
    ("https://daringfireball.net/", 80, 2),
    ("https://kottke.org/", 80, 2),
    ("https://xkcd.com/archive/", 150, 1),
    ("https://waitbutwhy.com/archive", 80, 1),
    // Culture, food, travel, outdoors
    ("https://www.seriouseats.com/", 150, 2),
    ("https://www.budgetbytes.com/", 120, 2),
    ("https://www.simplyrecipes.com/", 120, 2),
    ("https://www.si.edu/", 100, 2),
    ("https://www.openstreetmap.org/about", 30, 2),
    ("https://www.ifixit.com/Guide", 120, 2),
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

fn never(host: &str) -> bool {
    NEVER.iter().any(|d| host == *d || host.ends_with(&format!(".{d}")))
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
    discovered: usize,
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
        if never(&host) {
            return;
        }
        if !self.sites.contains_key(&host) {
            if depth > 2 || self.discovered >= DISCOVER_SITES {
                return;
            }
            self.discovered += 1;
            self.sites.insert(host.clone(), Site {
                queue: VecDeque::new(),
                next: Instant::now(),
                taken: 0,
                limit: DISCOVER_PAGES,
                max_depth: 1,
            });
            if self.seen.insert(url.clone()) {
                self.sites.get_mut(&host).unwrap().queue.push_back((url, 0));
            }
            return;
        }
        let site = self.sites.get_mut(&host).unwrap();
        if depth > site.max_depth || site.queue.len() + site.taken >= site.limit * 3 || !self.seen.insert(url.clone()) {
            return;
        }
        site.queue.push_back((url, depth));
    }

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
                let nofollow = e.attr("rel").map_or(false, |r| r.contains("nofollow")) || e.attr("hreflang").is_some();
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
    fn frontier_limits_and_discovery() {
        let mut f = Frontier::default();
        f.add_site("https://a.example/", 2, 1);
        f.push("https://a.example/x".into(), 1);
        f.push("https://a.example/too-deep".into(), 2);
        assert_eq!(f.pending(), 2);
        f.push("https://elsewhere.example/page".into(), 1); // discovered: a new site
        f.push("https://www.youtube.com/watch?v=1".into(), 1); // never crawled
        assert_eq!(f.pending(), 3);
        assert_eq!(f.discovered, 1);
    }
}
