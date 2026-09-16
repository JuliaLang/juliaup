//! Metadata deadlines apply to the response body, including on Windows.
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

#[test]
fn metadata_deadline_cancels_a_stalled_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        // A slow CI host may time out the client before it even connects.
        // Bound server cleanup too, rather than hanging in accept/join.
        let accept_deadline = Instant::now() + Duration::from_secs(2);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= accept_deadline {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => panic!("accept failed: {e}"),
            }
        };
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0; 4096];
        let _ = socket.read(&mut request);
        let _ = socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{");
        std::thread::sleep(Duration::from_millis(500));
    });
    let start = Instant::now();
    let result = juliaup::operations::download_text(
        &format!("http://{address}/db.json"),
        start + Duration::from_millis(100),
    );
    assert!(result.is_err());
    assert!(
        start.elapsed() < Duration::from_millis(450),
        "body read ignored deadline"
    );
    server.join().unwrap();
}

#[test]
fn regular_archives_can_be_installed_without_etags() {
    use flate2::{write::GzEncoder, Compression};
    let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    let bytes = b"test content";
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "julia/file", &bytes[..])
        .unwrap();
    let data = archive.into_inner().unwrap().finish().unwrap();
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}/archive.tar.gz", server.server_addr());
    let worker = std::thread::spawn(move || {
        let request = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        request
            .respond(tiny_http::Response::from_data(data))
            .unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        juliaup::operations::download_extract_sans_parent(&url, dir.path(), 1).unwrap(),
        ""
    );
    assert_eq!(std::fs::read(dir.path().join("file")).unwrap(), bytes);
    worker.join().unwrap();
}
