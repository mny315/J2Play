//! Scene picking, hit attributes and camera-facing sprite intersections.

use super::super::{M3gSpritePickContext, m3g_sprite_crop_sample};
use super::{
    EmuError, GeometryWork, Machine, m3g_scaled_sprite_half_extent, m3g_sprite_crop_coordinate,
    scene_children, type_error,
};

struct PickHit {
    node: m3g::Handle,
    submesh: usize,
    distance: f32,
    texture: [[f32; 2]; 2],
    normal: m3g::Vec3,
}

struct MeshHit {
    submesh: usize,
    appearance: m3g::Handle,
    indices: [usize; 3],
    hit: m3g::RayHit,
}

impl Machine<'_, '_> {
    pub(in crate::machine) fn m3g_pick_ray(
        &mut self,
        group: m3g::Handle,
        scope: u32,
        origin: m3g::Vec3,
        direction: m3g::Vec3,
        result: Option<m3g::Handle>,
        sprite_context: Option<M3gSpritePickContext>,
    ) -> Result<bool, EmuError> {
        self.m3g_check_geometry_cancellation()?;
        let mut work = GeometryWork::new(&self.limits);
        if self.m3g.runtime.children(group).is_err() {
            return Err(type_error());
        }
        let ray = m3g::Ray::new(origin, direction)?;
        let (_, root_picking, _, _) = self.m3g.runtime.node_state(group)?;
        if !root_picking {
            return Ok(false);
        }
        let mut pending = self
            .m3g
            .runtime
            .children(group)?
            .iter()
            .rev()
            .copied()
            .map(|node| Ok((node, self.m3g.runtime.composite_transform(node)?)))
            .collect::<Result<Vec<_>, EmuError>>()?;
        let mut visited = std::collections::BTreeSet::new();
        let mut best: Option<PickHit> = None;
        while let Some((node, transform)) = pending.pop() {
            self.m3g_check_geometry_cancellation()?;
            if !visited.insert(node) {
                continue;
            }
            let (_, node_picking, node_scope, _) = self.m3g.runtime.node_state(node)?;
            if !node_picking {
                continue;
            }
            for &child in scene_children(self.m3g.runtime.kind(node)?).iter().rev() {
                pending.push((
                    child,
                    transform.multiplied(self.m3g.runtime.composite_transform(child)?),
                ));
            }
            if node_scope & scope == 0 {
                continue;
            }
            if let m3g::ObjectKind::Sprite3D(sprite) = self.m3g.runtime.kind(node)? {
                if let Some(context) = sprite_context
                    && let Some((distance, texture)) = self
                        .m3g_pick_sprite(sprite, transform, origin, direction, context, &mut work)?
                    && best.as_ref().is_none_or(|best| distance < best.distance)
                {
                    best = Some(PickHit {
                        node,
                        submesh: 0,
                        distance,
                        texture: [texture, texture],
                        normal: m3g::Vec3::new(0.0, 0.0, 1.0),
                    });
                }
                continue;
            }
            let Some((mesh, geometry)) = self.m3g_retained_geometry(node, &mut work)? else {
                continue;
            };
            let transformed = geometry.transformed_vertices(transform, &mut work)?;
            let mut candidate: Option<MeshHit> = None;
            for (submesh_index, (indices, appearance)) in mesh
                .submeshes
                .iter()
                .copied()
                .zip(mesh.appearances.iter().copied())
                .enumerate()
            {
                let Some(appearance) = appearance else {
                    continue;
                };
                let indices = match self.m3g.runtime.kind(indices)? {
                    m3g::ObjectKind::TriangleStripArray(indices) => indices,
                    _ => return Err(type_error()),
                };
                work.indices(indices.index_count())?;
                let polygon = {
                    let m3g::ObjectKind::Appearance(appearance) =
                        self.m3g.runtime.kind(appearance)?
                    else {
                        return Err(type_error());
                    };
                    if let Some(polygon) = appearance.polygon_mode {
                        let m3g::ObjectKind::PolygonMode(polygon) =
                            self.m3g.runtime.kind(polygon)?
                        else {
                            return Err(type_error());
                        };
                        *polygon
                    } else {
                        m3g::PolygonModeState::default()
                    }
                };
                for (index, triangle) in indices.triangles(transformed.len())?.enumerate() {
                    if index % 256 == 0 {
                        self.m3g_check_geometry_cancellation()?;
                    }
                    let positions = triangle.map(|index| {
                        let value = transformed[index].position;
                        let inverse = if value.w.abs() > f32::EPSILON {
                            value.w.recip()
                        } else {
                            1.0
                        };
                        m3g::Vec3::new(value.x * inverse, value.y * inverse, value.z * inverse)
                    });
                    let Some(hit) = ray.intersect_triangle(positions)? else {
                        continue;
                    };
                    let front_facing = if polygon.winding == 168 {
                        hit.front_facing
                    } else {
                        !hit.front_facing
                    };
                    if polygon.culling == 160 && !front_facing
                        || polygon.culling == 161 && front_facing
                    {
                        continue;
                    }
                    if best
                        .as_ref()
                        .is_none_or(|best| hit.distance < best.distance)
                        && candidate
                            .as_ref()
                            .is_none_or(|best| hit.distance < best.hit.distance)
                    {
                        candidate = Some(MeshHit {
                            submesh: submesh_index,
                            appearance,
                            indices: triangle,
                            hit,
                        });
                    }
                }
            }
            if let Some(candidate) = candidate {
                let weights = [
                    1.0 - candidate.hit.u - candidate.hit.v,
                    candidate.hit.u,
                    candidate.hit.v,
                ];
                let (texture, normal) = if result.is_some() {
                    self.m3g_check_geometry_cancellation()?;
                    let texture = candidate.indices.map(|index| transformed[index].texture);
                    (
                        self.m3g_pick_texture(candidate.appearance, texture, weights)?,
                        geometry.interpolated_normal(candidate.indices, weights, &mut work)?,
                    )
                } else {
                    ([[0.0; 2]; 2], m3g::Vec3::new(0.0, 0.0, 1.0))
                };
                best = Some(PickHit {
                    node,
                    submesh: candidate.submesh,
                    distance: candidate.hit.distance,
                    texture,
                    normal,
                });
            }
        }
        self.m3g_check_geometry_cancellation()?;
        let Some(PickHit {
            node,
            submesh,
            distance,
            texture,
            normal,
        }) = best
        else {
            return Ok(false);
        };
        if let Some(result) = result {
            let m3g::ObjectKind::RayIntersection(state) = self.m3g.runtime.kind_mut(result)? else {
                return Err(type_error());
            };
            state.intersected = Some(node);
            state.ray = [
                origin.x,
                origin.y,
                origin.z,
                direction.x,
                direction.y,
                direction.z,
            ];
            state.distance = distance;
            state.submesh = i32::try_from(submesh).unwrap_or(i32::MAX);
            state.texture = texture;
            state.normal = normal;
        }
        Ok(true)
    }

