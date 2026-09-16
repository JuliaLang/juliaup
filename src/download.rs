//! Streaming downloads with bounded, concurrent byte-range requests.

use anyhow::{bail, Context, Result};
use reqwest::blocking::{Client, Response};
use reqwest::header::{ACCEPT_ENCODING, CONTENT_RANGE, ETAG, IF_MATCH, RANGE};
use reqwest::StatusCode;
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

/// Range workers, in addition to the initial streaming response.
const CONNECTIONS: usize = 4;

/// Balance request overhead against buffering.
const CHUNK_SIZE: u64 = 4 * 1024 * 1024;

/// How many chunks may be claimed by workers ahead of the consumer. Bounds
/// buffered range data to `(WINDOW + 1) * CHUNK_SIZE`, including the current chunk.
const WINDOW: u64 = 2 * CONNECTIONS as u64;

/// Attempts per chunk before the download is failed.
const ATTEMPTS: usize = 3;

/// A streaming download of a URL's body.
pub struct Download {
    /// Total body length, when the server reported one.
    pub content_length: Option<u64>,
    /// The `ETag` header of the response, when present.
    pub etag: Option<String>,
    reader: Box<dyn Read + Send>,
}

impl Read for Download {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buf)
    }
}

/// Starts downloading `url`, using concurrent range requests when the server
/// supports them.
pub fn start(client: &Client, url: &str) -> Result<Download> {
    start_with(client, url, CHUNK_SIZE, CONNECTIONS)
}

fn start_with(client: &Client, url: &str, chunk_size: u64, connections: usize) -> Result<Download> {
    let response = client
        .get(url)
        .header(ACCEPT_ENCODING, "identity")
        .header(RANGE, format!("bytes=0-{}", chunk_size - 1))
        .send()
        .with_context(|| format!("Failed to download from url `{}`.", url))?;

    let status = response.status();
    if status == StatusCode::RANGE_NOT_SATISFIABLE {
        drop(response);
        return plain(client, url);
    }
    if status != StatusCode::OK && status != StatusCode::PARTIAL_CONTENT {
        bail!(
            "Failed to download from url `{}` (status: {}).",
            url,
            status
        );
    }

    let etag = header_string(&response, ETAG);

    if status != StatusCode::PARTIAL_CONTENT {
        // Server ignored the range: stream the full body as it comes.
        log::debug!(
            "Server does not support range requests; streaming `{}`.",
            url
        );
        return Ok(Download {
            content_length: response.content_length(),
            etag,
            reader: Box::new(response),
        });
    }

    let content_range = header_string(&response, CONTENT_RANGE)
        .with_context(|| format!("Partial response from `{}` lacks Content-Range.", url))?;
    let (start, end, total) = parse_content_range(&content_range).with_context(|| {
        format!(
            "Malformed Content-Range `{}` from `{}`.",
            content_range, url
        )
    })?;

    if start != 0 || end >= chunk_size {
        bail!("Unexpected initial Content-Range `{content_range}`.");
    }
    let Some(total) = total else {
        drop(response);
        return plain(client, url);
    };
    let first_len = end + 1;
    if first_len != chunk_size.min(total) {
        bail!("Incomplete initial Content-Range `{content_range}`.");
    }
    if first_len == total {
        return Ok(Download {
            content_length: Some(total),
            etag,
            reader: Box::new(ExactReader::new(response, total)),
        });
    }
    // Weak validators cannot establish byte-for-byte identity across requests.
    if !etag
        .as_deref()
        .is_some_and(|tag| tag.len() >= 2 && tag.starts_with('"') && tag.ends_with('"'))
    {
        drop(response);
        return plain(client, url);
    }

    log::debug!(
        "Downloading `{}` ({} bytes) as {} byte ranges over {} connections.",
        url,
        total,
        total.div_ceil(chunk_size),
        connections
    );

    let reader = ChunkedReader::new(
        client.clone(),
        response,
        first_len,
        etag.clone(),
        total,
        chunk_size,
        connections,
    )?;

    Ok(Download {
        content_length: Some(total),
        etag,
        reader: Box::new(reader),
    })
}

