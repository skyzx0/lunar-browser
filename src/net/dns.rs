use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{connect_tls, http::read_response, Conn};
use std::io::Write;

/// (server IP, port, name its certificate is for, path)
const SERVERS: &[(&str, u16, &str, &str)] = &[
    ("1.1.1.1", 443, "1.1.1.1", "/dns-query"),
    ("9.9.9.9", 5053, "dns.quad9.net", "/dns-query"),
    ("1.0.0.1", 443, "1.0.0.1", "/dns-query"),
];
const MAX_TTL: Duration = Duration::from_secs(600);

pub struct Resolver {
    cache: Mutex<HashMap<String, (Vec<IpAddr>, Instant)>>,
}

impl Resolver {
    pub fn new() -> Self {
        Resolver { cache: Mutex::new(HashMap::new()) }
    }

    pub fn clear(&self) {
        self.cache.lock().unwrap().clear();
    }

    pub fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, String> {
        let host = host.trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
        if let Ok(ip) = host.parse::<IpAddr>() {
            return Ok(vec![ip]);
        }
        if host == "localhost" || host.ends_with(".localhost") {
            return Ok(vec!["127.0.0.1".parse().unwrap(), "::1".parse().unwrap()]);
        }
        if let Some((ips, until)) = self.cache.lock().unwrap().get(&host) {
            if Instant::now() < *until {
                return Ok(ips.clone());
            }
        }
        let mut last = String::new();
        for (ip, port, name, path) in SERVERS {
            match lookup(ip, *port, name, path, &host) {
                Ok((ips, ttl)) if !ips.is_empty() => {
                    let until = Instant::now() + ttl.min(MAX_TTL);
                    self.cache.lock().unwrap().insert(host.clone(), (ips.clone(), until));
                    return Ok(ips);
                }
                Ok(_) => return Err(format!("ERR_NAME_NOT_RESOLVED: {host}")),
                Err(e) => last = e,
            }
        }
        Err(format!("ERR_DNS: encrypted DNS lookup failed ({last})"))
    }
}

fn query(conn: &mut Conn, name: &str, path: &str, host: &str, kind: &str) -> Result<(Vec<IpAddr>, Duration), String> {
    let request = format!(
        "GET {path}?name={host}&type={kind} HTTP/1.1\r\nHost: {name}\r\nAccept: application/dns-json\r\nConnection: keep-alive\r\nUser-Agent: Lunar\r\n\r\n"
    );
    conn.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
    let response = read_response(conn, 1 << 20)?;
    if response.status != 200 {
        return Err(format!("DNS server answered {}", response.status));
    }
    let json: serde_json::Value = serde_json::from_slice(&response.body).map_err(|e| e.to_string())?;
    let mut ips = Vec::new();
    let mut ttl = MAX_TTL;
    if let Some(answers) = json.get("Answer").and_then(|a| a.as_array()) {
        for a in answers {
            let t = a.get("type").and_then(|t| t.as_u64()).unwrap_or(0);
            if t == 1 || t == 28 {
                if let Some(ip) = a.get("data").and_then(|d| d.as_str()).and_then(|d| d.parse().ok()) {
                    ips.push(ip);
                    let secs = a.get("TTL").and_then(|t| t.as_u64()).unwrap_or(60);
                    ttl = ttl.min(Duration::from_secs(secs.max(30)));
                }
            }
        }
    }
    Ok((ips, ttl))
}

fn lookup(ip: &str, port: u16, name: &str, path: &str, host: &str) -> Result<(Vec<IpAddr>, Duration), String> {
    let ip: IpAddr = ip.parse().unwrap();
    let mut conn = connect_tls(&[ip], port, name)?;
    let (mut ips, ttl4) = query(&mut conn, name, path, host, "A")?;
    if let Ok((v6, _)) = query(&mut conn, name, path, host, "AAAA") {
        ips.extend(v6);
    }
    Ok((ips, ttl4))
}
