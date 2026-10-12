use super::{
    ArrayKind, CallOutcome, EmuError, Machine, Value, float_argument, optional_reference_argument,
    reference_argument, type_error,
};

impl Machine<'_, '_> {
    pub(super) fn invoke_m3g_camera_native(
        &mut self,
        name: &str,
        descriptor: &str,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let camera = self.m3g_receiver(args)?;
        let projection = match self.m3g.runtime.kind(camera)? {
            m3g::ObjectKind::Camera { projection, .. } => *projection,
            _ => return Err(type_error()),
        };
        match (name, descriptor) {
            ("setParallel" | "setPerspective", "(FFFF)V") => {
                let values = [
                    float_argument(args, 1)?,
                    float_argument(args, 2)?,
                    float_argument(args, 3)?,
                    float_argument(args, 4)?,
                ];
                let projection = if name == "setParallel" {
                    m3g::CameraProjection::Parallel {
                        height: values[0],
                        aspect_ratio: values[1],
                        near: values[2],
                        far: values[3],
                    }
                } else {
                    m3g::CameraProjection::Perspective {
                        field_of_view: values[0],
                        aspect_ratio: values[1],
                        near: values[2],
                        far: values[3],
                    }
                };
                if projection.validate().is_err() {
                    return self.thread_exception(
                        "java/lang/IllegalArgumentException",
                        Some("invalid Camera projection parameters"),
                    );
                }
                let m3g::ObjectKind::Camera {
                    projection: current,
                    ..
                } = self.m3g.runtime.kind_mut(camera)?
                else {
                    return Err(type_error());
                };
                *current = projection;
                Ok(CallOutcome::Return(None))
            }
            ("setGeneric", "(Ljavax/microedition/m3g/Transform;)V") => {
                let transform = self.m3g_handle(reference_argument(args, 1)?)?;
                let matrix = self.m3g.runtime.transform_value(transform)?;
                let m3g::ObjectKind::Camera {
                    projection: current,
                    ..
                } = self.m3g.runtime.kind_mut(camera)?
                else {
                    return Err(type_error());
                };
                *current = m3g::CameraProjection::Generic(matrix);
                Ok(CallOutcome::Return(None))
            }
            ("getProjection", "(Ljavax/microedition/m3g/Transform;)I") => {
                if let Some(destination) = optional_reference_argument(args, 1)? {
                    let destination = self.m3g_handle(destination)?;
                    if projection.has_empty_volume() {
                        return self.thread_exception(
                            "java/lang/ArithmeticException",
                            Some("Camera near and far clipping distances are equal"),
                        );
                    }
                    self.m3g
                        .runtime
                        .set_transform_value(destination, projection.render_matrix()?)?;
                }
                Ok(CallOutcome::Return(Some(Value::Int(
                    projection.projection_type(),
                ))))
            }
            ("getProjection", "([F)I") => {
                if let Some(destination) = optional_reference_argument(args, 1)? {
                    if self
                        .m3g_primitive_array_elements(destination, &ArrayKind::Float)?
                        .len()
                        < 4
                    {
                        return self.thread_exception(
                            "java/lang/IllegalArgumentException",
                            Some("Camera projection array requires four elements"),
                        );
                    }
                    if let Some(values) = projection.parameters() {
                        self.m3g_write_float_array(destination, &values)?;
                    }
                }
                Ok(CallOutcome::Return(Some(Value::Int(
                    projection.projection_type(),
                ))))
            }
            _ => self.m3g_unsupported_native(),
        }
    }
}
