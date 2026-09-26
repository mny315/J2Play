//! `Micro3D` object state and resource accounting.

use super::{
    EffectState, EmuError, FigureLayoutState, LightState, ObjectKind, Projection, Runtime,
    runtime_error,
};

impl Runtime {
    pub fn update_light(
        &mut self,
        guest: u64,
        update: impl FnOnce(&mut LightState),
    ) -> Result<(), EmuError> {
        let ObjectKind::Light(state) = self.kind_mut(guest)? else {
            return Err(runtime_error("type", "Micro3D object is not a Light"));
        };
        update(state);
        Ok(())
    }

    pub fn update_effect(
        &mut self,
        guest: u64,
        update: impl FnOnce(&mut EffectState),
    ) -> Result<(), EmuError> {
        let ObjectKind::Effect(state) = self.kind_mut(guest)? else {
            return Err(runtime_error("type", "Micro3D object is not an Effect3D"));
        };
        update(state);
        Ok(())
    }

    /// Returns scalar layout state and its selected affine without cloning the affine list.
    pub fn layout_render_snapshot(
        &self,
        guest: u64,
    ) -> Result<(FigureLayoutState, Option<u64>), EmuError> {
        let ObjectKind::Layout(state) = self.kind(guest)? else {
            return Err(runtime_error(
                "type",
                "Micro3D object is not a FigureLayout",
            ));
        };
        let selected = state.affines.get(state.selected_affine).copied();
        Ok((
            FigureLayoutState {
                affines: Vec::new(),
                selected_affine: state.selected_affine,
                scale: state.scale,
                center: state.center,
                projection: state.projection,
            },
            selected,
        ))
    }

    pub fn layout_affines(&self, guest: u64) -> Result<&[u64], EmuError> {
        let ObjectKind::Layout(state) = self.kind(guest)? else {
            return Err(runtime_error(
                "type",
                "Micro3D object is not a FigureLayout",
            ));
        };
        Ok(&state.affines)
    }

    pub fn layout_affine(&self, guest: u64, index: usize) -> Result<Option<u64>, EmuError> {
        Ok(self.layout_affines(guest)?.get(index).copied())
    }

    pub fn set_layout_affines(&mut self, guest: u64, affines: Vec<u64>) -> Result<(), EmuError> {
        let new_bytes = self.layout_replacement_bytes(guest, affines.capacity())?;
        let new_live_bytes = self.replacement_live_bytes(guest, new_bytes)?;
        let ObjectKind::Layout(state) = self.kind_mut(guest)? else {
            unreachable!("validated FigureLayout changed kind");
        };
        state.affines = affines;
        state.selected_affine = 0;
        self.commit_replacement_bytes(guest, new_bytes, new_live_bytes);
        Ok(())
    }

    pub fn check_layout_affine_capacity(
        &self,
        guest: u64,
        capacity: usize,
    ) -> Result<(), EmuError> {
        let new_bytes = self.layout_replacement_bytes(guest, capacity)?;
        self.replacement_live_bytes(guest, new_bytes).map(|_| ())
    }

    pub fn select_layout_affine(&mut self, guest: u64, selected: usize) -> Result<(), EmuError> {
        let ObjectKind::Layout(state) = self.kind_mut(guest)? else {
            return Err(runtime_error(
                "type",
                "Micro3D object is not a FigureLayout",
            ));
        };
        if selected >= state.affines.len() {
            return Err(runtime_error("index", "affine index is out of bounds"));
        }
        state.selected_affine = selected;
        Ok(())
    }

    pub fn set_layout_scale(&mut self, guest: u64, scale: [i32; 2]) -> Result<(), EmuError> {
        let ObjectKind::Layout(state) = self.kind_mut(guest)? else {
            return Err(runtime_error(
                "type",
                "Micro3D object is not a FigureLayout",
            ));
        };
        state.scale = scale;
        state.projection = Projection::ParallelScale;
        Ok(())
    }

    pub fn set_layout_center(&mut self, guest: u64, center: [i32; 2]) -> Result<(), EmuError> {
        let ObjectKind::Layout(state) = self.kind_mut(guest)? else {
            return Err(runtime_error(
                "type",
                "Micro3D object is not a FigureLayout",
            ));
        };
        state.center = center;
        Ok(())
    }

