use super::{EmuError, Handle, Mat4, ObjectKind, Runtime, TransformableState, graph_error};

impl Runtime {
    /// Updates world active camera after validating its native type.
    pub fn set_world_camera(&mut self, world: Handle, camera: Handle) -> Result<(), EmuError> {
        if !matches!(self.objects.get(camera)?.kind, ObjectKind::Camera { .. }) {
            return Err(graph_error("not-camera", "active camera is not a Camera"));
        }
        match &mut self.objects.get_mut(world)?.kind {
            ObjectKind::World { camera: active, .. } => *active = Some(camera),
            _ => return Err(graph_error("not-world", "object is not a World")),
        }
        Ok(())
    }

    /// Updates world background after validating its native type.
    pub fn set_world_background(
        &mut self,
        world: Handle,
        background: Option<Handle>,
    ) -> Result<(), EmuError> {
        if let Some(handle) = background
            && !matches!(self.objects.get(handle)?.kind, ObjectKind::Background(_))
        {
            return Err(graph_error(
                "not-background",
                "world background is not a Background",
            ));
        }
        match &mut self.objects.get_mut(world)?.kind {
            ObjectKind::World {
                background: active, ..
            } => *active = background,
            _ => return Err(graph_error("not-world", "object is not a World")),
        }
        Ok(())
    }

    pub(super) fn transformable(&self, handle: Handle) -> Result<&TransformableState, EmuError> {
        self.objects
            .get(handle)?
            .kind
            .transformable()
            .ok_or_else(|| graph_error("not-transformable", "object is not Transformable"))
    }

    pub(super) fn transformable_mut(
        &mut self,
        handle: Handle,
    ) -> Result<&mut TransformableState, EmuError> {
        self.objects
            .get_mut(handle)?
            .kind
            .transformable_mut()
            .ok_or_else(|| graph_error("not-transformable", "object is not Transformable"))
    }

    /// Computes a node-to-root transform iteratively.
    pub fn world_transform(&self, node: Handle) -> Result<Mat4, EmuError> {
        self.transform_chain(&self.node_chain(node)?)
    }

    pub(super) fn node_chain(&self, node: Handle) -> Result<Vec<Handle>, EmuError> {
        let mut chain = Vec::new();
        let mut current = Some(node);
        while let Some(handle) = current {
            if chain.len() >= self.objects.live_objects() {
                return Err(graph_error("graph-cycle", "parent chain is cyclic"));
            }
            chain.push(handle);
            current = self.parent(handle)?;
        }
        Ok(chain)
    }

    pub(super) fn transform_chain(&self, chain: &[Handle]) -> Result<Mat4, EmuError> {
        let mut result = Mat4::IDENTITY;
        for &node in chain.iter().rev() {
            result = result.multiplied(self.composite_transform(node)?);
        }
        Ok(result)
    }
}