fn plain(client: &Client, url: &str) -> Result<Download> {
    let response = client.get(url).header(ACCEPT_ENCODING, "identity").send()?;
    if response.status() != StatusCode::OK {
        bail!(
            "Expected a full response from `{url}`, got {}.",
            response.status()
        );
    }
    Ok(Download {
        content_length: response.content_length(),
        etag: header_string(&response, ETAG),
        reader: Box::new(response),
    })
}

// Check the body as well as its headers, including responses without Content-Length.
struct ExactReader<R> {
    inner: R,
    remaining: u64,
}

impl<R> ExactReader<R> {
    fn new(inner: R, remaining: u64) -> Self {
        Self { inner, remaining }
    }
}

impl<R: Read> Read for ExactReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return match self.inner.read(&mut [0])? {
                0 => Ok(0),
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Range body exceeds its declared length.",
                )),
            };
        }
        let len = self.remaining.min(buf.len() as u64) as usize;
        let n = self.inner.read(&mut buf[..len])?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Incomplete range body.",
            ));
        }
        self.remaining -= n as u64;
        Ok(n)
    }
}

fn header_string(response: &Response, name: reqwest::header::HeaderName) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

/// Parses and validates `bytes START-END/TOTAL` (`*` means unknown total).
fn parse_content_range(value: &str) -> Option<(u64, u64, Option<u64>)> {
    let rest = value.trim().strip_prefix("bytes ")?;
    let (range, total) = rest.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start: u64 = start.trim().parse().ok()?;
    let end: u64 = end.trim().parse().ok()?;
    if end < start {
        return None;
    }
    let total = match total.trim() {
        "*" => None,
        t => Some(t.parse().ok()?),
    };
    if end == u64::MAX || total.is_some_and(|total| end >= total) {
        return None;
    }
    Some((start, end, total))
}

/// Scheduling state shared between the consumer and the workers.
struct Shared {
    /// (next chunk index to claim, number of chunks the consumer has taken)
    state: Mutex<(u64, u64)>,
    changed: Condvar,
    cancelled: AtomicBool,
}

/// Reassembles concurrently fetched chunks into an ordered byte stream.
struct ChunkedReader {
    /// The initial response, streamed as chunk 0.
    first: Option<ExactReader<Response>>,
    rx: Receiver<(u64, io::Result<Vec<u8>>)>,
    pending: BTreeMap<u64, Vec<u8>>,
    next_chunk: u64,
    total_chunks: u64,
    current: io::Cursor<Vec<u8>>,
    shared: Arc<Shared>,
    failed: bool,
}

impl ChunkedReader {
    fn new(
        client: Client,
        first: Response,
        first_len: u64,
        etag: Option<String>,
        total: u64,
        chunk_size: u64,
        connections: usize,
    ) -> io::Result<Self> {
        // Pin subsequent requests to the initial response URL and validator.
        let url = first.url().to_string();
        let total_chunks = total.div_ceil(chunk_size);
        let shared = Arc::new(Shared {
            state: Mutex::new((1, 1)),
            changed: Condvar::new(),
            cancelled: AtomicBool::new(false),
        });
        let (tx, rx) = mpsc::channel();
        let reader = Self {
            first: Some(ExactReader::new(first, first_len)),
            rx,
            pending: BTreeMap::new(),
            next_chunk: 1,
            total_chunks,
            current: io::Cursor::new(Vec::new()),
            shared: Arc::clone(&shared),
            failed: false,
        };
        for _ in 0..(connections as u64).min(total_chunks - 1) {
            let worker = Worker {
                client: client.clone(),
                url: url.clone(),
                etag: etag.clone(),
                total,
                chunk_size,
                total_chunks,
                shared: Arc::clone(&shared),
                tx: tx.clone(),
            };
            thread::Builder::new()
                .name("juliaup-download".into())
                .spawn(move || worker.run())?;
        }
        Ok(reader)
    }

    /// Fetches the next in-order chunk into `current`. Returns false at EOF.
    fn advance(&mut self) -> io::Result<bool> {
        if self.next_chunk >= self.total_chunks {
            return Ok(false);
        }
        let idx = self.next_chunk;
        let data = loop {
            if let Some(data) = self.pending.remove(&idx) {
                break data;
            }
            let (i, result) = self
                .rx
                .recv()
                .map_err(|_| io::Error::other("Download workers exited unexpectedly."))?;
            self.pending.insert(i, result?);
        };
        self.next_chunk += 1;
        {
            let mut state = self.shared.state.lock().unwrap();
            state.1 = self.next_chunk;
            self.shared.changed.notify_all();
        }
        self.current = io::Cursor::new(data);
        Ok(true)
    }
}

