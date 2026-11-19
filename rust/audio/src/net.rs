//! Opening a stream: redirects (including https→http, which some stations use), ICY metadata, and a
//! raw-socket fallback for old Shoutcast servers that answer `ICY 200 OK`.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use url::Url;

pub struct Connection {
    pub url: Url,
    pub content_type: Option<String>,
    /// `icy-metaint`: audio bytes between metadata blocks, when the server sends titles.
    pub meta_int: Option<usize>,
    /// `icy-br`: the station's stated bitrate in kbps.
    pub bitrate: Option<u32>,
    pub reader: Box<dyn Read + Send>,
}

/// `icy-br` values look like "128" or "128,128"; the first number counts.
pub fn parse_icy_br(v: &str) -> Option<u32> {
    v.split(|c: char| !c.is_ascii_digit()).find(|t| !t.is_empty())?.parse().ok().filter(|&n| n > 0)
}

#[derive(Debug)]
pub enum NetError {
    Http(u16),
    Other(String),
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetError::Http(code) => write!(f, "The station answered with HTTP {code}."),
            NetError::Other(s) => f.write_str(s),
        }
    }
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Between reads, not for the whole stream: a live stream never ends.
const READ_TIMEOUT: Duration = Duration::from_secs(20);

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout_read(READ_TIMEOUT)
        .redirects(0) // followed below, so https→http works and headers stay ours
        .build()
}

/// GET `url` with `Icy-MetaData: 1` (when `icy`), following up to 5 redirects.
pub fn open(url: &Url, user_agent: &str, icy: bool) -> Result<Connection, NetError> {
    let agent = agent();
    let mut url = url.clone();
    for _ in 0..6 {
        let mut req = agent.get(url.as_str()).set("User-Agent", user_agent);
        if icy {
            req = req.set("Icy-MetaData", "1");
        }
        let resp = match req.call() {
            Ok(r) => r,
            Err(ureq::Error::Status(code, r)) if (300..400).contains(&code) => r,
            Err(ureq::Error::Status(code, _)) => return Err(NetError::Http(code)),
            Err(e) => {
                // ureq can't parse "ICY 200 OK"; old Shoutcast servers send it.
                if url.scheme() == "http" {
                    if let Ok(c) = open_raw(&url, user_agent, icy) {
                        return Ok(c);
                    }
                }
                return Err(NetError::Other(format!("The stream could not be opened: {e}")));
            }
        };
        let status = resp.status();
        if (300..400).contains(&status) {
            let location = resp.header("Location").ok_or_else(|| NetError::Other("Redirect without a location.".into()))?;
            url = url.join(location).map_err(|e| NetError::Other(format!("Bad redirect: {e}")))?;
            continue;
        }
        let content_type = resp.header("Content-Type").map(str::to_string);
        let meta_int = resp.header("icy-metaint").and_then(|v| v.trim().parse::<usize>().ok()).filter(|&n| n > 0);
        let bitrate = resp.header("icy-br").and_then(parse_icy_br);
        let final_url = Url::parse(resp.get_url()).unwrap_or(url);
        return Ok(Connection { url: final_url, content_type, meta_int, bitrate, reader: Box::new(resp.into_reader()) });
    }
    Err(NetError::Other("Too many redirects.".into()))
}

/// GET over a plain socket, accepting `ICY 200 OK` as well as HTTP/1.x status lines.
pub fn open_raw(url: &Url, user_agent: &str, icy: bool) -> io::Result<Connection> {
    let host = url.host_str().ok_or_else(|| io::Error::other("no host"))?;
    let port = url.port_or_known_default().unwrap_or(80);
    let addr = (host, port).to_socket_addrs()?.next().ok_or_else(|| io::Error::other("no address"))?;
    let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)?;
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    let path = match url.query() {
        Some(q) => format!("{}?{}", url.path(), q),
        None => url.path().to_string(),
    };
    let mut req = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nUser-Agent: {user_agent}\r\nAccept: */*\r\n");
    if icy {
        req.push_str("Icy-MetaData: 1\r\n");
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes())?;
    let mut reader = BufReader::new(stream);
    let mut status = String::new();
    reader.read_line(&mut status)?;
    let ok = status.starts_with("ICY 200") || (status.starts_with("HTTP/1.") && status.split_whitespace().nth(1) == Some("200"));
    if !ok {
        return Err(io::Error::other(format!("status: {}", status.trim())));
    }
    let (mut content_type, mut meta_int, mut bitrate) = (None, None, None);
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            match k.trim().to_ascii_lowercase().as_str() {
                "content-type" => content_type = Some(v.trim().to_string()),
                "icy-metaint" => meta_int = v.trim().parse::<usize>().ok().filter(|&n| n > 0),
                "icy-br" => bitrate = parse_icy_br(v),
                _ => {}
            }
        }
    }
    Ok(Connection { url: url.clone(), content_type, meta_int, bitrate, reader: Box::new(reader) })
}

/// Fetches a small text body (a playlist), up to `max` bytes.
pub fn fetch_text(url: &Url, user_agent: &str, max: usize) -> Result<(Url, Option<String>, String), NetError> {
    let c = open(url, user_agent, false)?;
    let mut body = Vec::new();
    c.reader.take(max as u64).read_to_end(&mut body).map_err(|e| NetError::Other(e.to_string()))?;
    Ok((c.url, c.content_type, String::from_utf8_lossy(&body).into_owned()))
}

/// Fetches a whole body (an HLS segment).
pub fn fetch_bytes(url: &Url, user_agent: &str) -> Result<Vec<u8>, NetError> {
    let c = open(url, user_agent, false)?;
    let mut body = Vec::new();
    c.reader.take(16 << 20).read_to_end(&mut body).map_err(|e| NetError::Other(e.to_string()))?;
    Ok(body)
}
