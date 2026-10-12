//! Project-owned differential performance fixture; never loads a commercial JAR.
#[path = "../../../tests/unit/native-code/support.rs"]
mod support;

use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut compiler = std::env::args_os()
        .nth(1)
        .map_or_else(native_code::Compiler::new, |root| {
            native_code::Compiler::with_cache(root.into())
        });
    let mut program = vm::Program::new();
    let limits = vm::Limits {
        max_instructions: 10_000_000,
        ..vm::Limits::default()
    };
    program.add_class(&support::fixture(10_000), &limits)?;
    let start = Instant::now();
    let interpreted = program.execute("NumericProbe", "run", "()I", limits.clone(), false)?;
    let interpreter_time = start.elapsed();
    let prepared = program.prepare_integer_methods(&mut compiler, &|| false);
    if prepared != 1 {
        return Err(format!("expected one numeric method, got {prepared}").into());
    }
    let start = Instant::now();
    let accelerated = program.execute("NumericProbe", "run", "()I", limits, false)?;
    let native_time = start.elapsed();
    if interpreted.value != accelerated.value
        || interpreted.instructions != accelerated.instructions
    {
        return Err(format!(
            "differential mismatch: {:?}/{} vs {:?}/{}",
            interpreted.value,
            interpreted.instructions,
            accelerated.value,
            accelerated.instructions
        )
        .into());
    }
    if compiler.run_stats().calls == 0 {
        return Err("no generated function executed".into());
    }
    println!(
        "architecture={} verified_value={:?} guest_bytecodes={}",
        std::env::consts::ARCH,
        accelerated.value,
        accelerated.instructions
    );
    println!(
        "interpreter_ms={:.3} native_ms={:.3} ratio={:.3}",
        interpreter_time.as_secs_f64() * 1000.0,
        native_time.as_secs_f64() * 1000.0,
        interpreter_time.as_secs_f64() / native_time.as_secs_f64()
    );
    println!(
        "preparation={:?} execution={:?}",
        compiler.preparation_stats(),
        compiler.run_stats()
    );
    Ok(())
}
