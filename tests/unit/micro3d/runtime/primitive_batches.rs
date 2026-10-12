use super::*;

struct PrimitiveBatch {
    command: i32,
    coordinates: Vec<i32>,
    normals: Vec<i32>,
    colors: Vec<i32>,
}

impl PrimitiveBatch {
    fn new(quads: bool, per_vertex: bool) -> Self {
        let mut batch = Self {
            command: if quads { 0x0400_0800 } else { 0x0300_0800 }
                | if per_vertex { 0x0300 } else { 0x0200 },
            coordinates: Vec::new(),
            normals: Vec::new(),
            colors: Vec::new(),
        };
        let count = if quads { 4 } else { 3 };
        for primitive in 0..128 {
            let x = primitive % 16 * 4 - 30;
            let y = primitive / 16 * 4 - 14;
            for [dx, dy] in [[-1, -1], [1, -1], [1, 1], [-1, 1]].into_iter().take(count) {
                batch.coordinates.extend([x + dx, y + dy, 64]);
            }
            let normal = [(primitive % 7 - 3) * 512, (primitive % 5 - 2) * 512, 4096];
            for _ in 0..if per_vertex { count } else { 1 } {
                batch.normals.extend(normal);
            }
            batch.colors.push(0x0080_4020 + primitive * 67);
        }
        batch
    }

    fn draw(&self, runtime: &mut Runtime, light: bool, sphere: bool) {
        runtime
            .render_primitives(
                None,
                0,
                0,
                FigureLayoutState {
                    center: [32, 32],
                    scale: [4096, 4096],
                    projection: Projection::ParallelScale,
                    ..FigureLayoutState::default()
                },
                AffineTrans::new([4096, 512, 0, 0, 0, 3072, 0, 0, 0, 0, 5120, 0]),
                EffectState {
                    light: light.then_some(1),
                    sphere_texture: sphere.then_some(2),
                    ..EffectState::default()
                },
                PrimitiveData::new(
                    self.command | i32::from(light) | (i32::from(sphere) << 1),
                    128,
                    &self.coordinates,
                    &self.normals,
                    &[],
                    &self.colors,
                ),
            )
            .unwrap();
    }
}

fn batch_runtime() -> Runtime {
    let mut runtime = Runtime::new(16, 1 << 20, 64, 64).unwrap();
    runtime
        .create(
            1,
            ObjectKind::Light(LightState {
                direction: Vector3D::new(512, 0, 4096),
                ..LightState::default()
            }),
        )
        .unwrap();
    runtime
        .create(
            2,
            ObjectKind::Texture(TextureData {
                width: 8,
                height: 8,
                pixels: (0..64)
                    .map(|index| 0xff00_0000 | (index * 197_371))
                    .collect(),
                for_model: false,
                color_key: 0,
            }),
        )
        .unwrap();
    runtime
}

#[test]
fn shared_primitive_normals_match_expanded_vertex_normals() {
    for quads in [false, true] {
        for light in [false, true] {
            for sphere in [false, true] {
                let mut shared = batch_runtime();
                let mut expanded = batch_runtime();
                PrimitiveBatch::new(quads, false).draw(&mut shared, light, sphere);
                PrimitiveBatch::new(quads, true).draw(&mut expanded, light, sphere);
                assert!(shared.metrics().shaded_pixels > 0);
                assert_eq!(shared.target_pixels(), expanded.target_pixels());
                assert_eq!(shared.renderer.depth(), expanded.renderer.depth());
            }
        }
    }
}

#[test]
#[ignore = "manual primitive normal transformation throughput measurement"]
fn primitive_normal_throughput() {
    for quads in [false, true] {
        for per_vertex in [false, true] {
            let batch = PrimitiveBatch::new(quads, per_vertex);
            for (light, sphere) in [(false, false), (true, false), (false, true), (true, true)] {
                let mut runtime = batch_runtime();
                let background = [0xff00_0000; 64 * 64];
                let started = std::time::Instant::now();
                for _ in 0..256 {
                    runtime.load_target(64, 64, &background).unwrap();
                    batch.draw(&mut runtime, light, sphere);
                    std::hint::black_box(runtime.target_pixels());
                }
                let elapsed = started.elapsed();
                let checksum = runtime
                    .target_pixels()
                    .iter()
                    .chain(runtime.renderer.depth())
                    .fold(0_u64, |sum, value| {
                        sum.wrapping_mul(31).wrapping_add(u64::from(*value))
                    });
                eprintln!(
                    "quads={quads} per_vertex={per_vertex} light={light} sphere={sphere} elapsed={elapsed:?} checksum={checksum:016x}"
                );
            }
        }
    }
}
