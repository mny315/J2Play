use super::*;

#[test]
fn callback_classes_are_valid_and_every_implementation_is_native_rust() {
    let scratch = crate::test_storage::Scratch::new();
    let root = &scratch.0;
    generate(root).unwrap();
    let exports = include_str!("../../../../../crates/j2play-android/src/android_adapter/ffi.rs");
    let mut callbacks = 0;
    for name in [
        "J2PlayActivity",
        "Callbacks",
        "BackCallback",
        "BridgeEditor",
        "EditorConnection",
    ] {
        let bytes = fs::read(root.join(format!("{PACKAGE}/{name}.class"))).unwrap();
        let class = classfile::parse(&bytes).unwrap();
        assert_eq!(
            class.class_name(class.this_class),
            Some(format!("{PACKAGE}/{name}").as_str())
        );
        for method in &class.methods {
            let method_name = class.utf8(method.name_index).unwrap();
            if method_name == "<init>" || method_name == "<clinit>" {
                continue;
            }
            assert_eq!(method.access_flags & 0x0100, 0x0100);
            assert!(
                method.attributes.is_empty(),
                "callbacks cannot contain bytecode"
            );
            let symbol = format!("Java_io_github_mny315_j2play_{name}_{method_name}");
            assert!(
                exports.contains(&symbol),
                "missing native implementation: {symbol}"
            );
            callbacks += 1;
        }
    }
    assert_eq!(callbacks, 27);
}
