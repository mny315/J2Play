use super::{EmuError, Machine, type_error, vm_error};

impl Machine<'_, '_> {
    pub(in crate::machine) fn m3g_animate(
        &mut self,
        object: m3g::Handle,
        world_time: i32,
    ) -> Result<i32, EmuError> {
        // Validate the complete referenced graph before changing the scene.
        let check_cancelled = || {
            if self.native_context.execution_cancelled() {
                Err(vm_error("execution-cancelled", "M3G animation cancelled"))
            } else {
                Ok(())
            }
        };
        let mut pending = vec![object];
        let mut visited = std::collections::BTreeSet::new();
        let mut updates = Vec::new();
        while let Some(target) = pending.pop() {
            check_cancelled()?;
            if !visited.insert(target) {
                continue;
            }
            let start = pending.len();
            self.m3g.runtime.extend_references(target, &mut pending)?;
            pending[start..].reverse();
            let mut samples: std::collections::BTreeMap<i32, Vec<(Vec<f32>, f32)>> =
                std::collections::BTreeMap::new();
            for (index, &track) in self
                .m3g
                .runtime
                .animation_tracks(target)?
                .iter()
                .enumerate()
            {
                if index.is_multiple_of(64) {
                    check_cancelled()?;
                }
                let (sequence, controller, property) = match self.m3g.runtime.kind(track)? {
                    m3g::ObjectKind::AnimationTrack {
                        sequence,
                        controller,
                        property,
                    } => (*sequence, *controller, *property),
                    _ => return Err(type_error()),
                };
                let Some(controller) = controller else {
                    // JSR-184 leaves the target property unchanged unless its
                    // track is associated with an active controller.
                    continue;
                };
                let (sequence_time, weight) = {
                    let m3g::ObjectKind::AnimationController(controller) =
                        self.m3g.runtime.kind(controller)?
                    else {
                        return Err(type_error());
                    };
                    (
                        controller.position(world_time),
                        controller.effective_weight(world_time),
                    )
                };
                if weight <= 0.0 {
                    continue;
                }
                let m3g::ObjectKind::KeyframeSequence(sequence) =
                    self.m3g.runtime.kind(sequence)?
                else {
                    return Err(type_error());
                };
                let value = sequence.sample(sequence_time).map_err(|error| {
                    if matches!(error.code(), "invalid-duration" | "unordered-keyframe") {
                        // These constraints are checked when applying the
                        // animation, rather than rejecting a setter argument.
                        vm_error("illegal-state-exception", error.to_string())
                    } else {
                        error
                    }
                })?;
                samples.entry(property).or_default().push((value, weight));
            }
            for (property, values) in samples {
                let value = m3g::blend_samples(&values)?;
                self.m3g_validate_animation_property(target, property, &value)?;
                updates.push((target, property, value));
            }
        }
        // Cancellation discards prepared values; the final commit remains atomic.
        check_cancelled()?;
        if updates.is_empty() {
            return Ok(i32::MAX);
        }
        self.m3g.metrics.animation_samples = self
            .m3g
            .metrics
            .animation_samples
            .saturating_add(u64::try_from(updates.len()).unwrap_or(u64::MAX));
        for (target, property, value) in updates {
            self.m3g_apply_animation_property(target, property, &value)?;
        }
        Ok(1)
    }

    fn m3g_validate_animation_property(
        &self,
        object: m3g::Handle,
        property: i32,
        value: &[f32],
    ) -> Result<(), EmuError> {
        self.m3g
            .runtime
            .validate_animation_property(object, property, value.len())?;
        if !value.iter().all(|component| component.is_finite()) {
            return Err(vm_error(
                "illegal-argument-exception",
                "animation sample contains a non-finite component",
            ));
        }
        if property == 268 {
            m3g::Quaternion {
                x: value[0],
                y: value[1],
                z: value[2],
                w: value[3],
            }
            .normalized()?;
        }
        Ok(())
    }

