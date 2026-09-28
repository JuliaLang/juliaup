//! Downloads that fetch a file as concurrent byte ranges, and pick up where
//! they stopped after a transfer error.

use anyhow::{ensure, Context, Result};
use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_RANGE, ETAG, IF_MATCH, RANGE};
use reqwest::StatusCode;
use std::io::{self, Read};
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Range requests in flight, next to the streamed first chunk. A single
/// connection to the Julia CDN tops out at about half of what four reach.
const WORKERS: u64 = 4;

/// Balance request overhead against buffering.
#[cfg(not(test))]
const CHUNK_SIZE: u64 = 4 * 1024 * 1024;
#[cfg(test)]
const CHUNK_SIZE: u64 = 4;

/// Attempts to resume without receiving any data before the download fails.
const ATTEMPTS: u32 = 3;

/// Pause before resuming again, multiplied by the attempts so far. The first
/// attempt is immediate: falling back from ranges is expected, not an error.
#[cfg(not(test))]
const RETRY_DELAY: Duration = Duration::from_secs(1);
#[cfg(test)]
const RETRY_DELAY: Duration = Duration::ZERO;

/// A streaming download of a URL's body.
pub struct Download {
    /// Total body length, when the server reported one.
    pub content_length: Option<u64>,
    /// The `ETag` header of the response, when present.
    pub etag: Option<String>,
    pub body: Box<dyn Read + Send>,
}

/// Starts downloading `url`.
///
/// If the server answers the first chunk with a range, the rest is fetched as
/// concurrent range requests, pinned by `If-Match` to the ETag of that first
/// response. After a transfer error, including a read that times out, the
/// rest of the file is requested in one piece from the first missing byte.
/// Both need the total length and a strong ETag; without them the body is
/// streamed as is.
pub fn get(client: &Client, url: &str) -> Result<Download> {
    let context = || format!("Failed to download from url `{url}`.");
    let first = client
        .get(url)
        .header(RANGE, format!("bytes=0-{}", CHUNK_SIZE - 1))
        .send()
        .with_context(context)?;
    let response = match first.status() {
        StatusCode::PARTIAL_CONTENT => {
            match first_chunk_total(&first).and_then(|total| Source::new(client, &first, total)) {
                Some(source) => {
                    return Ok(Download {
                        content_length: Some(source.total),
                        etag: Some(source.etag.clone()),
                        body: stream(source, first, WORKERS),
                    })
                }
                // Chunks of an unknown length or version cannot be put together.
                None => client.get(url).send().with_context(context)?,
            }
        }
        _ => first,
    };
    ensure!(
        response.status() == StatusCode::OK,
        "Failed to download from url `{url}` (status: {}).",
        response.status()
    );
    let content_length = response.content_length();
    let etag = header(&response, ETAG).map(str::to_owned);
    let body = match content_length.and_then(|total| Source::new(client, &response, total)) {
        Some(source) => stream(source, response, 0),
        None => Box::new(response),
    };
    Ok(Download {
        content_length,
        etag,
        body,
    })
}

/// The total length of the file, if `response` holds exactly its first chunk.
fn first_chunk_total(response: &Response) -> Option<u64> {
    let range = header(response, CONTENT_RANGE)?;
    let total = range.rsplit_once('/')?.1.parse().ok()?;
    (total > 0 && range == format!("bytes 0-{}/{total}", CHUNK_SIZE.min(total) - 1))
        .then_some(total)
}

fn header(response: &Response, name: reqwest::header::HeaderName) -> Option<&str> {
    response.headers().get(name).and_then(|v| v.to_str().ok())
}

/// Streams `first`, the start of `source`, while `workers` threads fetch the
/// chunks after it. Worker `w` fetches chunks `w + 1`, `w + 1 + workers`, ...
/// into its own queue, so taking from the queues in turn yields them in order.
fn stream(source: Source, first: Response, workers: u64) -> Box<dyn Read + Send> {
    let source = Arc::new(source);
    let first_len = match workers {
        0 => source.total,
        _ => CHUNK_SIZE.min(source.total),
    };
    let queues = (0..workers)
        .map(|w| {
            // Hand chunks over directly, so that each worker holds at most one.
            let (tx, rx) = sync_channel(0);
            let source = Arc::clone(&source);
            let starts =
                (CHUNK_SIZE * (w + 1)..source.total).step_by((CHUNK_SIZE * workers) as usize);
            thread::spawn(move || {
                for start in starts {
                    let result = source.fetch(start, (start + CHUNK_SIZE).min(source.total) - 1);
                    let failed = result.is_err();
                    // Stop once the reader is gone, or after a failure: the
                    // reader takes over from there.
                    if tx.send(result).is_err() || failed {
                        break;
                    }
                }
            });
            rx
        })
        .collect();
    Box::new(Body {
        source,
        pos: 0,
        failures: 0,
        body: Some(first.take(first_len)),
        queues,
        chunk: io::Cursor::default(),
    })
}

