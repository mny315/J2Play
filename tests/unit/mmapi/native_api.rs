use super::*;

struct SiemensResourceContext;

impl natives::HostServices for SiemensResourceContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        0
    }

    fn system_property(&self, _name: &str) -> Option<&str> {
        None
    }

    fn read_resource(&self, name: &str) -> Result<Option<Vec<u8>>, EmuError> {
        assert_eq!(name, "/0.mid");
        Ok(Some(b"MThd".to_vec()))
    }

    fn mmapi_create_bytes(&mut self, content_type: &str, data: &[u8]) -> Result<u64, EmuError> {
        assert_eq!(content_type, "audio/midi");
        assert_eq!(data, b"MThd");
        Ok(41)
    }
}

impl natives::VmAccess for SiemensResourceContext {
    fn read_java_string(&self, reference: u64) -> Result<String, EmuError> {
        assert_eq!(reference, 1);
        Ok("/0.mid".to_owned())
    }
}

#[test]
fn siemens_manager_accepts_absolute_suite_resource_locators() {
    let mut registry = NativeRegistry::new();
    register_natives(&mut registry).unwrap();
    let mut context = SiemensResourceContext;
    let argument = [NativeValue::Reference(Some(1))];

    let siemens = NativeSignature::new(
        "com/siemens/mp/media/PlayerImpl",
        "createLocator0",
        "(Ljava/lang/String;)J",
    );
    assert_eq!(
        registry.invoke(&siemens, &mut context, &argument).unwrap(),
        Some(NativeValue::Long(41))
    );

    let standard = NativeSignature::new(
        "javax/microedition/media/PlayerImpl",
        "createLocator0",
        "(Ljava/lang/String;)J",
    );
    assert_eq!(
        registry
            .invoke(&standard, &mut context, &argument)
            .unwrap_err()
            .code(),
        "media-unsupported-locator"
    );
}
