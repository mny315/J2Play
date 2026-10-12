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

fn request_for(address: std::net::SocketAddr) -> TransportRequest {
    let limits = Limits::default();
    TransportRequest {
        url: format!("http://fixture.invalid:{}/probe", address.port()),
        method: "GET".into(),
        headers: vec![],
        body: vec![],
        connect_timeout: limits.connect_timeout,
        read_timeout: limits.read_timeout,
        total_timeout: limits.connect_timeout + limits.read_timeout,
        addresses: vec![address],
    }
}

#[test]
fn delayed_headers_and_chunked_body_can_exceed_two_seconds() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut stream = accept_request(&listener, "GET /probe HTTP/1.1\r");
        std::thread::sleep(Duration::from_millis(1100));
        stream
            .write_all(
                b"HTTP/1.1 200 Fixture Ready\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nok\r\n",
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        stream.write_all(b"3\r\nYES\r\n0\r\n\r\n").unwrap();
    });
    let result = UreqTransport.execute(&request_for(address), 16);
    server.join().unwrap();
    let response = result.unwrap();
    assert_eq!(response.body, b"okYES");
    assert_eq!(response.reason, "Fixture Ready");
}

#[test]
fn cancellation_closes_connections_during_headers_body_and_tls_handshake() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    for stage in ["headers", "body", "tls"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let server = std::thread::spawn(move || {
            let mut stream = if stage == "tls" {
                listener.set_nonblocking(true).unwrap();
                let deadline = Instant::now() + Duration::from_secs(2);
                loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline);
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("{error}"),
                    }
                }
            } else {
                accept_request(&listener, "GET /probe HTTP/1.1\r")
            };
            if stage == "body" {
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nx")
                    .unwrap();
            }
            std::thread::sleep(Duration::from_millis(100));
            let started = Instant::now();
            flag.store(true, Ordering::Release);
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = [0; 4096];
            loop {
                match stream.read(&mut bytes) {
                    Ok(0) => break,
                    Ok(_) => assert!(started.elapsed() < Duration::from_secs(2)),
                    Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => break,
                    Err(error) => panic!("cancelled socket remained open: {error}"),
                }
            }
            started.elapsed()
        });
        let cancellation = Cancellation::new(move || cancelled.load(Ordering::Acquire));
        let mut request = request_for(address);
        if stage == "tls" {
            request.url = request.url.replacen("http:", "https:", 1);
        }
        let result = UreqTransport.execute_cancellable(&request, 128, &cancellation);
        let elapsed = server.join().unwrap();
        assert_eq!(result.unwrap_err().code(), "network-io", "{stage}");
        assert!(elapsed < Duration::from_millis(750), "{stage}: {elapsed:?}");
    }
}

#[test]
fn interim_responses_preserve_only_final_headers_and_reason() {
    let response = execute_local_response(b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 Custom Reason\r\nContent-Length: 2\r\nX-Final: yes\r\n\r\nok").unwrap();
    assert_eq!(response.reason, "Custom Reason");
    assert_eq!(
        response.headers,
        vec![
            ("content-length".into(), "2".into()),
            ("x-final".into(), "yes".into())
        ]
    );
    assert_eq!(response.body, b"ok");
}

#[test]
fn cancellation_prevents_dns_and_socket_creation() {
    let cancel = Cancellation::new(|| true);
    let mut resolver = SystemResolver::default();
    let error = resolver
        .resolve_cancellable("fixture.invalid", 80, Duration::from_secs(30), &cancel)
        .unwrap_err();
    assert_eq!(error.code(), "network-io");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let request = request_for(listener.local_addr().unwrap());
    assert_eq!(
        UreqTransport
            .execute_cancellable(&request, 16, &cancel)
            .unwrap_err()
            .code(),
        "network-io"
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn partial_tls_records_do_not_restart_the_connection_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("{error}"),
            }
        };
        stream
            .set_write_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        // Announce a TLS handshake record, but provide its body very slowly.
        let _ = stream.write_all(&[22, 3, 3, 0, 64]);
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(40));
            if stream.write_all(&[0]).is_err() {
                break;
            }
        }
    });
    let mut request = request_for(address);
    request.url = request.url.replacen("http:", "https:", 1);
    request.connect_timeout = Duration::from_millis(150);
    request.total_timeout = Duration::from_secs(5);
    let started = Instant::now();
    let result = UreqTransport.execute(&request, 16);
    let elapsed = started.elapsed();
    server.join().unwrap();
    assert_eq!(result.unwrap_err().code(), "network-io");
    assert!(
        elapsed < Duration::from_millis(600),
        "TLS deadline was restarted: {elapsed:?}"
    );
}
