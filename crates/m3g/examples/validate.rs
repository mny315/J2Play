use m3g::{LoaderLimits, M3gFile, Runtime, instantiate_file};
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: validate <scene.m3g>")?;
    let limits = LoaderLimits::default();
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(u64::try_from(limits.file_bytes)?.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > limits.file_bytes {
        return Err("M3G file exceeds the loader input limit".into());
    }
    let file = M3gFile::parse(&bytes, limits)?;
    let count = file.objects.len();
    let mut runtime = Runtime::default();
    let instantiated =
        instantiate_file(&file, &mut runtime, &vec![None; count], &vec![None; count])?;
    println!(
        "objects={} instantiated={} counters={:?}",
        count,
        instantiated.handles.iter().flatten().count(),
        runtime.counters()
    );
    Ok(())
}
