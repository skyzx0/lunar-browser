use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use flate2::{read::GzDecoder, write::GzEncoder, Compression};

const HEADER: &str = "LUNAR-INDEX 1";
const TITLE_WEIGHT: u32 = 4;
pub const MAX_TEXT: usize = 5_000;

const STOP: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "in", "is", "it", "its", "of", "on",
    "or", "that", "the", "to", "was", "were", "will", "with", "this", "these", "those", "but", "not", "you",
];

pub fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && w.chars().count() <= 40)
        .map(|w| {
            let w = w.to_lowercase();
            if w.len() > 3 && w.ends_with('s') && !w.ends_with("ss") { w[..w.len() - 1].to_string() } else { w }
        })
        .filter(|w| !STOP.contains(&w.as_str()) && (w.chars().count() > 1 || w.chars().all(|c| c.is_numeric())))
        .collect()
}

#[derive(Clone)]
pub struct Page {
    pub url: String,
    pub title: String,
    pub text: String,
}

pub struct Hit {
    pub url: String,
    pub title: String,
    pub snippet: String,
}

#[derive(Default)]
pub struct Index {
    pages: Vec<Page>,
    lengths: Vec<u32>,
    total_length: u64,
    postings: HashMap<String, Vec<(u32, u32)>>,
    by_url: HashMap<String, u32>,
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Index {
    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    pub fn urls(&self) -> impl Iterator<Item = &str> {
        self.pages.iter().map(|p| p.url.as_str())
    }

    pub fn add(&mut self, page: Page) {
        if self.by_url.contains_key(&page.url) {
            return;
        }
        let id = self.pages.len() as u32;
        let mut counts: HashMap<String, u32> = HashMap::new();
        for w in words(&page.title) {
            *counts.entry(w).or_default() += TITLE_WEIGHT;
        }
        let mut length = 0u32;
        for w in words(&page.text) {
            *counts.entry(w).or_default() += 1;
            length += 1;
        }
        for (w, n) in counts {
            self.postings.entry(w).or_default().push((id, n));
        }
        self.lengths.push(length.max(1));
        self.total_length += length.max(1) as u64;
        self.by_url.insert(page.url.clone(), id);
        self.pages.push(page);
    }

    pub fn search(&self, query: &str, limit: usize) -> (Vec<Hit>, usize) {
        let mut terms = words(query);
        terms.sort();
        terms.dedup();
        if terms.is_empty() || self.pages.is_empty() {
            return (Vec::new(), 0);
        }
        let n = self.pages.len() as f64;
        let avg = self.total_length as f64 / n;
        let (k1, b) = (1.2, 0.75);
        let mut scores: HashMap<u32, (f64, usize)> = HashMap::new();
        for term in &terms {
            let Some(list) = self.postings.get(term) else { continue };
            let idf = ((n - list.len() as f64 + 0.5) / (list.len() as f64 + 0.5) + 1.0).ln();
            for &(doc, tf) in list {
                let len = self.lengths[doc as usize] as f64;
                let tf = tf as f64;
                let s = idf * tf * (k1 + 1.0) / (tf + k1 * (1.0 - b + b * len / avg));
                let entry = scores.entry(doc).or_default();
                entry.0 += s;
                entry.1 += 1;
            }
        }
        let matched = scores.len();
        let mut ranked: Vec<(u32, f64)> = scores
            .into_iter()
            .map(|(doc, (score, hits))| {
                let page = &self.pages[doc as usize];
                let coverage = (hits as f64 / terms.len() as f64).powi(2);
                let url = page.url.to_lowercase();
                let in_url = terms.iter().filter(|t| url.contains(t.as_str())).count() as f64;
                let depth = page.url.matches('/').count().saturating_sub(2) as f64;
                (doc, score * coverage * (1.0 + 0.25 * in_url) / (1.0 + 0.05 * depth))
            })
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let hits = ranked
            .into_iter()
            .take(limit)
            .map(|(doc, _)| {
                let page = &self.pages[doc as usize];
                Hit { url: page.url.clone(), title: page.title.clone(), snippet: snippet(&page.text, &terms) }
            })
            .collect();
        (hits, matched)
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        let mut z = GzEncoder::new(std::fs::File::create(&tmp)?, Compression::fast());
        writeln!(z, "{HEADER}")?;
        for p in &self.pages {
            writeln!(z, "{}\t{}\t{}", clean(&p.url), clean(&p.title).replace('\t', " "), clean(&p.text).replace('\t', " "))?;
        }
        z.finish()?;
        std::fs::rename(tmp, path)
    }

    pub fn load(path: &Path) -> Index {
        let mut index = Index::default();
        let Ok(file) = std::fs::File::open(path) else { return index };
        let mut lines = BufReader::new(GzDecoder::new(file)).lines();
        if lines.next().and_then(Result::ok).as_deref() != Some(HEADER) {
            return index;
        }
        for line in lines.map_while(Result::ok) {
            let mut parts = line.splitn(3, '\t');
            if let (Some(url), Some(title), Some(text)) = (parts.next(), parts.next(), parts.next()) {
                index.add(Page { url: url.into(), title: title.into(), text: text.into() });
            }
        }
        index
    }
}

fn snippet(text: &str, terms: &[String]) -> String {
    let lower = text.to_lowercase();
    let at = terms.iter().filter_map(|t| lower.find(t.as_str())).min().unwrap_or(0);
    let chars: Vec<char> = text.chars().collect();
    let char_at = lower[..at].chars().count();
    let start = char_at.saturating_sub(80);
    let end = (start + 240).min(chars.len());
    let mut window: String = chars[start..end].iter().collect();
    if start > 0 {
        window = format!("…{}", window.split_once(' ').map_or(window.as_str(), |(_, r)| r));
    }
    if end < chars.len() {
        window = format!("{}…", window.rsplit_once(' ').map_or(window.as_str(), |(l, _)| l));
    }
    let escaped = window.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut out = String::new();
    for (i, word) in escaped.split(' ').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let bare = words(word);
        if bare.iter().any(|w| terms.contains(w)) {
            out.push_str(&format!("<b>{word}</b>"));
        } else {
            out.push_str(word);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(url: &str, title: &str, text: &str) -> Page {
        Page { url: url.into(), title: title.into(), text: text.into() }
    }

    #[test]
    fn ranks_and_snippets() {
        let mut i = Index::default();
        i.add(page("https://a.example/moon", "Moon", "The Moon is Earth's natural satellite. Lunar phases repeat monthly."));
        i.add(page("https://b.example/cheese", "Cheese", "Cheese is made from milk. Some say the moon is made of cheese."));
        i.add(page("https://c.example/sun", "Sun", "The Sun is a star."));
        let (hits, matched) = i.search("lunar phases", 10);
        assert_eq!(matched, 1);
        assert_eq!(hits[0].url, "https://a.example/moon");
        assert!(hits[0].snippet.contains("<b>phases</b>"));
        let (hits, _) = i.search("moon", 10);
        assert_eq!(hits[0].url, "https://a.example/moon"); // title match wins
    }

    #[test]
    fn saves_and_loads() {
        let mut i = Index::default();
        i.add(page("https://a.example/", "A\ttitle", "some text"));
        let path = std::env::temp_dir().join(format!("lunar-index-test-{}.gz", std::process::id()));
        i.save(&path).unwrap();
        let j = Index::load(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!(j.len(), 1);
        assert_eq!(j.search("text", 5).0.len(), 1);
    }
}
