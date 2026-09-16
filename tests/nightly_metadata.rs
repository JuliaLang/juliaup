use std::time::Duration;

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
