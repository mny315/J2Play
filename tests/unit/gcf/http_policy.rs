use super::*;

struct ResolverSequence {
    calls: Rc<RefCell<usize>>,
    values: Vec<Vec<IpAddr>>,
}
impl Resolver for ResolverSequence {
    fn resolve(&mut self, _: &str, _: u16, _: Duration) -> Result<Vec<IpAddr>, EmuError> {
        let index = *self.calls.borrow();
        *self.calls.borrow_mut() += 1;
        Ok(self.values[index].clone())
    }
}

#[test]
fn network_address_policy_covers_ipv4_mapped_and_six_to_four_boundaries() {
    for (address, allowed) in [
        ("0.0.0.0", false),
        ("0.255.255.255", false),
        ("1.0.0.0", true),
        ("10.0.0.1", false),
        ("100.63.255.255", true),
        ("100.64.0.0", false),
        ("100.127.255.255", false),
        ("100.128.0.0", true),
        ("127.0.0.1", false),
        ("169.254.1.1", false),
        ("172.16.0.0", false),
        ("192.0.0.1", false),
        ("192.0.2.1", false),
        ("192.168.1.1", false),
        ("198.18.0.0", false),
        ("198.19.255.255", false),
        ("198.20.0.0", true),
        ("198.51.100.1", false),
        ("203.0.113.1", false),
        ("224.0.0.1", false),
        ("239.255.255.255", false),
        ("240.0.0.0", false),
        ("255.255.255.255", false),
    ] {
        let ipv4: std::net::Ipv4Addr = address.parse().unwrap();
        assert_eq!(is_public_address(IpAddr::V4(ipv4)), allowed, "{address}");
        assert_eq!(
            is_public_address(IpAddr::V6(ipv4.to_ipv6_mapped())),
            allowed,
            "mapped {address}",
        );
        let [a, b, c, d] = ipv4.octets();
        let six_to_four = std::net::Ipv6Addr::new(
            0x2002,
            u16::from_be_bytes([a, b]),
            u16::from_be_bytes([c, d]),
            0,
            0,
            0,
            0,
            1,
        );
        assert_eq!(
            is_public_address(IpAddr::V6(six_to_four)),
            allowed,
            "6to4 {address}"
        );
    }
    for address in [
        "::",
        "::1",
        "::127.0.0.1",
        "fc00::1",
        "fe80::1",
        "fec0::1",
        "ff00::1",
        "2001::1",
        "2001:db8::1",
    ] {
        assert!(!is_public_address(address.parse().unwrap()), "{address}");
    }
    assert!(is_public_address("2606:4700:4700::1111".parse().unwrap()));
}

#[test]
fn denial_and_offline_happen_before_transport() {
    for (allowed, offline, expected) in [
        (&[][..], false, "security-exception"),
        (&[HTTP_PERMISSION][..], true, "network-offline"),
    ] {
        let calls = Rc::new(RefCell::new(0));
        let (mut runtime, _directory) = runtime(
            allowed,
            calls.clone(),
            vec![],
            vec!["93.184.216.34".parse().unwrap()],
            offline,
        );
        assert_eq!(
            runtime.permission_allowed(HTTP_PERMISSION),
            !allowed.is_empty()
        );
        assert!(!runtime.permission_allowed(HTTPS_PERMISSION));
        assert!(!runtime.permission_allowed("unknown.permission"));
        let error = runtime
            .http(GcfHttpRequest {
                url: "http://example.com/".to_owned(),
                method: "GET".to_owned(),
                headers: vec![],
                body: vec![],
            })
            .unwrap_err();
        assert_eq!(error.code(), expected);
        assert_eq!(*calls.borrow(), 0);
    }
}

#[test]
fn rejects_private_targets_and_rechecks_redirects() {
    let calls = Rc::new(RefCell::new(0));
    let (mut runtime, _) = runtime(
        &[HTTP_PERMISSION],
        calls.clone(),
        vec![],
        vec!["127.0.0.1".parse().unwrap()],
        false,
    );
    assert_eq!(
        runtime
            .http(GcfHttpRequest {
                url: "http://localhost/".to_owned(),
                method: "GET".to_owned(),
                headers: vec![],
                body: vec![]
            })
            .unwrap_err()
            .code(),
        "network-ssrf"
    );
    assert_eq!(*calls.borrow(), 0);

    let transport_calls = Rc::new(RefCell::new(0));
    let resolver_calls = Rc::new(RefCell::new(0));
    let directory = Scratch::new();
    let mut runtime = Runtime::new(
        PermissionPolicy::new([HTTP_PERMISSION.to_owned()]),
        Box::new(CountingTransport {
            calls: transport_calls.clone(),
            responses: vec![response(
                302,
                vec![("Location".to_owned(), "http://internal.test/".to_owned())],
                b"",
            )],
        }),
        Box::new(ResolverSequence {
            calls: resolver_calls.clone(),
            values: vec![
                vec!["93.184.216.34".parse().unwrap()],
                vec!["10.0.0.1".parse().unwrap()],
            ],
        }),
        &directory.0,
        Limits::default(),
        false,
    )
    .unwrap();
    let error = runtime
        .http(GcfHttpRequest {
            url: "http://public.test/".to_owned(),
            method: "GET".to_owned(),
            headers: vec![],
            body: vec![],
        })
        .unwrap_err();
    assert_eq!(error.code(), "network-ssrf");
    assert_eq!(*transport_calls.borrow(), 1);
    assert_eq!(*resolver_calls.borrow(), 2);
}

