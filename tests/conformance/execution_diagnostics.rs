use crate::support::guest::{utf8, write_fixture};

#[test]
fn static_execution_captures_return_value_trace_and_flight_events() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "static_execution_captures_return_value_trace_and_flight_events"
    )) {
        return;
    }
    let scratch = crate::support::Scratch::new();
    let directory = &scratch.0;
    let jar_path = directory.join("fixture.jar");
    write_fixture(
        &jar_path,
        b"Manifest-Version: 1.0\r\n\r\n",
        "Fixture.class",
        &static_fixture_class(),
    );

    let output = crate::support::Fixture::new(&jar_path)
        .trace()
        .run_static("Fixture", "main", "()I");
    assert!(output.success(), "{}", output.diagnostics);
    let diagnostics = &output.diagnostics;
    assert_eq!(output.int_value(), Some(42), "{diagnostics}");
    assert_eq!(output.execution.as_ref().unwrap().instructions, 2);
    let stderr = &output.diagnostics;
    assert!(stderr.contains("bipush"), "{stderr}");
    let log = output.flight_events.join("\n");
    assert!(log.contains("call-enter Fixture::main()I args=[]"), "{log}");
    assert!(
        log.contains("call-exit Fixture::main()I return Some(Int(42))"),
        "{log}"
    );
}

fn static_fixture_class() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0xcafe_babe_u32.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&50_u16.to_be_bytes());
    bytes.extend_from_slice(&8_u16.to_be_bytes());
    utf8(&mut bytes, "Fixture");
    bytes.extend_from_slice(&[7, 0, 1]);
    utf8(&mut bytes, "java/lang/Object");
    bytes.extend_from_slice(&[7, 0, 3]);
    utf8(&mut bytes, "main");
    utf8(&mut bytes, "()I");
    utf8(&mut bytes, "Code");
    bytes.extend_from_slice(&0x0021_u16.to_be_bytes());
    bytes.extend_from_slice(&2_u16.to_be_bytes());
    bytes.extend_from_slice(&4_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&0x0009_u16.to_be_bytes());
    bytes.extend_from_slice(&5_u16.to_be_bytes());
    bytes.extend_from_slice(&6_u16.to_be_bytes());
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&7_u16.to_be_bytes());
    bytes.extend_from_slice(&15_u32.to_be_bytes());
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&3_u32.to_be_bytes());
    bytes.extend_from_slice(&[0x10, 42, 0xac]);
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes.extend_from_slice(&0_u16.to_be_bytes());
    bytes
}
