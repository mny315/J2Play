use super::*;
use natives::{HostServices, VmAccess};

#[derive(Default)]
struct Context {
    requests: Vec<(u64, RmsMetadataField)>,
    value: i64,
    record_requests: Vec<(u64, i32)>,
    record_bytes: usize,
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

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }

    fn rms_metadata(&mut self, handle: u64, field: RmsMetadataField) -> Result<i64, EmuError> {
        self.requests.push((handle, field));
        Ok(self.value)
    }

    fn rms_record_size(&mut self, handle: u64, record_id: i32) -> Result<usize, EmuError> {
        self.record_requests.push((handle, record_id));
        Ok(self.record_bytes)
    }
}

impl VmAccess for Context {}

#[test]
fn record_sizes_need_no_payload_read_or_java_array_and_reject_numeric_overflow() {
    let mut registry = NativeRegistry::new();
    register_natives(&mut registry).unwrap();
    let signature =
        NativeSignature::new("javax/microedition/rms/RecordStore", "recordSize0", "(JI)I");
    let mut context = Context::default();
    for size in [0, 1_048_576, i32::MAX as usize] {
        context.record_bytes = size;
        context.record_requests.clear();
        assert_eq!(
            registry
                .invoke(
                    &signature,
                    &mut context,
                    &[NativeValue::Long(7), NativeValue::Int(9)]
                )
                .unwrap(),
            Some(NativeValue::Int(i32::try_from(size).unwrap())),
        );
        assert_eq!(context.record_requests, [(7, 9)]);
    }
    context.record_bytes += 1;
    assert_eq!(
        registry
            .invoke(
                &signature,
                &mut context,
                &[NativeValue::Long(7), NativeValue::Int(9)]
            )
            .unwrap_err()
            .code(),
        "metadata-range",
    );
    context.record_requests.clear();
    assert_eq!(
        registry
            .invoke(
                &signature,
                &mut context,
                &[NativeValue::Long(0), NativeValue::Int(9)]
            )
            .unwrap_err()
            .code(),
        "store-not-open",
    );
    assert!(context.record_requests.is_empty());
}

#[test]
fn metadata_natives_request_one_field_and_preserve_java_numeric_widths() {
    let mut registry = NativeRegistry::new();
    register_natives(&mut registry).unwrap();
    let int_signature = NativeSignature::new(
        "javax/microedition/rms/RecordStore",
        "metadataInt0",
        "(JI)I",
    );
    let mut context = Context::default();
    for (selector, field) in [
        (0, RmsMetadataField::Version),
        (1, RmsMetadataField::RecordCount),
        (2, RmsMetadataField::Size),
        (3, RmsMetadataField::SizeAvailable),
        (4, RmsMetadataField::NextRecordId),
    ] {
        context.value = i64::from(i32::MAX);
        context.requests.clear();
        let result = registry.invoke(
            &int_signature,
            &mut context,
            &[NativeValue::Long(7), NativeValue::Int(selector)],
        );
        assert_eq!(result.unwrap(), Some(NativeValue::Int(i32::MAX)));
        assert_eq!(context.requests, [(7, field)]);
    }
    context.value = i64::MAX;
    context.requests.clear();
    let signature = NativeSignature::new(
        "javax/microedition/rms/RecordStore",
        "lastModified0",
        "(J)J",
    );
    let result = registry.invoke(&signature, &mut context, &[NativeValue::Long(7)]);
    assert_eq!(result.unwrap(), Some(NativeValue::Long(i64::MAX)));
    assert_eq!(context.requests, [(7, RmsMetadataField::LastModified)]);
    assert_eq!(
        registry
            .invoke(
                &int_signature,
                &mut context,
                &[NativeValue::Long(7), NativeValue::Int(0)],
            )
            .unwrap_err()
            .code(),
        "metadata-range"
    );
}

#[test]
fn invalid_metadata_selectors_do_not_call_the_host() {
    let mut registry = NativeRegistry::new();
    register_natives(&mut registry).unwrap();
    let signature = NativeSignature::new(
        "javax/microedition/rms/RecordStore",
        "metadataInt0",
        "(JI)I",
    );
    let mut context = Context::default();
    for selector in [-1, 5, i32::MAX] {
        assert_eq!(
            registry
                .invoke(
                    &signature,
                    &mut context,
                    &[NativeValue::Long(7), NativeValue::Int(selector)],
                )
                .unwrap_err()
                .code(),
            "native-arguments"
        );
    }
    assert!(context.requests.is_empty());
}
