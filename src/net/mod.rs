//! Lunar's networking: its own HTTP/1.1 client and DNS-over-HTTPS resolver.
//! Every connection is TLS (rustls, with Mozilla's root certificates); the
//! only plain connections allowed are to this computer (localhost).

pub mod dns;
pub mod http;

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use rustls::pki_types::ServerName;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(12);
pub const IO_TIMEOUT: Duration = Duration::from_secs(20);

fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let mut config = rustls::ClientConfig::builder_with_provider(provider)
                .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
                .expect("TLS 1.2 and 1.3 are supported")
                .with_root_certificates(roots)
                .with_no_client_auth();
            config.alpn_protocols = vec![b"http/1.1".to_vec()];
            Arc::new(config)
        })
        .clone()
}

/// A connection: TLS, or plain TCP for localhost only.
pub enum Conn {
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
    Plain(TcpStream),
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let result = match self {
            Conn::Tls(s) => s.read(buf),
            Conn::Plain(s) => s.read(buf),
        };
        // Many servers close without a TLS goodbye; treat that as the end.
        match result {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(0),
            other => other,
        }
    }
}

impl Write for Conn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Conn::Tls(s) => s.write(buf),
            Conn::Plain(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Conn::Tls(s) => s.flush(),
            Conn::Plain(s) => s.flush(),
        }
    }
}

fn tcp(ips: &[IpAddr], port: u16) -> Result<TcpStream, String> {
    let mut last = String::from("no address");
    for ip in ips {
        match TcpStream::connect_timeout(&SocketAddr::new(*ip, port), CONNECT_TIMEOUT) {
            Ok(s) => {
                s.set_read_timeout(Some(IO_TIMEOUT)).ok();
                s.set_write_timeout(Some(IO_TIMEOUT)).ok();
                s.set_nodelay(true).ok();
                return Ok(s);
            }
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!("could not connect: {last}"))
}

/// Opens a TLS connection to `ips` and checks the certificate is for `name`.
pub fn connect_tls(ips: &[IpAddr], port: u16, name: &str) -> Result<Conn, String> {
    let server_name = ServerName::try_from(name.to_string()).map_err(|_| format!("bad host name {name}"))?;
    let tcp = tcp(ips, port)?;
    let conn = rustls::ClientConnection::new(tls_config(), server_name).map_err(|e| e.to_string())?;
    let mut stream = rustls::StreamOwned::new(conn, tcp);
    // Finish the handshake now so certificate problems surface as such.
    while stream.conn.is_handshaking() {
        stream.conn.complete_io(&mut stream.sock).map_err(|e| {
            let text = e.to_string();
            if text.contains("certificate") || text.contains("Certificate") {
                format!("ERR_CERT: {text}")
            } else {
                format!("ERR_SSL: {text}")
            }
        })?;
    }
    Ok(Conn::Tls(Box::new(stream)))
}

pub fn connect_plain(ips: &[IpAddr], port: u16) -> Result<Conn, String> {
    Ok(Conn::Plain(tcp(ips, port)?))
}

pub fn is_local(host: &str) -> bool {
    host == "localhost" || host.ends_with(".localhost") || host == "127.0.0.1" || host == "[::1]" || host == "::1"
}