/// Whether `etag` is one strong entity-tag, not a weak tag or a list.
fn is_strong(etag: &str) -> bool {
    etag.strip_prefix('"')
        .and_then(|tag| tag.strip_suffix('"'))
        .is_some_and(|tag| tag.bytes().all(|b| b == 0x21 || b >= 0x23 && b != 0x7f))
}

/// The file that range requests are made against.
struct Source {
    client: Client,
    /// The URL after redirects, which range requests go to directly.
    url: String,
    etag: String,
    total: u64,
}

impl Source {
    /// Pins range requests for a file of `total` bytes to the URL and ETag of
    /// `response`, if the ETag is strong.
    fn new(client: &Client, response: &Response, total: u64) -> Option<Self> {
        let etag = header(response, ETAG).filter(|tag| is_strong(tag))?;
        (total > 0).then(|| Source {
            client: client.clone(),
            url: response.url().to_string(),
            etag: etag.to_owned(),
            total,
        })
    }

    /// Requests bytes `start..=end`, which must come from the same file.
    fn request(&self, start: u64, end: u64) -> io::Result<io::Take<Response>> {
        let response = self
            .client
            .get(&self.url)
            .header(RANGE, format!("bytes={start}-{end}"))
            .header(IF_MATCH, &self.etag)
            .send()
            .map_err(io::Error::other)?;
        let status = response.status();
        if status == StatusCode::PRECONDITION_FAILED
            || status == StatusCode::PARTIAL_CONTENT
                && header(&response, ETAG) != Some(self.etag.as_str())
        {
            return Err(io::Error::other(
                "The file changed on the server while it was being downloaded.",
            ));
        }
        let range = format!("bytes {start}-{end}/{}", self.total);
        if status != StatusCode::PARTIAL_CONTENT
            || response.url().as_str() != self.url
            || header(&response, CONTENT_RANGE) != Some(range.as_str())
        {
            return Err(io::Error::other(format!(
                "Unexpected response {status} from `{}` for `{range}`.",
                response.url()
            )));
        }
        Ok(response.take(end - start + 1))
    }

    fn fetch(&self, start: u64, end: u64) -> io::Result<Vec<u8>> {
        let mut data = vec![0; (end - start + 1) as usize];
        self.request(start, end)?.read_exact(&mut data)?;
        Ok(data)
    }
}

struct Body {
    source: Arc<Source>,
    /// Bytes delivered so far.
    pos: u64,
    /// Failed attempts since data last arrived.
    failures: u32,
    /// The streamed response: the first chunk, the whole file, or after a
    /// failure the rest of it, requested when next read.
    body: Option<io::Take<Response>>,
    /// The workers' queues, emptied when the download resumes in one request.
    queues: Vec<Receiver<io::Result<Vec<u8>>>>,
    chunk: io::Cursor<Vec<u8>>,
}

impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            let err = match self.read_some(buf) {
                Ok(n) => {
                    self.pos += n as u64;
                    if n > 0 {
                        self.failures = 0;
                    }
                    return Ok(n);
                }
                Err(err) => err,
            };
            self.failures += 1;
            if self.failures > ATTEMPTS {
                return Err(err);
            }
            log::debug!("Resuming `{}` at byte {}: {err}", self.source.url, self.pos);
            thread::sleep(RETRY_DELAY * (self.failures - 1));
            // Continue in one request, since the server may limit connections,
            // and let the workers stop instead of holding their chunks.
            self.queues.clear();
            self.body = None;
        }
    }
}

