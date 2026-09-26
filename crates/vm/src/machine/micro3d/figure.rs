use super::{
    ArrayKind, EmuError, Handle, Machine, Value, int_argument, reference_argument, type_error,
    vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn micro3d_figure_call(
        &mut self,
        receiver: Handle,
        name: &str,
        args: &[Value],
    ) -> Result<Option<Value>, EmuError> {
        let result = match name {
            "getTexture" => {
                let micro3d::ObjectKind::Figure(state) =
                    self.micro3d.runtime.kind(receiver.to_raw())?
                else {
                    return Err(type_error());
                };
                Some(Value::Reference(
                    state
                        .textures
                        .get(state.selected_texture)
                        .copied()
                        .map(Handle::from_raw),
                ))
            }
            "getNumTextures" => {
                let micro3d::ObjectKind::Figure(state) =
                    self.micro3d.runtime.kind(receiver.to_raw())?
                else {
                    return Err(type_error());
                };
                Some(Value::Int(
                    i32::try_from(state.textures.len()).unwrap_or(i32::MAX),
                ))
            }
            "getNumPattern" => {
                let micro3d::ObjectKind::Figure(state) =
                    self.micro3d.runtime.kind(receiver.to_raw())?
                else {
                    return Err(type_error());
                };
                Some(Value::Int(i32::from(state.data.pattern_count)))
            }
            "setTexture" => {
                let textures = if matches!(args.get(1), Some(Value::Reference(Some(handle))) if matches!(self.heap.managed.array_kind(*handle), Ok(ArrayKind::Reference(_))))
                {
                    self.m3g_reference_array(reference_argument(args, 1)?)?
                        .into_iter()
                        .map(|value| {
                            value
                                .ok_or_else(|| vm_error("null-pointer-exception", "null Texture"))
                                .map(Handle::to_raw)
                        })
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    vec![reference_argument(args, 1)?.to_raw()]
                };
                for texture in &textures {
                    self.micro3d_validate_texture_kind(*texture, true)?;
                }
                self.micro3d_set_figure_textures_native(receiver.to_raw(), textures, args)?;
                None
            }
            "selectTexture" => {
                let selected = usize::try_from(int_argument(args, 1)?).unwrap_or(usize::MAX);
                let micro3d::ObjectKind::Figure(state) =
                    self.micro3d.runtime.kind(receiver.to_raw())?
                else {
                    return Err(type_error());
                };
                if selected >= state.textures.len() {
                    return Err(vm_error(
                        "index-out-of-bounds-exception",
                        "texture index is out of bounds",
                    ));
                }
                self.micro3d
                    .runtime
                    .select_figure_texture(receiver.to_raw(), selected)?;
                None
            }
            "setPattern" => {
                let pattern = int_argument(args, 1)?;
                self.micro3d
                    .runtime
                    .set_figure_pattern(receiver.to_raw(), pattern)?;
                None
            }
            "setPosture" => {
                let action = reference_argument(args, 1)?.to_raw();
                let action_index = usize::try_from(int_argument(args, 2)?).unwrap_or(usize::MAX);
                let frame = int_argument(args, 3)?;
                let pattern = match self.micro3d.runtime.kind(action)? {
                    micro3d::ObjectKind::Action(table) => table
                        .actions
                        .get(action_index)
                        .map(|selected| selected.sample_pattern(frame))
                        .ok_or_else(|| {
                            vm_error(
                                "illegal-argument-exception",
                                "Micro3D posture action index is invalid",
                            )
                        })?,
                    _ => {
                        return Err(vm_error(
                            "illegal-argument-exception",
                            "Micro3D posture action object is invalid",
                        ));
                    }
                };
                self.micro3d.runtime.set_figure_posture(
                    receiver.to_raw(),
                    (action, action_index, frame),
                    pattern,
                )?;
                None
            }
            _ => return Err(vm_error("method-not-found", name)),
        };
        Ok(result)
    }
}
