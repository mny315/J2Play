use super::*;
use natives::{HostServices, VmAccess};

struct Context {
    properties: Properties,
    property_name: String,
    returned: Option<String>,
}

impl HostServices for Context {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        0
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn bluetooth_property(&self, name: &str) -> Option<&str> {
        self.properties.get(name)
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
}

impl VmAccess for Context {
    fn read_java_string(&self, reference: u64) -> Result<String, EmuError> {
        assert_eq!(reference, 7);
        Ok(self.property_name.clone())
    }

    fn intern_java_string(&mut self, value: &str) -> Result<u64, EmuError> {
        self.returned = Some(value.to_owned());
        Ok(11)
    }
}

#[test]
fn bluetooth_properties_follow_each_profile_without_inventing_unknown_facts() {
    for (bytes, version) in [
        (
            include_bytes!("../../../profiles/sony-ericsson/featurephone.json").as_slice(),
            "1.0",
        ),
        (
            include_bytes!("../../../profiles/nokia/featurephone.json").as_slice(),
            "1.0",
        ),
        (
            include_bytes!("../../../profiles/nokia/s60-touch.json").as_slice(),
            "1.1",
        ),
    ] {
        let profile = DeviceProfile::from_reader(bytes).unwrap();
        let properties = Properties::from_profile(&profile);
        assert_eq!(
            properties.get("bluetooth.api.version"),
            Some(version),
            "{}",
            profile.profile_id()
        );
        assert_eq!(
            properties.get("bluetooth.connected.devices.max"),
            None,
            "{}",
            profile.profile_id()
        );
    }
}

#[test]
fn get_property_returns_known_value_and_unknown_null() {
    let profile = DeviceProfile::from_reader(
        include_bytes!("../../../profiles/sony-ericsson/featurephone.json").as_slice(),
    )
    .unwrap();
    let mut context = Context {
        properties: Properties::from_profile(&profile),
        property_name: "bluetooth.api.version".to_owned(),
        returned: None,
    };
    let signature = NativeSignature::new(
        "javax/bluetooth/LocalDevice",
        "getProperty",
        "(Ljava/lang/String;)Ljava/lang/String;",
    );
    let mut registry = NativeRegistry::new();
    register_natives(&mut registry).unwrap();
    assert_eq!(
        registry
            .invoke(&signature, &mut context, &[NativeValue::Reference(Some(7))],)
            .unwrap(),
        Some(NativeValue::Reference(Some(11)))
    );
    assert_eq!(context.returned.as_deref(), Some("1.0"));

    context.property_name = "bluetooth.connected.devices.max".to_owned();
    assert_eq!(
        registry
            .invoke(&signature, &mut context, &[NativeValue::Reference(Some(7))],)
            .unwrap(),
        Some(NativeValue::Reference(None))
    );
    assert_eq!(
        registry
            .invoke(&signature, &mut context, &[NativeValue::Reference(None)],)
            .unwrap_err()
            .code(),
        "null-pointer-exception"
    );
}
