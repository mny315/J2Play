use super::{
    CallOutcome, Category, EmuError, Handle, HeapValue, Machine, Value, heap_error, type_error,
};

const VECTOR_FIELDS: [&str; 3] = [
    "com/mascotcapsule/micro3d/v3/Vector3D.x:I",
    "com/mascotcapsule/micro3d/v3/Vector3D.y:I",
    "com/mascotcapsule/micro3d/v3/Vector3D.z:I",
];

const AFFINE_FIELDS: [&str; 12] = [
    "com/mascotcapsule/micro3d/v3/AffineTrans.m00:I",
    "com/mascotcapsule/micro3d/v3/AffineTrans.m01:I",
    "com/mascotcapsule/micro3d/v3/AffineTrans.m02:I",
    "com/mascotcapsule/micro3d/v3/AffineTrans.m03:I",
    "com/mascotcapsule/micro3d/v3/AffineTrans.m10:I",
    "com/mascotcapsule/micro3d/v3/AffineTrans.m11:I",
    "com/mascotcapsule/micro3d/v3/AffineTrans.m12:I",
    "com/mascotcapsule/micro3d/v3/AffineTrans.m13:I",
    "com/mascotcapsule/micro3d/v3/AffineTrans.m20:I",
    "com/mascotcapsule/micro3d/v3/AffineTrans.m21:I",
    "com/mascotcapsule/micro3d/v3/AffineTrans.m22:I",
    "com/mascotcapsule/micro3d/v3/AffineTrans.m23:I",
];

impl Machine<'_, '_> {
    pub(in crate::machine) fn micro3d_argument_error(message: impl Into<String>) -> EmuError {
        EmuError::new(Category::Micro3d, "invalid-argument", message)
    }

    pub(in crate::machine) fn micro3d_validate_texture_kind(
        &self,
        guest: u64,
        for_model: bool,
    ) -> Result<(), EmuError> {
        match self.micro3d.runtime.kind(guest)? {
            micro3d::ObjectKind::Texture(texture) if texture.for_model == for_model => Ok(()),
            micro3d::ObjectKind::Texture(_) if for_model => Err(Self::micro3d_argument_error(
                "model rendering requires a model texture",
            )),
            micro3d::ObjectKind::Texture(_) => Err(Self::micro3d_argument_error(
                "sphere mapping requires an environment texture",
            )),
            _ => Err(Self::micro3d_argument_error(
                "Micro3D texture argument has the wrong type",
            )),
        }
    }

    pub(in crate::machine) fn micro3d_validate_effect_state(
        &self,
        state: micro3d::EffectState,
    ) -> Result<(), EmuError> {
        if !matches!(state.shading, 0 | 1) {
            return Err(Self::micro3d_argument_error(
                "Effect3D shading type must be NORMAL_SHADING or TOON_SHADING",
            ));
        }
        if ![state.toon_threshold, state.toon_high, state.toon_low]
            .into_iter()
            .all(|value| (0..=255).contains(&value))
        {
            return Err(Self::micro3d_argument_error(
                "Effect3D toon parameters must be between 0 and 255",
            ));
        }
        if let Some(light) = state.light
            && !matches!(
                self.micro3d.runtime.kind(light)?,
                micro3d::ObjectKind::Light(_)
            )
        {
            return Err(Self::micro3d_argument_error(
                "Effect3D light argument has the wrong type",
            ));
        }
        if let Some(texture) = state.sphere_texture {
            self.micro3d_validate_texture_kind(texture, false)?;
        }
        Ok(())
    }

    fn micro3d_read_math_fields<const N: usize>(
        &self,
        object: Handle,
        class: &str,
        names: &[&str; N],
    ) -> Result<[i32; N], EmuError> {
        if !self
            .program
            .is_assignable_to(&self.object_class(object)?, class)
        {
            return Err(type_error());
        }
        let mut values = [0; N];
        for (slot, name) in values.iter_mut().zip(names) {
            let HeapValue::Int(value) =
                self.heap.managed.field(object, name).map_err(heap_error)?
            else {
                return Err(type_error());
            };
            *slot = value;
        }
        Ok(values)
    }

    fn micro3d_write_math_fields<const N: usize>(
        &mut self,
        object: Handle,
        class: &str,
        names: &[&str; N],
        values: [i32; N],
    ) -> Result<(), EmuError> {
        if !self
            .program
            .is_assignable_to(&self.object_class(object)?, class)
        {
            return Err(type_error());
        }
        for (name, value) in names.iter().zip(values) {
            self.heap
                .managed
                .set_field(object, name, HeapValue::Int(value))
                .map_err(heap_error)?;
        }
        Ok(())
    }

    pub(in crate::machine) fn micro3d_vector(
        &self,
        object: Handle,
    ) -> Result<micro3d::Vector3D, EmuError> {
        let [x, y, z] = self.micro3d_read_math_fields(
            object,
            "com/mascotcapsule/micro3d/v3/Vector3D",
            &VECTOR_FIELDS,
        )?;
        Ok(micro3d::Vector3D::new(x, y, z))
    }

