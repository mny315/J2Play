use crate::support::gcf_transport::{FixedResolver, MockExchange, MockTransport};
use std::path::PathBuf;

struct MockContext {
    runtime: gcf::Runtime,
}

impl natives::HostServices for MockContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }
    fn wall_clock_millis(&self) -> i64 {
        0
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, diagnostics::EmuError> {
        Ok(None)
    }
    fn gcf_http(
        &mut self,
        request: natives::GcfHttpRequest,
    ) -> Result<natives::GcfHttpResponse, diagnostics::EmuError> {
        self.runtime.http(request)
    }
}

fn run(method: &str, permissions: &[&str], root: &std::path::Path) -> i32 {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output =
        crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
            .permissions(permissions)
            .file_root(root)
            .run_static("fixtures/Stage14Fixtures", method, "()I");
    assert!(output.success(), "{}", output.diagnostics);
    output.int_value().expect("integer fixture result")
}

#[test]
fn java_file_connection_round_trip_and_listing_run_on_rust_vm() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "java_file_connection_round_trip_and_listing_run_on_rust_vm"
    )) {
        return;
    }
    let scratch = crate::support::Scratch::new();
    let root = &scratch.0;
    let permissions = &[
        "javax.microedition.io.Connector.file.read",
        "javax.microedition.io.Connector.file.write",
    ];
    assert_eq!(run("fileRoundTrip", permissions, root), 1401);
    assert_eq!(run("directoryAndList", permissions, root), 1402);
    std::fs::write(root.join("offset.dat"), b"abc").unwrap();
    assert_eq!(
        run(
            "writeOnlyOffset",
            &["javax.microedition.io.Connector.file.write"],
            root
        ),
        1405
    );
    assert_eq!(std::fs::read(root.join("offset.dat")).unwrap(), b"aZc");
}

#[test]
fn java_denial_and_offline_network_are_catchable() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "java_denial_and_offline_network_are_catchable"
    )) {
        return;
    }
    let scratch = crate::support::Scratch::new();
    let root = &scratch.0;
    assert_eq!(run("deniedBeforeAccess", &[], root), 1403);
    assert_eq!(
        run(
            "offlineHttp",
            &["javax.microedition.io.Connector.http"],
            root
        ),
        1404
    );
}

#[test]
fn midlet_offline_policy_blocks_https_before_transport() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "midlet_offline_policy_blocks_https_before_transport"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = crate::support::Scratch::new();
    let root = &scratch.0;
    let output =
        crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
            .profile("se-featurephone")
            .permissions(&["javax.microedition.io.Connector.https"])
            .file_root(root)
            .rms_root(root.join("rms"))
            .midlet(8)
            .run_midlet();
    assert!(output.success(), "{}", output.diagnostics);
    let diagnostics = &output.diagnostics;
    assert_eq!(
        output.ams_state,
        Some(midp::LifecycleState::Destroyed),
        "{diagnostics}"
    );
}

#[test]
fn java_http_connection_passes_through_deterministic_mock_transport() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "java_http_connection_passes_through_deterministic_mock_transport"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let jar_path = repository.join("tests/fixtures/java-me/conformance.jar");
    let mut program = vm::Program::new();
    let limits = vm::Limits::default();
    for class in runtime_bootstrap::production_bootstrap_classes() {
        program.add_class(&class, &limits).unwrap();
    }
    for resource in jar::read_class_entries_path(&jar_path).unwrap() {
        let parsed = classfile::parse(&resource.bytes).unwrap();
        program.add_class(&parsed, &limits).unwrap();
    }
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    gcf::register_natives(program.native_registry_mut()).unwrap();
    for date in [
        "Sun, 06 Nov 1994 08:49:37 GMT",
        "Sunday, 06-Nov-94 08:49:37 GMT",
        "Sun Nov  6 08:49:37 1994",
    ] {
        let response = gcf::TransportResponse {
            status: 200,
            reason: "OK".to_owned(),
            headers: vec![
                ("X-Test".to_owned(), "stage14".to_owned()),
                ("Date".to_owned(), date.to_owned()),
            ],
            body: 1406_i32.to_be_bytes().to_vec(),
        };
        let scratch = crate::support::Scratch::new();
        let root = &scratch.0;
        let runtime = gcf::Runtime::new(
            gcf::PermissionPolicy::new([gcf::HTTPS_PERMISSION.to_owned()]),
            Box::new(MockTransport::new([MockExchange {
                expected_url: "https://example.test/api?q=1".to_owned(),
                outcome: Ok(response),
            }])),
            Box::new(FixedResolver {
                addresses: vec!["93.184.216.34".parse().unwrap()],
            }),
            root,
            gcf::Limits::default(),
            false,
        )
        .unwrap();
        let mut context = MockContext { runtime };
        let execution = program
            .execute_with_context(
                "fixtures/Stage14Fixtures",
                "mockHttp",
                "()I",
                limits.clone(),
                false,
                &mut context,
            )
            .unwrap();
        assert_eq!(execution.value, Some(vm::Value::Int(1406)));
    }
}
