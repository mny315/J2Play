//! Attribute decoding and lighting shared by plain and deformed geometry.

use super::{
    EmuError, LightSource, Mat4, MaterialState, Vec3, Vec4, Vertex, VertexBufferState,
    geometry_error, vertex_bounds,
};

pub(super) struct VertexLighting {
    model: Mat4,
    camera_transform: Mat4,
    normal_transform: Option<Mat4>,
    camera_position: Vec3,
    lighting: crate::appearance::PreparedLighting,
    vertex_color_tracking: bool,
    alpha: f32,
}

impl VertexLighting {
    pub(super) fn new(
        model: Mat4,
        view_transform: Mat4,
        camera_position: Vec3,
        material: MaterialState,
        vertex_color_tracking: bool,
        lights: &[LightSource],
        alpha: f32,
    ) -> Result<Self, EmuError> {
        // Ambient and emissive light do not observe normals, so a model that
        // collapses an axis does not require an inverse in that case.
        let normal_transform = lights
            .iter()
            .any(|light| !matches!(light, LightSource::Ambient { .. }))
            .then(|| model.inverted().map(Mat4::transposed))
            .transpose()?;
        Ok(Self {
            model,
            camera_transform: view_transform.multiplied(model),
            normal_transform,
            camera_position,
            lighting: crate::appearance::PreparedLighting::new(material, lights)?,
            vertex_color_tracking,
            alpha,
        })
    }

    pub(super) fn transforms_normals(&self) -> bool {
        self.normal_transform.is_some()
    }

    pub(super) fn apply(&self, mut vertex: Vertex, normal: Vec4) -> Result<Vertex, EmuError> {
        let world = self.model.transform(vertex.position);
        let normal = self
            .normal_transform
            .map_or(normal, |transform| transform.transform(normal));
        let view = Vec3::new(
            self.camera_position.x - world.x,
            self.camera_position.y - world.y,
            self.camera_position.z - world.z,
        );
        let color = self.lighting.shade(
            vertex.color,
            self.vertex_color_tracking,
            Vec3::new(normal.x, normal.y, normal.z),
            Vec3::new(world.x, world.y, world.z),
            view,
        )?;
        vertex.color =
            ((((color >> 24) & 0xff) as f32 * self.alpha.clamp(0.0, 1.0)).round() as u32) << 24
                | color & 0x00ff_ffff;
        vertex.position = self.camera_transform.transform(vertex.position);
        Ok(vertex)
    }
}

impl VertexBufferState {
    /// Interpolates vertex normals in mesh coordinates and normalizes the result.
    /// Missing or cancelling normals have the deterministic undefined value +Z.
    pub fn interpolated_normal(
        &self,
        indices: [usize; 3],
        weights: [f32; 3],
    ) -> Result<Vec3, EmuError> {
        let Some(normals) = &self.normals else {
            return Ok(Vec3::new(0.0, 0.0, 1.0));
        };
        let [a, b, c] = indices;
        interpolate_normal(
            [normals.normal(a)?, normals.normal(b)?, normals.normal(c)?],
            weights,
        )
    }

    /// Produces transformed immediate vertices after validating required positions.
    pub fn transformed_vertices(&self, transform: Mat4) -> Result<Vec<Vertex>, EmuError> {
        let count = self.position_count()?;
        (0..count)
            .map(|index| {
                let mut vertex = self.vertex(index)?;
                vertex.position = transform.transform(vertex.position);
                Ok(vertex)
            })
            .collect()
    }

    /// Produces camera-space vertices lit from world-space normals and positions.
    pub fn transformed_lit_vertices(
        &self,
        model: Mat4,
        view_transform: Mat4,
        camera_position: Vec3,
        material: MaterialState,
        vertex_color_tracking: bool,
        lights: &[LightSource],
        alpha: f32,
    ) -> Result<Vec<Vertex>, EmuError> {
        let count = self.position_count()?;
        let normals = self.normals.as_ref().ok_or_else(|| {
            geometry_error("missing-normals", "lit VertexBuffer has no normal array")
        })?;
        let lighting = VertexLighting::new(
            model,
            view_transform,
            camera_position,
            material,
            vertex_color_tracking,
            lights,
            alpha,
        )?;
        (0..count)
            .map(|index| lighting.apply(self.vertex(index)?, normals.normal(index)?))
            .collect()
    }

    pub(super) fn position_count(&self) -> Result<usize, EmuError> {
        self.positions
            .as_ref()
            .map(|(array, ..)| array.vertex_count())
            .ok_or_else(|| {
                geometry_error("missing-positions", "VertexBuffer has no position array")
            })
    }

    pub(super) fn vertex(&self, index: usize) -> Result<Vertex, EmuError> {
        let (positions, scale, bias) = self.positions.as_ref().ok_or_else(|| {
            geometry_error("missing-positions", "VertexBuffer has no position array")
        })?;
        let [x, y, z, ..] = positions.vertex_components(index)? else {
            return Err(vertex_bounds());
        };
        let position = Vec4::new(
            f32::from(*x) * *scale + bias[0],
            f32::from(*y) * *scale + bias[1],
            f32::from(*z) * *scale + bias[2],
            1.0,
        );
        let color = if let Some(colors) = &self.colors {
            let [red, green, blue, alpha @ ..] = colors.vertex_components(index)? else {
                return Err(vertex_bounds());
            };
            let component = |value: i16| u32::from(value as u8);
            component(alpha.first().copied().unwrap_or(-1)) << 24
                | component(*red) << 16
                | component(*green) << 8
                | component(*blue)
        } else {
            self.default_color
        };
        let mut texture = [[0.0, 0.0, 1.0]; 2];
        for (coordinates, attribute) in texture.iter_mut().zip(&self.texture_coordinates) {
            if let Some((array, scale, bias)) = attribute {
                *coordinates = [0.0; 3];
                for ((coordinate, bias), component) in coordinates
                    .iter_mut()
                    .zip(bias)
                    .zip(array.vertex_components(index)?)
                {
                    *coordinate = f32::from(*component) * *scale + *bias;
                }
            }
        }
        Ok(Vertex::with_textures(position, color, texture))
    }
}

pub(super) fn interpolate_normal(normals: [Vec4; 3], weights: [f32; 3]) -> Result<Vec3, EmuError> {
    let components = normals.map(|normal| [normal.x, normal.y, normal.z]);
    let [x, y, z] = std::array::from_fn(|component| {
        (0..3)
            .map(|vertex| f64::from(weights[vertex]) * f64::from(components[vertex][component]))
            .sum::<f64>() as f32
    });
    match Vec3::new(x, y, z).normalized() {
        Ok(normal) => Ok(normal),
        Err(error) if error.code() == "zero-length" => Ok(Vec3::new(0.0, 0.0, 1.0)),
        Err(error) => Err(error),
    }
}
