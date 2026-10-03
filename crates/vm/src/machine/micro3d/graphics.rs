use super::{
    CallOutcome, EmuError, Handle, Machine, Value, int_argument, micro3d_primitive_payload_counts,
    optional_reference_argument, reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    #[allow(clippy::too_many_lines)]
    pub(in crate::machine) fn micro3d_graphics_call(
        &mut self,
        _receiver: Handle,
        name: &str,
        _descriptor: &str,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        match name {
            "bind" => {
                if self.micro3d.target.is_some() {
                    return self.thread_exception(
                        "java/lang/IllegalStateException",
                        Some("Graphics3D is already bound"),
                    );
                }
                let graphics = reference_argument(args, 1)?;
                let (_, width, height) = self.graphics_target(graphics)?;
                self.micro3d.target_scissor =
                    self.micro3d_graphics_scissor(graphics, width, height)?;
                self.micro3d.command_scissor = self.micro3d.target_scissor;
                self.micro3d.target = Some(graphics);
                self.micro3d.render_pending = false;
            }
            "flush" => self.micro3d_flush_target()?,
            "release" => {
                let graphics = reference_argument(args, 1)?;
                if self.micro3d.target != Some(graphics) {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("Graphics3D target does not match"),
                    );
                }
                self.micro3d.target = None;
                self.micro3d.target_scissor = [0; 4];
                self.micro3d.command_scissor = [0; 4];
                self.micro3d.render_pending = false;
            }
            "renderFigure" | "drawFigure" => {
                self.micro3d_prepare_render_target()?;
                let figure = reference_argument(args, 1)?.to_raw();
                let layout_guest = reference_argument(args, 4)?.to_raw();
                let effect_guest = reference_argument(args, 5)?.to_raw();
                let (layout, selected_affine) =
                    self.micro3d.runtime.layout_render_snapshot(layout_guest)?;
                let effect = match self.micro3d.runtime.kind(effect_guest)? {
                    micro3d::ObjectKind::Effect(value) => *value,
                    _ => return Err(type_error()),
                };
                let affine = selected_affine
                    .map(|guest| self.micro3d_affine(Handle::from_raw(guest)))
                    .transpose()?
                    .unwrap_or(micro3d::AffineTrans::IDENTITY);
                let diagnostic_call = self.micro3d.render_diagnostic_calls;
                self.micro3d.render_diagnostic_calls = diagnostic_call.saturating_add(1);
                // Geometry snapshots are trace data: computing them repeats the
                // model transforms and must not tax ordinary gameplay.
                if self.execution.profiling
                    && (diagnostic_call < 16 || diagnostic_call.is_multiple_of(128))
                {
                    let figure_state = match self.micro3d.runtime.kind(figure)? {
                        micro3d::ObjectKind::Figure(state) => state,
                        _ => return Err(type_error()),
                    };
                    let face_attributes = figure_state
                        .data
                        .faces
                        .iter()
                        .fold(0_u32, |attributes, face| attributes | face.attributes);
                    let first_bone = figure_state
                        .data
                        .bones
                        .first()
                        .map(|bone| (bone.vertex_count, bone.parent, bone.transform.values));
                    let mut model_minimum = [i32::MAX; 3];
                    let mut model_maximum = [i32::MIN; 3];
                    let mut view_minimum = [i32::MAX; 3];
                    let mut view_maximum = [i32::MIN; 3];
                    for vertex in &figure_state.data.vertices {
                        let transformed = affine.transform(*vertex);
                        for (axis, (model, view)) in [vertex.x, vertex.y, vertex.z]
                            .into_iter()
                            .zip([transformed.x, transformed.y, transformed.z])
                            .enumerate()
                        {
                            model_minimum[axis] = model_minimum[axis].min(model);
                            model_maximum[axis] = model_maximum[axis].max(model);
                            view_minimum[axis] = view_minimum[axis].min(view);
                            view_maximum[axis] = view_maximum[axis].max(view);
                        }
                    }
                    if self.micro3d.render_diagnostics.len() == 64 {
                        self.micro3d.render_diagnostics.pop_front();
                    }
                    self.micro3d.render_diagnostics.push_back(format!(
                        "call={diagnostic_call} figure={figure} vertices={} faces={} face_attributes=0x{face_attributes:08x} first_bone={first_bone:?} pattern={} textures={} selected_texture={} semitransparent={} xy=({}, {}) center={:?} projection={:?} model_bounds={model_minimum:?}..{model_maximum:?} view_bounds={view_minimum:?}..{view_maximum:?} affine={:?}",
                        figure_state.data.vertices.len(),
                        figure_state.data.faces.len(),
                        figure_state.pattern,
                        figure_state.textures.len(),
                        figure_state.selected_texture,
                        effect.transparency,
                        int_argument(args, 2)?,
                        int_argument(args, 3)?,
                        layout.center,
                        layout.projection,
                        affine.values,
                    ));
                }
                if let Err(error) = self.micro3d.runtime.render_figure(
                    figure,
                    int_argument(args, 2)?,
                    int_argument(args, 3)?,
                    layout,
                    affine,
                    effect,
                ) {
                    if self.micro3d.render_diagnostics.len() == 64 {
                        self.micro3d.render_diagnostics.pop_front();
                    }
                    self.micro3d.render_diagnostics.push_back(format!(
                        "call={diagnostic_call} error={} message={}",
                        error.code(),
                        error.message()
                    ));
                    return self.micro3d_failure(error, args);
                }
                self.micro3d.render_pending = true;
                if name == "drawFigure" {
                    // drawFigure is the immediate counterpart to queued
                    // renderFigure. Publish it now; this also gives the next
                    // immediate draw a fresh depth buffer, matching the API's
                    // painter-ordered draw semantics.
                    self.micro3d_flush_target()?;
                }
            }
            "renderPrimitives" => {
                self.micro3d_prepare_render_target()?;
                let layout_guest = reference_argument(args, 4)?.to_raw();
                let effect_guest = reference_argument(args, 5)?.to_raw();
                let (layout, selected_affine) =
                    self.micro3d.runtime.layout_render_snapshot(layout_guest)?;
                let effect = match self.micro3d.runtime.kind(effect_guest)? {
                    micro3d::ObjectKind::Effect(value) => *value,
                    _ => return Err(type_error()),
                };
                let affine = selected_affine
                    .map(|guest| self.micro3d_affine(Handle::from_raw(guest)))
                    .transpose()?
                    .unwrap_or(micro3d::AffineTrans::IDENTITY);
                let command = int_argument(args, 6)?;
                let primitive_count = usize::try_from(int_argument(args, 7)?).unwrap_or(usize::MAX);
                let (vertex_count, normal_count, texture_count, color_count) =
                    micro3d_primitive_payload_counts(command, primitive_count)?;
                let diagnostic_call = self.micro3d.render_diagnostic_calls;
                self.micro3d.render_diagnostic_calls = diagnostic_call.saturating_add(1);
                let coordinates =
                    self.m3g_int_array_prefix(reference_argument(args, 8)?, vertex_count * 3)?;
                if self.execution.profiling
                    && (diagnostic_call < 16 || diagnostic_call.is_multiple_of(128))
                {
                    let mut model_minimum = [i32::MAX; 3];
                    let mut model_maximum = [i32::MIN; 3];
                    let mut view_minimum = [i32::MAX; 3];
                    let mut view_maximum = [i32::MIN; 3];
                    for values in coordinates.chunks_exact(3) {
                        let transformed = affine
                            .transform(micro3d::Vector3D::new(values[0], values[1], values[2]));
                        for (axis, (model, view)) in [values[0], values[1], values[2]]
                            .into_iter()
                            .zip([transformed.x, transformed.y, transformed.z])
                            .enumerate()
                        {
                            model_minimum[axis] = model_minimum[axis].min(model);
                            model_maximum[axis] = model_maximum[axis].max(model);
                            view_minimum[axis] = view_minimum[axis].min(view);
                            view_maximum[axis] = view_maximum[axis].max(view);
                        }
                    }
                    if self.micro3d.render_diagnostics.len() == 64 {
                        self.micro3d.render_diagnostics.pop_front();
                    }
                    self.micro3d.render_diagnostics.push_back(format!(
                        "call={diagnostic_call} primitives command=0x{:08x} count={} xy=({}, {}) center={:?} projection={:?} model_bounds={model_minimum:?}..{model_maximum:?} view_bounds={view_minimum:?}..{view_maximum:?} affine={:?}",
                        command.cast_unsigned(),
                        primitive_count,
                        int_argument(args, 2)?,
                        int_argument(args, 3)?,
                        layout.center,
                        layout.projection,
                        affine.values,
                    ));
                }
                let texture_coordinates = optional_reference_argument(args, 10)?
                    .map(|array| self.m3g_int_array_prefix(array, texture_count))
                    .transpose()?
                    .unwrap_or_default();
                let normals = optional_reference_argument(args, 9)?
                    .map(|array| self.m3g_int_array_prefix(array, normal_count))
                    .transpose()?
                    .unwrap_or_default();
                let colors = optional_reference_argument(args, 11)?
                    .map(|array| self.m3g_int_array_prefix(array, color_count))
                    .transpose()?
                    .unwrap_or_default();
                let result = self.micro3d.runtime.render_primitives(
                    optional_reference_argument(args, 1)?.map(Handle::to_raw),
                    int_argument(args, 2)?,
                    int_argument(args, 3)?,
                    layout,
                    affine,
                    effect,
                    micro3d::PrimitiveData::new(
                        command,
                        primitive_count,
                        &coordinates,
                        &normals,
                        &texture_coordinates,
                        &colors,
                    ),
                );
                if let Err(error) = result {
                    return self.micro3d_failure(error, args);
                }
                self.micro3d.render_pending = true;
            }
            "drawCommandList" => {
                let result = self.micro3d_draw_command_list(args);
                if let Err(error) = result {
                    return self.micro3d_failure(error, args);
                }
            }
            _ => return Err(vm_error("method-not-found", name)),
        }
        Ok(CallOutcome::Return(None))
    }
}
