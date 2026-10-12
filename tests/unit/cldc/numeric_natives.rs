use super::*;
use natives::{HostServices, VmAccess};

struct StringContext(&'static str);

impl HostServices for StringContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        0
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
}

impl VmAccess for StringContext {
    fn read_java_string(&self, reference: u64) -> Result<String, EmuError> {
        assert_eq!(reference, 1);
        Ok(self.0.to_owned())
    }
}

#[test]
fn floating_point_natives_distinguish_null_from_invalid_text() {
    let mut registry = NativeRegistry::default();
    register_core_natives(&mut registry).unwrap();
    for (class, method, descriptor, expected) in [
        (
            "java/lang/Float",
            "parseFloat",
            "(Ljava/lang/String;)F",
            NativeValue::Float(1.25),
        ),
        (
            "java/lang/Double",
            "parseDouble",
            "(Ljava/lang/String;)D",
            NativeValue::Double(1.25),
        ),
    ] {
        let signature = NativeSignature::new(class, method, descriptor);
        let invoke = |context: &mut StringContext, reference| {
            registry.invoke(&signature, context, &[NativeValue::Reference(reference)])
        };
        assert_eq!(
            invoke(&mut StringContext("unused"), None)
                .unwrap_err()
                .code(),
            "null-pointer-exception",
            "{class}"
        );
        for invalid in ["", "NaNf", "no number"] {
            assert_eq!(
                invoke(&mut StringContext(invalid), Some(1))
                    .unwrap_err()
                    .code(),
                "number-format",
                "{class}: {invalid:?}"
            );
        }
        assert_eq!(
            invoke(&mut StringContext(" 1.25f "), Some(1)).unwrap(),
            Some(expected)
        );
    }
}
