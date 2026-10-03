use super::{
    EmuError, Handle, Machine, Value, int_argument, optional_reference_argument, type_error,
    vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn micro3d_effect_call(
        &mut self,
        receiver: Handle,
        name: &str,
        args: &[Value],
    ) -> Result<Option<Value>, EmuError> {
        let mut state = match self.micro3d.runtime.kind(receiver.to_raw())? {
            micro3d::ObjectKind::Effect(state) => *state,
            _ => return Err(type_error()),
        };
        let result = match name {
            "getLight" => Some(Value::Reference(state.light.map(Handle::from_raw))),
            "getShading" | "getShadingType" => Some(Value::Int(state.shading)),
            "getThreshold" | "getToonThreshold" => Some(Value::Int(state.toon_threshold)),
            "getThresholdHigh" | "getToonHigh" => Some(Value::Int(state.toon_high)),
            "getThresholdLow" | "getToonLow" => Some(Value::Int(state.toon_low)),
            "isSemiTransparentEnabled" | "isTransparency" => {
                Some(Value::Int(i32::from(state.transparency)))
            }
            "getSphereMap" | "getSphereTexture" => {
                Some(Value::Reference(state.sphere_texture.map(Handle::from_raw)))
            }
            "setLight" => {
                state.light = optional_reference_argument(args, 1)?.map(Handle::to_raw);
                self.micro3d_validate_effect_state(state)?;
                self.micro3d
                    .runtime
                    .update_effect(receiver.to_raw(), |current| *current = state)?;
                None
            }
            "setShading" | "setShadingType" => {
                state.shading = int_argument(args, 1)?;
                self.micro3d_validate_effect_state(state)?;
                self.micro3d
                    .runtime
                    .update_effect(receiver.to_raw(), |current| *current = state)?;
                None
            }
            "setThreshold" | "setToonParams" => {
                state.toon_threshold = int_argument(args, 1)?;
                state.toon_high = int_argument(args, 2)?;
                state.toon_low = int_argument(args, 3)?;
                self.micro3d_validate_effect_state(state)?;
                self.micro3d
                    .runtime
                    .update_effect(receiver.to_raw(), |current| *current = state)?;
                None
            }
            "setSemiTransparentEnabled" | "setTransparency" => {
                state.transparency = int_argument(args, 1)? != 0;
                self.micro3d
                    .runtime
                    .update_effect(receiver.to_raw(), |current| *current = state)?;
                None
            }
            "setSphereMap" | "setSphereTexture" => {
                state.sphere_texture = optional_reference_argument(args, 1)?.map(Handle::to_raw);
                self.micro3d_validate_effect_state(state)?;
                self.micro3d
                    .runtime
                    .update_effect(receiver.to_raw(), |current| *current = state)?;
                None
            }
            _ => return Err(vm_error("method-not-found", name)),
        };
        Ok(result)
    }
}
