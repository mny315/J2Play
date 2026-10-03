use super::*;

#[test]
fn compatibility_jsr_bootstrap_is_suite_requested() {
    let source = include_bytes!("../../../profiles/sony-ericsson/featurephone.json");
    let profile = device_profile::DeviceProfile::from_reader(source.as_slice()).unwrap();
    let empty = HashSet::new();
    assert!(profile_supports_suite_bootstrap(
        &profile,
        BootstrapRequirement::Jsr("184"),
        &empty,
    ));
    assert!(!profile_supports_suite_bootstrap(
        &profile,
        BootstrapRequirement::Jsr("239"),
        &empty,
    ));
    assert!(!profile_supports_suite_bootstrap(
        &profile,
        BootstrapRequirement::Jsr("239"),
        &HashSet::from(["239".to_owned()]),
    ));

    let mut synthetic: serde_json::Value = serde_json::from_slice(source).unwrap();
    synthetic["profile_id"] = serde_json::json!("synthetic-jsr239-profile");
    synthetic["java"]["compatibility_jsrs"] = serde_json::json!({
        "value": ["239"],
        "confidence": "confirmed",
        "sources": ["j2play-cross-vendor-compatibility"]
    });
    let bytes = serde_json::to_vec(&synthetic).unwrap();
    let synthetic = device_profile::DeviceProfile::from_reader(bytes.as_slice()).unwrap();
    assert!(profile_supports_suite_bootstrap(
        &synthetic,
        BootstrapRequirement::Jsr("239"),
        &HashSet::from(["239".to_owned()]),
    ));
    assert!(descriptor_requests_jsr239(
        "Ljavax/microedition/khronos/egl/EGL11;"
    ));
    assert!(descriptor_requests_jsr239("Ljava/nio/FloatBuffer;"));
    assert!(!descriptor_requests_jsr239(
        "Ljavax/microedition/m3g/Graphics3D;"
    ));

    let string_only = classfile::ClassFile {
        minor_version: 0,
        major_version: 52,
        constant_pool: vec![
            None,
            Some(classfile::Constant::Utf8(
                "javax/microedition/khronos/egl/EGL11".to_owned(),
            )),
            Some(classfile::Constant::String { string_index: 1 }),
        ],
        access_flags: 0,
        this_class: 0,
        super_class: 0,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: Vec::new(),
        attributes: Vec::new(),
    };
    assert!(!class_requests_jsr239(&string_only));
}
