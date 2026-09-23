//! Downloads that pick up where they stopped after a transfer error.

use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_RANGE, ETAG, IF_MATCH, RANGE};
use reqwest::StatusCode;
use std::io::{self, Read};
use std::thread;
use std::time::Duration;

/// Attempts to resume without receiving any data before the download fails.
const ATTEMPTS: u32 = 3;

/// Pause before resuming, multiplied by the attempt number.
#[cfg(not(test))]
const RETRY_DELAY: Duration = Duration::from_secs(1);
#[cfg(test)]
const RETRY_DELAY: Duration = Duration::ZERO;

/// Wraps the body of `response` so that a transfer error continues the
/// download with a `Range` request from the first missing byte. The blocking
/// client times out each read after 30 s by default, so this also covers a
/// server that stops sending.
///
/// Resuming needs the total length and a strong ETag, which `If-Match` pins so
/// that the rest cannot come from a different file. Without them, or for a
/// response other than `200 OK`, the body is returned as is.
pub fn resumable(client: &Client, response: Response) -> Box<dyn Read + Send> {
    let etag = header(&response, ETAG).filter(|tag| is_strong(tag));
    match (response.status(), response.content_length(), etag) {
        (StatusCode::OK, Some(total), Some(etag)) if total > 0 => Box::new(Resumable {
            client: client.clone(),
            url: response.url().to_string(),
            etag: etag.to_owned(),
            total,
            pos: 0,
            failures: 0,
            body: response.take(total),
        }),
        _ => Box::new(response),
    }
}

fn header(response: &Response, name: reqwest::header::HeaderName) -> Option<&str> {
    response.headers().get(name).and_then(|v| v.to_str().ok())
}

/// Whether `etag` is one strong entity-tag, not a weak tag or a list.
fn is_strong(etag: &str) -> bool {
    etag.strip_prefix('"')
        .and_then(|tag| tag.strip_suffix('"'))
        .is_some_and(|tag| tag.bytes().all(|b| b == 0x21 || b >= 0x23 && b != 0x7f))
}

struct Resumable {
    client: Client,
    /// The URL after redirects, which resumed requests go to directly.
    url: String,
    etag: String,
    total: u64,
    /// Bytes delivered so far.
    pos: u64,
    /// Failed attempts since data last arrived.
    failures: u32,
    body: io::Take<Response>,
}

impl Read for Resumable {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            let err = match self.body.read(buf) {
                Ok(0) if self.pos < self.total => {
                    io::Error::new(io::ErrorKind::UnexpectedEof, "The download ended early.")
                }
                Ok(n) => {
                    self.pos += n as u64;
                    if n > 0 {
                        self.failures = 0;
                    }
                    return Ok(n);
                }
                Err(e) => e,
            };
            self.resume(err)?;
        }
    }
}

impl Resumable {
    /// Replaces the failed body with a request for the rest of the file, or
    /// returns the error that ends the download.
    fn resume(&mut self, mut err: io::Error) -> io::Result<()> {
        loop {
            self.failures += 1;
            if self.failures > ATTEMPTS {
                return Err(err);
            }
            log::debug!("Resuming `{}` at byte {}: {err}", self.url, self.pos);
            thread::sleep(RETRY_DELAY * self.failures);
            match self.request_rest() {
                Ok(body) => {
                    self.body = body;
                    return Ok(());
                }
                Err(Resume::Changed) => {
                    return Err(io::Error::other(
                        "The file changed on the server while it was being downloaded.",
                    ))
                }
                Err(Resume::Failed(e)) => err = e,
            }
        }
    }

    fn request_rest(&self) -> Result<io::Take<Response>, Resume> {
        let last = self.total - 1;
        let response = self
            .client
            .get(&self.url)
            .header(RANGE, format!("bytes={}-{last}", self.pos))
            .header(IF_MATCH, &self.etag)
            .send()
            .map_err(|e| Resume::Failed(io::Error::other(e)))?;
        let status = response.status();
        if status == StatusCode::PRECONDITION_FAILED
            || status == StatusCode::PARTIAL_CONTENT
                && header(&response, ETAG) != Some(self.etag.as_str())
        {
            return Err(Resume::Changed);
        }
        let range = format!("bytes {}-{last}/{}", self.pos, self.total);
        if status != StatusCode::PARTIAL_CONTENT
            || response.url().as_str() != self.url
            || header(&response, CONTENT_RANGE) != Some(range.as_str())
        {
            return Err(Resume::Failed(io::Error::other(format!(
                "Unexpected response {status} from `{}` when resuming at `{range}`.",
                response.url()
            ))));
        }
        Ok(response.take(self.total - self.pos))
    }
}

enum Resume {
    /// The server now serves different content; retrying cannot help.
    Changed,
    Failed(io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    const BODY: &[u8] = b"0123456789";

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
            resumable(client, client.get(&self.url).send().unwrap())
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

    /// The first response, which declares the whole body but sends `sent` bytes.
    fn first(stream: &mut TcpStream, sent: usize) {
        reply(stream, "200 OK", "ETag: \"v1\"\r\n", 10, &BODY[..sent]);
    }

    /// A correct partial response from `start`, which sends `sent` bytes.
    fn rest(stream: &mut TcpStream, start: usize, sent: usize) {
        let headers = format!("ETag: \"v1\"\r\nContent-Range: bytes {start}-9/10\r\n");
        let body = &BODY[start..start + sent];
        reply(stream, "206 Partial Content", &headers, 10 - start, body);
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
    fn fails_at_once_when_the_file_changed() {
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
            assert_eq!(server.requests().len(), 2);
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
            assert!(err.to_string().contains("when resuming"), "{status}: {err}");
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