#[test]
fn legacy_wap_proxy_fails_fast_and_reports_its_target() {
    let calls = Rc::new(RefCell::new(0));
    let (mut runtime, _) = runtime(
        &[HTTP_PERMISSION],
        calls.clone(),
        vec![],
        vec!["93.184.216.34".parse().unwrap()],
        false,
    );
    let request = GcfHttpRequest {
        url: "http://10.0.0.172:80/api/login?build=14".to_owned(),
        method: "POST".to_owned(),
        headers: vec![
            ("x-online-host".to_owned(), " game.example:8080 ".to_owned()),
            (
                "Content-Type".to_owned(),
                "application/octet-stream".to_owned(),
            ),
        ],
        body: b"request".to_vec(),
    };
    assert_eq!(
        http_request_destination(&request).as_deref(),
        Some("http://game.example:8080")
    );

    let error = runtime.http(request).unwrap_err();
    assert_eq!(error.code(), "connection-not-found");
    assert!(error.message().contains("http://game.example:8080"));
    assert_eq!(*calls.borrow(), 0);
}

#[test]
fn legacy_wap_proxy_keeps_permission_and_offline_checks() {
    for (allowed, offline, expected) in [
        (&[][..], false, "security-exception"),
        (&[HTTP_PERMISSION][..], true, "network-offline"),
    ] {
        let calls = Rc::new(RefCell::new(0));
        let (mut runtime, _) = runtime(
            allowed,
            calls.clone(),
            vec![],
            vec!["93.184.216.34".parse().unwrap()],
            offline,
        );
        let error = runtime
            .http(GcfHttpRequest {
                url: "http://10.0.0.172:80/login".to_owned(),
                method: "GET".to_owned(),
                headers: vec![("X-Online-Host".to_owned(), "game.example".to_owned())],
                body: vec![],
            })
            .unwrap_err();
        assert_eq!(error.code(), expected);
        assert_eq!(*calls.borrow(), 0);
    }
}

#[test]
fn pinned_transport_addresses_prevent_dns_rebinding() {
    struct InspectTransport;
    impl HttpTransport for InspectTransport {
        fn execute(
            &mut self,
            request: &TransportRequest,
            _: usize,
        ) -> Result<TransportResponse, EmuError> {
            assert_eq!(
                request.addresses,
                vec!["93.184.216.34:443".parse().unwrap()]
            );
            Ok(response(200, vec![], b"secure"))
        }
    }
    let directory = Scratch::new();
    let mut runtime = Runtime::new(
        PermissionPolicy::new([HTTPS_PERMISSION.to_owned()]),
        Box::new(InspectTransport),
        Box::new(FixedResolver {
            addresses: vec!["93.184.216.34".parse().unwrap()],
        }),
        &directory.0,
        Limits::default(),
        false,
    )
    .unwrap();
    assert_eq!(
        runtime
            .http(GcfHttpRequest {
                url: "https://example.com/".to_owned(),
                method: "GET".to_owned(),
                headers: vec![],
                body: vec![]
            })
            .unwrap()
            .body,
        b"secure"
    );
}

#[test]
fn deterministic_mock_transport_covers_timeout() {
    let directory = Scratch::new();
    let mut runtime = Runtime::new(
        PermissionPolicy::new([HTTPS_PERMISSION.to_owned()]),
        Box::new(MockTransport::new([MockExchange {
            expected_url: "https://example.com/slow".to_owned(),
            outcome: Err(api_error("network-timeout", "mock HTTP request timed out")),
        }])),
        Box::new(FixedResolver {
            addresses: vec!["93.184.216.34".parse().unwrap()],
        }),
        &directory.0,
        Limits::default(),
        false,
    )
    .unwrap();
    assert_eq!(
        runtime
            .http(GcfHttpRequest {
                url: "https://example.com/slow".to_owned(),
                method: "GET".to_owned(),
                headers: vec![],
                body: vec![]
            })
            .unwrap_err()
            .code(),
        "network-timeout"
    );
}

#[test]
fn request_and_response_size_limits_fail_closed() {
    let calls = Rc::new(RefCell::new(0));
    let (mut runtime, _directory) = runtime(
        &[HTTP_PERMISSION],
        calls.clone(),
        vec![],
        vec!["93.184.216.34".parse().unwrap()],
        false,
    );
    let mut oversized = GcfHttpRequest {
        url: "http://example.com/".to_owned(),
        method: "POST".to_owned(),
        headers: vec![],
        body: vec![0; Limits::default().max_request_bytes + 1],
    };
    assert_eq!(
        runtime.http(oversized.clone()).unwrap_err().code(),
        "network-limit"
    );
    assert_eq!(*calls.borrow(), 0);

    let directory = Scratch::new();
    let limits = Limits {
        max_response_bytes: 3,
        ..Limits::default()
    };
    let mut bounded = Runtime::new(
        PermissionPolicy::new([HTTP_PERMISSION.to_owned()]),
        Box::new(CountingTransport {
            calls,
            responses: vec![response(200, vec![], b"four")],
        }),
        Box::new(FixedResolver {
            addresses: vec!["93.184.216.34".parse().unwrap()],
        }),
        &directory.0,
        limits,
        false,
    )
    .unwrap();
    oversized.body.clear();
    oversized.method = "GET".to_owned();
    assert_eq!(bounded.http(oversized).unwrap_err().code(), "network-limit");
}

#[test]
fn malformed_http_urls_are_rejected() {
    for url in [
        "",
        "http:/broken",
        "ftp://example.com/",
        "http://user@example.com/",
        "http://[::1",
        "http://example.com:0/",
        "http://exa mple.com/",
    ] {
        assert!(ParsedHttpUrl::parse(url, 2_048).is_err(), "{url}");
    }
}
