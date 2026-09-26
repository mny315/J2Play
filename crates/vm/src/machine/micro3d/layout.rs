use super::{
    EmuError, Handle, Machine, Value, int_argument, optional_reference_argument,
    reference_argument, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn micro3d_layout_call(
        &mut self,
        receiver: Handle,
        name: &str,
        descriptor: &str,
        args: &[Value],
    ) -> Result<Option<Value>, EmuError> {
        let (state, selected_affine) = self
            .micro3d
            .runtime
            .layout_render_snapshot(receiver.to_raw())?;
        let result = match name {
            "getAffineTrans" => Some(Value::Reference(selected_affine.map(Handle::from_raw))),
            "getScaleX" => Some(Value::Int(state.scale[0])),
            "getScaleY" => Some(Value::Int(state.scale[1])),
            "getParallelWidth" => Some(Value::Int(match state.projection {
                micro3d::Projection::Parallel { width, .. } => width,
                _ => 0,
            })),
            "getParallelHeight" => Some(Value::Int(match state.projection {
                micro3d::Projection::Parallel { height, .. } => height,
                _ => 0,
            })),
            "getCenterX" => Some(Value::Int(state.center[0])),
            "getCenterY" => Some(Value::Int(state.center[1])),
            "setAffineTrans" if descriptor.starts_with("(L") => {
                let value = optional_reference_argument(args, 1)?;
                let affines = value.into_iter().map(Handle::to_raw).collect::<Vec<_>>();
                self.micro3d_set_layout_affines_native(receiver.to_raw(), affines, args)?;
                None
            }
            "setAffineTrans" | "setAffineTransArray" => {
                let affines = self
                    .m3g_reference_array(reference_argument(args, 1)?)?
                    .into_iter()
                    .map(|value| {
                        value
                            .ok_or_else(|| vm_error("null-pointer-exception", "null AffineTrans"))
                            .map(Handle::to_raw)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.micro3d_set_layout_affines_native(receiver.to_raw(), affines, args)?;
                None
            }
            "selectAffineTrans" => {
                let selected = usize::try_from(int_argument(args, 1)?).unwrap_or(usize::MAX);
                if selected
                    >= self
                        .micro3d
                        .runtime
                        .layout_affines(receiver.to_raw())?
                        .len()
                {
                    return Err(vm_error(
                        "index-out-of-bounds-exception",
                        "affine index is out of bounds",
                    ));
                }
                self.micro3d
                    .runtime
                    .select_layout_affine(receiver.to_raw(), selected)?;
                None
            }
            "setScale" => {
                let scale = [int_argument(args, 1)?, int_argument(args, 2)?];
                self.micro3d
                    .runtime
                    .set_layout_scale(receiver.to_raw(), scale)?;
                None
            }
            "setParallelSize" => {
                let projection = micro3d::Projection::Parallel {
                    width: int_argument(args, 1)?,
                    height: int_argument(args, 2)?,
                };
                self.micro3d
                    .runtime
                    .set_layout_projection(receiver.to_raw(), projection)?;
                None
            }
            "setCenter" => {
                let center = [int_argument(args, 1)?, int_argument(args, 2)?];
                self.micro3d
                    .runtime
                    .set_layout_center(receiver.to_raw(), center)?;
                None
            }
            "setPerspective" if descriptor == "(III)V" => {
                let projection = micro3d::Projection::PerspectiveFov {
                    near: int_argument(args, 1)?,
                    far: int_argument(args, 2)?,
                    angle: int_argument(args, 3)?,
                };
                self.micro3d
                    .runtime
                    .set_layout_projection(receiver.to_raw(), projection)?;
                None
            }
            "setPerspective" => {
                let projection = micro3d::Projection::PerspectiveSize {
                    near: int_argument(args, 1)?,
                    far: int_argument(args, 2)?,
                    width: int_argument(args, 3)?,
                    height: int_argument(args, 4)?,
                };
                self.micro3d
                    .runtime
                    .set_layout_projection(receiver.to_raw(), projection)?;
                None
            }
            _ => return Err(vm_error("method-not-found", name)),
        };
        Ok(result)
    }
}
