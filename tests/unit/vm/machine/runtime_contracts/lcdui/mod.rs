use super::program_with_lcdui_natives as program;
use super::*;

mod choice_flags;
mod gauge;

fn invoke(
    machine: &mut Machine<'_, '_>,
    class: &str,
    receiver: Handle,
    name: &str,
    descriptor: &str,
    args: &[Value],
) -> CallOutcome {
    let method = &machine.program.methods[&MethodKey {
        class: class.into(),
        name: name.into(),
        descriptor: descriptor.into(),
    }];
    machine
        .call(
            method,
            std::iter::once(Value::Reference(Some(receiver)))
                .chain(args.iter().copied())
                .collect::<Vec<_>>(),
            1,
        )
        .unwrap()
}

fn invoke_collecting(
    machine: &mut Machine<'_, '_>,
    class: &str,
    receiver: Handle,
    name: &str,
    descriptor: &str,
    args: &[Value],
    quantum: u64,
) -> CallOutcome {
    machine.scheduler.quantum_remaining = quantum;
    let mut outcome = invoke(machine, class, receiver, name, descriptor, args);
    let mut yields = 0;
    while let CallOutcome::Suspend(continuation) = outcome {
        assert!(yields < 4096, "{class}.{name} failed to progress");
        let mut roots = machine.roots(&[], args);
        roots.push(receiver);
        continuation.roots(&mut roots);
        machine.collect_heap(roots);
        machine.scheduler.quantum_remaining = quantum;
        outcome = machine.resume_suspended_call(continuation, 1).unwrap();
        yields += 1;
    }
    if quantum == 1 {
        assert!(yields > 0);
    }
    outcome
}
