use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use flate2::{read::DeflateDecoder, read::GzDecoder, write::DeflateEncoder, write::GzEncoder, Compression};

const HEADER: &str = "LUNAR-INDEX 1";
const TITLE_WEIGHT: u32 = 4;
pub const MAX_TEXT: usize = 5_000;
const DOC_BITS: u32 = 24;
const TF_MAX: u32 = 255;
const MERGE_AFTER: usize = 2_000;

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

struct Meta {
    url: Box<str>,
    title: Box<str>,
    text_at: u64,
    text_len: u32,
}

fn pack(text: &str) -> Vec<u8> {
    let mut z = DeflateEncoder::new(Vec::with_capacity(text.len() / 3), Compression::default());
    let _ = z.write_all(text.as_bytes());
    z.finish().unwrap_or_default()
}

fn unpack(data: &[u8]) -> String {
    let mut out = String::new();
    let _ = DeflateDecoder::new(data).read_to_string(&mut out);
    out
}

fn url_key(url: &str) -> u64 {
    let mut h = DefaultHasher::new();
    url.hash(&mut h);
    h.finish()
}

enum Texts {
    Memory(Vec<u8>),
    File(Mutex<File>, u64),
}

impl Default for Texts {
    fn default() -> Self {
        Texts::Memory(Vec::new())
    }
}

impl Texts {
    fn open(path: PathBuf) -> Texts {
        match OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&path) {
            Ok(file) => Texts::File(Mutex::new(file), 0),
            Err(_) => Texts::default(),
        }
    }

    fn append(&mut self, data: &[u8]) -> u64 {
        match self {
            Texts::Memory(buf) => {
                buf.extend_from_slice(data);
                (buf.len() - data.len()) as u64
            }
            Texts::File(file, end) => {
                let at = *end;
                let mut f = file.lock().unwrap();
                if f.seek(SeekFrom::Start(at)).and_then(|_| f.write_all(data)).is_ok() {
                    *end += data.len() as u64;
                }
                at
            }
        }
    }

    fn read(&self, at: u64, len: u32) -> String {
        match self {
            Texts::Memory(buf) => buf.get(at as usize..at as usize + len as usize).map(unpack).unwrap_or_default(),
            Texts::File(file, _) => {
                let mut data = vec![0; len as usize];
                let mut f = file.lock().unwrap();
                match f.seek(SeekFrom::Start(at)).and_then(|_| f.read_exact(&mut data)) {
                    Ok(()) => unpack(&data),
                    Err(_) => String::new(),
                }
            }
        }
    }
}

#[derive(Default)]
struct Packed {
    letters: Vec<u8>,
    word_at: Vec<u32>,
    post_at: Vec<u32>,
    postings: Vec<u32>,
}

impl Packed {
    fn len(&self) -> usize {
        self.word_at.len().saturating_sub(1)
    }

    fn word(&self, i: usize) -> &[u8] {
        &self.letters[self.word_at[i] as usize..self.word_at[i + 1] as usize]
    }

    fn list(&self, i: usize) -> &[u32] {
        &self.postings[self.post_at[i] as usize..self.post_at[i + 1] as usize]
    }

    fn get(&self, word: &str) -> &[u32] {
        let (mut lo, mut hi) = (0, self.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            match self.word(mid).cmp(word.as_bytes()) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => return self.list(mid),
            }
        }
        &[]
    }

    fn merged(self, recent: HashMap<Box<str>, Vec<u32>>) -> Packed {
        let mut recent: Vec<(Box<str>, Vec<u32>)> = recent.into_iter().collect();
        recent.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let mut out = Packed {
            letters: Vec::with_capacity(self.letters.len() + recent.iter().map(|r| r.0.len()).sum::<usize>()),
            word_at: Vec::with_capacity(self.len() + recent.len() + 1),
            post_at: Vec::with_capacity(self.len() + recent.len() + 1),
            postings: Vec::with_capacity(self.postings.len() + recent.iter().map(|r| r.1.len()).sum::<usize>()),
        };
        out.word_at.push(0);
        out.post_at.push(0);
        let push = |out: &mut Packed, word: &[u8], lists: &[&[u32]]| {
            out.letters.extend_from_slice(word);
            for list in lists {
                out.postings.extend_from_slice(list);
            }
            out.word_at.push(out.letters.len() as u32);
            out.post_at.push(out.postings.len() as u32);
        };
        let (mut i, mut j) = (0, 0);
        while i < self.len() || j < recent.len() {
            let take_old = j >= recent.len() || (i < self.len() && self.word(i) <= recent[j].0.as_bytes());
            if take_old {
                if j < recent.len() && self.word(i) == recent[j].0.as_bytes() {
                    push(&mut out, self.word(i), &[self.list(i), &recent[j].1]);
                    j += 1;
                } else {
                    push(&mut out, self.word(i), &[self.list(i)]);
                }
                i += 1;
            } else {
                push(&mut out, recent[j].0.as_bytes(), &[&recent[j].1]);
                j += 1;
            }
        }
        out
    }
}