    pub(in crate::machine) fn micro3d_set_vector(
        &mut self,
        object: Handle,
        value: micro3d::Vector3D,
    ) -> Result<(), EmuError> {
        self.micro3d_write_math_fields(
            object,
            "com/mascotcapsule/micro3d/v3/Vector3D",
            &VECTOR_FIELDS,
            [value.x, value.y, value.z],
        )
    }

    pub(in crate::machine) fn micro3d_affine(
        &self,
        object: Handle,
    ) -> Result<micro3d::AffineTrans, EmuError> {
        let values = self.micro3d_read_math_fields(
            object,
            "com/mascotcapsule/micro3d/v3/AffineTrans",
            &AFFINE_FIELDS,
        )?;
        Ok(micro3d::AffineTrans::new(values))
    }

    pub(in crate::machine) fn micro3d_set_affine(
        &mut self,
        object: Handle,
        value: micro3d::AffineTrans,
    ) -> Result<(), EmuError> {
        self.micro3d_write_math_fields(
            object,
            "com/mascotcapsule/micro3d/v3/AffineTrans",
            &AFFINE_FIELDS,
            value.values,
        )
    }

    pub(in crate::machine) fn micro3d_allocate_vector(
        &mut self,
        value: micro3d::Vector3D,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        let class = "com/mascotcapsule/micro3d/v3/Vector3D";
        let object = self.allocate_native_instance(class, roots)?;
        self.micro3d_set_vector(object, value)?;
        Ok(object)
    }

    pub(in crate::machine) fn micro3d_allocate_affine(
        &mut self,
        value: micro3d::AffineTrans,
        roots: &[Value],
    ) -> Result<Handle, EmuError> {
        let class = "com/mascotcapsule/micro3d/v3/AffineTrans";
        let object = self.allocate_native_instance(class, roots)?;
        self.micro3d_set_affine(object, value)?;
        Ok(object)
    }

    pub(in crate::machine) fn micro3d_resource_bytes(
        &self,
        string: Handle,
    ) -> Result<Option<Vec<u8>>, EmuError> {
        let name = self
            .heap
            .string_values
            .get(&string)
            .map(|value| String::from_utf16_lossy(value))
            .ok_or_else(type_error)?;
        self.native_context
            .read_resource(name.strip_prefix('/').unwrap_or(&name))
    }

    pub(in crate::machine) fn micro3d_allocate_native(
        &mut self,
        roots: &[Value],
        mut allocate: impl FnMut(&mut micro3d::Runtime) -> Result<(), EmuError>,
    ) -> Result<(), EmuError> {
        let result = allocate(&mut self.micro3d.runtime);
        if !result
            .as_ref()
            .is_err_and(|error| error.code() == "resource-limit")
        {
            return result;
        }

        // The guest heap can have ample byte capacity while short-lived native
        // wrappers fill the smaller Micro3D arena. Treat native allocation
        // pressure like Java heap pressure: collect with the complete current
        // root set, sweep the suite-owned arena, and retry exactly once.
        let gc_roots = self.roots(&[], roots);
        self.collect_heap(gc_roots);
        allocate(&mut self.micro3d.runtime)
    }

    pub(in crate::machine) fn micro3d_set_layout_affines_native(
        &mut self,
        guest: u64,
        affines: Vec<u64>,
        roots: &[Value],
    ) -> Result<(), EmuError> {
        let check = self
            .micro3d
            .runtime
            .check_layout_affine_capacity(guest, affines.capacity());
        if !check
            .as_ref()
            .is_err_and(|error| error.code() == "resource-limit")
        {
            check?;
            return self.micro3d.runtime.set_layout_affines(guest, affines);
        }
        let gc_roots = self.roots(&[], roots);
        self.collect_heap(gc_roots);
        self.micro3d.runtime.set_layout_affines(guest, affines)
    }

    pub(in crate::machine) fn micro3d_set_figure_textures_native(
        &mut self,
        guest: u64,
        textures: Vec<u64>,
        roots: &[Value],
    ) -> Result<(), EmuError> {
        let check = self
            .micro3d
            .runtime
            .check_figure_texture_capacity(guest, textures.capacity());
        if !check
            .as_ref()
            .is_err_and(|error| error.code() == "resource-limit")
        {
            check?;
            return self.micro3d.runtime.set_figure_textures(guest, textures);
        }
        let gc_roots = self.roots(&[], roots);
        self.collect_heap(gc_roots);
        self.micro3d.runtime.set_figure_textures(guest, textures)
    }

    pub(in crate::machine) fn micro3d_failure(
        &mut self,
        error: EmuError,
        roots: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        if error.code() == "execution-cancelled" {
            return Err(error);
        }
        let class = match error.code() {
            "resource-limit" | "render-budget" | "texture-budget" | "figure-budget" => {
                "java/lang/OutOfMemoryError"
            }
            "disposed-object" | "unknown-object" => "java/lang/IllegalStateException",
            _ => "java/lang/IllegalArgumentException",
        };
        if self.program.classes.contains_key(class) {
            let exception = self.allocate_error_exception(class, &error, &[], roots)?;
            Ok(CallOutcome::Throw(exception))
        } else {
            Err(error)
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/vm/machine/micro3d/values.rs"]
mod tests;
