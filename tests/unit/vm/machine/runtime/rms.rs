use super::*;

mod enumeration;

const STORE: &str = "javax/microedition/rms/RecordStore";
const LARGE_RECORD_BYTES: usize = 32_768;

fn record_length(arguments: &[NativeValue]) -> Result<usize, EmuError> {
    let [NativeValue::Long(7), NativeValue::Int(id)] = arguments else {
        panic!("record operation received the wrong handle or arguments");
    };
    match id {
        1 => Ok(LARGE_RECORD_BYTES),
        2 => Ok(0),
        _ => Err(EmuError::new(
            Category::Api,
            "invalid-record-id",
            "record does not exist",
        )),
    }
}

#[test]
fn record_size_does_not_allocate_a_record_array_and_preserves_errors() {
    let mut program = Program::new();
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    for entry in runtime_bootstrap::production_bootstrap_inventory_for_display(1, 1) {
        program.add_class(&entry.class, &Limits::default()).unwrap();
    }
    program
        .native_registry_mut()
        .register(
            NativeSignature::new(STORE, "get0", "(JI)[B"),
            |context, arguments| {
                let length = record_length(arguments)?;
                if length == 0 {
                    Ok(Some(NativeValue::Reference(None)))
                } else {
                    context
                        .allocate_java_byte_array(&vec![0; length])
                        .map(|reference| Some(NativeValue::Reference(Some(reference))))
                }
            },
        )
        .unwrap();
    program
        .native_registry_mut()
        .register(
            NativeSignature::new(STORE, "recordSize0", "(JI)I"),
            |_, arguments| {
                record_length(arguments).map(|length| Some(NativeValue::Int(length as i32)))
            },
        )
        .unwrap();
    let size_method = program.methods[&MethodKey::new(STORE, "getRecordSize", "(I)I")].clone();

    for tracing in [false, true] {
        for open_count in [1, 0] {
            for record_id in [1, 2, 0, -1, i32::MAX] {
                let mut host = DefaultNativeContext;
                let mut machine = program.machine(
                    Limits {
                        max_heap_bytes: 16_384,
                        ..Limits::default()
                    },
                    tracing,
                    &mut host,
                );
                let store = machine
                    .heap
                    .managed
                    .allocate_object(
                        STORE,
                        HashMap::from([
                            (format!("{STORE}.openCount:I"), HeapValue::Int(open_count)),
                            (format!("{STORE}.handle:J"), HeapValue::Long(7)),
                        ]),
                    )
                    .unwrap();
                let outcome = machine.call(
                    &size_method,
                    [Value::Reference(Some(store)), Value::Int(record_id)],
                    1,
                );
                let actual = match outcome {
                    Ok(CallOutcome::Return(Some(Value::Int(size)))) => format!("size:{size}"),
                    Ok(CallOutcome::Throw(exception)) => {
                        machine.object_class(exception).unwrap().into_owned()
                    }
                    Err(error) => error.code().into(),
                    _ => panic!("unexpected getRecordSize outcome"),
                };
                let expected = if open_count == 0 {
                    "javax/microedition/rms/RecordStoreNotOpenException".into()
                } else {
                    match record_id {
                        1 => format!("size:{LARGE_RECORD_BYTES}"),
                        2 => "size:0".into(),
                        _ => "javax/microedition/rms/InvalidRecordIDException".into(),
                    }
                };
                assert_eq!(
                    actual, expected,
                    "tracing={tracing}, open={open_count}, record={record_id}"
                );
            }
        }
    }
}