impl ChunkedReader {
    fn read_inner(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if let Some(response) = self.first.as_mut() {
            let n = response.read(buf)?;
            if n > 0 {
                return Ok(n);
            }
            self.first = None;
        }
        loop {
            let n = self.current.read(buf)?;
            if n > 0 {
                return Ok(n);
            }
            self.current = io::Cursor::new(Vec::new());
            if !self.advance()? {
                return Ok(0);
            }
        }
    }
}

impl Read for ChunkedReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.failed {
            return Err(io::Error::other("Download failed."));
        }
        let result = self.read_inner(buf);
        if result.is_err() {
            self.failed = true;
            self.shared.cancel();
        }
        result
    }
}

impl Shared {
    fn cancel(&self) {
        // Hold the lock so cancellation cannot miss a worker entering wait.
        let _state = self.state.lock().unwrap();
        self.cancelled.store(true, Ordering::Relaxed);
        self.changed.notify_all();
    }
}

impl Drop for ChunkedReader {
    fn drop(&mut self) {
        self.shared.cancel();
    }
}

struct Worker {
    client: Client,
    url: String,
    etag: Option<String>,
    total: u64,
    chunk_size: u64,
    total_chunks: u64,
    shared: Arc<Shared>,
    tx: Sender<(u64, io::Result<Vec<u8>>)>,
}

impl Worker {
    fn run(self) {
        while let Some(idx) = self.claim() {
            let result = self.fetch_with_retry(idx);
            let failed = result.is_err();
            if self.tx.send((idx, result)).is_err() || failed {
                self.shared.cancel();
                return;
            }
        }
    }

    /// Claims the next chunk index, waiting while the in-flight window is
    /// full. Returns None when there is nothing left to do.
    fn claim(&self) -> Option<u64> {
        let mut state = self.shared.state.lock().unwrap();
        loop {
            if self.shared.cancelled.load(Ordering::Relaxed) || state.0 >= self.total_chunks {
                return None;
            }
            if state.0 - state.1 < WINDOW {
                let idx = state.0;
                state.0 += 1;
                return Some(idx);
            }
            state = self.shared.changed.wait(state).unwrap();
        }
    }

    fn fetch_with_retry(&self, idx: u64) -> io::Result<Vec<u8>> {
        let mut last_err = None;
        for attempt in 1..=ATTEMPTS {
            if self.shared.cancelled.load(Ordering::Relaxed) {
                break;
            }
            match self.fetch(idx) {
                Ok(data) => return Ok(data),
                Err(FetchError::Changed) => {
                    return Err(io::Error::other(
                        "The file changed on the server while it was being downloaded.",
                    ))
                }
                Err(FetchError::Transient(e)) => {
                    log::debug!(
                        "Attempt {} of {} for range {} of `{}` failed: {}",
                        attempt,
                        ATTEMPTS,
                        idx,
                        self.url,
                        e
                    );
                    last_err = Some(e);
                }
            }
            if self.shared.cancelled.load(Ordering::Relaxed) {
                break;
            }
        }
        Err(io::Error::other(
            last_err.unwrap_or_else(|| "Download cancelled.".to_string()),
        ))
    }

