use super::*;
use natives::{HostServices, VmAccess};
use std::cell::Cell;

#[derive(Default)]
struct HttpContext {
    headers: String,
    requests: Vec<GcfHttpRequest>,
    body_reads: Cell<usize>,
    now_millis: i64,
}

impl HostServices for HttpContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        self.now_millis
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }

    fn gcf_http(&mut self, request: GcfHttpRequest) -> Result<GcfHttpResponse, EmuError> {
        self.requests.push(request);
        Ok(GcfHttpResponse {
            status: 200,
            reason: "OK".to_owned(),
            headers: Vec::new(),
            body: Vec::new(),
        })
    }
}

impl VmAccess for HttpContext {
    fn read_java_string(&self, reference: u64) -> Result<String, EmuError> {
        Ok(match reference {
            1 => "http://example.com/".to_owned(),
            2 => "POST".to_owned(),
            3 => self.headers.clone(),
            _ => panic!("unexpected string reference"),
        })
    }

    fn read_java_byte_array(&self, reference: u64) -> Result<Vec<u8>, EmuError> {
        assert_eq!(reference, 4);
        self.body_reads.set(self.body_reads.get() + 1);
        Ok(vec![42])
    }

    fn allocate_java_byte_array(&mut self, _: &[u8]) -> Result<u64, EmuError> {
        Ok(5)
    }
}

#[test]
fn date_bridge_uses_the_host_clock_for_two_digit_years() {
    let mut registry = NativeRegistry::new();
    register_natives(&mut registry).unwrap();
    let signature = NativeSignature::new(
        "javax/microedition/io/HttpConnectionImpl",
        "parseDate0",
        "(Ljava/lang/String;)J",
    );
    for (now_millis, expected) in [(0, 784_111_777_000), (3_155_760_000_000, 3_939_871_777_000)] {
        let mut context = HttpContext {
            headers: "Sunday, 06-Nov-94 08:49:37 GMT".to_owned(),
            now_millis,
            ..HttpContext::default()
        };
        assert_eq!(
            registry
                .invoke(&signature, &mut context, &[NativeValue::Reference(Some(3))])
                .unwrap(),
            Some(NativeValue::Long(expected)),
        );
    }
}

#[test]
fn http_bridge_bounds_headers_before_reading_the_body_or_calling_the_host() {
    let mut registry = NativeRegistry::new();
    register_natives(&mut registry).unwrap();
    let signature = NativeSignature::new(
        "javax/microedition/io/HttpConnectionImpl",
        "request0",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;[B)[B",
    );
    let arguments = [1, 2, 3, 4].map(|reference| NativeValue::Reference(Some(reference)));
    for (headers, code) in [
        (vec!["X:"; 65].join("\n"), "network-limit"),
        (format!("X:{}", "a".repeat(32_768)), "network-limit"),
        (format!("X:{}", "é".repeat(16_384)), "network-limit"),
        (format!("X:{}", "a".repeat(1_048_576)), "network-limit"),
        ("X:bad\rvalue".to_owned(), "network-header"),
        ("malformed".to_owned(), "network-header"),
    ] {
        let mut context = HttpContext {
            headers,
            ..HttpContext::default()
        };
        let error = registry
            .invoke(&signature, &mut context, &arguments)
            .unwrap_err();
        assert_eq!(error.code(), code);
        assert_eq!(context.body_reads.get(), 0);
        assert!(context.requests.is_empty());
    }
}

#[test]
fn flattened_headers_preserve_order_duplicates_and_exact_byte_and_count_limits() {
    assert!(parse_flat_headers("").unwrap().is_empty());
    assert_eq!(
        parse_flat_headers("X:first\nX:\nY:é:界").unwrap(),
        [
            ("X".to_owned(), "first".to_owned()),
            ("X".to_owned(), String::new()),
            ("Y".to_owned(), "é:界".to_owned()),
        ]
    );
    let value = format!("X:{}", "a".repeat(511));
    let flattened = vec![value; 64].join("\n");
    assert_eq!(flattened.len(), 32_768 + 127);
    let headers = parse_flat_headers(&flattened).unwrap();
    assert_eq!(headers.len(), 64);
    assert!(
        headers
            .iter()
            .all(|(name, value)| name == "X" && value.len() == 511)
    );
}

#[test]
fn response_strings_use_modified_utf8_with_an_exact_length_prefix() {
    let mut output = vec![42];
    write_utf(&mut output, "A\0é界🙂").unwrap();
    assert_eq!(
        output,
        [
            42, 0, 14, b'A', 0xc0, 0x80, 0xc3, 0xa9, 0xe7, 0x95, 0x8c, 0xed, 0xa0, 0xbd, 0xed,
            0xb9, 0x82
        ]
    );
    write_utf(&mut output, "").unwrap();
    assert_eq!(&output[output.len() - 2..], [0, 0]);
}

#[test]
fn oversized_response_strings_are_rejected_before_modifying_the_output() {
    let mut output = vec![42];
    write_utf(&mut output, &"界".repeat(21_845)).unwrap();
    assert_eq!(&output[..3], [42, 255, 255]);
    assert_eq!(output.len(), 1 + 2 + 65_535);
    for value in ["a".repeat(65_536), "\0".repeat(32_768), "界".repeat(21_846)] {
        let mut output = vec![42];
        assert_eq!(
            write_utf(&mut output, &value).unwrap_err().code(),
            "network-limit"
        );
        assert_eq!(output, [42]);
    }
}
