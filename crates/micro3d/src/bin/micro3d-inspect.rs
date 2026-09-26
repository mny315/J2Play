use std::{env, fs::File, io, io::Read, process::ExitCode};

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;

fn read_bounded(mut reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("input exceeds {limit} byte limit"),
        ));
    }
    Ok(bytes)
}

fn figure_summary(value: &micro3d::FigureData) -> String {
    let bounds = value.vertices.iter().fold(
        ([i32::MAX; 3], [i32::MIN; 3]),
        |(mut minimum, mut maximum), vertex| {
            for (axis, component) in [vertex.x, vertex.y, vertex.z].into_iter().enumerate() {
                minimum[axis] = minimum[axis].min(component);
                maximum[axis] = maximum[axis].max(component);
            }
            (minimum, maximum)
        },
    );
    let colored_faces = value
        .faces
        .iter()
        .filter(|face| face.color.is_some())
        .count();
    let textured_faces = value.faces.len().saturating_sub(colored_faces);
    let colors = value
        .faces
        .iter()
        .filter_map(|face| face.color)
        .take(8)
        .collect::<Vec<_>>();
    let uv_bounds = value.faces.iter().flat_map(|face| face.uv).fold(
        ([u32::MAX; 2], [u32::MIN; 2]),
        |(mut minimum, mut maximum), uv| {
            for axis in 0..2 {
                minimum[axis] = minimum[axis].min(uv[axis]);
                maximum[axis] = maximum[axis].max(uv[axis]);
            }
            (minimum, maximum)
        },
    );
    let mut materials = value
        .faces
        .iter()
        .filter_map(|face| face.material)
        .collect::<Vec<_>>();
    materials.sort_unstable();
    materials.dedup();
    let normals = value.normals.iter().take(8).collect::<Vec<_>>();
    let mut attributes = value
        .faces
        .iter()
        .map(|face| face.attributes)
        .collect::<Vec<_>>();
    attributes.sort_unstable();
    attributes.dedup();
    let bones = value
        .bones
        .iter()
        .enumerate()
        .map(|(index, bone)| {
            format!(
                "{index}:vertices={} parent={} transform={:?}",
                bone.vertex_count, bone.parent, bone.transform.values
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "figure version={} vertices={} normals={} faces={} bones={} patterns={} materials={} material_slots={materials:?} bounds={:?}..{:?} uv_bounds={:?}..{:?} colored_faces={} textured_faces={} attributes={attributes:?} first_normals={normals:?} colors={colors:08x?} bone_table=[{bones}]",
        value.format_version,
        value.vertices.len(),
        value.normals.len(),
        value.faces.len(),
        value.bones.len(),
        value.pattern_count,
        value.material_count,
        bounds.0,
        bounds.1,
        uv_bounds.0,
        uv_bounds.1,
        colored_faces,
        textured_faces,
    )
}

fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let Some(kind) = arguments.next() else {
        eprintln!("usage: micro3d-inspect <figure|action|texture-model|texture-sprite> <file>");
        return ExitCode::from(2);
    };
    let Some(path) = arguments.next() else {
        eprintln!("micro3d-inspect: missing input file");
        return ExitCode::from(2);
    };
    let bytes = if path == "-" {
        read_bounded(std::io::stdin().lock(), MAX_INPUT_BYTES)
    } else {
        File::open(&path).and_then(|file| read_bounded(file, MAX_INPUT_BYTES))
    };
    let bytes = match bytes {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("micro3d-inspect: {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let limits = micro3d::LoaderLimits::default();
    let result = match kind.as_str() {
        "figure" => micro3d::FigureData::parse(&bytes, limits).map(|value| figure_summary(&value)),
        "action" => micro3d::ActionTableData::parse(&bytes, limits).map(|value| {
            format!(
                "action version={} actions={}",
                value.format_version,
                value.frame_counts.len()
            )
        }),
        "texture-model" | "texture-sprite" => {
            micro3d::TextureData::parse(&bytes, kind == "texture-model", limits).map(|value| {
                format!(
                    "texture width={} height={} pixels={}",
                    value.width,
                    value.height,
                    value.pixels.len()
                )
            })
        }
        _ => {
            eprintln!("micro3d-inspect: unknown kind {kind}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "{}[{}]: {}",
                error.category().as_str(),
                error.code(),
                error.message()
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/micro3d/bin/micro3d-inspect/mod.rs"]
mod tests;