#[derive(Default)]
struct Builder {
    ids: HashMap<u64, u32>,
    letters: Vec<u8>,
    word_at: Vec<u32>,
    entries: Vec<u64>,
}

impl Builder {
    fn add(&mut self, word: &str, packed: u32) {
        let mut h = DefaultHasher::new();
        word.hash(&mut h);
        let id = match self.ids.get(&h.finish()) {
            Some(&id) => id,
            None => {
                if self.word_at.is_empty() {
                    self.word_at.push(0);
                }
                self.letters.extend_from_slice(word.as_bytes());
                self.word_at.push(self.letters.len() as u32);
                let id = (self.word_at.len() - 2) as u32;
                self.ids.insert(h.finish(), id);
                id
            }
        };
        self.entries.push(((id as u64) << 32) | packed as u64);
    }

    fn finish(self) -> Packed {
        let Builder { ids, letters, word_at, mut entries } = self;
        drop(ids);
        let words = word_at.len().saturating_sub(1);
        let word = |i: usize| &letters[word_at[i] as usize..word_at[i + 1] as usize];
        entries.sort_unstable();
        let mut first = vec![0u32; words + 1];
        for e in &entries {
            first[(e >> 32) as usize + 1] += 1;
        }
        for i in 0..words {
            first[i + 1] += first[i];
        }
        let mut order: Vec<u32> = (0..words as u32).collect();
        order.sort_unstable_by(|&a, &b| word(a as usize).cmp(word(b as usize)));
        let mut out = Packed {
            letters: Vec::with_capacity(letters.len()),
            word_at: Vec::with_capacity(words + 1),
            post_at: Vec::with_capacity(words + 1),
            postings: Vec::with_capacity(entries.len()),
        };
        out.word_at.push(0);
        out.post_at.push(0);
        for &w in &order {
            let w = w as usize;
            out.letters.extend_from_slice(word(w));
            out.postings.extend(entries[first[w] as usize..first[w + 1] as usize].iter().map(|e| *e as u32));
            out.word_at.push(out.letters.len() as u32);
            out.post_at.push(out.postings.len() as u32);
        }
        out
    }
}