    fn m3g_pick_texture(
        &self,
        appearance: m3g::Handle,
        vertices: [[[f32; 3]; 2]; 3],
        weights: [f32; 3],
    ) -> Result<[[f32; 2]; 2], EmuError> {
        let m3g::ObjectKind::Appearance(appearance) = self.m3g.runtime.kind(appearance)? else {
            return Err(type_error());
        };
        let mut texture = [[0.0; 2]; 2];
        for (unit, coordinates) in texture.iter_mut().enumerate() {
            let Some(handle) = appearance.textures[unit] else {
                continue;
            };
            if !matches!(
                self.m3g.runtime.kind(handle)?,
                m3g::ObjectKind::Texture2D(_)
            ) {
                return Err(type_error());
            }
            let [s, t, r] = std::array::from_fn(|component| {
                weights[0].mul_add(
                    vertices[0][unit][component],
                    weights[1].mul_add(
                        vertices[1][unit][component],
                        weights[2] * vertices[2][unit][component],
                    ),
                )
            });
            // Linear interpolation commutes with the texture matrix. The
            // homogeneous divide follows both operations, before any wrapping.
            let value = self
                .m3g
                .runtime
                .composite_transform(handle)?
                .transform(m3g::Vec4::new(s, t, r, 1.0));
            *coordinates = [value.x / value.w, value.y / value.w];
        }
        Ok(texture)
    }

