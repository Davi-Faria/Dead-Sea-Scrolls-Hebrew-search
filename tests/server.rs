//! Starts the real program and talks HTTP to it, as a browser would.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};

struct Running {
    child: Child,
    port: u16,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start() -> Running {
    // port 0: the operating system picks a free one, which the program then prints
    let mut child = Command::new(env!("CARGO_BIN_EXE_dss-search"))
        .args(["--no-browser", "--port", "0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the program starts");
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let port = loop {
        let line = lines.next().expect("the program prints its address").unwrap();
        if let Some(rest) = line.strip_prefix("Serving at http://127.0.0.1:") {
            break rest.split('/').next().unwrap().parse().unwrap();
        }
    };
    // keep draining the output so the program never blocks on a full pipe
    std::thread::spawn(move || for _ in lines {});
    Running { child, port }
}

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("a JSON body")
    }
}

fn request(port: u16, method: &str, path: &str, extra_headers: &[(&str, &str)]) -> Reply {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut text = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
    for (k, v) in extra_headers {
        text += &format!("{k}: {v}\r\n");
    }
    text += "\r\n";
    stream.write_all(text.as_bytes()).unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).unwrap();
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").expect("a complete reply");
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.lines();
    let status = lines.next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
    let headers: Vec<(String, String)> =
        lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    let mut body = raw[split + 4..].to_vec();
    let chunked = headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("transfer-encoding") && v.eq_ignore_ascii_case("chunked"));
    // a reply to HEAD, or a 304, has the headers of a body but none
    if chunked && method != "HEAD" && status != 304 {
        body = unchunk(&body);
    }
    Reply { status, headers, body }
}

/// Undo "chunked" transfer encoding: each piece is its size in hex, a line break, the bytes, a line break.
fn unchunk(mut rest: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let eol = rest.windows(2).position(|w| w == b"\r\n").expect("a chunk size line");
        let size_text = String::from_utf8_lossy(&rest[..eol]).into_owned();
        let size = usize::from_str_radix(size_text.split(';').next().unwrap().trim(), 16).expect("a hex chunk size");
        rest = &rest[eol + 2..];
        if size == 0 {
            return out;
        }
        out.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..];
    }
}

fn get(port: u16, path: &str) -> Reply {
    request(port, "GET", path, &[])
}

#[test]
fn serves_the_page_and_its_files() {
    let server = start();
    let page = get(server.port, "/");
    assert_eq!(page.status, 200);
    assert!(page.header("content-type").unwrap().starts_with("text/html"));
    assert!(page.text().contains("Dead Sea Scrolls"));
    for (path, kind) in [
        ("/style.css", "text/css"),
        ("/app.js", "text/javascript"),
        ("/search.js", "text/javascript"),
        ("/data/dss.json", "application/json"),
        ("/data/bible.json", "application/json"),
        ("/data/scroll-names.json", "application/json"),
    ] {
        let reply = get(server.port, path);
        assert_eq!(reply.status, 200, "{path}");
        assert!(reply.header("content-type").unwrap().starts_with(kind), "{path}");
        assert!(!reply.body.is_empty(), "{path}");
    }
    let data = get(server.port, "/data/dss.json").json();
    assert_eq!(data["meta"]["words"], 389518);
}

#[test]
fn unknown_and_unsafe_requests_are_refused() {
    let server = start();
    assert_eq!(get(server.port, "/nope").status, 404);
    assert_eq!(get(server.port, "/../Cargo.toml").status, 404);
    assert_eq!(get(server.port, "/%2e%2e/Cargo.toml").status, 404);
    assert_eq!(get(server.port, "/data/").status, 404);
    assert_eq!(request(server.port, "POST", "/", &[]).status, 405);
}

#[test]
fn the_browser_can_reuse_files_it_has() {
    let server = start();
    let first = get(server.port, "/app.js");
    let etag = first.header("etag").expect("an ETag").to_string();
    let again = request(server.port, "GET", "/app.js", &[("If-None-Match", &etag)]);
    assert_eq!(again.status, 304);
    assert!(again.body.is_empty());
    let head = request(server.port, "HEAD", "/app.js", &[]);
    assert_eq!(head.status, 200);
    assert!(head.body.is_empty());
}

#[test]
fn answers_the_statistics_questions() {
    let server = start();
    let status = get(server.port, "/api/status").json();
    assert_eq!(status["stats"], true);

    let freq = get(server.port, "/api/stats/freq?top=3");
    assert_eq!(freq.status, 200);
    let body = freq.json();
    assert_eq!(body["analysis"], "freq");
    assert_eq!(body["rows"].as_array().unwrap().len(), 3);
    assert_eq!(body["rows"][0][1], "כל");
    assert_eq!(freq.header("cache-control"), Some("no-store"));

    // Hebrew in the address, percent-encoded as a browser sends it
    let dist = get(server.port, "/api/stats/dist?word=%D7%99%D7%97%D7%93&by=cave").json();
    assert_eq!(dist["summary"]["total"], 193);
    assert_eq!(dist["summary"]["found"], true);

    let csv = get(server.port, "/api/stats/freq?top=2&format=csv");
    assert!(csv.header("content-type").unwrap().starts_with("text/csv"));
    assert!(csv.header("content-disposition").unwrap().contains("dss-freq.csv"));
    assert!(csv.body.starts_with(b"\xef\xbb\xbfrank,item,lang,count"));
}

#[test]
fn explains_what_went_wrong() {
    let server = start();
    let bad = get(server.port, "/api/stats/colloc?word=");
    assert_eq!(bad.status, 400);
    assert_eq!(bad.json()["error"], "Enter a Hebrew word.");
    let bad = get(server.port, "/api/stats/freq?corpus=nope");
    assert_eq!(bad.status, 400);
    assert!(bad.json()["error"].as_str().unwrap().contains("corpus must be one of"));
    let unknown = get(server.port, "/api/stats/nope");
    assert_eq!(unknown.status, 404);
    assert!(unknown.json()["error"].as_str().unwrap().contains("freq, dist, colloc, keyness"));
    assert_eq!(get(server.port, "/api/other").status, 404);
}

#[test]
fn answers_several_questions_at_once() {
    let server = start();
    let port = server.port;
    let handles: Vec<_> = (0..12)
        .map(|i| {
            std::thread::spawn(move || {
                let path = match i % 3 {
                    0 => "/api/stats/freq?top=5",
                    1 => "/api/stats/colloc?word=%D7%99%D7%97%D7%93&top=5",
                    _ => "/api/stats/keyness?target=1QS&top=5",
                };
                get(port, path)
            })
        })
        .collect();
    for h in handles {
        let reply = h.join().unwrap();
        assert_eq!(reply.status, 200);
        assert_eq!(reply.json()["rows"].as_array().unwrap().len(), 5);
    }
}
