use super::{
    CallOutcome, EmuError, Machine, Value, float_argument, int_argument, m3g_non_negative_usize,
    optional_reference_argument, reference_argument, type_error, vm_error,
};
use crate::machine::m3g_runtime::GeometryWork;

impl Machine<'_, '_> {
    pub(in crate::machine) fn invoke_m3g_graphics3d_native(
        &mut self,
        class: &str,
        name: &str,
        descriptor: &str,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let outcome = match (class, name, descriptor) {
            (
                "javax/microedition/m3g/Graphics3D",
                "getInstance",
                "()Ljavax/microedition/m3g/Graphics3D;",
            ) => {
                let instance = if let Some(instance) = self.m3g.graphics3d {
                    instance
                } else {
                    let instance =
                        self.allocate_native_instance("javax/microedition/m3g/Graphics3D", args)?;
                    let constructor_roots = [Value::Reference(Some(instance))];
                    self.m3g_allocate_native(&constructor_roots, |runtime| {
                        runtime.create(Some(instance.to_raw()), m3g::ObjectKind::Object)
                    })?;
                    self.m3g.graphics3d = Some(instance);
                    instance
                };
                CallOutcome::Return(Some(Value::Reference(Some(instance))))
            }
            (
                "javax/microedition/m3g/Graphics3D",
                "render",
                "(Ljavax/microedition/m3g/World;)V",
            ) => {
                let world_guest = reference_argument(args, 1)?;
                let world = self.m3g_handle(world_guest)?;
                self.m3g_render_retained(world, None, true)?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Graphics3D",
                "render",
                "(Ljavax/microedition/m3g/Node;Ljavax/microedition/m3g/Transform;)V",
            ) => {
                let node = self.m3g_handle(reference_argument(args, 1)?)?;
                let transform = self.m3g_optional_transform(args, 2)?;
                self.m3g_render_retained(node, Some(transform), false)?;
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Graphics3D",
                "render",
                "(Ljavax/microedition/m3g/VertexBuffer;Ljavax/microedition/m3g/IndexBuffer;Ljavax/microedition/m3g/Appearance;Ljavax/microedition/m3g/Transform;)V",
            )
            | (
                "javax/microedition/m3g/Graphics3D",
                "render",
                "(Ljavax/microedition/m3g/VertexBuffer;Ljavax/microedition/m3g/IndexBuffer;Ljavax/microedition/m3g/Appearance;Ljavax/microedition/m3g/Transform;I)V",
            ) => {
                if self.m3g.graphics.target.is_none() {
                    return self.thread_exception(
                        "java/lang/IllegalStateException",
                        Some("Graphics3D render requires a bound target"),
                    );
                }
                self.m3g_check_geometry_cancellation()?;
                let mut work = GeometryWork::new(&self.limits);
                self.m3g_refresh_bound_target()?;
                let render_started = std::time::Instant::now();
                let vertex_buffer = self.m3g_handle(reference_argument(args, 1)?)?;
                let indices = self.m3g_handle(reference_argument(args, 2)?)?;
                let appearance = self.m3g_handle(reference_argument(args, 3)?)?;
                let model = self.m3g_optional_transform(args, 4)?;
                let (camera, camera_transform) = self.m3g.graphics.camera.ok_or_else(|| {
                    vm_error("illegal-state-exception", "Graphics3D has no active camera")
                })?;
                let camera = self.m3g_handle(camera)?;
                let projection = match self.m3g.runtime.kind(camera)? {
                    m3g::ObjectKind::Camera { projection, .. } => *projection,
                    _ => return Err(type_error()),
                };
                let projection = projection.render_matrix()?;
                let scope = if args.len() > 5 {
                    int_argument(args, 5)?.cast_unsigned()
                } else {
                    u32::MAX
                };
                let (_, _, camera_scope, _) = self.m3g.runtime.node_state(camera)?;
                if scope & camera_scope == 0 {
                    return Ok(CallOutcome::Return(None));
                }
                let view_transform = camera_transform.inverted()?;
                if !matches!(
                    self.m3g.runtime.kind(vertex_buffer)?,
                    m3g::ObjectKind::VertexBuffer { .. }
                ) {
                    return Err(type_error());
                }
                let vertices = self.m3g.runtime.resolved_vertex_buffer(vertex_buffer)?;
                let indices = match self.m3g.runtime.kind(indices)? {
                    m3g::ObjectKind::TriangleStripArray(state) => state.clone(),
                    _ => return Err(type_error()),
                };
                work.vertices(vertices.vertex_count())?;
                work.indices(indices.index_count())?;
                let lights = self.m3g_light_sources(scope)?;
                let material = self.m3g_material_state(appearance)?;
                let camera_origin = camera_transform.transform(m3g::Vec4::new(0.0, 0.0, 0.0, 1.0));
                let mut transformed =
                    if let Some(material) = material.filter(|_| vertices.has_normals()) {
                        vertices.transformed_lit_vertices(
                            model,
                            view_transform,
                            m3g::Vec3::new(camera_origin.x, camera_origin.y, camera_origin.z),
                            material.material,
                            material.vertex_color_tracking,
                            &lights,
                            1.0,
                        )?
                    } else {
                        vertices.transformed_vertices(view_transform.multiplied(model))?
                    };
                for vertex in &mut transformed {
                    vertex.project(projection);
                }
                self.m3g_begin_render_batch();
                self.m3g_apply_appearance(appearance)?;
                self.m3g
                    .graphics
                    .renderer
                    .draw_transformed_indexed(&transformed, &indices)?;
                self.m3g_check_geometry_cancellation()?;
                self.m3g_publish_bound_target()?;
                self.m3g_record_render_time(render_started);
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Graphics3D",
                "setCamera",
                "(Ljavax/microedition/m3g/Camera;Ljavax/microedition/m3g/Transform;)V",
            ) => {
                let camera = optional_reference_argument(args, 1)?;
                let transform = self.m3g_optional_transform(args, 2)?;
                if let Some(camera) = camera {
                    let native = self.m3g_handle(camera)?;
                    if !matches!(
                        self.m3g.runtime.kind(native)?,
                        m3g::ObjectKind::Camera { .. }
                    ) {
                        return Err(type_error());
                    }
                }
                self.m3g.graphics.camera = camera.map(|camera| (camera, transform));
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Graphics3D",
                "getCamera",
                "(Ljavax/microedition/m3g/Transform;)Ljavax/microedition/m3g/Camera;",
            ) => {
                let destination = optional_reference_argument(args, 1)?;
                if let Some(destination) = destination {
                    let destination = self.m3g_handle(destination)?;
                    let transform = self
                        .m3g
                        .graphics
                        .camera
                        .map_or(m3g::Mat4::IDENTITY, |(_, transform)| transform);
                    self.m3g
                        .runtime
                        .set_transform_value(destination, transform)?;
                }
                CallOutcome::Return(Some(Value::Reference(
                    self.m3g.graphics.camera.map(|(camera, _)| camera),
                )))
            }
            (
                "javax/microedition/m3g/Graphics3D",
                "addLight",
                "(Ljavax/microedition/m3g/Light;Ljavax/microedition/m3g/Transform;)I",
            ) => {
                if self.m3g.graphics.lights.len() >= self.limits.m3g_max_lights {
                    return self.thread_exception(
                        "java/lang/IllegalStateException",
                        Some("active profile M3G light limit exceeded"),
                    );
                }
                let light = reference_argument(args, 1)?;
                let native = self.m3g_handle(light)?;
                if !matches!(
                    self.m3g.runtime.kind(native)?,
                    m3g::ObjectKind::Light { .. }
                ) {
                    return Err(type_error());
                }
                let transform = self.m3g_optional_transform(args, 2)?;
                self.m3g.graphics.lights.push((light, transform));
                CallOutcome::Return(Some(Value::Int(
                    i32::try_from(self.m3g.graphics.lights.len() - 1).unwrap_or(i32::MAX),
                )))
            }
            ("javax/microedition/m3g/Graphics3D", "resetLights", "()V") => {
                self.m3g.graphics.lights.clear();
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Graphics3D", "getLightCount", "()I") => {
                CallOutcome::Return(Some(Value::Int(
                    i32::try_from(self.m3g.graphics.lights.len()).unwrap_or(i32::MAX),
                )))
            }
            (
                "javax/microedition/m3g/Graphics3D",
                "setLight",
                "(ILjavax/microedition/m3g/Light;Ljavax/microedition/m3g/Transform;)V",
            ) => {
                let index = m3g_non_negative_usize(int_argument(args, 1)?)?;
                let light = reference_argument(args, 2)?;
                let native = self.m3g_handle(light)?;
                if !matches!(
                    self.m3g.runtime.kind(native)?,
                    m3g::ObjectKind::Light { .. }
                ) {
                    return Err(type_error());
                }
                let transform = self.m3g_optional_transform(args, 3)?;
                let slot = self.m3g.graphics.lights.get_mut(index).ok_or_else(|| {
                    vm_error(
                        "index-out-of-bounds-exception",
                        "light index is out of bounds",
                    )
                })?;
                *slot = (light, transform);
                CallOutcome::Return(None)
            }
            (
                "javax/microedition/m3g/Graphics3D",
                "getLight",
                "(ILjavax/microedition/m3g/Transform;)Ljavax/microedition/m3g/Light;",
            ) => {
                let index = m3g_non_negative_usize(int_argument(args, 1)?)?;
                let destination = optional_reference_argument(args, 2)?;
                let (light, transform) = *self.m3g.graphics.lights.get(index).ok_or_else(|| {
                    vm_error(
                        "index-out-of-bounds-exception",
                        "light index is out of bounds",
                    )
                })?;
                if let Some(destination) = destination {
                    let destination = self.m3g_handle(destination)?;
                    self.m3g
                        .runtime
                        .set_transform_value(destination, transform)?;
                }
                CallOutcome::Return(Some(Value::Reference(Some(light))))
            }
            ("javax/microedition/m3g/Graphics3D", "getProperties", "()Ljava/util/Hashtable;") => {
                let properties = self.m3g_properties_table(args)?;
                CallOutcome::Return(Some(Value::Reference(Some(properties))))
            }
            ("javax/microedition/m3g/Graphics3D", "bindTarget", "(Ljava/lang/Object;)V")
            | ("javax/microedition/m3g/Graphics3D", "bindTarget", "(Ljava/lang/Object;ZI)V") => {
                if self.m3g.graphics.target.is_some() {
                    return self.thread_exception(
                        "java/lang/IllegalStateException",
                        Some("Graphics3D target is already bound"),
                    );
                }
                let target = reference_argument(args, 1)?;
                let (depth_enabled, hints) = if args.len() > 2 {
                    let hints = int_argument(args, 3)?;
                    if hints & !0x1e != 0 {
                        return self.thread_exception(
                            "java/lang/IllegalArgumentException",
                            Some("Graphics3D hints contain reserved bits"),
                        );
                    }
                    (int_argument(args, 2)? != 0, hints)
                } else {
                    (true, 0)
                };
                let (width, height, pixels, scissor) = self.m3g_target_snapshot(target)?;
                let origin = if self.is_instance(
                    &self.object_class(target)?,
                    "javax/microedition/lcdui/Graphics",
                ) {
                    [
                        self.graphics_int_field(target, "javax/microedition/lcdui/Graphics.tx:I")
                            .unwrap_or(0),
                        self.graphics_int_field(target, "javax/microedition/lcdui/Graphics.ty:I")
                            .unwrap_or(0),
                    ]
                } else {
                    [0, 0]
                };
                self.m3g.graphics.bind_target_surface(
                    target,
                    width,
                    height,
                    pixels,
                    scissor,
                    depth_enabled,
                    self.limits.m3g_render,
                )?;
                self.m3g.graphics.hints = hints;
                self.m3g.graphics.target_origin = origin;
                self.m3g.graphics.viewport = [
                    i32::try_from(scissor[0])
                        .unwrap_or(i32::MAX)
                        .wrapping_sub(origin[0]),
                    i32::try_from(scissor[1])
                        .unwrap_or(i32::MAX)
                        .wrapping_sub(origin[1]),
                    i32::try_from(scissor[2]).unwrap_or(i32::MAX),
                    i32::try_from(scissor[3]).unwrap_or(i32::MAX),
                ];
                self.m3g.graphics.target = Some(target);
                self.m3g.graphics.binding_thread = Some(self.scheduler.current_thread);
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Graphics3D", "releaseTarget", "()V") => {
                let Some(_) = self.m3g.graphics.target else {
                    return Ok(CallOutcome::Return(None));
                };
                if self.m3g.graphics.binding_thread != Some(self.scheduler.current_thread) {
                    return self.thread_exception(
                        "java/lang/IllegalStateException",
                        Some("Graphics3D target belongs to another thread"),
                    );
                }
                self.m3g.graphics.target = None;
                self.m3g.graphics.binding_thread = None;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Graphics3D", "getTarget", "()Ljava/lang/Object;") => {
                CallOutcome::Return(Some(Value::Reference(self.m3g.graphics.target)))
            }
            ("javax/microedition/m3g/Graphics3D", "getHints", "()I") => {
                CallOutcome::Return(Some(Value::Int(self.m3g.graphics.hints)))
            }
            ("javax/microedition/m3g/Graphics3D", "isDepthBufferEnabled", "()Z") => {
                CallOutcome::Return(Some(Value::Int(i32::from(self.m3g.graphics.depth_enabled))))
            }
            ("javax/microedition/m3g/Graphics3D", "setViewport", "(IIII)V") => {
                let viewport = [
                    int_argument(args, 1)?,
                    int_argument(args, 2)?,
                    int_argument(args, 3)?,
                    int_argument(args, 4)?,
                ];
                if viewport[2] <= 0
                    || viewport[3] <= 0
                    || viewport[2].cast_unsigned() > self.limits.m3g_render.max_viewport_width
                    || viewport[3].cast_unsigned() > self.limits.m3g_render.max_viewport_height
                {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid Graphics3D viewport"),
                    );
                }
                // Viewport coordinates remain guest-relative; rasterization
                // uses the Graphics origin that was captured by bindTarget.
                let origin = self.m3g.graphics.target_origin;
                self.m3g.graphics.renderer.set_viewport(
                    viewport[0].saturating_add(origin[0]),
                    viewport[1].saturating_add(origin[1]),
                    viewport[2] as u32,
                    viewport[3] as u32,
                )?;
                self.m3g.graphics.viewport = viewport;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Graphics3D", "getViewportX", "()I")
            | ("javax/microedition/m3g/Graphics3D", "getViewportY", "()I")
            | ("javax/microedition/m3g/Graphics3D", "getViewportWidth", "()I")
            | ("javax/microedition/m3g/Graphics3D", "getViewportHeight", "()I") => {
                let index = match name {
                    "getViewportX" => 0,
                    "getViewportY" => 1,
                    "getViewportWidth" => 2,
                    _ => 3,
                };
                CallOutcome::Return(Some(Value::Int(self.m3g.graphics.viewport[index])))
            }
            ("javax/microedition/m3g/Graphics3D", "setDepthRange", "(FF)V") => {
                let near = float_argument(args, 1)?;
                let far = float_argument(args, 2)?;
                if !near.is_finite()
                    || !far.is_finite()
                    || !(0.0..=1.0).contains(&near)
                    || !(0.0..=1.0).contains(&far)
                {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid Graphics3D depth range"),
                    );
                }
                self.m3g.graphics.depth_range = [near, far];
                self.m3g.graphics.renderer.set_depth_range(
                    self.m3g.graphics.depth_range[0],
                    self.m3g.graphics.depth_range[1],
                )?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/Graphics3D", "getDepthRangeNear", "()F") => {
                CallOutcome::Return(Some(Value::Float(self.m3g.graphics.depth_range[0])))
            }
            ("javax/microedition/m3g/Graphics3D", "getDepthRangeFar", "()F") => {
                CallOutcome::Return(Some(Value::Float(self.m3g.graphics.depth_range[1])))
            }
            (
                "javax/microedition/m3g/Graphics3D",
                "clear",
                "(Ljavax/microedition/m3g/Background;)V",
            ) => {
                if self.m3g.graphics.target.is_none() {
                    return self.thread_exception(
                        "java/lang/IllegalStateException",
                        Some("Graphics3D clear requires a bound target"),
                    );
                }
                let background = optional_reference_argument(args, 1)?;
                let (color, depth, image) = if let Some(background) = background {
                    let handle = self.m3g_handle(background)?;
                    match self.m3g.runtime.kind(handle)? {
                        m3g::ObjectKind::Background(state) => {
                            let image = state
                                .image
                                .filter(|_| state.color_clear)
                                .map(|image| match self.m3g.runtime.kind(image)? {
                                    m3g::ObjectKind::Image2D(image) => Ok((
                                        image.clone(),
                                        state.crop,
                                        state.mode_x == 33,
                                        state.mode_y == 33,
                                    )),
                                    _ => Err(type_error()),
                                })
                                .transpose()?;
                            (
                                state.color_clear.then_some(state.color),
                                state.depth_clear,
                                image,
                            )
                        }
                        _ => return Err(type_error()),
                    }
                } else {
                    (Some(0), true, None)
                };
                self.m3g_refresh_bound_target()?;
                self.m3g.graphics.renderer.clear(color, depth);
                if let Some((image, crop, repeat_x, repeat_y)) = image {
                    self.m3g
                        .graphics
                        .renderer
                        .draw_background(&image, crop, repeat_x, repeat_y)?;
                }
                self.m3g_publish_bound_target()?;
                CallOutcome::Return(None)
            }
            _ => return self.m3g_unsupported_native(),
        };
        Ok(outcome)
    }
}
