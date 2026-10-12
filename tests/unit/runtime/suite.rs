use super::*;
use std::cell::Cell;

fn plain_class_resource() -> jar::ClassResource {
    class_resource("java/lang/Object")
}

fn class_resource(super_class: &str) -> jar::ClassResource {
    let mut bytes =
        b"\xca\xfe\xba\xbe\x00\x00\x00\x32\x00\x05\x01\x00\x05Probe\x07\x00\x01\x01".to_vec();
    bytes.extend_from_slice(&u16::try_from(super_class.len()).unwrap().to_be_bytes());
    bytes.extend_from_slice(super_class.as_bytes());
    bytes
        .extend_from_slice(b"\x07\x00\x03\x00\x21\x00\x02\x00\x04\x00\x00\x00\x00\x00\x00\x00\x00");
    jar::ClassResource {
        name: "Probe.class".to_owned(),
        bytes,
    }
}

fn profile_with_optional_jsr239(enabled: bool) -> device_profile::DeviceProfile {
    let mut profile: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../profiles/sony-ericsson/featurephone.json"
    ))
    .unwrap();
    if enabled {
        profile["java"]["compatibility_jsrs"] = serde_json::json!({
            "value": ["239"],
            "confidence": "confirmed",
            "sources": ["j2play-cross-vendor-compatibility"]
        });
    }
    device_profile::DeviceProfile::from_reader(serde_json::to_vec(&profile).unwrap().as_slice())
        .unwrap()
}

#[test]
fn compatibility_scan_honors_cancellation_between_classes() {
    let profile = profile_with_optional_jsr239(true);
    let resources = [
        plain_class_resource(),
        jar::ClassResource {
            name: "NeverParsed.class".to_owned(),
            bytes: vec![0],
        },
    ];
    let polls = Cell::new(0);
    let result = suite_requested_compatibility_jsrs(&resources, &profile, || {
        let previous = polls.get();
        polls.set(previous + 1);
        previous == 1
    })
    .unwrap();
    assert!(result.is_none());
    assert_eq!(polls.get(), 2);

    assert_eq!(
        suite_requested_compatibility_jsrs(&resources, &profile, || false)
            .unwrap_err()
            .code(),
        "class-parse",
    );
}

#[test]
fn compatibility_scan_distinguishes_cancellation_from_no_optional_apis() {
    let resources = [plain_class_resource()];
    for enabled in [false, true] {
        let profile = profile_with_optional_jsr239(enabled);
        assert!(
            suite_requested_compatibility_jsrs(&resources, &profile, || true)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            suite_requested_compatibility_jsrs(&resources, &profile, || false).unwrap(),
            Some(HashSet::new()),
        );
    }
}

#[test]
fn suite_references_enable_only_profile_allowed_optional_apis() {
    let resources = [class_resource("java/nio/Buffer")];
    for enabled in [false, true] {
        let profile = profile_with_optional_jsr239(enabled);
        let requested = suite_requested_compatibility_jsrs(&resources, &profile, || false)
            .unwrap()
            .unwrap();
        assert_eq!(requested.contains("239"), enabled);

        let resolved = profile
            .resolve(device_profile::ProfileOverrides::default())
            .unwrap();
        let mut program = vm::Program::new();
        install_rust_bootstrap(
            &mut program,
            &vm::Limits::default(),
            &resolved,
            &requested,
            false,
        )
        .unwrap();
        assert_eq!(program.contains_class("java/nio/Buffer"), enabled);
    }
}

#[test]
fn skipping_optional_api_inspection_keeps_class_installation_validation() {
    let profile = profile_with_optional_jsr239(false);
    let resources = vec![jar::ClassResource {
        name: "Damaged.class".to_owned(),
        bytes: vec![0],
    }];
    assert_eq!(
        suite_requested_compatibility_jsrs(&resources, &profile, || false).unwrap(),
        Some(HashSet::new()),
    );
    let error = install_suite_classes(
        &mut vm::Program::new(),
        &vm::Limits::default(),
        resources,
        || false,
    )
    .unwrap_err();
    assert_eq!(error.code(), "class-parse");
}

#[test]
#[ignore = "manual release measurement of optional bootstrap inspection"]
fn compatibility_scan_throughput() {
    let resources = vec![plain_class_resource(); 512];
    let profile = profile_with_optional_jsr239(false);
    let started = std::time::Instant::now();
    for _ in 0..1_024 {
        let requested = suite_requested_compatibility_jsrs(
            std::hint::black_box(&resources),
            std::hint::black_box(&profile),
            || false,
        )
        .unwrap()
        .unwrap();
        assert!(requested.is_empty());
        std::hint::black_box(requested);
    }
    eprintln!(
        "512-class compatibility scan, 1024 batches: {:?}",
        started.elapsed()
    );
}