    fn m3g_pick_sprite(
        &self,
        sprite: &m3g::SpriteState,
        transform: m3g::Mat4,
        origin: m3g::Vec3,
        direction: m3g::Vec3,
        context: M3gSpritePickContext,
        work: &mut GeometryWork,
    ) -> Result<Option<(f32, [f32; 2])>, EmuError> {
        let Some(appearance) = sprite.appearance else {
            return Ok(None);
        };
        if !sprite.scaled {
            return Ok(None);
        }
        if sprite.crop[2] == 0 || sprite.crop[3] == 0 {
            return Ok(None);
        }
        work.vertices(4)?;
        let model_view = context.group_to_camera.multiplied(transform);
        let center_camera = model_view.transform(m3g::Vec4::new(0.0, 0.0, 0.0, 1.0));
        let center_clip = context.projection.transform(center_camera);
        if center_clip.w == 0.0 {
            return Ok(None);
        }
        let (half_width, half_height) =
            m3g_scaled_sprite_half_extent(context.projection, model_view, center_camera)?;
        if half_width <= 0.0 || half_height <= 0.0 {
            return Ok(None);
        }
        let inverse_w = center_clip.w.recip();
        let center_x = (center_clip.x * inverse_w + 1.0) * 0.5;
        let center_y = (1.0 - center_clip.y * inverse_w) * 0.5;
        let u = (context.viewport_point[0] - center_x) / (2.0 * half_width) + 0.5;
        let v = (context.viewport_point[1] - center_y) / (2.0 * half_height) + 0.5;
        if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
            return Ok(None);
        }
        let m3g::ObjectKind::Image2D(image) = self.m3g.runtime.kind(sprite.image)? else {
            return Err(type_error());
        };
        let source_x = m3g_sprite_crop_sample(sprite.crop[0], sprite.crop[2], u);
        let source_y = m3g_sprite_crop_sample(sprite.crop[1], sprite.crop[3], v);
        if source_x < 0
            || source_y < 0
            || source_x >= image.width() as i32
            || source_y >= image.height() as i32
        {
            return Ok(None);
        }
        let source_index = source_y as usize * image.width() as usize + source_x as usize;
        let alpha = (image.pixels()[source_index] >> 24) as u8;
        let m3g::ObjectKind::Appearance(appearance) = self.m3g.runtime.kind(appearance)? else {
            return Err(type_error());
        };
        let alpha_threshold = if let Some(compositing) = appearance.compositing_mode {
            let m3g::ObjectKind::CompositingMode(compositing) =
                self.m3g.runtime.kind(compositing)?
            else {
                return Err(type_error());
            };
            (compositing.alpha_threshold * 255.0).round() as u8
        } else {
            0
        };
        if alpha < alpha_threshold {
            return Ok(None);
        }
        // A Sprite3D is a plane at its camera-space Z. Projecting the
        // center onto an oblique ray gives a different, incorrect depth.
        let camera_origin = context
            .group_to_camera
            .transform(m3g::Vec4::new(origin.x, origin.y, origin.z, 1.0));
        let camera_direction = context.group_to_camera.transform(m3g::Vec4::new(
            direction.x,
            direction.y,
            direction.z,
            0.0,
        ));
        let distance = (center_camera.z - camera_origin.z) / camera_direction.z;
        if !distance.is_finite() || distance < 0.0 {
            return Ok(None);
        }
        let texture = [
            m3g_sprite_crop_coordinate(sprite.crop[0], sprite.crop[2], u) / image.width() as f32,
            m3g_sprite_crop_coordinate(sprite.crop[1], sprite.crop[3], v) / image.height() as f32,
        ];
        Ok(Some((distance, texture)))
    }
}
