//! The local server: serves the page and answers the Statistics tab's questions.
//!
//! It listens on this computer only (127.0.0.1), so nobody else on the network can reach it.
//! The routes are a fixed list (the page and the data files) plus `/api/status` and
//! `/api/stats/<analysis>`, which are the same addresses the Python version used.

use crate::assets::{fingerprint, Asset, Sources, ASSETS};
use dss_search::corpus::Corpus;
use dss_search::query::{parse_query, query};
use dss_search::stats::ANALYSES;
use serde_json::{json, Value as Json};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::thread;
use tiny_http::{Header, Method, Request, Response, Server};

const WORKERS: usize = 4;

struct State {
    corpus: Corpus,
    sources: Sources,
    /// fingerprints of the built-in files, so that the browser can reuse what it has
    etags: HashMap<&'static str, String>,
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("a valid header")
}

fn send(request: Request, status: u16, content_type: &str, body: Vec<u8>, extra: &[(&str, String)]) {
    let mut response = Response::from_data(body)
        .with_status_code(status)
        .with_header(header("Content-Type", content_type))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    for (name, value) in extra {
        response = response.with_header(header(name, value));
    }
    // the browser may have gone away; there is nobody to tell
    let _ = request.respond(response);
}

fn send_json(request: Request, status: u16, payload: &Json) {
    let body = serde_json::to_vec(payload).unwrap_or_else(|_| b"{}".to_vec());
    send(request, status, "application/json; charset=utf-8", body, &[("Cache-Control", "no-store".to_string())]);
}

fn request_header(request: &Request, name: &str) -> Option<String> {
    request.headers().iter().find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name)).map(|h| h.value.as_str().to_string())
}

fn serve_asset(state: &State, request: Request, asset: &'static Asset) {
    let overridden = state.sources.override_path(asset).is_some();
    if !overridden {
        let etag = &state.etags[asset.route];
        if request_header(&request, "If-None-Match").as_deref() == Some(etag.as_str()) {
            let _ = request.respond(Response::empty(304).with_header(header("ETag", etag)));
            return;
        }
        // "no-cache" lets the browser keep the file but ask each time whether it changed (a 304 is cheap)
        send(request, 200, asset.mime, asset.bytes.to_vec(), &[("ETag", etag.clone()), ("Cache-Control", "no-cache".to_string())]);
        return;
    }
    match state.sources.read(asset) {
        Ok(bytes) => send(request, 200, asset.mime, bytes.into_owned(), &[("Cache-Control", "no-store".to_string())]),
        Err(message) => send_json(request, 500, &json!({ "error": message })),
    }
}

fn api(state: &State, request: Request, path: &str, query_string: &str) {
    let path = path.trim_end_matches('/');
    if path == "/api/status" {
        send_json(
            request,
            200,
            &json!({ "stats": true, "engine": "built-in", "version": env!("CARGO_PKG_VERSION"), "reason": null, "hint": null }),
        );
        return;
    }
    let name = path.strip_prefix("/api/stats/").unwrap_or("");
    if !ANALYSES.contains(&name) {
        send_json(request, 404, &json!({ "error": format!("There is no such question. Try one of: {}.", ANALYSES.join(", ")) }));
        return;
    }
    let mut params = parse_query(query_string);
    let format = params.remove("format").unwrap_or_else(|| "json".to_string());
    // a bug must not take the server down: report it in the terminal and tell the page
    let outcome = catch_unwind(AssertUnwindSafe(|| query(&state.corpus, name, &params)));
    match outcome {
        Ok(Ok(answer)) => {
            if format == "csv" {
                send(
                    request,
                    200,
                    "text/csv; charset=utf-8",
                    answer.table.csv_bytes(),
                    &[
                        ("Content-Disposition", format!("attachment; filename=\"dss-{name}.csv\"")),
                        ("Cache-Control", "no-store".to_string()),
                    ],
                );
            } else {
                let mut payload = answer.to_json();
                payload["analysis"] = json!(name);
                send_json(request, 200, &payload);
            }
        }
        Ok(Err(error)) => send_json(request, 400, &json!({ "error": error.0 })),
        Err(_) => {
            send_json(request, 500, &json!({ "error": "Something went wrong while computing this. The terminal window has details." }))
        }
    }
}

fn handle(state: &State, request: Request) {
    let method = request.method().clone();
    if method != Method::Get && method != Method::Head {
        let _ = request.respond(Response::empty(405).with_header(header("Allow", "GET, HEAD")));
        return;
    }
    let url = request.url().to_string();
    let (path, query_string) = url.split_once('?').unwrap_or((url.as_str(), ""));
    if path.starts_with("/api/") {
        api(state, request, path, query_string);
        return;
    }
    let route = if path == "/" { "/index.html" } else { path };
    match Sources::find(route) {
        Some(asset) => serve_asset(state, request, asset),
        None => send(request, 404, "text/plain; charset=utf-8", b"Not found".to_vec(), &[]),
    }
}

/// Listen on the wanted port (0 lets the operating system pick a free one); without one, the
/// first free port from 8000 and, failing that, any.
fn bind(port: Option<u16>) -> Result<Server, String> {
    let try_port = |p: u16| Server::http(("127.0.0.1", p));
    match port {
        Some(p) => try_port(p)
            .map_err(|e| format!("Could not listen on port {p}: {e}. Is another copy already running? Try --port with another number.")),
        None => {
            for p in 8000..8020 {
                if let Ok(server) = try_port(p) {
                    return Ok(server);
                }
            }
            try_port(0).map_err(|e| format!("Could not start the server: {e}"))
        }
    }
}

/// A server that is answering requests on background threads.
pub struct Started {
    /// the address of the page, for example `http://127.0.0.1:8000/`
    pub url: String,
    workers: Vec<thread::JoinHandle<()>>,
}

impl Started {
    /// Wait for as long as the server runs (that is, until the program is stopped).
    pub fn wait(self) {
        for worker in self.workers {
            let _ = worker.join();
        }
    }
}

/// Load the data and start answering requests. Returns as soon as the server is listening.
pub fn start(sources: Sources, port: Option<u16>) -> Result<Started, String> {
    for path in sources.describe_overrides() {
        println!("Using {path} instead of the built-in copy.");
    }
    let corpus = sources.load_corpus()?;
    let server = bind(port)?;
    let actual = server.server_addr().to_ip().map(|a| a.port()).ok_or("The server has no network address.")?;
    let url = format!("http://127.0.0.1:{actual}/");

    let etags = ASSETS.iter().map(|a| (a.route, format!("\"{:016x}-{}\"", fingerprint(a.bytes), env!("CARGO_PKG_VERSION")))).collect();
    let state = Arc::new(State { corpus, sources, etags });
    let server = Arc::new(server);
    let workers = (0..WORKERS)
        .map(|_| {
            let (server, state) = (Arc::clone(&server), Arc::clone(&state));
            thread::spawn(move || {
                while let Ok(request) = server.recv() {
                    handle(&state, request);
                }
            })
        })
        .collect();
    Ok(Started { url, workers })
}

/// `dss-search serve`: answer requests until stopped, with no window (for use from any browser).
pub fn run(sources: Sources, port: Option<u16>) -> Result<(), String> {
    let started = start(sources, port)?;
    println!("Dead Sea Scrolls Hebrew search {}", env!("CARGO_PKG_VERSION"));
    println!("Serving at {}  (press Ctrl+C to stop)", started.url);
    started.wait();
    Ok(())
}