#[derive(Default)]
pub struct Index {
    pages: Vec<Meta>,
    lengths: Vec<u32>,
    total_length: u64,
    packed: Packed,
    recent: HashMap<Box<str>, Vec<u32>>,
    recent_pages: usize,
    by_url: HashMap<u64, u32>,
    texts: Texts,
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Index {
    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn into_pages(self) -> impl Iterator<Item = Page> {
        let Index { pages, texts, .. } = self;
        pages.into_iter().map(move |p| Page { text: texts.read(p.text_at, p.text_len), url: p.url.into(), title: p.title.into() })
    }

    pub fn urls(&self) -> impl Iterator<Item = &str> {
        self.pages.iter().map(|p| &*p.url)
    }

    pub fn stats(&self) -> String {
        format!(
            "pages {}  words {}  postings {} ({} MB)  recently added words {}",
            self.pages.len(),
            self.packed.len(),
            self.packed.postings.len(),
            (self.packed.postings.len() * 4) >> 20,
            self.recent.len()
        )
    }

    pub fn compact(&mut self) {
        let recent = std::mem::take(&mut self.recent);
        self.packed = std::mem::take(&mut self.packed).merged(recent);
        self.recent_pages = 0;
        self.pages.shrink_to_fit();
        self.lengths.shrink_to_fit();
        self.by_url.shrink_to_fit();
    }

    pub fn add(&mut self, page: Page) {
        self.insert(page);
        if self.recent_pages >= MERGE_AFTER {
            self.compact();
        }
    }

    fn insert(&mut self, page: Page) {
        self.insert_into(page, None);
    }

    fn insert_into(&mut self, page: Page, mut builder: Option<&mut Builder>) {
        let key = url_key(&page.url);
        if self.by_url.contains_key(&key) || self.pages.len() >= (1 << DOC_BITS) {
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
            let packed = (id << (32 - DOC_BITS)) | n.min(TF_MAX);
            if let Some(b) = builder.as_deref_mut() {
                b.add(&w, packed);
                continue;
            }
            match self.recent.get_mut(w.as_str()) {
                Some(list) => list.push(packed),
                None => {
                    self.recent.insert(w.into_boxed_str(), vec![packed]);
                }
            }
        }
        self.lengths.push(length.max(1));
        self.total_length += length.max(1) as u64;
        self.by_url.insert(key, id);
        let text = pack(&page.text);
        let text_at = self.texts.append(&text);
        self.pages.push(Meta { url: page.url.into_boxed_str(), title: page.title.into_boxed_str(), text_at, text_len: text.len() as u32 });
        if builder.is_none() {
            self.recent_pages += 1;
        }
    }

    fn postings_for<'a>(&'a self, word: &str) -> impl Iterator<Item = u32> + 'a {
        let recent = self.recent.get(word).map(Vec::as_slice).unwrap_or(&[]);
        self.packed.get(word).iter().chain(recent.iter()).copied()
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
            let df = self.postings_for(term).count();
            if df == 0 {
                continue;
            }
            let idf = ((n - df as f64 + 0.5) / (df as f64 + 0.5) + 1.0).ln();
            for packed in self.postings_for(term) {
                let (doc, tf) = (packed >> (32 - DOC_BITS), packed & TF_MAX);
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
                let text = self.texts.read(page.text_at, page.text_len);
                Hit { url: page.url.to_string(), title: page.title.to_string(), snippet: snippet(&text, &terms) }
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
            let text = self.texts.read(p.text_at, p.text_len);
            writeln!(z, "{}\t{}\t{}", clean(&p.url), clean(&p.title).replace('\t', " "), clean(&text).replace('\t', " "))?;
        }
        z.finish()?;
        std::fs::rename(tmp, path)
    }

    pub fn load(path: &Path) -> Index {
        let mut index = Index { texts: Texts::open(path.with_extension("texts")), ..Index::default() };
        let Ok(file) = std::fs::File::open(path) else { return index };
        let mut lines = BufReader::new(GzDecoder::new(file)).lines();
        if lines.next().and_then(Result::ok).as_deref() != Some(HEADER) {
            return index;
        }
        let mut builder = Builder::default();
        for line in lines.map_while(Result::ok) {
            let mut parts = line.splitn(3, '\t');
            if let (Some(url), Some(title), Some(text)) = (parts.next(), parts.next(), parts.next()) {
                index.insert_into(Page { url: url.into(), title: title.into(), text: text.into() }, Some(&mut builder));
            }
        }
        index.packed = builder.finish();
        index.compact();
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
        assert_eq!(hits[0].url, "https://a.example/moon");
    }

    #[test]
    fn packed_and_recent_words_both_found() {
        let mut i = Index::default();
        i.add(page("https://a.example/", "Alpha", "zebra yak"));
        i.compact();
        i.add(page("https://b.example/", "Beta", "yak walrus"));
        assert_eq!(i.search("yak", 5).1, 2);
        assert_eq!(i.search("zebra", 5).1, 1);
        assert_eq!(i.search("walrus", 5).1, 1);
        i.compact();
        assert_eq!(i.search("yak", 5).1, 2);
        assert_eq!(i.search("aardvark", 5).1, 0);
    }

    #[test]
    fn saves_and_loads() {
        let mut i = Index::default();
        i.add(page("https://a.example/", "A\ttitle", "some text"));
        let path = std::env::temp_dir().join(format!("lunar-index-test-{}.gz", std::process::id()));
        i.save(&path).unwrap();
        let j = Index::load(&path);
        std::fs::remove_file(&path).ok();
        std::fs::remove_file(path.with_extension("texts")).ok();
        assert_eq!(j.len(), 1);
        let (hits, _) = j.search("text", 5);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.contains("<b>text</b>"));
    }
}
