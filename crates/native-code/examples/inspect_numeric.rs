//! Static compilation inspection only; never executes guest code.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;
    let path = std::env::args_os().nth(1).ok_or("expected JAR path")?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(64 * 1_024 * 1_024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1_024 * 1_024 {
        return Err("JAR exceeds inspection limit".into());
    }
    let mut program = vm::Program::new();
    let limits = vm::Limits::default();
    for resource in jar::read_class_entries_bytes(&bytes)? {
        program.add_class(&classfile::parse(&resource.bytes)?, &limits)?;
    }
    let mut compiler = native_code::Compiler::new();
    let prepared = program.prepare_integer_methods(&mut compiler, &|| false);
    println!("prepared={prepared} {:?}", compiler.preparation_stats());
    Ok(())
}