    pub fn set_layout_projection(
        &mut self,
        guest: u64,
        projection: Projection,
    ) -> Result<(), EmuError> {
        let ObjectKind::Layout(state) = self.kind_mut(guest)? else {
            return Err(runtime_error(
                "type",
                "Micro3D object is not a FigureLayout",
            ));
        };
        state.projection = projection;
        Ok(())
    }

    pub fn set_figure_textures(&mut self, guest: u64, textures: Vec<u64>) -> Result<(), EmuError> {
        let new_bytes = self.figure_replacement_bytes(guest, textures.capacity())?;
        let new_live_bytes = self.replacement_live_bytes(guest, new_bytes)?;
        let ObjectKind::Figure(state) = self.kind_mut(guest)? else {
            unreachable!("validated Figure changed kind");
        };
        state.textures = textures;
        state.selected_texture = 0;
        self.commit_replacement_bytes(guest, new_bytes, new_live_bytes);
        Ok(())
    }

    pub fn check_figure_texture_capacity(
        &self,
        guest: u64,
        capacity: usize,
    ) -> Result<(), EmuError> {
        let new_bytes = self.figure_replacement_bytes(guest, capacity)?;
        self.replacement_live_bytes(guest, new_bytes).map(|_| ())
    }

    pub fn select_figure_texture(&mut self, guest: u64, selected: usize) -> Result<(), EmuError> {
        let ObjectKind::Figure(state) = self.kind_mut(guest)? else {
            return Err(runtime_error("type", "Micro3D object is not a Figure"));
        };
        if selected >= state.textures.len() {
            return Err(runtime_error("index", "texture index is out of bounds"));
        }
        state.selected_texture = selected;
        Ok(())
    }

    pub fn set_figure_pattern(&mut self, guest: u64, pattern: i32) -> Result<(), EmuError> {
        let ObjectKind::Figure(state) = self.kind_mut(guest)? else {
            return Err(runtime_error("type", "Micro3D object is not a Figure"));
        };
        state.pattern = pattern;
        Ok(())
    }

    pub fn set_figure_posture(
        &mut self,
        guest: u64,
        posture: (u64, usize, i32),
        pattern: i32,
    ) -> Result<(), EmuError> {
        let ObjectKind::Figure(state) = self.kind_mut(guest)? else {
            return Err(runtime_error("type", "Micro3D object is not a Figure"));
        };
        state.posture = Some(posture);
        state.pattern = pattern;
        Ok(())
    }

    fn replacement_live_bytes(&self, guest: u64, new_bytes: usize) -> Result<usize, EmuError> {
        let object = self.objects.get(&guest).ok_or_else(|| {
            runtime_error("disposed-object", "Micro3D object is absent or disposed")
        })?;
        object.kind.as_ref().ok_or_else(|| {
            runtime_error("disposed-object", "Micro3D object is absent or disposed")
        })?;
        let old_bytes = object.bytes;
        self.metrics
            .live_bytes
            .checked_sub(old_bytes)
            .and_then(|bytes| bytes.checked_add(new_bytes))
            .filter(|bytes| *bytes <= self.max_bytes)
            .ok_or_else(|| {
                runtime_error("resource-limit", "Micro3D native object budget exhausted")
            })
    }

    fn layout_replacement_bytes(&self, guest: u64, capacity: usize) -> Result<usize, EmuError> {
        let ObjectKind::Layout(_) = self.kind(guest)? else {
            return Err(runtime_error(
                "type",
                "Micro3D object is not a FigureLayout",
            ));
        };
        Ok(size_of::<ObjectKind>().saturating_add(capacity.saturating_mul(size_of::<u64>())))
    }

    fn figure_replacement_bytes(&self, guest: u64, capacity: usize) -> Result<usize, EmuError> {
        let ObjectKind::Figure(state) = self.kind(guest)? else {
            return Err(runtime_error("type", "Micro3D object is not a Figure"));
        };
        Ok(size_of::<ObjectKind>()
            .saturating_add(state.data.allocated_bytes())
            .saturating_add(capacity.saturating_mul(size_of::<u64>())))
    }

    fn commit_replacement_bytes(&mut self, guest: u64, new_bytes: usize, new_live_bytes: usize) {
        let object = self
            .objects
            .get_mut(&guest)
            .expect("validated Micro3D object remains live");
        object.bytes = new_bytes;
        self.metrics.live_bytes = new_live_bytes;
        self.metrics.peak_bytes = self.metrics.peak_bytes.max(new_live_bytes);
    }
}
