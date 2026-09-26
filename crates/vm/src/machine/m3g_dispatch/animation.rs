use super::{
    CallOutcome, EmuError, Machine, Value, float_argument, int_argument,
    optional_reference_argument, reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    pub(in crate::machine) fn invoke_m3g_animation_native(
        &mut self,
        class: &str,
        name: &str,
        descriptor: &str,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let outcome = match (class, name, descriptor) {
            ("javax/microedition/m3g/AnimationController", "setActiveInterval", "(II)V") => {
                let handle = self.m3g_receiver(args)?;
                let start = int_argument(args, 1)?;
                let end = int_argument(args, 2)?;
                let m3g::ObjectKind::AnimationController(state) =
                    self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                state.set_active_interval(start, end)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/AnimationController", "setSpeed", "(FI)V") => {
                let handle = self.m3g_receiver(args)?;
                let speed = float_argument(args, 1)?;
                let time = int_argument(args, 2)?;
                let m3g::ObjectKind::AnimationController(state) =
                    self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                state.set_speed(speed, time)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/AnimationController", "setPosition", "(FI)V") => {
                let handle = self.m3g_receiver(args)?;
                let position = float_argument(args, 1)?;
                let time = int_argument(args, 2)?;
                let m3g::ObjectKind::AnimationController(state) =
                    self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                state.set_position(position, time)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/AnimationController", "setWeight", "(F)V") => {
                let handle = self.m3g_receiver(args)?;
                let weight = float_argument(args, 1)?;
                let m3g::ObjectKind::AnimationController(state) =
                    self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                state.set_weight(weight)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/AnimationController", _, "()I" | "()F" | "(I)F") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::AnimationController(state) = self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                match name {
                    "getActiveIntervalStart" => {
                        CallOutcome::Return(Some(Value::Int(state.active_interval().0)))
                    }
                    "getActiveIntervalEnd" => {
                        CallOutcome::Return(Some(Value::Int(state.active_interval().1)))
                    }
                    "getRefWorldTime" => {
                        CallOutcome::Return(Some(Value::Int(state.reference_world_time())))
                    }
                    "getSpeed" => CallOutcome::Return(Some(Value::Float(state.speed()))),
                    "getWeight" => CallOutcome::Return(Some(Value::Float(state.weight()))),
                    "getPosition" => CallOutcome::Return(Some(Value::Float(
                        state.position(int_argument(args, 1)?),
                    ))),
                    _ => return Err(type_error()),
                }
            }
            (
                "javax/microedition/m3g/AnimationTrack",
                "setController",
                "(Ljavax/microedition/m3g/AnimationController;)V",
            ) => {
                let handle = self.m3g_receiver(args)?;
                let controller = optional_reference_argument(args, 1)?
                    .map(|guest| self.m3g_handle(guest))
                    .transpose()?;
                if let Some(controller) = controller
                    && !matches!(
                        self.m3g.runtime.kind(controller)?,
                        m3g::ObjectKind::AnimationController(_)
                    )
                {
                    return Err(type_error());
                }
                let m3g::ObjectKind::AnimationTrack {
                    controller: slot, ..
                } = self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                *slot = controller;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/AnimationTrack", _, _) => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::AnimationTrack {
                    sequence,
                    controller,
                    property,
                } = self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                match name {
                    "getController" => CallOutcome::Return(Some(Value::Reference(
                        self.m3g_guest_handle(*controller),
                    ))),
                    "getKeyframeSequence" => CallOutcome::Return(Some(Value::Reference(
                        self.m3g_guest_handle(Some(*sequence)),
                    ))),
                    "getTargetProperty" => CallOutcome::Return(Some(Value::Int(*property))),
                    _ => return Err(type_error()),
                }
            }
            ("javax/microedition/m3g/KeyframeSequence", "setKeyframe", "(II[F)V") => {
                let handle = self.m3g_receiver(args)?;
                let index = int_argument(args, 1)?;
                let time = int_argument(args, 2)?;
                let source = reference_argument(args, 3)?;
                let m3g::ObjectKind::KeyframeSequence(state) = self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                let index = keyframe_index(index, state.keyframe_count())?;
                let mut value = vec![0.0; state.component_count()];
                self.m3g_read_float_array(source, &mut value)?;
                let m3g::ObjectKind::KeyframeSequence(state) = self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                state.set_keyframe(index, time, &value)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/KeyframeSequence", "getKeyframe", "(I[F)I") => {
                let handle = self.m3g_receiver(args)?;
                let index = int_argument(args, 1)?;
                let destination = optional_reference_argument(args, 2)?;
                let m3g::ObjectKind::KeyframeSequence(state) = self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                let (time, value) =
                    state.keyframe(keyframe_index(index, state.keyframe_count())?)?;
                if let Some(destination) = destination {
                    let value = value.to_vec();
                    self.m3g_write_float_array(destination, &value)?;
                }
                CallOutcome::Return(Some(Value::Int(time)))
            }
            ("javax/microedition/m3g/KeyframeSequence", "setValidRange", "(II)V") => {
                let handle = self.m3g_receiver(args)?;
                let first = int_argument(args, 1)?;
                let last = int_argument(args, 2)?;
                let m3g::ObjectKind::KeyframeSequence(state) = self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                let first = keyframe_index(first, state.keyframe_count())?;
                let last = keyframe_index(last, state.keyframe_count())?;
                state.set_valid_range(first, last)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/KeyframeSequence", "setDuration", "(I)V") => {
                let handle = self.m3g_receiver(args)?;
                let duration = int_argument(args, 1)?;
                let m3g::ObjectKind::KeyframeSequence(state) = self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                state.set_duration(duration)?;
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/KeyframeSequence", "setRepeatMode", "(I)V") => {
                let mode = match int_argument(args, 1)? {
                    192 => m3g::RepeatMode::Constant,
                    193 => m3g::RepeatMode::Loop,
                    _ => {
                        return self.thread_exception(
                            "java/lang/IllegalArgumentException",
                            Some("invalid KeyframeSequence repeat mode"),
                        );
                    }
                };
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::KeyframeSequence(state) = self.m3g.runtime.kind_mut(handle)?
                else {
                    return Err(type_error());
                };
                state.set_repeat_mode(mode);
                CallOutcome::Return(None)
            }
            ("javax/microedition/m3g/KeyframeSequence", _, "()I") => {
                let handle = self.m3g_receiver(args)?;
                let m3g::ObjectKind::KeyframeSequence(state) = self.m3g.runtime.kind(handle)?
                else {
                    return Err(type_error());
                };
                let value = match name {
                    "getComponentCount" => {
                        i32::try_from(state.component_count()).unwrap_or(i32::MAX)
                    }
                    "getKeyframeCount" => i32::try_from(state.keyframe_count()).unwrap_or(i32::MAX),
                    "getInterpolationType" => match state.interpolation() {
                        m3g::Interpolation::Linear => 176,
                        m3g::Interpolation::Slerp => 177,
                        m3g::Interpolation::Spline => 178,
                        m3g::Interpolation::Squad => 179,
                        m3g::Interpolation::Step => 180,
                    },
                    "getValidRangeFirst" => {
                        i32::try_from(state.valid_range().0).unwrap_or(i32::MAX)
                    }
                    "getValidRangeLast" => i32::try_from(state.valid_range().1).unwrap_or(i32::MAX),
                    "getDuration" => state.duration(),
                    "getRepeatMode" => match state.repeat_mode() {
                        m3g::RepeatMode::Constant => 192,
                        m3g::RepeatMode::Loop => 193,
                    },
                    _ => return Err(type_error()),
                };
                CallOutcome::Return(Some(Value::Int(value)))
            }
            _ => return self.m3g_unsupported_native(),
        };
        Ok(outcome)
    }
}

fn keyframe_index(index: i32, count: usize) -> Result<usize, EmuError> {
    usize::try_from(index)
        .ok()
        .filter(|index| *index < count)
        .ok_or_else(|| {
            vm_error(
                "index-out-of-bounds-exception",
                "keyframe index is out of bounds",
            )
        })
}

#[cfg(test)]
#[path = "../../../../../tests/unit/vm/machine/m3g_dispatch/animation.rs"]
mod tests;
