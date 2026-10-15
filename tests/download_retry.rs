//! Small metadata downloads retry transient failures.
#![cfg(not(windows))]
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

/// Serves one canned response per connection, in order, then exits.
/// Returns the server URL and a handle yielding the number of requests served.
fn serve(responses: Vec<&'static [u8]>) -> (String, std::thread::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut served = 0;
        for response in responses {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 4096];
            let _ = socket.read(&mut request);
            socket.write_all(response).unwrap();
            served += 1;
        }
        served
    });
    (format!("http://{address}/VERSION"), server)
}

const UNAVAILABLE: &[u8] =
    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
const NOT_FOUND: &[u8] =
    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\n1.2.3\n";

#[test]
fn juliaup_version_download_retries_server_errors() {
    let (url, server) = serve(vec![UNAVAILABLE, UNAVAILABLE, OK]);
    let version = juliaup::operations::download_juliaup_version(&url).unwrap();
    assert_eq!(version, semver::Version::new(1, 2, 3));
    assert_eq!(server.join().unwrap(), 3);
}

#[test]
fn juliaup_version_download_gives_up_after_bounded_retries() {
    let (url, server) = serve(vec![UNAVAILABLE; 4]);
    let err = juliaup::operations::download_juliaup_version(&url).unwrap_err();
    assert!(format!("{err:#}").contains("after 4 attempt(s)"), "{err:#}");
    assert_eq!(server.join().unwrap(), 4);
}

#[test]
fn juliaup_version_download_does_not_retry_client_errors() {
    let (url, server) = serve(vec![NOT_FOUND]);
    let err = juliaup::operations::download_juliaup_version(&url).unwrap_err();
    assert!(format!("{err:#}").contains("after 1 attempt(s)"), "{err:#}");
    assert_eq!(server.join().unwrap(), 1);
}
