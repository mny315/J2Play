use super::*;

struct Context;
impl HostServices for Context {
    fn monotonic_millis(&self) -> i64 {
        7
    }
    fn wall_clock_millis(&self) -> i64 {
        11
    }
    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
}
impl VmAccess for Context {}

#[test]
fn default_host_can_cancel_when_no_platform_request_is_pending() {
    assert!(!Context.platform_request("").unwrap());
    assert!(
        Context
            .platform_request("https://example.invalid/")
            .is_err()
    );
}

#[test]
fn exact_registration_and_diagnostic() {
    let signature = NativeSignature::new("java/lang/System", "currentTimeMillis", "()J");
    let mut registry = NativeRegistry::new();
    registry
        .register(signature.clone(), |context, args| {
            assert!(args.is_empty());
            Ok(Some(NativeValue::Long(context.wall_clock_millis())))
        })
        .unwrap();
    assert_eq!(
        registry.invoke(&signature, &mut Context, &[]).unwrap(),
        Some(NativeValue::Long(11))
    );
    assert!(
        registry
            .register(signature.clone(), |_, _| Ok(Some(NativeValue::Long(99))))
            .is_err()
    );
    assert_eq!(
        registry.invoke(&signature, &mut Context, &[]).unwrap(),
        Some(NativeValue::Long(11))
    );
    let missing = NativeSignature::new("x/Y", "z", "()V");
    assert!(
        registry
            .invoke(&missing, &mut Context, &[])
            .unwrap_err()
            .to_string()
            .contains("x/Y::z()V")
    );
}
