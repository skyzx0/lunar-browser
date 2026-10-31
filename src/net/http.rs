//! Lunar's HTTP/1.1 client.
//!
//! - HTTPS only: http:// is upgraded (except localhost), redirects too
//! - cookies live in memory, are only sent back to the exact host that set
//!   them, and vanish when Lunar quits
//! - sends Global Privacy Control and Do Not Track, no Referer, a generic
//!   user agent
//! - only the page itself is ever fetched: no scripts, images or trackers

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Mutex;

use url::Url;

use super::{connect_plain, connect_tls, dns::Resolver, is_local, Conn};

pub const USER_AGENT: &str = "Mozilla/5.0 (compatible; Lunar/1.0; +text)";
const MAX_BODY: usize = 12 << 20;
const MAX_REDIRECTS: usize = 10;

pub struct Response {
    pub url: Url,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// Cookies, by exact host. Session only: never written anywhere.
#[derive(Default)]
pub struct CookieJar {
    by_host: HashMap<String, Vec<(String, String, String)>>,
}

impl CookieJar {
    pub fn clear(&mut self) {
        self.by_host.clear();
    }

    pub fn count(&self) -> usize {
        self.by_host.values().map(Vec::len).sum()
    }

    fn header_for(&self, url: &Url) -> Option<String> {
        let cookies = self.by_host.get(url.host_str()?)?;
        let path = url.path();
        let pairs: Vec<String> = cookies
            .iter()
            .filter(|(_, _, p)| path.starts_with(p.as_str()))
            .map(|(k, v, _)| format!("{k}={v}"))
            .collect();
        (!pairs.is_empty()).then(|| pairs.join("; "))
    }