    fn fetch(&self, idx: u64) -> std::result::Result<Vec<u8>, FetchError> {
        let start = idx * self.chunk_size;
        let end = start.saturating_add(self.chunk_size).min(self.total) - 1;
        let expected = end - start + 1;

        let response = self
            .client
            .get(&self.url)
            .header(RANGE, format!("bytes={}-{}", start, end))
            .header(ACCEPT_ENCODING, "identity")
            .header(IF_MATCH, self.etag.as_deref().unwrap())
            // Detached workers must eventually exit after the reader is dropped.
            .timeout(Duration::from_secs(120))
            .send()
            .map_err(|e| FetchError::Transient(e.to_string()))?;

        if response.status() == StatusCode::PRECONDITION_FAILED {
            return Err(FetchError::Changed);
        }
        if response.status() != StatusCode::PARTIAL_CONTENT {
            return Err(FetchError::Transient(format!(
                "unexpected status {} for range request",
                response.status()
            )));
        }
        if response.url().as_str() != self.url {
            return Err(FetchError::Transient(
                "Range request redirected to a different URL.".into(),
            ));
        }
        if header_string(&response, ETAG) != self.etag {
            return Err(FetchError::Changed);
        }
        if header_string(&response, CONTENT_RANGE)
            .as_deref()
            .and_then(parse_content_range)
            != Some((start, end, Some(self.total)))
        {
            return Err(FetchError::Transient("Unexpected Content-Range.".into()));
        }

        let mut data = vec![0; expected as usize];
        let mut response = ExactReader::new(response, expected);
        response
            .read_exact(&mut data)
            .map_err(|e| FetchError::Transient(e.to_string()))?;
        response
            .read(&mut [0])
            .map_err(|e| FetchError::Transient(e.to_string()))?;
        Ok(data)
    }
}

