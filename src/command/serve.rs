//! Local dev server with live reload.
//!
//! Serves `config.output_dir` over HTTP, runs the file watcher in parallel,
//! and pushes a reload event to connected browsers via SSE after every
//! *successful* rebuild. A tiny `<script>` tag is auto-injected into HTML
//! responses so no template changes are needed.
//!
//! Tokio is confined to this module. The watcher loop runs on its own OS
//! thread and signals reloads through a single broadcast channel.

use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full, StreamBody, combinators::UnsyncBoxBody};
use hyper::body::{Frame, Incoming};
use hyper::header::{CACHE_CONTROL, CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tower::ServiceExt;
use tower_http::services::ServeDir;

use crate::config::Config;

const LIVERELOAD_JS: &str =
    "new EventSource('/__livereload').onmessage = () => location.reload();\n";
const LIVERELOAD_TAG: &str = "<script src=\"/__livereload.js\"></script>";

type ServeBody = UnsyncBoxBody<Bytes, io::Error>;

pub fn run(host: IpAddr, port: u16) -> Result<()> {
    // serve is the local preview, so drafts are included (here and in the
    // watch-driven rebuilds below).
    crate::build::run(true).context("initial build")?;

    let (config, _) = Config::load_with_theme(Path::new("config.yaml"))?;
    let output_dir = config.output_dir.clone();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime")?;

    let (reload_tx, _) = broadcast::channel::<()>(16);

    let reload_tx_watcher = reload_tx.clone();
    std::thread::spawn(move || {
        // serve already built the site before binding (above); watch_loop is
        // purely the loop and does no initial build, so the site is built once.
        let result = crate::command::watch::watch_loop(|build_result| {
            if build_result.is_ok() {
                let _ = reload_tx_watcher.send(());
            }
        });
        if let Err(e) = result {
            eprintln!("watcher error: {e:#}");
        }
    });

    runtime.block_on(serve(host, port, output_dir, reload_tx))
}

async fn serve(
    host: IpAddr,
    port: u16,
    output_dir: PathBuf,
    reload_tx: broadcast::Sender<()>,
) -> Result<()> {
    let addr = SocketAddr::new(host, port);
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    eprintln!("serving http://{addr}");

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let output_dir = output_dir.clone();
        let reload_tx = reload_tx.clone();
        tokio::spawn(async move {
            let service = service_fn(move |req: Request<Incoming>| {
                let output_dir = output_dir.clone();
                let reload_tx = reload_tx.clone();
                async move { handle(req, output_dir, reload_tx).await }
            });
            // Connection errors (most commonly: client closed an SSE stream)
            // aren't actionable in a dev server, so we don't log them.
            let _ = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, service)
                .await;
        });
    }
}

async fn handle(
    req: Request<Incoming>,
    output_dir: PathBuf,
    reload_tx: broadcast::Sender<()>,
) -> Result<Response<ServeBody>, Infallible> {
    match req.uri().path() {
        "/__livereload" => Ok(sse_response(reload_tx)),
        "/__livereload.js" => Ok(js_response()),
        _ => Ok(serve_files(req, &output_dir).await),
    }
}

fn js_response() -> Response<ServeBody> {
    let body = Bytes::from_static(LIVERELOAD_JS.as_bytes());
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/javascript; charset=utf-8")
        .header(CONTENT_LENGTH, body.len() as u64)
        .body(full(body))
        .unwrap()
}

fn sse_response(reload_tx: broadcast::Sender<()>) -> Response<ServeBody> {
    let rx = reload_tx.subscribe();
    // Treat lag (slow client missing events) as a reload signal too —
    // the right answer in either case is "reload now."
    let stream = BroadcastStream::new(rx).map(|_| {
        Ok::<Frame<Bytes>, io::Error>(Frame::data(Bytes::from_static(b"data: reload\n\n")))
    });
    let body = StreamBody::new(stream).boxed_unsync();
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream")
        .header(CACHE_CONTROL, "no-cache")
        .header(CONNECTION, "keep-alive")
        .body(body)
        .unwrap()
}

async fn serve_files(req: Request<Incoming>, output_dir: &Path) -> Response<ServeBody> {
    let service = ServeDir::new(output_dir);
    let response = match service.oneshot(req).await {
        Ok(r) => r,
        Err(never) => match never {},
    };

    let is_html = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| s.starts_with("text/html"));

    if !is_html {
        let (parts, body) = response.into_parts();
        let boxed = body.map_err(io::Error::other).boxed_unsync();
        return Response::from_parts(parts, boxed);
    }

    let (mut parts, body) = response.into_parts();
    let collected = match body.collect().await {
        Ok(c) => c.to_bytes(),
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let injected = inject_livereload(&collected);
    parts
        .headers
        .insert(CONTENT_LENGTH, HeaderValue::from(injected.len() as u64));
    Response::from_parts(parts, full(Bytes::from(injected)))
}

fn inject_livereload(html: &[u8]) -> Vec<u8> {
    let needle = b"</body>";
    let pos = find_last_ci(html, needle);
    let mut out = Vec::with_capacity(html.len() + LIVERELOAD_TAG.len());
    match pos {
        Some(p) => {
            out.extend_from_slice(&html[..p]);
            out.extend_from_slice(LIVERELOAD_TAG.as_bytes());
            out.extend_from_slice(&html[p..]);
        }
        None => {
            out.extend_from_slice(html);
            out.extend_from_slice(LIVERELOAD_TAG.as_bytes());
        }
    }
    out
}

fn find_last_ci(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).rev().find(|&i| {
        haystack[i..i + needle.len()]
            .iter()
            .zip(needle)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

fn error_response(status: StatusCode, msg: String) -> Response<ServeBody> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(full(Bytes::from(msg)))
        .unwrap()
}

fn full(bytes: Bytes) -> ServeBody {
    Full::new(bytes).map_err(|never| match never {}).boxed_unsync()
}