    fn store(&mut self, url: &Url, header: &str) {
        let Some(host) = url.host_str() else { return };
        let mut parts = header.split(';');
        let Some((name, value)) = parts.next().and_then(|p| p.split_once('=')) else { return };
        let (name, value) = (name.trim().to_string(), value.trim().to_string());
        let mut path = "/".to_string();
        let mut expired = false;
        for attr in parts {
            let (k, v) = attr.split_once('=').unwrap_or((attr, ""));
            match k.trim().to_ascii_lowercase().as_str() {
                "path" if v.trim().starts_with('/') => path = v.trim().to_string(),
                "max-age" => expired = v.trim().parse::<i64>().map_or(false, |n| n <= 0),
                _ => {}
            }
        }
        let list = self.by_host.entry(host.to_string()).or_default();
        list.retain(|(k, _, p)| !(k == &name && p == &path));
        if !expired && list.len() < 60 {
            list.push((name, value, path));
        }
    }
}

pub struct Request {
    pub url: Url,
    pub method: String,
    pub body: Option<Vec<u8>>,
}

/// http:// -> https:// (and ws -> wss) unless it is this computer.
pub fn upgrade(mut url: Url) -> Url {
    if url.scheme() == "http" && !url.host_str().map_or(false, is_local) {
        let _ = url.set_scheme("https");
    }
    url
}

fn open(url: &Url, resolver: &Resolver) -> Result<Conn, String> {
    let host = url.host_str().ok_or("no host")?;
    let port = url.port_or_known_default().ok_or("no port")?;
    let ips = resolver.resolve(host)?;
    match url.scheme() {
        "https" => connect_tls(&ips, port, host.trim_start_matches('[').trim_end_matches(']')),
        "http" if is_local(host) => connect_plain(&ips, port),
        _ => Err("ERR_HTTPS_ONLY: Lunar only opens encrypted (https) connections".into()),
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Reads one response off `conn`: head, then a body sized by
/// Content-Length, chunked encoding, or the end of the connection.
pub fn read_response(conn: &mut Conn, max: usize) -> Result<Response, String> {
    let mut buf = Vec::with_capacity(16 * 1024);
    let mut chunk = [0u8; 16 * 1024];
    let head_end = loop {
        if let Some(i) = find(&buf, b"\r\n\r\n") {
            break i;
        }
        if buf.len() > 64 * 1024 {
            return Err("response headers too large".into());
        }
        let n = conn.read(&mut chunk).map_err(|e| format!("ERR_CONNECTION: {e}"))?;
        if n == 0 {
            return Err("ERR_EMPTY_RESPONSE: the server closed the connection".into());
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut body = buf[head_end + 4..].to_vec();
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or("");
    let status: u16 = status_line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or("bad status line")?;
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let get = |name: &str| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone());

    let chunked = get("transfer-encoding").map_or(false, |v| v.to_ascii_lowercase().contains("chunked"));
    let length: Option<usize> = get("content-length").and_then(|v| v.parse().ok());
    if status == 204 || status == 304 || (100..200).contains(&status) {
        body.clear();
    } else if chunked {
        body = read_chunked(conn, body, max)?;
    } else if let Some(len) = length {
        if len > max {
            return Err("page too large".into());
        }
        while body.len() < len {
            let n = conn.read(&mut chunk).map_err(|e| format!("ERR_CONNECTION: {e}"))?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
        }
        body.truncate(len);
    } else {
        loop {
            let n = conn.read(&mut chunk).map_err(|e| format!("ERR_CONNECTION: {e}"))?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
            if body.len() > max {
                return Err("page too large".into());
            }
        }
    }

    let body = match get("content-encoding").map(|v| v.to_ascii_lowercase()) {
        Some(e) if e.contains("gzip") => inflate(flate2::read::GzDecoder::new(&body[..]), max)?,
        Some(e) if e.contains("deflate") => inflate(flate2::read::ZlibDecoder::new(&body[..]), max)
            .or_else(|_| inflate(flate2::read::DeflateDecoder::new(&body[..]), max))?,
        _ => body,
    };
    Ok(Response { url: Url::parse("about:blank").unwrap(), status, headers, body })
}

fn inflate(reader: impl Read, max: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    reader.take(max as u64 + 1).read_to_end(&mut out).map_err(|e| format!("bad compressed data: {e}"))?;
    if out.len() > max {
        return Err("page too large".into());
    }
    Ok(out)
}

fn read_chunked(conn: &mut Conn, mut pending: Vec<u8>, max: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    let mut fill = |pending: &mut Vec<u8>, conn: &mut Conn| -> Result<bool, String> {
        let n = conn.read(&mut chunk).map_err(|e| format!("ERR_CONNECTION: {e}"))?;
        pending.extend_from_slice(&chunk[..n]);
        Ok(n > 0)
    };
    loop {
        let line_end = loop {
            if let Some(i) = find(&pending, b"\r\n") {
                break i;
            }
            if !fill(&mut pending, conn)? {
                return Ok(out);
            }
        };
        let size_text = String::from_utf8_lossy(&pending[..line_end]).to_string();
        let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16)
            .map_err(|_| "bad chunked encoding")?;
        pending.drain(..line_end + 2);
        if size == 0 {
            return Ok(out);
        }
        while pending.len() < size + 2 {
            if !fill(&mut pending, conn)? {
                out.extend_from_slice(&pending[..pending.len().min(size)]);
                return Ok(out);
            }
        }
        out.extend_from_slice(&pending[..size]);
        pending.drain(..size + 2);
        if out.len() > max {
            return Err("page too large".into());
        }
    }
}

fn send(req: &Request, jar: &Mutex<CookieJar>, resolver: &Resolver) -> Result<Response, String> {
    let url = &req.url;
    let mut conn = open(url, resolver)?;
    let host = match url.port() {
        Some(p) => format!("{}:{p}", url.host_str().unwrap_or("")),
        None => url.host_str().unwrap_or("").to_string(),
    };
    let mut path = url.path().to_string();
    if let Some(q) = url.query() {
        path.push('?');
        path.push_str(q);
    }
    let mut head = format!(
        "{} {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: {USER_AGENT}\r\n\
         Accept: text/html,application/xhtml+xml,text/plain;q=0.9,*/*;q=0.1\r\n\
         Accept-Language: en-US,en;q=0.8\r\nAccept-Encoding: gzip, deflate\r\n\
         Connection: close\r\nDNT: 1\r\nSec-GPC: 1\r\nUpgrade-Insecure-Requests: 1\r\n",
        req.method
    );
    if let Some(cookies) = jar.lock().unwrap().header_for(url) {
        head.push_str(&format!("Cookie: {cookies}\r\n"));
    }
    if let Some(body) = &req.body {
        head.push_str(&format!(
            "Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    head.push_str("\r\n");
    conn.write_all(head.as_bytes()).map_err(|e| format!("ERR_CONNECTION: {e}"))?;
    if let Some(body) = &req.body {
        conn.write_all(body).map_err(|e| format!("ERR_CONNECTION: {e}"))?;
    }
    conn.flush().ok();
    let mut response = read_response(&mut conn, MAX_BODY)?;
    response.url = url.clone();
    let mut jar = jar.lock().unwrap();
    for (k, v) in &response.headers {
        if k.eq_ignore_ascii_case("set-cookie") {
            jar.store(url, v);
        }
    }
    Ok(response)
}

/// Fetches a page, following redirects (each one upgraded to https).
pub fn fetch(mut req: Request, jar: &Mutex<CookieJar>, resolver: &Resolver) -> Result<Response, String> {
    req.url = upgrade(req.url);
    for _ in 0..=MAX_REDIRECTS {
        let response = send(&req, jar, resolver)?;
        if matches!(response.status, 301 | 302 | 303 | 307 | 308) {
            let location = response.header("location").ok_or("redirect without a location")?;
            let next = req.url.join(location).map_err(|e| format!("bad redirect: {e}"))?;
            if !matches!(next.scheme(), "http" | "https") {
                return Err("ERR_BLOCKED: redirect to a non-web address".into());
            }
            if !matches!(response.status, 307 | 308) {
                req.method = "GET".into();
                req.body = None;
            }
            req.url = upgrade(next);
            continue;
        }
        return Ok(response);
    }
    Err("ERR_TOO_MANY_REDIRECTS".into())
}

/// Decodes a body to text using the charset the server or page declares.
pub fn decode_text(response: &Response) -> String {
    let declared = response
        .header("content-type")
        .and_then(|ct| ct.to_ascii_lowercase().split("charset=").nth(1).map(|c| c.trim_matches(['"', ' ', ';']).to_string()));
    let sniffed = || {
        let head = String::from_utf8_lossy(&response.body[..response.body.len().min(2048)]).to_ascii_lowercase();
        head.split("charset=").nth(1).map(|c| {
            c.trim_start_matches(['"', '\'']).chars().take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_').collect::<String>()
        })
    };
    let charset = declared.or_else(sniffed).unwrap_or_else(|| "utf-8".into());
    if matches!(charset.as_str(), "iso-8859-1" | "latin1" | "latin-1" | "windows-1252" | "cp1252" | "us-ascii")
        && std::str::from_utf8(&response.body).is_err()
    {
        response.body.iter().map(|&b| b as char).collect()
    } else {
        String::from_utf8_lossy(&response.body).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_http() {
        assert_eq!(upgrade(Url::parse("http://example.com/a").unwrap()).as_str(), "https://example.com/a");
        assert_eq!(upgrade(Url::parse("http://localhost:8000/").unwrap()).as_str(), "http://localhost:8000/");
    }

    #[test]
    fn cookies_stay_on_their_host() {
        let mut jar = CookieJar::default();
        let a = Url::parse("https://a.example/x").unwrap();
        jar.store(&a, "sid=1; Path=/; Domain=example; HttpOnly");
        assert_eq!(jar.header_for(&a).as_deref(), Some("sid=1"));
        assert_eq!(jar.header_for(&Url::parse("https://b.example/").unwrap()), None);
    }
}