    fn m3g_apply_animation_property(
        &mut self,
        object: m3g::Handle,
        property: i32,
        value: &[f32],
    ) -> Result<(), EmuError> {
        let component = |index: usize| {
            value.get(index).copied().ok_or_else(|| {
                vm_error(
                    "illegal-argument-exception",
                    "animation sample has too few components",
                )
            })
        };
        match property {
            256 => {
                let alpha = component(0)?.clamp(0.0, 1.0);
                let alpha_byte = (alpha * 255.0).round() as u32;
                match self.m3g.runtime.kind_mut(object)? {
                    m3g::ObjectKind::Background(state) => {
                        state.color = alpha_byte << 24 | state.color & 0x00ff_ffff;
                        Ok(())
                    }
                    m3g::ObjectKind::VertexBuffer { state, .. } => {
                        state.set_default_color(
                            alpha_byte << 24 | state.default_color() & 0x00ff_ffff,
                        );
                        Ok(())
                    }
                    m3g::ObjectKind::Material(state) => {
                        state.material.diffuse =
                            alpha_byte << 24 | state.material.diffuse & 0x00ff_ffff;
                        Ok(())
                    }
                    _ => {
                        let (rendering, picking, scope, _) = self.m3g.runtime.node_state(object)?;
                        self.m3g
                            .runtime
                            .set_node_state(object, rendering, picking, scope, alpha)
                    }
                }
            }
            269 | 276 => {
                let (rendering, picking, scope, alpha) = self.m3g.runtime.node_state(object)?;
                let enabled = component(0)? >= 0.5;
                self.m3g.runtime.set_node_state(
                    object,
                    if property == 276 { enabled } else { rendering },
                    if property == 269 { enabled } else { picking },
                    scope,
                    alpha,
                )
            }
            270 => {
                let x = component(0)?;
                let scale = if value.len() == 1 {
                    m3g::Vec3::new(x, x, x)
                } else {
                    m3g::Vec3::new(x, component(1)?, component(2)?)
                };
                self.m3g.runtime.set_scale(object, scale)
            }
            275 => self.m3g.runtime.set_translation(
                object,
                m3g::Vec3::new(component(0)?, component(1)?, component(2)?),
            ),
            268 => self.m3g.runtime.set_orientation_quaternion(
                object,
                m3g::Quaternion {
                    x: component(0)?,
                    y: component(1)?,
                    z: component(2)?,
                    w: component(3)?,
                },
            ),
            259 => {
                let maximum =
                    i32::try_from(self.limits.m3g_max_sprite_crop_dimension).unwrap_or(i32::MAX);
                match self.m3g.runtime.kind_mut(object)? {
                    m3g::ObjectKind::Background(state) => {
                        apply_crop(&mut state.crop, value, 0, i32::MAX);
                    }
                    m3g::ObjectKind::Sprite3D(state) => {
                        apply_crop(&mut state.crop, value, -maximum, maximum);
                    }
                    _ => return Err(type_error()),
                }
                Ok(())
            }
            260 => {
                let m3g::ObjectKind::Fog(state) = self.m3g.runtime.kind_mut(object)? else {
                    return Err(type_error());
                };
                state.density = component(0)?.max(0.0);
                Ok(())
            }
            263 | 267 => {
                let value = component(0)?;
                match self.m3g.runtime.kind_mut(object)? {
                    m3g::ObjectKind::Fog(state) => {
                        if property == 263 {
                            state.far = value;
                        } else {
                            state.near = value;
                        }
                    }
                    m3g::ObjectKind::Camera { projection, .. } => match projection {
                        m3g::CameraProjection::Parallel { near, far, .. } => {
                            if property == 263 {
                                *far = value;
                            } else {
                                *near = value;
                            }
                        }
                        m3g::CameraProjection::Perspective { near, far, .. } => {
                            let value = value.max(f32::from_bits(1));
                            if property == 263 {
                                *far = value;
                            } else {
                                *near = value;
                            }
                        }
                        m3g::CameraProjection::Generic(_) => {}
                    },
                    _ => return Err(type_error()),
                }
                Ok(())
            }
            264 => {
                let m3g::ObjectKind::Camera { projection, .. } =
                    self.m3g.runtime.kind_mut(object)?
                else {
                    return Err(type_error());
                };
                match projection {
                    m3g::CameraProjection::Parallel { height, .. } => {
                        *height = component(0)?.max(f32::from_bits(1));
                    }
                    m3g::CameraProjection::Perspective { field_of_view, .. } => {
                        *field_of_view =
                            component(0)?.clamp(f32::from_bits(1), 180.0_f32.next_down());
                    }
                    m3g::CameraProjection::Generic(_) => {}
                }
                Ok(())
            }
            265 | 273 | 274 => {
                let m3g::ObjectKind::Light { light, .. } = self.m3g.runtime.kind_mut(object)?
                else {
                    return Err(type_error());
                };
                match property {
                    265 => light.intensity = component(0)?,
                    273 => light.spot_angle = component(0)?.clamp(0.0, 90.0),
                    _ => light.spot_exponent = component(0)?.clamp(0.0, 128.0),
                }
                Ok(())
            }
            271 => {
                let m3g::ObjectKind::Material(state) = self.m3g.runtime.kind_mut(object)? else {
                    return Err(type_error());
                };
                state.material.shininess = component(0)?.clamp(0.0, 128.0);
                Ok(())
            }
            266 => {
                let m3g::ObjectKind::MorphingMesh { weights, .. } =
                    self.m3g.runtime.kind_mut(object)?
                else {
                    return Err(type_error());
                };
                for (index, weight) in weights.iter_mut().enumerate() {
                    *weight = value.get(index).copied().unwrap_or(0.0);
                }
                Ok(())
            }
            258 | 257 | 261 | 262 | 272 => {
                let byte = |index: usize| (value[index].clamp(0.0, 1.0) * 255.0).round() as u32;
                let color = byte(0) << 16 | byte(1) << 8 | byte(2);
                let with_alpha = |previous| previous & 0xff00_0000 | color;
                match self.m3g.runtime.kind_mut(object)? {
                    m3g::ObjectKind::Background(state) if property == 258 => {
                        state.color = with_alpha(state.color);
                    }
                    m3g::ObjectKind::Fog(state) if property == 258 => {
                        state.color = color;
                    }
                    m3g::ObjectKind::Light { light, .. } if property == 258 => {
                        light.color = color;
                    }
                    m3g::ObjectKind::Texture2D(state) if property == 258 => {
                        state.blend_color = color;
                    }
                    m3g::ObjectKind::VertexBuffer { state, .. } if property == 258 => {
                        state.set_default_color(with_alpha(state.default_color()));
                    }
                    m3g::ObjectKind::Material(state) => match property {
                        257 => state.material.ambient = color,
                        261 => state.material.diffuse = with_alpha(state.material.diffuse),
                        262 => state.material.emissive = color,
                        272 => state.material.specular = color,
                        _ => return Err(type_error()),
                    },
                    _ => return Err(type_error()),
                }
                Ok(())
            }
            _ => Err(vm_error(
                "unsupported-native",
                "animation target property is not implemented for this object",
            )),
        }
    }
}

fn apply_crop(crop: &mut [i32; 4], value: &[f32], minimum: i32, maximum: i32) {
    for (index, (destination, value)) in crop.iter_mut().zip(value).enumerate() {
        let value = value.round() as i32;
        *destination = if index < 2 {
            value
        } else {
            value.clamp(minimum, maximum)
        };
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/vm/machine/m3g_runtime/animation.rs"]
mod tests;
