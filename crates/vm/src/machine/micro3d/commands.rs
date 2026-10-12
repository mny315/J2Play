use super::{
    ArrayKind, EmuError, Handle, Machine, Value, command_slice, command_value, int_argument,
    micro3d_command_scissor, micro3d_primitive_payload_counts, optional_reference_argument,
    reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    #[allow(clippy::too_many_lines)]
    pub(in crate::machine) fn micro3d_draw_command_list(
        &mut self,
        args: &[Value],
    ) -> Result<(), EmuError> {
        self.micro3d_check_command_cancellation()?;
        let (_, target_width, target_height) = self.micro3d_bound_target()?;
        let texture_handles = if let Some(texture_or_array) = optional_reference_argument(args, 1)?
        {
            if matches!(
                self.heap.managed.array_kind(texture_or_array),
                Ok(ArrayKind::Reference(_))
            ) {
                self.m3g_reference_array(texture_or_array)?
                    .into_iter()
                    .map(|value| value.map(Handle::to_raw))
                    .collect::<Vec<_>>()
            } else {
                vec![Some(texture_or_array.to_raw())]
            }
        } else {
            Vec::new()
        };
        for texture in texture_handles.iter().flatten() {
            self.micro3d_validate_texture_kind(*texture, true)?;
        }
        let layout_guest = reference_argument(args, 4)?.to_raw();
        let effect_guest = reference_argument(args, 5)?.to_raw();
        let (mut layout, _) = self.micro3d.runtime.layout_render_snapshot(layout_guest)?;
        let mut effect = match self.micro3d.runtime.kind(effect_guest)? {
            micro3d::ObjectKind::Effect(value) => *value,
            _ => return Err(type_error()),
        };
        let mut command_light = match effect.light {
            Some(guest) => match self.micro3d.runtime.kind(guest)? {
                micro3d::ObjectKind::Light(value) => *value,
                _ => return Err(type_error()),
            },
            None => micro3d::LightState::default(),
        };
        let mut command_attributes = None;
        let commands = self.m3g_int_array(reference_argument(args, 6)?)?;
        if commands.first().copied() != Some(-33_554_431) {
            return Err(vm_error(
                "illegal-argument-exception",
                "Micro3D command list has no Version 1.0 marker",
            ));
        }
        let mut cursor = 1_usize;
        let mut texture_index = 0_usize;
        // Guest math fields cannot change during this native call. Resolve on
        // first use and discard the snapshot when a command selects an affine.
        let mut cached_affine = None;
        let mut remaining_triangles = self.limits.m3g_render.triangles;
        let mut remaining_fragments = self.limits.m3g_render.fragments;
        while let Some(&command) = commands.get(cursor) {
            self.micro3d_check_command_cancellation()?;
            cursor += 1;
            let opcode = command.cast_unsigned() & 0xff00_0000;
            match opcode {
                0x8000_0000 => break,
                0x8100_0000 => {}
                0x8200_0000 => self.micro3d_flush_target()?,
                0x8300_0000 => {
                    let attributes = command.cast_unsigned() & 0x00ff_ffff;
                    if attributes & !0x0f != 0 {
                        return Err(vm_error(
                            "illegal-argument-exception",
                            "Micro3D command contains unknown environment attributes",
                        ));
                    }
                    command_attributes = Some(attributes as u8);
                }
                0x8400_0000 => {
                    let clip = [
                        command_value(&commands, &mut cursor)?,
                        command_value(&commands, &mut cursor)?,
                        command_value(&commands, &mut cursor)?,
                        command_value(&commands, &mut cursor)?,
                    ];
                    self.micro3d.command_scissor = micro3d_command_scissor(
                        target_width,
                        target_height,
                        self.micro3d.target_scissor,
                        clip,
                    )?;
                }
                0x8500_0000 => {
                    layout.center = [
                        command_value(&commands, &mut cursor)?,
                        command_value(&commands, &mut cursor)?,
                    ];
                }
                0x8600_0000 => {
                    texture_index = usize::try_from(command.cast_unsigned() & 0x00ff_ffff)
                        .unwrap_or(usize::MAX);
                    if texture_index >= texture_handles.len() {
                        return Err(vm_error(
                            "illegal-argument-exception",
                            "Micro3D command texture index is out of bounds",
                        ));
                    }
                }
                0x8700_0000 => {
                    layout.selected_affine = usize::try_from(command.cast_unsigned() & 0x00ff_ffff)
                        .unwrap_or(usize::MAX);
                    if self
                        .micro3d
                        .runtime
                        .layout_affine(layout_guest, layout.selected_affine)?
                        .is_none()
                    {
                        return Err(vm_error(
                            "illegal-argument-exception",
                            "Micro3D command affine index is out of bounds",
                        ));
                    }
                    cached_affine = None;
                }
                0x9000_0000 => {
                    layout.scale = [
                        command_value(&commands, &mut cursor)?,
                        command_value(&commands, &mut cursor)?,
                    ];
                    layout.projection = micro3d::Projection::ParallelScale;
                }
                0x9100_0000 => {
                    layout.projection = micro3d::Projection::Parallel {
                        width: command_value(&commands, &mut cursor)?,
                        height: command_value(&commands, &mut cursor)?,
                    };
                }
                0x9200_0000 => {
                    layout.projection = micro3d::Projection::PerspectiveFov {
                        near: command_value(&commands, &mut cursor)?,
                        far: command_value(&commands, &mut cursor)?,
                        angle: command_value(&commands, &mut cursor)?,
                    };
                }
                0x9300_0000 => {
                    layout.projection = micro3d::Projection::PerspectiveSize {
                        near: command_value(&commands, &mut cursor)?,
                        far: command_value(&commands, &mut cursor)?,
                        width: command_value(&commands, &mut cursor)?,
                        height: command_value(&commands, &mut cursor)?,
                    };
                }
                0xaf00_0000 => {
                    let toon = [
                        command_value(&commands, &mut cursor)?,
                        command_value(&commands, &mut cursor)?,
                        command_value(&commands, &mut cursor)?,
                    ];
                    if !toon.into_iter().all(|value| (0..=255).contains(&value)) {
                        return Err(vm_error(
                            "illegal-argument-exception",
                            "Micro3D toon parameters must be between 0 and 255",
                        ));
                    }
                    [effect.toon_threshold, effect.toon_high, effect.toon_low] = toon;
                }
                0xa000_0000 => {
                    command_light.ambient_intensity = command_value(&commands, &mut cursor)?;
                }
                0xa100_0000 => {
                    command_light.direction = micro3d::Vector3D::new(
                        command_value(&commands, &mut cursor)?,
                        command_value(&commands, &mut cursor)?,
                        command_value(&commands, &mut cursor)?,
                    );
                    command_light.directional_intensity = command_value(&commands, &mut cursor)?;
                }
                _ if matches!(
                    opcode,
                    0x0100_0000 | 0x0200_0000 | 0x0300_0000 | 0x0400_0000 | 0x0500_0000
                ) =>
                {
                    let primitive_count = usize::try_from((command.cast_unsigned() >> 16) & 0xff)
                        .unwrap_or(usize::MAX);
                    if primitive_count == 0 {
                        return Err(vm_error(
                            "illegal-argument-exception",
                            "empty primitive command",
                        ));
                    }
                    let (vertex_count, normal_count, texture_count, color_count) =
                        micro3d_primitive_payload_counts(command, primitive_count)?;
                    let coordinates =
                        command_slice(&commands, &mut cursor, vertex_count.saturating_mul(3))?;
                    let normals = command_slice(&commands, &mut cursor, normal_count)?;
                    let texture_coordinates = command_slice(&commands, &mut cursor, texture_count)?;
                    let colors = command_slice(&commands, &mut cursor, color_count)?;
                    let affine = if let Some(affine) = cached_affine {
                        affine
                    } else {
                        let affine = self
                            .micro3d
                            .runtime
                            .layout_affine(layout_guest, layout.selected_affine)?
                            .map(|guest| self.micro3d_affine(Handle::from_raw(guest)))
                            .transpose()?
                            .unwrap_or(micro3d::AffineTrans::IDENTITY);
                        cached_affine = Some(affine);
                        affine
                    };
                    let texture = match texture_handles.get(texture_index).copied() {
                        Some(Some(texture)) => Some(texture),
                        Some(None) => {
                            return Err(vm_error(
                                "null-pointer-exception",
                                "Micro3D command selected a null Texture",
                            ));
                        }
                        None => None,
                    };
                    self.micro3d_prepare_render_target()?;
                    self.micro3d.runtime.render_primitives_with_environment(
                        texture,
                        int_argument(args, 2)?,
                        int_argument(args, 3)?,
                        micro3d::FigureLayoutState {
                            affines: Vec::new(),
                            selected_affine: layout.selected_affine,
                            scale: layout.scale,
                            center: layout.center,
                            projection: layout.projection,
                        },
                        affine,
                        effect,
                        micro3d::PrimitiveData::new(
                            command,
                            primitive_count,
                            coordinates,
                            normals,
                            texture_coordinates,
                            colors,
                        ),
                        micro3d::PrimitiveEnvironment {
                            attributes: command_attributes,
                            light_override: Some(command_light),
                        },
                    )?;
                    // Each primitive batch has its own renderer limit. Also bound the
                    // entire list, including batches separated by explicit flushes.
                    // At most one individually bounded batch can overshoot this budget.
                    let stats = self.micro3d.runtime.render_stats();
                    (remaining_triangles, remaining_fragments) = remaining_triangles
                        .checked_sub(stats.submitted_triangles)
                        .zip(remaining_fragments.checked_sub(stats.tested_fragments))
                        .ok_or_else(|| {
                            vm_error(
                                "render-budget",
                                "Micro3D command list render budget exhausted",
                            )
                        })?;
                    self.micro3d.render_pending = true;
                }
                _ => {
                    return Err(vm_error(
                        "illegal-argument-exception",
                        format!("unsupported Micro3D command opcode 0x{opcode:08x}"),
                    ));
                }
            }
        }
        Ok(())
    }

    fn micro3d_check_command_cancellation(&self) -> Result<(), EmuError> {
        if self.native_context.execution_cancelled() {
            Err(vm_error(
                "execution-cancelled",
                "Micro3D command list cancelled",
            ))
        } else {
            Ok(())
        }
    }
}
