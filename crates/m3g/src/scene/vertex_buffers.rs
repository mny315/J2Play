use super::{
    EmuError, Handle, ObjectKind, Runtime, VertexArrayState, VertexBufferState, graph_error,
};

impl Runtime {
    /// Resolves a vertex buffer against its currently referenced vertex arrays.
    ///
    /// JSR-184 vertex buffers retain their source arrays; mutating a `VertexArray`
    /// after attachment must therefore affect subsequent renders. The renderer
    /// owns a compact decoded snapshot, so refresh that snapshot at the point of
    /// use instead of treating the arrays as copy-on-attach values.
    pub fn resolved_vertex_buffer(&self, handle: Handle) -> Result<VertexBufferState, EmuError> {
        let (state, arrays) = match &self.objects.get(handle)?.kind {
            ObjectKind::VertexBuffer { state, arrays } => (state, *arrays),
            _ => {
                return Err(graph_error(
                    "not-vertex-buffer",
                    "object is not a VertexBuffer",
                ));
            }
        };

        let source = |array: Handle| -> Result<VertexArrayState, EmuError> {
            match &self.objects.get(array)?.kind {
                ObjectKind::VertexArray(state) => Ok(state.clone()),
                _ => Err(graph_error(
                    "invalid-vertex-array",
                    "VertexBuffer source is not a VertexArray",
                )),
            }
        };
        let mut resolved = VertexBufferState::default();
        resolved.set_default_color(state.default_color());

        if let Some((stored, scale, bias)) = state.positions() {
            let positions = arrays[0]
                .map(&source)
                .transpose()?
                .unwrap_or_else(|| stored.clone());
            resolved.set_positions(Some(positions), scale, bias)?;
        } else if arrays[0].is_some() {
            return Err(graph_error(
                "invalid-vertex-buffer",
                "position reference has no decoded state",
            ));
        }
        if let Some(stored) = state.normals() {
            let normals = arrays[1]
                .map(&source)
                .transpose()?
                .unwrap_or_else(|| stored.clone());
            resolved.set_normals(Some(normals))?;
        } else if arrays[1].is_some() {
            return Err(graph_error(
                "invalid-vertex-buffer",
                "normal reference has no decoded state",
            ));
        }
        if let Some(stored) = state.colors() {
            let colors = arrays[2]
                .map(&source)
                .transpose()?
                .unwrap_or_else(|| stored.clone());
            resolved.set_colors(Some(colors))?;
        } else if arrays[2].is_some() {
            return Err(graph_error(
                "invalid-vertex-buffer",
                "color reference has no decoded state",
            ));
        }
        for unit in 0..2 {
            if let Some((stored, scale, bias)) = state.texture_coordinates(unit) {
                let coordinates = arrays[3 + unit]
                    .map(&source)
                    .transpose()?
                    .unwrap_or_else(|| stored.clone());
                resolved.set_texture_coordinates(unit, Some(coordinates), scale, bias)?;
            } else if arrays[3 + unit].is_some() {
                return Err(graph_error(
                    "invalid-vertex-buffer",
                    "texture-coordinate reference has no decoded state",
                ));
            }
        }
        Ok(resolved)
    }

    /// Transactionally replaces the retained position snapshot and guest source.
    pub fn set_vertex_buffer_positions(
        &mut self,
        buffer: Handle,
        state: Option<VertexArrayState>,
        source: Option<Handle>,
        scale: f32,
        bias: [f32; 3],
    ) -> Result<(), EmuError> {
        self.update_vertex_buffer(buffer, move |buffer_state, arrays| {
            buffer_state.set_positions(state, scale, bias)?;
            arrays[0] = source;
            Ok(())
        })
    }

    /// Transactionally replaces the retained normal snapshot and guest source.
    pub fn set_vertex_buffer_normals(
        &mut self,
        buffer: Handle,
        state: Option<VertexArrayState>,
        source: Option<Handle>,
    ) -> Result<(), EmuError> {
        self.update_vertex_buffer(buffer, move |buffer_state, arrays| {
            buffer_state.set_normals(state)?;
            arrays[1] = source;
            Ok(())
        })
    }

    /// Transactionally replaces the retained color snapshot and guest source.
    pub fn set_vertex_buffer_colors(
        &mut self,
        buffer: Handle,
        state: Option<VertexArrayState>,
        source: Option<Handle>,
    ) -> Result<(), EmuError> {
        self.update_vertex_buffer(buffer, move |buffer_state, arrays| {
            buffer_state.set_colors(state)?;
            arrays[2] = source;
            Ok(())
        })
    }

    /// Transactionally replaces one retained texture-coordinate snapshot and source.
    pub fn set_vertex_buffer_texture_coordinates(
        &mut self,
        buffer: Handle,
        unit: usize,
        state: Option<VertexArrayState>,
        source: Option<Handle>,
        scale: f32,
        bias: [f32; 3],
    ) -> Result<(), EmuError> {
        self.update_vertex_buffer(buffer, move |buffer_state, arrays| {
            let slot = arrays[3..].get_mut(unit).ok_or_else(|| {
                graph_error(
                    "invalid-texture-unit",
                    "texture unit exceeds the supported limit",
                )
            })?;
            buffer_state.set_texture_coordinates(unit, state, scale, bias)?;
            *slot = source;
            Ok(())
        })
    }

    fn update_vertex_buffer(
        &mut self,
        buffer: Handle,
        update: impl FnOnce(&mut VertexBufferState, &mut [Option<Handle>; 5]) -> Result<(), EmuError>,
    ) -> Result<(), EmuError> {
        let ObjectKind::VertexBuffer { state, arrays } = &self.objects.get(buffer)?.kind else {
            return Err(graph_error(
                "not-vertex-buffer",
                "object is not a VertexBuffer",
            ));
        };
        let previous_allocation = state.allocated_bytes();
        let mut state = state.clone();
        let mut arrays = *arrays;
        update(&mut state, &mut arrays)?;
        self.reaccount_capacity_change(buffer, previous_allocation, state.allocated_bytes())?;
        self.objects
            .get_mut(buffer)
            .expect("reaccounted VertexBuffer remains live")
            .kind = ObjectKind::VertexBuffer { state, arrays };
        Ok(())
    }
}
