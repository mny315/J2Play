use super::*;

struct RecordingTransport {
    requests: Rc<RefCell<Vec<TransportRequest>>>,
    responses: Vec<TransportResponse>,
}
impl HttpTransport for RecordingTransport {
    fn execute(
        &mut self,
        request: &TransportRequest,
        _: usize,
    ) -> Result<TransportResponse, EmuError> {
        self.requests.borrow_mut().push(request.clone());
        Ok(self.responses.remove(0))
    }
}

fn follow_redirect(base: &str, location: &str, limits: Limits) -> Result<String, EmuError> {
    let requests = Rc::new(RefCell::new(Vec::new()));
    let directory = Scratch::new();
    let mut runtime = Runtime::new(
        PermissionPolicy::new([HTTP_PERMISSION.to_owned()]),
        Box::new(RecordingTransport {
            requests: Rc::clone(&requests),
            responses: vec![
                response(302, vec![("Location".to_owned(), location.to_owned())], b""),
                response(200, vec![], b"redirected"),
            ],
        }),
        Box::new(FixedResolver {
            addresses: vec!["93.184.216.34".parse().unwrap()],
        }),
        &directory.0,
        limits,
        false,
    )?;
    let result = runtime.http(GcfHttpRequest {
        url: base.to_owned(),
        method: "GET".to_owned(),
        headers: vec![],
        body: vec![],
    });
    let requests = requests.borrow();
    match result {
        Ok(result) => {
            assert_eq!(result.body, b"redirected");
            assert_eq!(requests.len(), 2);
            Ok(requests[1].url.clone())
        }
        Err(error) => {
            assert_eq!(
                requests.len(),
                1,
                "invalid redirect must not reach transport"
            );
            Err(error)
        }
    }
}

#[test]
fn redirects_preserve_head_and_only_rewrite_post_for_301_302_303() {
    for status in [301, 302, 303, 307, 308] {
        for method in ["GET", "HEAD", "POST"] {
            let requests = Rc::new(RefCell::new(Vec::new()));
            let directory = Scratch::new();
            let mut runtime = Runtime::new(
                PermissionPolicy::new([HTTP_PERMISSION.to_owned()]),
                Box::new(RecordingTransport {
                    requests: Rc::clone(&requests),
                    responses: vec![
                        response(
                            status,
                            vec![("Location".to_owned(), "/next".to_owned())],
                            b"",
                        ),
                        response(200, vec![], b""),
                    ],
                }),
                Box::new(FixedResolver {
                    addresses: vec!["93.184.216.34".parse().unwrap()],
                }),
                &directory.0,
                Limits::default(),
                false,
            )
            .unwrap();
            let body = if method == "POST" { &b"data"[..] } else { &[] };
            runtime
                .http(GcfHttpRequest {
                    url: "http://example.test/start".to_owned(),
                    method: method.to_owned(),
                    headers: vec![],
                    body: body.to_vec(),
                })
                .unwrap();
            let requests = requests.borrow();
            assert_eq!(requests.len(), 2);
            let rewritten = method == "POST" && matches!(status, 301..=303);
            assert_eq!(
                requests[1].method,
                if rewritten { "GET" } else { method },
                "{status} redirected {method}"
            );
            assert_eq!(requests[1].body, if rewritten { &[][..] } else { body });
        }
    }
}

#[test]
fn relative_redirects_resolve_against_the_complete_request_url() {
    let base = "http://example.test/b/c/d;p?q";
    // HTTP forms from RFC 3986 section 5.4, with a project-owned authority.
    for (location, target) in [
        ("g", "http://example.test/b/c/g"),
        ("./g", "http://example.test/b/c/g"),
        ("g/", "http://example.test/b/c/g/"),
        ("/g", "http://example.test/g"),
        ("//other.test/g", "http://other.test/g"),
        ("?y", "http://example.test/b/c/d;p?y"),
        ("g?y", "http://example.test/b/c/g?y"),
        ("#s", "http://example.test/b/c/d;p?q#s"),
        ("g#s", "http://example.test/b/c/g#s"),
        ("g?y#s", "http://example.test/b/c/g?y#s"),
        (";x", "http://example.test/b/c/;x"),
        ("g;x?y#s", "http://example.test/b/c/g;x?y#s"),
        ("", "http://example.test/b/c/d;p?q"),
        (".", "http://example.test/b/c/"),
        ("..", "http://example.test/b/"),
        ("../g", "http://example.test/b/g"),
        ("../..", "http://example.test/"),
        ("../../g", "http://example.test/g"),
        ("../../../g", "http://example.test/g"),
        ("/./g", "http://example.test/g"),
        ("g/../h", "http://example.test/b/c/h"),
        ("g?y/../h", "http://example.test/b/c/g?y/../h"),
        ("HTTP://OTHER.TEST/g", "http://other.test/g"),
    ] {
        assert_eq!(
            follow_redirect(base, location, Limits::default()).unwrap(),
            target,
            "Location: {location}"
        );
    }
}