impl Body {
    fn read_some(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let n = self.chunk.read(buf)?;
            if n > 0 {
                return Ok(n);
            }
            let body = match &mut self.body {
                Some(body) => body,
                body => body.insert(self.source.request(self.pos, self.source.total - 1)?),
            };
            let n = body.read(buf)?;
            if n > 0 || self.pos == self.source.total {
                return Ok(n);
            }
            if body.limit() > 0 || self.queues.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "The download ended early.",
                ));
            }
            let chunk = self.queues[0]
                .recv()
                .map_err(|_| io::Error::other("A download worker exited unexpectedly."))??;
            self.chunk = io::Cursor::new(chunk);
            self.queues.rotate_left(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    const BODY: &[u8] = b"0123456789";
    /// Seven chunks: the first, and the next six striped over four workers.
    const LONG: &[u8] = b"abcdefghijklmnopqrstuvwxyz";

    /// A server that answers each request on its own connection and thread,
    /// then closes it. Records the request heads, lowercased.
    struct Server {
        url: String,
        requests: Arc<Mutex<Vec<String>>>,
    }

    impl Server {
        fn new(respond: impl Fn(usize, &str, &mut TcpStream) + Send + Sync + 'static) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/julia.tar.gz", listener.local_addr().unwrap());
            let requests = Arc::new(Mutex::new(Vec::new()));
            let (seen, respond) = (Arc::clone(&requests), Arc::new(respond));
            thread::spawn(move || {
                for mut stream in listener.incoming().map(Result::unwrap) {
                    let mut head = String::new();
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    while reader.read_line(&mut head).unwrap() > 2 {}
                    let head = head.to_lowercase();
                    let i = {
                        let mut seen = seen.lock().unwrap();
                        seen.push(head.clone());
                        seen.len() - 1
                    };
                    let respond = Arc::clone(&respond);
                    thread::spawn(move || respond(i, &head, &mut stream));
                }
            });
            Self { url, requests }
        }

        fn get(&self, client: &Client) -> Box<dyn Read + Send> {
            get(client, &self.url).unwrap().body
        }

        fn requests(&self) -> Vec<String> {
            self.requests.lock().unwrap().clone()
        }
    }

    /// Writes a response head declaring `len` bytes, then `body`.
    fn reply(stream: &mut TcpStream, status: &str, headers: &str, len: usize, body: &[u8]) {
        let head = format!(
            "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {len}\r\n{headers}\r\n"
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(body);
    }

    /// The first response, from a server that ignores ranges, which declares
    /// the whole body but sends `sent` bytes.
    fn first(stream: &mut TcpStream, sent: usize) {
        reply(stream, "200 OK", "ETag: \"v1\"\r\n", 10, &BODY[..sent]);
    }

    /// A correct partial response from `start`, which sends `sent` bytes.
    fn rest(stream: &mut TcpStream, start: usize, sent: usize) {
        let headers = format!("ETag: \"v1\"\r\nContent-Range: bytes {start}-9/10\r\n");
        let body = &BODY[start..start + sent];
        reply(stream, "206 Partial Content", &headers, 10 - start, body);
    }

    /// Answers like a server with ranges, for `LONG` with ETag "v1".
    fn serve(head: &str, stream: &mut TcpStream) {
        let Some(range) = head.split("range: bytes=").nth(1) else {
            return reply(stream, "200 OK", "ETag: \"v1\"\r\n", LONG.len(), LONG);
        };
        let (start, end) = range.split_once("\r\n").unwrap().0.split_once('-').unwrap();
        let start: usize = start.parse().unwrap();
        let end = end.parse::<usize>().unwrap().min(LONG.len() - 1);
        let headers = format!("ETag: \"v1\"\r\nContent-Range: bytes {start}-{end}/26\r\n");
        reply(
            stream,
            "206 Partial Content",
            &headers,
            end + 1 - start,
            &LONG[start..=end],
        );
    }

    #[test]
    fn downloads_in_concurrent_ranges() {
        let server = Server::new(|_, head, stream| serve(head, stream));
        let mut download = get(&Client::new(), &server.url).unwrap();
        assert_eq!(download.content_length, Some(26));
        assert_eq!(download.etag.as_deref(), Some("\"v1\""));
        let mut got = Vec::new();
        download.body.read_to_end(&mut got).unwrap();
        assert_eq!(got, LONG);
        let mut ranges: Vec<_> = server.requests()[1..]
            .iter()
            .map(|request| {
                assert!(request.contains("if-match: \"v1\"\r\n"), "{request}");
                request
                    .split("range: bytes=")
                    .nth(1)
                    .unwrap()
                    .split_once('\r')
                    .unwrap()
                    .0
                    .to_owned()
            })
            .collect();
        ranges.sort_by_key(|range| range.split('-').next().unwrap().parse::<u32>().unwrap());
        assert_eq!(ranges, ["4-7", "8-11", "12-15", "16-19", "20-23", "24-25"]);
    }

    #[test]
    fn workers_fetch_at_most_one_chunk_ahead() {
        let server = Server::new(|_, head, stream| serve(head, stream));
        let mut body = server.get(&Client::new());
        body.read_exact(&mut [0; 4]).unwrap();
        thread::sleep(Duration::from_millis(200));
        assert_eq!(server.requests().len(), 1 + WORKERS as usize);
        drop(body);
        thread::sleep(Duration::from_millis(100));
        assert_eq!(server.requests().len(), 1 + WORKERS as usize);
    }

    #[test]
    fn gets_the_whole_file_when_ranges_are_unusable() {
        for headers in [
            "ETag: W/\"v1\"\r\nContent-Range: bytes 0-3/26\r\n",
            "ETag: \"v1\"\r\nContent-Range: bytes 0-3/*\r\n",
            "ETag: \"v1\"\r\nContent-Range: bytes 0-1/26\r\n",
        ] {
            let server = Server::new(move |i, head, stream| match i {
                0 => reply(stream, "206 Partial Content", headers, 4, &LONG[..4]),
                _ => {
                    assert!(!head.contains("range:"), "{head}");
                    serve(head, stream);
                }
            });
            let mut got = Vec::new();
            server.get(&Client::new()).read_to_end(&mut got).unwrap();
            assert_eq!(got, LONG, "{headers}");
            assert_eq!(server.requests().len(), 2);
        }
    }

    #[test]
    fn resumes_in_one_request_when_a_range_fails() {
        let wrong_range = "ETag: \"v1\"\r\nContent-Range: bytes 4-7/27\r\n";
        for (status, headers, sent) in [
            ("503 Service Unavailable", "", 0),
            (
                "206 Partial Content",
                "ETag: \"v1\"\r\nContent-Range: bytes 4-7/26\r\n",
                2,
            ),
            ("206 Partial Content", wrong_range, 4),
            ("200 OK", "ETag: \"v1\"\r\n", 4),
        ] {
            let server = Server::new(move |_, head, stream| {
                if head.contains("range: bytes=4-7\r\n") {
                    reply(stream, status, headers, 4, &LONG[4..4 + sent]);
                } else {
                    serve(head, stream);
                }
            });
            let mut got = Vec::new();
            server.get(&Client::new()).read_to_end(&mut got).unwrap();
            assert_eq!(got, LONG, "{status} {headers}");
            let requests = server.requests();
            assert!(requests.iter().any(|r| r.contains("range: bytes=4-25\r\n")));
        }
    }

    #[test]
    fn resumes_when_the_first_chunk_ends_early() {
        // Without Content-Length, only the range says where the first chunk ends.
        let server = Server::new(|i, head, stream| match i {
            0 => {
                let head = "HTTP/1.1 206 Partial Content\r\nConnection: close\r\n\
                    ETag: \"v1\"\r\nContent-Range: bytes 0-3/26\r\n\r\nab";
                let _ = stream.write_all(head.as_bytes());
            }
            _ => serve(head, stream),
        });
        let mut got = Vec::new();
        server.get(&Client::new()).read_to_end(&mut got).unwrap();
        assert_eq!(got, LONG);
        let requests = server.requests();
        assert!(requests.iter().any(|r| r.contains("range: bytes=2-25\r\n")));
    }

    #[test]
    fn fails_when_a_range_finds_the_file_changed() {
        // From byte 8 on, If-Match no longer matches, for the range and when resuming.
        let server = Server::new(|_, head, stream| {
            if head.contains("range: bytes=8-") {
                reply(stream, "412 Precondition Failed", "", 0, b"");
            } else {
                serve(head, stream);
            }
        });
        let mut got = Vec::new();
        let err = server
            .get(&Client::new())
            .read_to_end(&mut got)
            .unwrap_err();
        assert!(err.to_string().contains("changed"), "{err}");
        assert_eq!(got, &LONG[..8]);
    }

    #[test]
    fn keeps_resuming_while_data_arrives() {
        // Every connection drops after one byte, more times than ATTEMPTS.
        let server = Server::new(|i, _, stream| match i {
            0 => first(stream, 1),
            _ => rest(stream, i, 1),
        });
        let mut got = Vec::new();
        server.get(&Client::new()).read_to_end(&mut got).unwrap();
        assert_eq!(got, BODY);
        let requests = server.requests();
        assert_eq!(requests.len(), 10);
        for (i, request) in requests.iter().enumerate().skip(1) {
            assert!(
                request.contains(&format!("range: bytes={i}-9\r\n")),
                "{request}"
            );
            assert!(request.contains("if-match: \"v1\"\r\n"), "{request}");
        }
    }

    #[test]
    fn resumes_after_a_stall() {
        let server = Server::new(|i, _, stream| match i {
            0 => {
                first(stream, 4);
                thread::sleep(Duration::from_secs(2));
            }
            _ => rest(stream, 4, 6),
        });
        let client = Client::builder()
            .timeout(Duration::from_millis(500))
            .build()
            .unwrap();
        let mut got = Vec::new();
        server.get(&client).read_to_end(&mut got).unwrap();
        assert_eq!(got, BODY);
        assert_eq!(server.requests().len(), 2);
    }

    #[test]
    fn fails_when_the_file_changed() {
        for changed in ["412 Precondition Failed", "206 Partial Content"] {
            let server = Server::new(move |i, _, stream| match i {
                0 => first(stream, 4),
                _ => reply(
                    stream,
                    changed,
                    "ETag: \"v2\"\r\nContent-Range: bytes 4-9/10\r\n",
                    6,
                    &BODY[4..],
                ),
            });
            let err = server
                .get(&Client::new())
                .read_to_end(&mut Vec::new())
                .unwrap_err();
            assert!(err.to_string().contains("changed"), "{changed}: {err}");
            assert_eq!(server.requests().len(), 1 + ATTEMPTS as usize);
        }
    }

    #[test]
    fn gives_up_after_failed_attempts() {
        let wrong_range = "ETag: \"v1\"\r\nContent-Range: bytes 0-9/10\r\n";
        for (status, headers) in [
            ("503 Service Unavailable", ""),
            ("200 OK", "ETag: \"v1\"\r\n"),
            ("206 Partial Content", wrong_range),
        ] {
            let server = Server::new(move |i, _, stream| match i {
                0 => first(stream, 4),
                _ => reply(stream, status, headers, 0, b""),
            });
            let err = server
                .get(&Client::new())
                .read_to_end(&mut Vec::new())
                .unwrap_err();
            assert!(
                err.to_string().contains("Unexpected response"),
                "{status}: {err}"
            );
            assert_eq!(server.requests().len(), 1 + ATTEMPTS as usize);
        }
    }

    #[test]
    fn resumes_at_the_redirected_url_only() {
        // The download is redirected once; resuming must go to where it ended up,
        // and a resumed request that is redirected elsewhere does not count.
        for redirect_again in [false, true] {
            let server = Server::new(move |i, head, stream| {
                let path = head.split(' ').nth(1).unwrap();
                match (i, path) {
                    (0, "/julia.tar.gz") => reply(stream, "302 Found", "Location: /a\r\n", 0, b""),
                    (1, "/a") => first(stream, 4),
                    (_, "/a") if redirect_again => {
                        reply(stream, "302 Found", "Location: /b\r\n", 0, b"")
                    }
                    (_, "/a" | "/b") => rest(stream, 4, 6),
                    _ => panic!("unexpected request {head}"),
                }
            });
            let mut got = Vec::new();
            let result = server.get(&Client::new()).read_to_end(&mut got);
            assert_eq!(result.is_ok(), !redirect_again, "{result:?}");
            if !redirect_again {
                assert_eq!(got, BODY);
            }
        }
    }

    #[test]
    fn does_not_resume_without_a_strong_etag() {
        for headers in [
            "",
            "ETag: W/\"v1\"\r\n",
            "ETag: \"v1\", \"v2\"\r\n",
            "ETag: \"v 1\"\r\n",
        ] {
            let server = Server::new(move |_, _, stream| {
                reply(stream, "200 OK", headers, 10, &BODY[..4]);
            });
            let mut body = server.get(&Client::new());
            assert!(body.read_to_end(&mut Vec::new()).is_err(), "{headers}");
            assert_eq!(server.requests().len(), 1);
        }
    }
}
