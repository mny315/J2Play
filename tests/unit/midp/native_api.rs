use super::*;
use std::{cell::RefCell, collections::BTreeMap};

struct ImageDecodeContext {
    bytes: Vec<u8>,
    text_input_active: bool,
    decoded: Vec<i32>,
}

impl natives::HostServices for ImageDecodeContext {
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

    fn set_text_input_active(&mut self, active: bool) {
        self.text_input_active = active;
    }
}

impl natives::VmAccess for ImageDecodeContext {
    fn read_java_byte_array_range(
        &self,
        _: u64,
        offset: i32,
        length: i32,
    ) -> Result<Option<Vec<u8>>, EmuError> {
        let (Ok(offset), Ok(length)) = (usize::try_from(offset), usize::try_from(length)) else {
            return Ok(None);
        };
        Ok(offset
            .checked_add(length)
            .and_then(|end| self.bytes.get(offset..end))
            .map(<[u8]>::to_vec))
    }

    fn allocate_java_int_array(&mut self, value: &[i32]) -> Result<u64, EmuError> {
        self.decoded = value.to_vec();
        Ok(7)
    }
}

struct ImageResourceContext {
    name: String,
    caller: Option<String>,
    resources: BTreeMap<String, Vec<u8>>,
    reads: RefCell<Vec<String>>,
    allocated: Vec<u8>,
}

impl natives::HostServices for ImageResourceContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        0
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn read_resource(&self, name: &str) -> Result<Option<Vec<u8>>, EmuError> {
        self.reads.borrow_mut().push(name.to_owned());
        Ok(self.resources.get(name).cloned())
    }
}

impl natives::VmAccess for ImageResourceContext {
    fn native_caller_class(&self) -> Option<String> {
        self.caller.clone()
    }

    fn read_java_string(&self, _: u64) -> Result<String, EmuError> {
        Ok(self.name.clone())
    }

    fn allocate_java_byte_array(&mut self, value: &[u8]) -> Result<u64, EmuError> {
        self.allocated = value.to_vec();
        Ok(7)
    }
}

fn read_image_resource(
    context: &mut ImageResourceContext,
) -> Result<Option<NativeValue>, EmuError> {
    let mut registry = NativeRegistry::new();
    register_natives(&mut registry).unwrap();
    registry.invoke(
        &NativeSignature::new(
            "javax/microedition/lcdui/Image",
            "readResource",
            "(Ljava/lang/String;)[B",
        ),
        context,
        &[NativeValue::Reference(Some(1))],
    )
}

#[test]
fn image_resource_uses_bounded_caller_package_fallback() {
    let mut context = ImageResourceContext {
        name: "logo.png".to_owned(),
        caller: Some("fixture/ui/Splash".to_owned()),
        resources: BTreeMap::from([("fixture/ui/logo.png".to_owned(), b"package image".to_vec())]),
        reads: RefCell::new(Vec::new()),
        allocated: Vec::new(),
    };

    assert_eq!(
        read_image_resource(&mut context).unwrap(),
        Some(NativeValue::Reference(Some(7)))
    );
    assert_eq!(*context.reads.borrow(), ["logo.png", "fixture/ui/logo.png"]);
    assert_eq!(context.allocated, b"package image");
}

#[test]
fn image_resource_preserves_root_precedence_and_absolute_names() {
    let mut context = ImageResourceContext {
        name: "title.png".to_owned(),
        caller: Some("fixture/ui/Menu".to_owned()),
        resources: BTreeMap::from([
            ("title.png".to_owned(), b"root image".to_vec()),
            ("fixture/ui/title.png".to_owned(), b"package image".to_vec()),
        ]),
        reads: RefCell::new(Vec::new()),
        allocated: Vec::new(),
    };
    read_image_resource(&mut context).unwrap();
    assert_eq!(*context.reads.borrow(), ["title.png"]);
    assert_eq!(context.allocated, b"root image");

    context.name = "/missing.png".to_owned();
    context.reads.borrow_mut().clear();
    context.allocated.clear();
    assert_eq!(
        read_image_resource(&mut context).unwrap(),
        Some(NativeValue::Reference(None))
    );
    assert_eq!(*context.reads.borrow(), ["missing.png"]);
    assert!(context.allocated.is_empty());
}

#[test]
fn malformed_image_bytes_are_guest_illegal_argument_errors() {
    let mut registry = NativeRegistry::new();
    register_natives(&mut registry).unwrap();
    let signature = NativeSignature::new("javax/microedition/lcdui/Image", "decodePng", "([BII)[I");
    let mut context = ImageDecodeContext {
        bytes: b"encrypted image".to_vec(),
        text_input_active: false,
        decoded: Vec::new(),
    };

    let malformed = registry
        .invoke(
            &signature,
            &mut context,
            &[
                NativeValue::Reference(Some(1)),
                NativeValue::Int(0),
                NativeValue::Int(15),
            ],
        )
        .unwrap_err();
    assert_eq!(malformed.category(), Category::Api);
    assert_eq!(malformed.code(), "illegal-argument");
    assert!(malformed.message().contains("unsupported image signature"));

    let invalid_range = registry
        .invoke(
            &signature,
            &mut context,
            &[
                NativeValue::Reference(Some(1)),
                NativeValue::Int(-1),
                NativeValue::Int(1),
            ],
        )
        .unwrap_err();
    assert_eq!(invalid_range.code(), "array-index-out-of-bounds-exception");
}

#[test]
fn image_decode_reads_only_the_requested_resource_region() {
    let mut bitmap = vec![0; 58];
    bitmap[..2].copy_from_slice(b"BM");
    for (offset, value) in [(2, 58_u32), (10, 54), (14, 40), (18, 1), (22, 1), (34, 4)] {
        bitmap[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bitmap[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bitmap[28..30].copy_from_slice(&24_u16.to_le_bytes());
    bitmap[54..58].copy_from_slice(&[0x33, 0x22, 0x11, 0]);
    let mut context = ImageDecodeContext {
        bytes: vec![0x7f; 16_384],
        text_input_active: false,
        decoded: Vec::new(),
    };
    context.bytes.extend(&bitmap);
    context.bytes.extend(b"next resource");
    let mut registry = NativeRegistry::new();
    register_natives(&mut registry).unwrap();
    let result = registry
        .invoke(
            &NativeSignature::new("javax/microedition/lcdui/Image", "decodePng", "([BII)[I"),
            &mut context,
            &[
                NativeValue::Reference(Some(1)),
                NativeValue::Int(16_384),
                NativeValue::Int(58),
            ],
        )
        .unwrap();
    assert_eq!(result, Some(NativeValue::Reference(Some(7))));
    assert_eq!(context.decoded, [1, 1, 0xff11_2233_u32.cast_signed()]);
}

#[test]
fn display_text_input_state_reaches_the_frontend_boundary() {
    let mut registry = NativeRegistry::new();
    register_natives(&mut registry).unwrap();
    let signature = NativeSignature::new(
        "javax/microedition/lcdui/Display",
        "__setTextInputActive",
        "(Z)V",
    );
    let mut context = ImageDecodeContext {
        bytes: Vec::new(),
        text_input_active: false,
        decoded: Vec::new(),
    };
    registry
        .invoke(&signature, &mut context, &[NativeValue::Int(1)])
        .unwrap();
    assert!(context.text_input_active);
    registry
        .invoke(&signature, &mut context, &[NativeValue::Int(0)])
        .unwrap();
    assert!(!context.text_input_active);
}
