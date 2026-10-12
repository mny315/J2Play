use super::*;

#[test]
fn parked_call_chain_keeps_reference_slots_monitors_and_native_state_alive() {
    let method = method(&[0xb1], 1, 2);
    let mut heap = Heap::new(64 * 1024);
    let mut frames = Vec::new();
    let mut expected = Vec::new();
    for variant in 0..6 {
        let handles: [Handle; 6] =
            std::array::from_fn(|_| heap.allocate_object("Root", HashMap::new()).unwrap());
        expected.extend(handles);
        let native_resume = match variant {
            0 => NativeResume::FramePresentation {
                pixels: handles[5],
                deadline: 1,
            },
            1 => NativeResume::Join { target: handles[5] },
            2 => NativeResume::MonitorWait {
                object: handles[5],
                monitor_depth: 1,
            },
            3 => NativeResume::ClassNewInstance {
                instance: handles[5],
            },
            4 => NativeResume::DataInputReadUtf {
                input: handles[5],
                length: Some(1),
                bytes: vec![],
            },
            _ => NativeResume::InputStreamReaderClose { reader: handles[5] },
        };
        frames.push(SuspendedCall {
            method: method.clone(),
            locals: vec![
                None,
                Some(Value::Int(7)),
                Some(Value::Reference(Some(handles[0]))),
            ],
            stack: vec![
                Value::Reference(None),
                Value::Reference(Some(handles[1])),
                Value::Long(9),
            ],
            pc: 0,
            synchronized_monitor: Some(handles[2]),
            monitor_entry: Some(PendingMonitorEntry {
                object: handles[3],
                args: vec![Value::Int(11), Value::Reference(Some(handles[4]))],
            }),
            pending: None,
            native_resume: Some(native_resume),
            class_initialization: None,
        });
    }
    let mut chain = None;
    for mut frame in frames.into_iter().rev() {
        frame.pending = chain.map(|child| PendingCall { child, next_pc: 0 });
        chain = Some(Box::new(frame));
    }
    let mut roots = Vec::new();
    chain.unwrap().roots(&mut roots);
    assert_eq!(roots, expected);
    let unreferenced = heap.allocate_object("Garbage", HashMap::new()).unwrap();
    heap.collect(roots);
    assert!(expected.iter().all(|handle| heap.get(*handle).is_ok()));
    assert!(heap.get(unreferenced).is_err());
}