#[test]
fn redirect_fragment_is_inherited_only_when_location_omits_it() {
    let base = "http://example.test/a#original";
    for (location, target) in [
        ("b", "http://example.test/b#original"),
        ("/b#new", "http://example.test/b#new"),
        ("/b#", "http://example.test/b#"),
        ("", "http://example.test/a#original"),
    ] {
        assert_eq!(
            follow_redirect(base, location, Limits::default()).unwrap(),
            target
        );
    }
}

#[test]
fn resolved_redirects_keep_url_limits_and_protocol_permission_checks() {
    let base = "http://example.test/a/b";
    for (location, error) in [
        ("https://other.test/", "security-exception"),
        ("ftp://other.test/", "connection-not-found"),
        ("//user:secret@other.test/", "network-url"),
        ("x\0y", "network-redirect"),
    ] {
        assert_eq!(
            follow_redirect(base, location, Limits::default())
                .unwrap_err()
                .code(),
            error
        );
    }
    for length in [64, 65] {
        assert!(
            follow_redirect(
                base,
                &"x".repeat(length),
                Limits {
                    max_url_bytes: 64,
                    ..Limits::default()
                }
            )
            .is_err()
        );
    }
}

#[test]
fn redirect_chain_resolves_the_current_host_and_port_at_every_hop() {
    struct OrderedResolver {
        targets: std::array::IntoIter<(&'static str, u16), 3>,
    }
    impl Resolver for OrderedResolver {
        fn resolve(&mut self, host: &str, port: u16, _: Duration) -> Result<Vec<IpAddr>, EmuError> {
            assert_eq!((host, port), self.targets.next().unwrap());
            Ok(vec!["93.184.216.34".parse().unwrap()])
        }
    }
    let requests = Rc::new(RefCell::new(Vec::new()));
    let directory = Scratch::new();
    let mut runtime = Runtime::new(
        PermissionPolicy::new([HTTP_PERMISSION.to_owned(), HTTPS_PERMISSION.to_owned()]),
        Box::new(RecordingTransport {
            requests: Rc::clone(&requests),
            responses: vec![
                response(
                    307,
                    vec![(
                        "Location".to_owned(),
                        "https://second.test:8443/b".to_owned(),
                    )],
                    b"",
                ),
                response(
                    307,
                    vec![("Location".to_owned(), "//third.test/c".to_owned())],
                    b"",
                ),
                response(200, vec![], b"complete"),
            ],
        }),
        Box::new(OrderedResolver {
            targets: [
                ("first.test", 8080),
                ("second.test", 8443),
                ("third.test", 443),
            ]
            .into_iter(),
        }),
        &directory.0,
        Limits::default(),
        false,
    )
    .unwrap();
    let response = runtime
        .http(GcfHttpRequest {
            url: "http://first.test:8080/a".to_owned(),
            method: "GET".to_owned(),
            headers: vec![],
            body: vec![],
        })
        .unwrap();
    assert_eq!(response.body, b"complete");
    let requests = requests.borrow();
    assert_eq!(requests.len(), 3);
    for (request, expected) in requests.iter().zip([
        ("http://first.test:8080/a", 8080),
        ("https://second.test:8443/b", 8443),
        ("https://third.test/c", 443),
    ]) {
        assert_eq!(request.url, expected.0);
        assert_eq!(request.addresses.len(), 1);
        assert_eq!(request.addresses[0].port(), expected.1);
    }
}

#[test]
fn redirects_cannot_restart_the_request_deadline() {
    struct SlowRedirect(Rc<RefCell<usize>>);
    impl HttpTransport for SlowRedirect {
        fn execute(
            &mut self,
            request: &TransportRequest,
            _: usize,
        ) -> Result<TransportResponse, EmuError> {
            *self.0.borrow_mut() += 1;
            std::thread::sleep(request.total_timeout);
            Ok(response(
                302,
                vec![("Location".to_owned(), "/next".to_owned())],
                &[],
            ))
        }
    }
    let directory = Scratch::new();
    let calls = Rc::new(RefCell::new(0));
    let mut runtime = Runtime::new(
        PermissionPolicy::new([HTTP_PERMISSION.to_owned()]),
        Box::new(SlowRedirect(Rc::clone(&calls))),
        Box::new(FixedResolver {
            addresses: vec!["93.184.216.34".parse().unwrap()],
        }),
        &directory.0,
        Limits {
            connect_timeout: Duration::from_millis(30),
            read_timeout: Duration::from_millis(30),
            ..Limits::default()
        },
        false,
    )
    .unwrap();
    let error = runtime
        .http(GcfHttpRequest {
            url: "http://example.com/".to_owned(),
            method: "GET".to_owned(),
            headers: Vec::new(),
            body: Vec::new(),
        })
        .unwrap_err();
    assert_eq!(error.code(), "network-timeout");
    assert_eq!(*calls.borrow(), 1);
}

#[test]
fn redirects_preserve_only_headers_allowed_for_the_new_origin_and_method() {
    let sensitive = [
        ("aUtHoRiZaTiOn", "secret"),
        ("Proxy-Authorization", "proxy secret"),
        ("Cookie", "session=secret"),
    ];
    let entity = [
        ("Content-Type", "text/plain"),
        ("content-LENGTH", "4"),
        ("Content-Encoding", "identity"),
        ("Content-Language", "en"),
        ("Content-Location", "/start"),
        ("Transfer-Encoding", "chunked"),
    ];
    let ordinary = [("Accept", "text/plain"), ("X-Fixture", "retained")];
    let owned = |headers: &[(&str, &str)]| -> Vec<(String, String)> {
        headers
            .iter()
            .map(|(name, value)| ((*name).into(), (*value).into()))
            .collect()
    };
    let original = owned(&[sensitive.as_slice(), entity.as_slice(), ordinary.as_slice()].concat());
    for (location, target, same_origin) in [
        ("/end", "http://example.test/end", true),
        (
            "http://EXAMPLE.test:80/end",
            "http://example.test/end",
            true,
        ),
        ("//other.test/end", "http://other.test/end", false),
        ("http://other.test/end", "http://other.test/end", false),
        (
            "http://example.test:81/end",
            "http://example.test:81/end",
            false,
        ),
        (
            "https://example.test/end",
            "https://example.test/end",
            false,
        ),
    ] {
        for (status, method, body) in [(302, "GET", &b""[..]), (307, "POST", &b"data"[..])] {
            let requests = Rc::new(RefCell::new(Vec::new()));
            let directory = Scratch::new();
            let mut runtime = Runtime::new(
                PermissionPolicy::new([HTTP_PERMISSION.to_owned(), HTTPS_PERMISSION.to_owned()]),
                Box::new(RecordingTransport {
                    requests: Rc::clone(&requests),
                    responses: vec![
                        response(
                            status,
                            vec![("Location".to_owned(), location.to_owned())],
                            b"",
                        ),
                        response(200, vec![], b"ok"),
                    ],
                }),
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
                        url: "http://example.test/start".to_owned(),
                        method: "POST".to_owned(),
                        headers: original.clone(),
                        body: b"data".to_vec(),
                    })
                    .unwrap()
                    .body,
                b"ok"
            );
            let requests = requests.borrow();
            assert_eq!(requests.len(), 2);
            assert_eq!(requests[0].headers, original);
            assert_eq!(requests[1].url, target);
            assert_eq!(requests[1].method, method);
            assert_eq!(requests[1].body, body);
            let mut expected = Vec::new();
            if same_origin {
                expected.extend(owned(&sensitive));
            }
            if status == 307 {
                expected.extend(owned(&entity));
            }
            expected.extend(owned(&ordinary));
            assert_eq!(
                requests[1].headers, expected,
                "status={status}, Location={location}"
            );
        }
    }
}
