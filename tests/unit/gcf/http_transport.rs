use super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Instant;

fn accept_request(listener: &TcpListener, expected_line: &str) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(2);
    listener.set_nonblocking(true).unwrap();
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "HTTP fixture did not receive a connection"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("HTTP fixture could not accept a connection: {error}"),
        }
    };
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut request = Vec::new();
    while !request.ends_with(b"\r\n\r\n") {
        assert!(
            request.len() < 4096,
            "HTTP fixture request exceeds its bound"
        );
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .expect("HTTP fixture did not receive a complete request");
        stream.set_read_timeout(Some(remaining)).unwrap();
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        request.push(byte[0]);
    }
    assert_eq!(
        request.split(|byte| *byte == b'\n').next().unwrap(),
        expected_line.as_bytes()
    );
    stream
}

fn execute_local_response(response_bytes: &'static [u8]) -> Result<TransportResponse, EmuError> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut stream = accept_request(&listener, "GET /probe HTTP/1.1\r");
        stream.write_all(response_bytes).unwrap();
    });
    let url = format!("http://fixture.invalid:{}/probe", address.port());
    let response = UreqTransport.execute(
        &TransportRequest {
            url,
            method: "GET".to_owned(),
            headers: vec![],
            body: vec![],
            connect_timeout: Duration::from_secs(2),
            read_timeout: Duration::from_secs(2),
            total_timeout: Duration::from_secs(4),
            addresses: vec![address],
        },
        16,
    );
    server.join().unwrap();
    response
}

#[test]
fn resolver_budget_is_shared_across_runtime_instances() {
    let first = SystemResolver::default();
    let second = SystemResolver::default();
    assert!(std::sync::Arc::ptr_eq(&first.in_flight, &second.in_flight));
}

#[test]
fn transport_bounds_the_entire_response_even_when_bytes_keep_arriving() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut socket = accept_request(&listener, "GET / HTTP/1.1\r");
        let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n");
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(40));
            if socket.write_all(b"x").is_err() {
                break;
            }
        }
    });
    let result = UreqTransport.execute(
        &TransportRequest {
            url: format!("http://{address}/"),
            method: "GET".to_owned(),
            headers: Vec::new(),
            body: Vec::new(),
            connect_timeout: Duration::from_millis(100),
            read_timeout: Duration::from_millis(100),
            total_timeout: Duration::from_millis(200),
            addresses: vec![address],
        },
        100,
    );
    server.join().unwrap();
    assert_eq!(result.unwrap_err().code(), "network-io");
}

#[test]
fn local_mock_http_server_exercises_production_transport() {
    let response = execute_local_response(
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nSet-Cookie: first=1\r\nX-Test: local\r\nsEt-CoOkIe: second=2\r\nX-Test: later\r\n\r\nready",
    )
    .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"ready");
    assert_eq!(
        response.headers,
        [
            ("content-length", "5"),
            ("set-cookie", "first=1"),
            ("x-test", "local"),
            ("set-cookie", "second=2"),
            ("x-test", "later"),
        ]
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
    );
}

#[test]
fn unreadable_response_headers_are_reported_instead_of_silently_dropped() {
    assert_eq!(
        execute_local_response(
            b"HTTP/1.1 200 OK\r\nX-Test: first\r\nX-Test: \xff\r\nContent-Length: 0\r\n\r\n"
        )
        .unwrap_err()
        .code(),
        "network-header"
    );
}

#[test]
fn system_resolver_caps_unfinished_operations() {
    let mut resolver = SystemResolver {
        in_flight: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(4)),
        max_in_flight: 4,
    };
    assert_eq!(
        resolver
            .resolve("example.test", 80, Duration::from_millis(1))
            .unwrap_err()
            .code(),
        "network-limit"
    );
}
