use super::{
    EmuError, Machine, m3g_scaled_sprite_half_extent, m3g_sprite_crop_coordinate, type_error,
    vm_error,
};

mod geometry;
mod picking;
use super::GeometryWork;
use geometry::{MeshRenderSource, PreparedMesh, mesh_state};

fn scene_children(kind: &m3g::ObjectKind) -> &[m3g::Handle] {
    match kind {
        m3g::ObjectKind::Group { children, .. } | m3g::ObjectKind::World { children, .. } => {
            children
        }
        m3g::ObjectKind::SkinnedMesh { skeleton, .. } => std::slice::from_ref(skeleton),
        _ => &[],
    }
}

impl Machine<'_, '_> {
    pub(in crate::machine) fn m3g_render_retained(
        &mut self,
        root: m3g::Handle,
        supplied_transform: Option<m3g::Mat4>,
        world_mode: bool,
    ) -> Result<(), EmuError> {
        self.m3g_check_geometry_cancellation()?;
        let mut work = GeometryWork::new(&self.limits);
        if self.m3g.graphics.target.is_none() {
            return Err(vm_error(
                "illegal-state-exception",
                "Graphics3D render requires a bound target",
            ));
        }
        // Some commercial MIDP engines interleave LCDUI and M3G operations
        // while a Graphics target remains bound.  Although JSR-184 leaves that
        // usage unpredictable, eager synchronization matches the devices they
        // target and prevents valid M3G output from remaining in a private
        // back buffer until a much later releaseTarget call.
        self.m3g_refresh_bound_target()?;
        let render_started = std::time::Instant::now();
        if world_mode {
            let (camera, background) = match self.m3g.runtime.kind(root)? {
                m3g::ObjectKind::World {
                    camera, background, ..
                } => (*camera, *background),
                _ => return Err(type_error()),
            };
            let camera = camera
                .ok_or_else(|| vm_error("illegal-state-exception", "World has no active camera"))?;
            if !self.m3g.runtime.contains_node(root, camera)? {
                return Err(vm_error(
                    "illegal-state-exception",
                    "World active camera is not in that World",
                ));
            }
            let camera_guest = self
                .m3g_guest_handle(Some(camera))
                .ok_or_else(|| vm_error("illegal-state-exception", "camera lost guest binding"))?;
            let camera_transform = self.m3g.runtime.transform_to(camera, root)?;
            self.m3g.graphics.camera = Some((camera_guest, camera_transform));
            if let Some(background) = background {
                let m3g::ObjectKind::Background(background) = self.m3g.runtime.kind(background)?
                else {
                    return Err(type_error());
                };
                self.m3g.graphics.renderer.clear(
                    background.color_clear.then_some(background.color),
                    background.depth_clear,
                );
                if background.color_clear
                    && let Some(image) = background.image
                {
                    let m3g::ObjectKind::Image2D(image) = self.m3g.runtime.kind(image)? else {
                        return Err(type_error());
                    };
                    self.m3g.graphics.renderer.draw_background(
                        image,
                        background.crop,
                        background.mode_x == 33,
                        background.mode_y == 33,
                    )?;
                }
            } else {
                self.m3g.graphics.renderer.clear(Some(0), true);
            }
        }
        let (camera_guest, camera_transform) = self.m3g.graphics.camera.ok_or_else(|| {
            vm_error("illegal-state-exception", "Graphics3D has no active camera")
        })?;
        let camera = self.m3g_handle(camera_guest)?;
        let projection = match self.m3g.runtime.kind(camera)? {
            m3g::ObjectKind::Camera { projection, .. } => *projection,
            _ => return Err(type_error()),
        };
        let (_, _, camera_scope, _) = self.m3g.runtime.node_state(camera)?;
        let projection = projection.render_matrix()?;
        let view_transform = camera_transform.inverted()?;
        let root_transform = supplied_transform.unwrap_or(m3g::Mat4::IDENTITY);
        let mut pending = vec![(root, root_transform, 1.0_f32)];
        let mut jobs = Vec::new();
        let mut mesh_sources = Vec::new();
        let mut sprite_jobs = Vec::new();
        let mut render_order = 0_usize;
        let mut retained_lights = Vec::new();
        let mut visited = std::collections::BTreeSet::new();
        while let Some((node, model, ancestor_alpha)) = pending.pop() {
            self.m3g_check_geometry_cancellation()?;
            if !visited.insert(node) {
                continue;
            }
            let (rendering, _, node_scope, local_alpha) = self.m3g.runtime.node_state(node)?;
            if !rendering {
                continue;
            }
            // Alpha only modifies fragment alpha. Even zero-alpha geometry may
            // write color/depth, and Light/Camera nodes ignore alpha entirely.
            let alpha = ancestor_alpha * local_alpha;
            let kind = self.m3g.runtime.kind(node)?;
            for &child in scene_children(kind).iter().rev() {
                let child_transform =
                    model.multiplied(self.m3g.runtime.composite_transform(child)?);
                pending.push((child, child_transform, alpha));
            }
            if world_mode && matches!(kind, m3g::ObjectKind::Light { .. }) {
                if let Some(guest) = self.m3g_guest_handle(Some(node)) {
                    retained_lights.push((guest, model));
                }
                continue;
            }
            // Node scopes are independent of the scene hierarchy. Groups must
            // still be traversed, but their renderable descendants are only
            // visible when their own scope overlaps the active Camera scope.
            if node_scope & camera_scope == 0 {
                continue;
            }
            if let Some(mesh) =
                mesh_state(kind).filter(|mesh| mesh.appearances.iter().any(Option::is_some))
            {
                let source = mesh_sources.len();
                mesh_sources.push(MeshRenderSource {
                    node,
                    model,
                    scope: node_scope,
                    alpha,
                });
                for (indices, appearance) in mesh
                    .submeshes
                    .iter()
                    .copied()
                    .zip(mesh.appearances.iter().copied())
                {
                    let Some(appearance) = appearance else {
                        continue;
                    };
                    let indices = match self.m3g.runtime.kind(indices)? {
                        m3g::ObjectKind::TriangleStripArray(state) => state.clone(),
                        _ => return Err(type_error()),
                    };
                    let (layer, blended) = self.m3g_appearance_order(appearance)?;
                    jobs.push((layer, blended, render_order, source, indices, appearance));
                    render_order = render_order.saturating_add(1);
                }
            }
            if let m3g::ObjectKind::Sprite3D(sprite) = kind {
                let Some(appearance) = sprite.appearance else {
                    continue;
                };
                let m3g::ObjectKind::Image2D(image) = self.m3g.runtime.kind(sprite.image)? else {
                    return Err(type_error());
                };
                let image = image.clone();
                let crop_width = sprite.crop[2].unsigned_abs();
                let crop_height = sprite.crop[3].unsigned_abs();
                let model_view = view_transform.multiplied(model);
                let center_camera = model_view.transform(m3g::Vec4::new(0.0, 0.0, 0.0, 1.0));
                let center = projection.transform(center_camera);
                if crop_width != 0 && crop_height != 0 && center.w != 0.0 {
                    let width = self.m3g.graphics.viewport[2].max(1) as f32;
                    let height = self.m3g.graphics.viewport[3].max(1) as f32;
                    let (half_width, half_height) = if sprite.scaled {
                        m3g_scaled_sprite_half_extent(projection, model_view, center_camera)?
                    } else {
                        (crop_width as f32 / width, crop_height as f32 / height)
                    };
                    let dx = half_width * center.w;
                    let dy = half_height * center.w;
                    let left = m3g_sprite_crop_coordinate(sprite.crop[0], sprite.crop[2], 0.0)
                        / image.width() as f32;
                    let top = m3g_sprite_crop_coordinate(sprite.crop[1], sprite.crop[3], 0.0)
                        / image.height() as f32;
                    let right = m3g_sprite_crop_coordinate(sprite.crop[0], sprite.crop[2], 1.0)
                        / image.width() as f32;
                    let bottom = m3g_sprite_crop_coordinate(sprite.crop[1], sprite.crop[3], 1.0)
                        / image.height() as f32;
                    let color =
                        ((alpha.clamp(0.0, 1.0) * 255.0).round() as u32) << 24 | 0x00ff_ffff;
                    let mut quad = [
                        m3g::Vertex::textured(
                            m3g::Vec4::new(center.x - dx, center.y - dy, center.z, center.w),
                            color,
                            left,
                            bottom,
                        ),
                        m3g::Vertex::textured(
                            m3g::Vec4::new(center.x + dx, center.y - dy, center.z, center.w),
                            color,
                            right,
                            bottom,
                        ),
                        m3g::Vertex::textured(
                            m3g::Vec4::new(center.x - dx, center.y + dy, center.z, center.w),
                            color,
                            left,
                            top,
                        ),
                        m3g::Vertex::textured(
                            m3g::Vec4::new(center.x + dx, center.y + dy, center.z, center.w),
                            color,
                            right,
                            top,
                        ),
                    ];
                    for vertex in &mut quad {
                        vertex.eye_z = center_camera.z;
                    }
                    let (layer, blended) = self.m3g_appearance_order(appearance)?;
                    sprite_jobs.push((layer, blended, render_order, image, appearance, quad));
                    render_order = render_order.saturating_add(1);
                }
            }
        }
        if world_mode {
            if retained_lights.len() > self.limits.m3g_max_lights {
                return Err(vm_error(
                    "illegal-state-exception",
                    "retained scene exceeds the active profile M3G light limit",
                ));
            }
            self.m3g.graphics.lights = retained_lights;
        }
        // JSR-184 requires one ordering domain for Mesh submeshes and Sprite3D:
        // ascending layer, with every opaque item before every blended item at
        // the same layer. Traversal order only breaks otherwise equal ties.
        jobs.sort_unstable_by_key(|(layer, blended, order, ..)| (*layer, *blended, *order));
        sprite_jobs.sort_unstable_by_key(|(layer, blended, order, ..)| (*layer, *blended, *order));
        let camera_origin = camera_transform.transform(m3g::Vec4::new(0.0, 0.0, 0.0, 1.0));
        self.m3g.metrics.scene_nodes = self
            .m3g
            .metrics
            .scene_nodes
            .saturating_add(u64::try_from(visited.len()).unwrap_or(u64::MAX));
        self.m3g_begin_render_batch();
        let mut jobs = jobs.into_iter().peekable();
        let mut sprite_jobs = sprite_jobs.into_iter().peekable();
        // Keep only the current mesh's prepared attributes. Adjacent submeshes
        // reuse them without retaining expanded geometry for the entire scene.
        let mut prepared: Option<PreparedMesh> = None;
        // Appearance objects are immutable for the duration of this render.
        // Sprites replace texture/polygon state and invalidate this binding.
        let mut bound_appearance = None;
        loop {
            self.m3g_check_geometry_cancellation()?;
            let draw_mesh = match (jobs.peek(), sprite_jobs.peek()) {
                (Some(mesh), Some(sprite)) => {
                    (mesh.0, mesh.1, mesh.2) <= (sprite.0, sprite.1, sprite.2)
                }
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => break,
            };
            if draw_mesh {
                let (_, _, _, source, indices, appearance) =
                    jobs.next().expect("peeked mesh render job");
                work.indices(indices.index_count())?;
                if prepared.as_ref().is_none_or(|mesh| mesh.source != source) {
                    // Drop the previous allocation before preparing the next mesh.
                    drop(prepared.take());
                    let (_, geometry) = self
                        .m3g_retained_geometry(mesh_sources[source].node, &mut work)?
                        .ok_or_else(|| {
                            vm_error(
                                "illegal-state-exception",
                                "render source is no longer a visible mesh",
                            )
                        })?;
                    prepared = Some(PreparedMesh {
                        source,
                        geometry,
                        unlit: None,
                        lit: None,
                    });
                }
                let cached = prepared.as_mut().expect("prepared current mesh");
                let source = &mesh_sources[source];
                let vertices = if let Some(material) = self
                    .m3g_material_state(appearance)?
                    .filter(|_| cached.geometry.has_normals())
                {
                    // Camera, lights and mesh state remain fixed within this
                    // render call. Reuse the current material's lit vertices
                    // across submeshes, including distinct Appearances.
                    if cached
                        .lit
                        .as_ref()
                        .is_none_or(|(previous, _)| *previous != material)
                    {
                        // Release the previous buffer before allocating another.
                        drop(cached.lit.take());
                        let lights = self.m3g_light_sources(source.scope)?;
                        let mut vertices = cached.geometry.transformed_lit_vertices(
                            source.model,
                            view_transform,
                            m3g::Vec3::new(camera_origin.x, camera_origin.y, camera_origin.z),
                            material,
                            &lights,
                            source.alpha,
                            &mut work,
                        )?;
                        for vertex in &mut vertices {
                            vertex.project(projection);
                        }
                        cached.lit = Some((material, vertices));
                    }
                    &cached.lit.as_ref().expect("prepared lit mesh vertices").1
                } else {
                    if cached.unlit.is_none() {
                        let mut vertices = cached.geometry.transformed_vertices(
                            view_transform.multiplied(source.model),
                            &mut work,
                        )?;
                        for vertex in &mut vertices {
                            vertex.project(projection);
                            let alpha = ((vertex.color >> 24) & 0xff) as f32;
                            vertex.color = ((alpha * source.alpha.clamp(0.0, 1.0)).round() as u32)
                                << 24
                                | vertex.color & 0x00ff_ffff;
                        }
                        cached.unlit = Some(vertices);
                    }
                    cached
                        .unlit
                        .as_deref()
                        .expect("prepared unlit mesh vertices")
                };
                if bound_appearance != Some(appearance) {
                    self.m3g_apply_appearance(appearance)?;
                    bound_appearance = Some(appearance);
                }
                self.m3g
                    .graphics
                    .renderer
                    .draw_transformed_indexed(vertices, &indices)?;
            } else {
                bound_appearance = None;
                let (_, _, _, image, appearance, quad) =
                    sprite_jobs.next().expect("peeked sprite render job");
                work.vertices(4)?;
                work.indices(6)?;
                self.m3g_apply_sprite_appearance(appearance)?;
                // PolygonMode does not apply to Sprite3D; both generated triangles
                // must survive regardless of the Appearance's polygon component.
                self.m3g
                    .graphics
                    .renderer
                    .set_cull_mode(m3g::CullMode::None);
                let mut texture = m3g::Texture2DState::new(image);
                texture.set_wrapping(m3g::WrapMode::Clamp, m3g::WrapMode::Clamp);
                texture.set_blend_function(m3g::BlendFunction::Modulate);
                self.m3g.graphics.renderer.set_texture(Some(texture));
                self.m3g.graphics.renderer.set_texture_unit(1, None)?;
                self.m3g
                    .graphics
                    .renderer
                    .draw_triangle([quad[0], quad[1], quad[2]])?;
                self.m3g
                    .graphics
                    .renderer
                    .draw_triangle([quad[2], quad[1], quad[3]])?;
            }
        }
        self.m3g_check_geometry_cancellation()?;
        self.m3g_publish_bound_target()?;
        self.m3g_record_render_time(render_started);
        Ok(())
    }

    pub(in crate::machine) fn m3g_appearance_order(
        &self,
        appearance: m3g::Handle,
    ) -> Result<(i32, bool), EmuError> {
        match self.m3g.runtime.kind(appearance)? {
            m3g::ObjectKind::Appearance(appearance) => {
                let blended = match appearance.compositing_mode {
                    Some(handle) => match self.m3g.runtime.kind(handle)? {
                        m3g::ObjectKind::CompositingMode(mode) => mode.blending != 68,
                        _ => return Err(type_error()),
                    },
                    None => false,
                };
                Ok((appearance.layer, blended))
            }
            _ => Err(type_error()),
        }
    }
}