enum FetchError {
    /// The server now serves different content; retrying cannot help.
    Changed,
    /// A network or protocol error worth retrying.
    Transient(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::sync::atomic::AtomicUsize;
    use tiny_http::{Header, Response as HttpResponse, Server};

    /// A local server that serves `body`, honouring `Range` when `ranges` is
    /// set, and counting the requests it receives.
    struct MockServer {
        addr: SocketAddr,
        requests: Arc<AtomicUsize>,
        etag: Arc<Mutex<String>>,
        server: Arc<Server>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl MockServer {
        fn spawn(body: Vec<u8>, ranges: bool) -> Self {
            let server = Arc::new(Server::http("127.0.0.1:0").unwrap());
            let addr = server.server_addr().to_ip().unwrap();
            let requests = Arc::new(AtomicUsize::new(0));
            let etag = Arc::new(Mutex::new("\"v1\"".to_string()));
            let body = Arc::new(body);
            let (reqs, tag) = (Arc::clone(&requests), Arc::clone(&etag));
            let serving = Arc::clone(&server);
            let thread = thread::spawn(move || {
                for request in serving.incoming_requests() {
                    reqs.fetch_add(1, Ordering::SeqCst);
                    let etag = tag.lock().unwrap().clone();
                    let range = request
                        .headers()
                        .iter()
                        .find(|h| h.field.equiv("Range"))
                        .map(|h| h.value.to_string());
                    let etag_header = Header::from_bytes(&b"ETag"[..], etag.as_bytes()).unwrap();
                    let response = match (ranges, range) {
                        (true, Some(range)) => {
                            let spec = range.strip_prefix("bytes=").unwrap();
                            let (s, e) = spec.split_once('-').unwrap();
                            let s: usize = s.parse().unwrap();
                            let e: usize = e.parse::<usize>().unwrap().min(body.len() - 1);
                            let data = body[s..=e].to_vec();
                            let cr = format!("bytes {}-{}/{}", s, e, body.len());
                            HttpResponse::from_data(data)
                                .with_status_code(206)
                                .with_header(
                                    Header::from_bytes(&b"Content-Range"[..], cr.as_bytes())
                                        .unwrap(),
                                )
                                .with_header(etag_header)
                        }
                        // tiny_http switches to chunked encoding above 32 KiB
                        // by default; keep Content-Length like a real server.
                        _ => HttpResponse::from_data(body.to_vec())
                            .with_chunked_threshold(usize::MAX)
                            .with_header(etag_header),
                    };
                    let _ = request.respond(response);
                }
            });
            Self {
                addr,
                requests,
                etag,
                server,
                thread: Some(thread),
            }
        }

        fn url(&self) -> String {
            format!("http://{}/file", self.addr)
        }
    }

    impl Drop for MockServer {
        fn drop(&mut self) {
            self.server.unblock();
            self.thread.take().unwrap().join().unwrap();
        }
    }

    fn body(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    fn client() -> Client {
        Client::builder().http1_only().build().unwrap()
    }

    #[test]
    fn parses_content_range() {
        assert_eq!(
            parse_content_range("bytes 0-7/100"),
            Some((0, 7, Some(100)))
        );
        assert_eq!(parse_content_range("bytes 10-19/*"), Some((10, 19, None)));
        assert_eq!(parse_content_range("bytes 5-4/10"), None);
        assert_eq!(parse_content_range("items 0-7/100"), None);
        for invalid in [
            "bytes 0-0/0",
            "bytes 0-10/10",
            "bytes 0-18446744073709551615/*",
            "bytes 0-1/no",
        ] {
            assert_eq!(parse_content_range(invalid), None);
        }
    }

    #[test]
    fn ranged_download_reassembles_in_order() {
        let data = body(100_000 + 17);
        let server = MockServer::spawn(data.clone(), true);
        let mut download = start_with(&client(), &server.url(), 1000, 4).unwrap();
        assert_eq!(download.content_length, Some(data.len() as u64));
        assert_eq!(download.etag.as_deref(), Some("\"v1\""));
        let mut got = Vec::new();
        download.read_to_end(&mut got).unwrap();
        assert_eq!(got, data);
        assert_eq!(server.requests.load(Ordering::SeqCst), 101);
    }

    #[test]
    fn small_body_uses_single_request() {
        let data = body(500);
        let server = MockServer::spawn(data.clone(), true);
        let mut download = start_with(&client(), &server.url(), 1000, 4).unwrap();
        let mut got = Vec::new();
        download.read_to_end(&mut got).unwrap();
        assert_eq!(got, data);
        assert_eq!(server.requests.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn falls_back_when_server_ignores_range() {
        let data = body(50_000);
        let server = MockServer::spawn(data.clone(), false);
        let mut download = start_with(&client(), &server.url(), 1000, 4).unwrap();
        assert_eq!(download.content_length, Some(data.len() as u64));
        let mut got = Vec::new();
        download.read_to_end(&mut got).unwrap();
        assert_eq!(got, data);
        assert_eq!(server.requests.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn detects_content_change_mid_download() {
        let data = body(50_000);
        let server = MockServer::spawn(data, true);
        let mut download = start_with(&client(), &server.url(), 1000, 2).unwrap();
        let mut first = vec![0u8; 1000];
        download.read_exact(&mut first).unwrap();
        *server.etag.lock().unwrap() = "\"v2\"".to_string();
        let mut rest = Vec::new();
        let err = download.read_to_end(&mut rest).unwrap_err();
        assert!(err.to_string().contains("changed on the server"), "{err}");
    }
    impl MockServer {
        fn scripted(handler: impl Fn(tiny_http::Request, usize) + Send + 'static) -> Self {
            let server = Arc::new(Server::http("127.0.0.1:0").unwrap());
            let addr = server.server_addr().to_ip().unwrap();
            let requests = Arc::new(AtomicUsize::new(0));
            let reqs = Arc::clone(&requests);
            let serving = Arc::clone(&server);
            let thread = thread::spawn(move || {
                for request in serving.incoming_requests() {
                    let i = reqs.fetch_add(1, Ordering::SeqCst);
                    handler(request, i);
                }
            });
            Self {
                addr,
                requests,
                etag: Arc::new(Mutex::new(String::new())),
                server,
                thread: Some(thread),
            }
        }
    }

    fn partial(data: &[u8], range: &str, etag: Option<&str>) -> HttpResponse<io::Cursor<Vec<u8>>> {
        let mut response = HttpResponse::from_data(data.to_vec())
            .with_status_code(206)
            .with_header(Header::from_bytes("Content-Range", range).unwrap());
        if let Some(etag) = etag {
            response = response.with_header(Header::from_bytes("ETag", etag).unwrap());
        }
        response
    }

    #[test]
    fn rejects_invalid_initial_ranges() {
        for range in [
            "bytes 1-4/8",
            "bytes 0-2/8",
            "bytes 0-4/8",
            "bytes 0-3/3",
            "bytes 0-18446744073709551615/*",
        ] {
            let server = MockServer::scripted(move |request, _| {
                request
                    .respond(partial(b"abcd", range, Some("\"v1\"")))
                    .unwrap();
            });
            assert!(
                start_with(&client(), &server.url(), 4, 1).is_err(),
                "{range}"
            );
        }
    }

    #[test]
    fn checks_worker_headers_and_body_lengths() {
        for (range, tag, body) in [
            ("bytes 0-3/8", Some("\"v1\""), &b"efgh"[..]),
            ("bytes 4-7/9", Some("\"v1\""), &b"efgh"[..]),
            ("bytes 4-7/8", None, &b"efgh"[..]),
            ("bytes 4-7/8", Some("\"v2\""), &b"efgh"[..]),
            ("bytes 4-7/8", Some("\"v1\""), &b"efg"[..]),
            ("bytes 4-7/8", Some("\"v1\""), &b"efghi"[..]),
        ] {
            let server = MockServer::scripted(move |request, i| {
                let response = if i == 0 {
                    partial(b"abcd", "bytes 0-3/8", Some("\"v1\""))
                } else {
                    partial(body, range, tag)
                };
                let _ = request.respond(response);
            });
            let mut download = start_with(&client(), &server.url(), 4, 1).unwrap();
            assert!(
                download.read_to_end(&mut Vec::new()).is_err(),
                "{range} {tag:?} {body:?}"
            );
            assert!(download.read(&mut [0]).is_err());
        }
    }

    #[test]
    fn checks_first_body_length_including_small_files() {
        for total in [4, 8] {
            for data in [&b"abc"[..], &b"abcde"[..]] {
                let server = MockServer::scripted(move |request, _| {
                    let _ = request.respond(partial(
                        data,
                        &format!("bytes 0-3/{total}"),
                        Some("\"v1\""),
                    ));
                });
                let mut download = start_with(&client(), &server.url(), 4, 1).unwrap();
                assert!(download.read_to_end(&mut Vec::new()).is_err());
            }
        }
    }

    #[test]
    fn restarts_without_ranges_when_identity_or_total_is_unknown() {
        for (range, etag) in [
            ("bytes 0-3/*", Some("\"v1\"")),
            ("bytes 0-3/8", None),
            ("bytes 0-3/8", Some("W/\"v1\"")),
        ] {
            let server = MockServer::scripted(move |request, i| {
                if i == 0 {
                    request.respond(partial(b"abcd", range, etag)).unwrap();
                } else {
                    assert!(!request.headers().iter().any(|h| h.field.equiv("Range")));
                    request
                        .respond(HttpResponse::from_data(b"abcdefgh".to_vec()))
                        .unwrap();
                }
            });
            let mut download = start_with(&client(), &server.url(), 4, 1).unwrap();
            let mut got = Vec::new();
            download.read_to_end(&mut got).unwrap();
            assert_eq!(got, b"abcdefgh");
            assert_eq!(server.requests.load(Ordering::SeqCst), 2);
        }
    }

    #[test]
    fn retries_transient_errors_with_a_validator() {
        let server = MockServer::scripted(|request, i| {
            let response = match i {
                0 => partial(b"abcd", "bytes 0-3/8", Some("\"v1\"")),
                1 => HttpResponse::from_data(Vec::new()).with_status_code(503),
                _ => {
                    assert!(request
                        .headers()
                        .iter()
                        .any(|h| h.field.equiv("If-Match") && h.value.as_str() == "\"v1\""));
                    partial(b"efgh", "bytes 4-7/8", Some("\"v1\""))
                }
            };
            request.respond(response).unwrap();
        });
        let mut download = start_with(&client(), &server.url(), 4, 1).unwrap();
        let mut got = Vec::new();
        download.read_to_end(&mut got).unwrap();
        assert_eq!(got, b"abcdefgh");
        assert_eq!(server.requests.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn range_rejection_restarts_but_requires_a_full_response() {
        for status in [200, 206] {
            let server = MockServer::scripted(move |request, i| {
                request
                    .respond(
                        HttpResponse::from_data(Vec::new()).with_status_code(if i == 0 {
                            416
                        } else {
                            status
                        }),
                    )
                    .unwrap();
            });
            assert_eq!(
                start_with(&client(), &server.url(), 4, 1).is_ok(),
                status == 200
            );
        }
    }

    #[test]
    fn resolved_redirect_url_is_used_for_workers() {
        let server = MockServer::scripted(|request, i| {
            let response = match i {
                0 => {
                    assert_eq!(request.url(), "/file");
                    HttpResponse::from_data(Vec::new())
                        .with_status_code(302)
                        .with_header(Header::from_bytes("Location", "/resolved").unwrap())
                }
                1 => partial(b"abcd", "bytes 0-3/8", Some("\"v1\"")),
                _ => {
                    assert_eq!(request.url(), "/resolved");
                    partial(b"efgh", "bytes 4-7/8", Some("\"v1\""))
                }
            };
            request.respond(response).unwrap();
        });
        let mut download = start_with(&client(), &server.url(), 4, 1).unwrap();
        let mut got = Vec::new();
        download.read_to_end(&mut got).unwrap();
        assert_eq!(got, b"abcdefgh");
    }

    #[test]
    fn cancellation_wakes_workers_with_a_full_window() {
        let shared = Arc::new(Shared {
            state: Mutex::new((WINDOW + 1, 1)),
            changed: Condvar::new(),
            cancelled: AtomicBool::new(false),
        });
        let (tx, _rx) = mpsc::channel();
        let worker = Worker {
            client: client(),
            url: String::new(),
            etag: None,
            total: 100,
            chunk_size: 1,
            total_chunks: 100,
            shared: Arc::clone(&shared),
            tx,
        };
        let (done_tx, done_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            assert_eq!(worker.claim(), None);
            done_tx.send(()).unwrap();
        });
        shared.cancel();
        done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        handle.join().unwrap();
    }
    #[test]
    fn dropping_reader_releases_workers_and_bounds_prefetch() {
        let server = MockServer::spawn(body(100_000), true);
        let client = client();
        let first = client
            .get(server.url())
            .header(RANGE, "bytes=0-999")
            .send()
            .unwrap();
        let reader =
            ChunkedReader::new(client, first, 1000, Some("\"v1\"".into()), 100_000, 1000, 4)
                .unwrap();
        let shared = Arc::clone(&reader.shared);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while server.requests.load(Ordering::SeqCst) < WINDOW as usize + 1 {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(shared.state.lock().unwrap().0, WINDOW + 1);
        drop(reader);
        while Arc::strong_count(&shared) != 1 {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(server.requests.load(Ordering::SeqCst), WINDOW as usize + 1);
    }

    #[test]
    fn buffers_out_of_order_results_and_supports_short_reads() {
        let (tx, rx) = mpsc::channel();
        tx.send((2, Ok(b"efgh".to_vec()))).unwrap();
        tx.send((1, Ok(b"abcd".to_vec()))).unwrap();
        let mut reader = ChunkedReader {
            first: None,
            rx,
            pending: BTreeMap::new(),
            next_chunk: 1,
            total_chunks: 3,
            current: io::Cursor::new(Vec::new()),
            failed: false,
            shared: Arc::new(Shared {
                state: Mutex::new((3, 1)),
                changed: Condvar::new(),
                cancelled: AtomicBool::new(false),
            }),
        };
        assert_eq!(reader.read(&mut []).unwrap(), 0);
        let mut got = Vec::new();
        let mut buf = [0; 3];
        loop {
            let n = reader.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
        }
        assert_eq!(got, b"abcdefgh");
    }
    #[test]
    fn workers_reject_full_responses_and_precondition_failures() {
        for status in [200, 412] {
            let server = MockServer::scripted(move |request, i| {
                let response = if i == 0 {
                    partial(b"abcd", "bytes 0-3/8", Some("\"v1\""))
                } else {
                    HttpResponse::from_data(b"abcdefgh".to_vec()).with_status_code(status)
                };
                request.respond(response).unwrap();
            });
            let mut download = start_with(&client(), &server.url(), 4, 1).unwrap();
            assert!(download.read_to_end(&mut Vec::new()).is_err());
            assert_eq!(
                server.requests.load(Ordering::SeqCst),
                if status == 412 { 2 } else { 4 }
            );
        }
    }

    #[test]
    fn workers_reject_a_changed_redirect_destination() {
        let server = MockServer::scripted(|request, i| {
            let response = if i == 0 {
                partial(b"abcd", "bytes 0-3/8", Some("\"v1\""))
            } else if request.url() == "/file" {
                HttpResponse::from_data(Vec::new())
                    .with_status_code(302)
                    .with_header(Header::from_bytes("Location", "/other").unwrap())
            } else {
                partial(b"efgh", "bytes 4-7/8", Some("\"v1\""))
            };
            request.respond(response).unwrap();
        });
        let mut download = start_with(&client(), &server.url(), 4, 1).unwrap();
        assert!(download.read_to_end(&mut Vec::new()).is_err());
    }
}
