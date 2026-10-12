use super::{
    Allocation, ArrayKind, CallOutcome, EmuError, Handle, HeapValue, M3gSpritePickContext, Machine,
    Method, Value, finite_float_argument, finite_non_negative, float_argument, heap_error,
    int_argument, m3g_bounded_count, m3g_bounded_image, m3g_image_format, m3g_non_negative_u32,
    m3g_non_negative_usize, m3g_positive_usize, optional_reference_argument, reference_argument,
    type_error, vm_error,
};

mod animation;
mod appearance;
mod camera;
mod graphics3d;
mod mesh;
mod objects;
mod scene_graph;
mod world;

impl Machine<'_, '_> {
    pub(super) fn invoke_m3g_native(
        &mut self,
        method: &Method,
        args: &[Value],
        _depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        let class = method.key.class.as_str();
        let name = method.key.name.as_str();
        let descriptor = method.key.descriptor.as_str();

        if name == "<init>" {
            let receiver = reference_argument(args, 0)?;
            let kind = match class {
                "javax/microedition/m3g/Transform" => {
                    let value = if descriptor == "(Ljavax/microedition/m3g/Transform;)V" {
                        let source = self.m3g_handle(reference_argument(args, 1)?)?;
                        self.m3g.runtime.transform_value(source)?
                    } else {
                        m3g::Mat4::IDENTITY
                    };
                    m3g::ObjectKind::Transform(value)
                }
                "javax/microedition/m3g/Group" => m3g::ObjectKind::Group {
                    node: m3g::NodeState::default(),
                    children: Vec::new(),
                },
                "javax/microedition/m3g/World" => m3g::ObjectKind::World {
                    node: m3g::NodeState::default(),
                    children: Vec::new(),
                    camera: None,
                    background: None,
                },
                "javax/microedition/m3g/Camera" => m3g::ObjectKind::Camera {
                    node: m3g::NodeState::default(),
                    projection: m3g::CameraProjection::default(),
                },
                "javax/microedition/m3g/Light" => m3g::ObjectKind::Light {
                    node: m3g::NodeState::default(),
                    light: m3g::LightState::default(),
                },
                "javax/microedition/m3g/Background" => {
                    m3g::ObjectKind::Background(m3g::BackgroundState::default())
                }
                "javax/microedition/m3g/VertexArray" => {
                    let vertex_count = m3g_positive_usize(int_argument(args, 1)?)?;
                    m3g_bounded_count(vertex_count, self.limits.m3g_vertices, "vertex")?;
                    let component_count = m3g_positive_usize(int_argument(args, 2)?)?;
                    let component_type = match int_argument(args, 3)? {
                        1 => m3g::VertexComponent::Byte,
                        2 => m3g::VertexComponent::Short,
                        _ => {
                            return self.thread_exception(
                                "java/lang/IllegalArgumentException",
                                Some("VertexArray component size must be one or two"),
                            );
                        }
                    };
                    m3g::ObjectKind::VertexArray(m3g::VertexArrayState::new(
                        vertex_count,
                        component_count,
                        component_type,
                    )?)
                }
                "javax/microedition/m3g/VertexBuffer" => m3g::ObjectKind::VertexBuffer {
                    state: m3g::VertexBufferState::default(),
                    arrays: [None; 5],
                },
                "javax/microedition/m3g/TriangleStripArray" => {
                    let strips = self
                        .m3g_int_array(reference_argument(args, 2)?)?
                        .into_iter()
                        .map(m3g_positive_usize)
                        .collect::<Result<Vec<_>, _>>()?;
                    let state = if descriptor == "(I[I)V" {
                        let first = u32::try_from(int_argument(args, 1)?).map_err(|_| {
                            vm_error("illegal-argument-exception", "negative first index")
                        })?;
                        m3g::TriangleStripArrayState::implicit(first, strips)?
                    } else {
                        let indices = self
                            .m3g_int_array(reference_argument(args, 1)?)?
                            .into_iter()
                            .map(|value| {
                                u32::try_from(value).map_err(|_| {
                                    vm_error("illegal-argument-exception", "negative index")
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        m3g::TriangleStripArrayState::new(indices, strips)?
                    };
                    m3g::ObjectKind::TriangleStripArray(state)
                }
                "javax/microedition/m3g/Image2D" if descriptor == "(III)V" => {
                    let format = m3g_image_format(int_argument(args, 1)?)?;
                    let width = u32::try_from(int_argument(args, 2)?).map_err(|_| {
                        vm_error("illegal-argument-exception", "negative Image2D width")
                    })?;
                    let height = u32::try_from(int_argument(args, 3)?).map_err(|_| {
                        vm_error("illegal-argument-exception", "negative Image2D height")
                    })?;
                    m3g_bounded_image(width, height, self.limits.m3g_texture_pixels)?;
                    m3g::ObjectKind::Image2D(m3g::Image2DState::mutable(format, width, height)?)
                }
                "javax/microedition/m3g/Image2D" if descriptor == "(III[B)V" => {
                    let format = m3g_image_format(int_argument(args, 1)?)?;
                    let width = u32::try_from(int_argument(args, 2)?).map_err(|_| {
                        vm_error("illegal-argument-exception", "negative Image2D width")
                    })?;
                    let height = u32::try_from(int_argument(args, 3)?).map_err(|_| {
                        vm_error("illegal-argument-exception", "negative Image2D height")
                    })?;
                    let count = m3g_bounded_image(width, height, self.limits.m3g_texture_pixels)?;
                    let bytes = self.m3g_byte_array_prefix(
                        reference_argument(args, 4)?,
                        count * format.components(),
                    )?;
                    m3g::ObjectKind::Image2D(m3g::Image2DState::from_bytes(
                        format, width, height, &bytes,
                    )?)
                }
                "javax/microedition/m3g/Image2D" if descriptor == "(III[B[B)V" => {
                    let format = m3g_image_format(int_argument(args, 1)?)?;
                    let width = m3g_non_negative_u32(int_argument(args, 2)?)?;
                    let height = m3g_non_negative_u32(int_argument(args, 3)?)?;
                    let count = m3g_bounded_image(width, height, self.limits.m3g_texture_pixels)?;
                    let indices =
                        self.m3g_byte_array_prefix(reference_argument(args, 4)?, count)?;
                    let palette = self.m3g_byte_array_prefix(
                        reference_argument(args, 5)?,
                        256 * format.components(),
                    )?;
                    m3g::ObjectKind::Image2D(m3g::Image2DState::from_palette(
                        format, width, height, &indices, &palette,
                    )?)
                }
                "javax/microedition/m3g/Image2D" if descriptor == "(ILjava/lang/Object;)V" => {
                    let format = m3g_image_format(int_argument(args, 1)?)?;
                    let image = reference_argument(args, 2)?;
                    let class = self.object_class(image)?;
                    if !self.is_instance(&class, "javax/microedition/lcdui/Image") {
                        return self.thread_exception(
                            "java/lang/IllegalArgumentException",
                            Some("Image2D source object must be a MIDP Image"),
                        );
                    }
                    let pixels = self.graphics_reference_field(
                        image,
                        "javax/microedition/lcdui/Image.pixels:[I",
                    )?;
                    let width = m3g_non_negative_u32(
                        self.graphics_int_field(image, "javax/microedition/lcdui/Image.width:I")?,
                    )?;
                    let height = m3g_non_negative_u32(
                        self.graphics_int_field(image, "javax/microedition/lcdui/Image.height:I")?,
                    )?;
                    m3g_bounded_image(width, height, self.limits.m3g_texture_pixels)?;
                    let pixels = self
                        .graphics_int_array_snapshot(pixels)?
                        .into_iter()
                        .map(i32::cast_unsigned)
                        .collect::<Vec<_>>();
                    m3g::ObjectKind::Image2D(m3g::Image2DState::from_argb(
                        format, width, height, &pixels,
                    )?)
                }
                "javax/microedition/m3g/Appearance" => {
                    m3g::ObjectKind::Appearance(m3g::AppearanceState::default())
                }
                "javax/microedition/m3g/CompositingMode" => {
                    m3g::ObjectKind::CompositingMode(m3g::CompositingModeState::default())
                }
                "javax/microedition/m3g/PolygonMode" => {
                    m3g::ObjectKind::PolygonMode(m3g::PolygonModeState::default())
                }
                "javax/microedition/m3g/Fog" => m3g::ObjectKind::Fog(m3g::FogState {
                    color: 0,
                    mode: m3g::FogMode::Linear,
                    near: 0.0,
                    far: 1.0,
                    density: 1.0,
                }),
                "javax/microedition/m3g/Material" => {
                    m3g::ObjectKind::Material(m3g::MaterialObjectState::default())
                }
                "javax/microedition/m3g/Texture2D" => {
                    let image = self.m3g_handle(reference_argument(args, 1)?)?;
                    self.m3g_validate_texture_image(image)?;
                    m3g::ObjectKind::Texture2D(m3g::TextureObjectState {
                        transformable: m3g::TransformableState::default(),
                        image,
                        level_filter: 208,
                        image_filter: 210,
                        wrap_s: 241,
                        wrap_t: 241,
                        blending: 227,
                        blend_color: 0,
                    })
                }
                "javax/microedition/m3g/AnimationController" => {
                    m3g::ObjectKind::AnimationController(m3g::AnimationControllerState::default())
                }
                "javax/microedition/m3g/KeyframeSequence" => {
                    let keyframes = m3g_positive_usize(int_argument(args, 1)?)?;
                    m3g_bounded_count(keyframes, self.limits.m3g_keyframes, "keyframe")?;
                    let components = m3g_positive_usize(int_argument(args, 2)?)?;
                    let interpolation = match int_argument(args, 3)? {
                        176 => m3g::Interpolation::Linear,
                        177 => m3g::Interpolation::Slerp,
                        178 => m3g::Interpolation::Spline,
                        179 => m3g::Interpolation::Squad,
                        180 => m3g::Interpolation::Step,
                        _ => {
                            return self.thread_exception(
                                "java/lang/IllegalArgumentException",
                                Some("invalid KeyframeSequence interpolation mode"),
                            );
                        }
                    };
                    m3g::ObjectKind::KeyframeSequence(m3g::KeyframeSequenceState::new(
                        keyframes,
                        components,
                        interpolation,
                    )?)
                }
                "javax/microedition/m3g/AnimationTrack" => {
                    let sequence = self.m3g_handle(reference_argument(args, 1)?)?;
                    if !matches!(
                        self.m3g.runtime.kind(sequence)?,
                        m3g::ObjectKind::KeyframeSequence(_)
                    ) {
                        return Err(type_error());
                    }
                    let property = int_argument(args, 2)?;
                    m3g::ObjectKind::AnimationTrack {
                        sequence,
                        controller: None,
                        property,
                    }
                }
                "javax/microedition/m3g/Mesh"
                | "javax/microedition/m3g/MorphingMesh"
                | "javax/microedition/m3g/SkinnedMesh" => {
                    let vertices = self.m3g_handle(reference_argument(args, 1)?)?;
                    if !matches!(
                        self.m3g.runtime.kind(vertices)?,
                        m3g::ObjectKind::VertexBuffer { .. }
                    ) {
                        return Err(type_error());
                    }
                    let array_form = descriptor.contains("[Ljavax/microedition/m3g/IndexBuffer;");
                    let (submeshes, appearances) = if array_form {
                        let submeshes = self
                            .m3g_reference_array(reference_argument(
                                args,
                                if class == "javax/microedition/m3g/MorphingMesh" {
                                    3
                                } else {
                                    2
                                },
                            )?)?
                            .into_iter()
                            .map(|value| {
                                value
                                    .ok_or_else(|| {
                                        vm_error("null-pointer-exception", "null IndexBuffer")
                                    })
                                    .and_then(|guest| self.m3g_handle(guest))
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let appearances = self
                            .m3g_reference_array(reference_argument(
                                args,
                                if class == "javax/microedition/m3g/MorphingMesh" {
                                    4
                                } else {
                                    3
                                },
                            )?)?
                            .into_iter()
                            .map(|value| value.map(|guest| self.m3g_handle(guest)).transpose())
                            .collect::<Result<Vec<_>, _>>()?;
                        (submeshes, appearances)
                    } else {
                        let index_argument = if class == "javax/microedition/m3g/MorphingMesh" {
                            3
                        } else {
                            2
                        };
                        let appearance_argument = index_argument + 1;
                        (
                            vec![self.m3g_handle(reference_argument(args, index_argument)?)?],
                            vec![
                                optional_reference_argument(args, appearance_argument)?
                                    .map(|guest| self.m3g_handle(guest))
                                    .transpose()?,
                            ],
                        )
                    };
                    if submeshes.is_empty() || submeshes.len() != appearances.len() {
                        return self.thread_exception(
                            "java/lang/IllegalArgumentException",
                            Some("Mesh submesh and appearance arrays must have equal non-zero lengths"),
                        );
                    }
                    for index in &submeshes {
                        if !matches!(
                            self.m3g.runtime.kind(*index)?,
                            m3g::ObjectKind::TriangleStripArray(_)
                        ) {
                            return Err(type_error());
                        }
                    }
                    let mesh = m3g::MeshState {
                        node: m3g::NodeState::default(),
                        vertices,
                        submeshes,
                        appearances,
                    };
                    match class {
                        "javax/microedition/m3g/Mesh" => m3g::ObjectKind::Mesh(mesh),
                        "javax/microedition/m3g/MorphingMesh" => {
                            let targets = self
                                .m3g_reference_array(reference_argument(args, 2)?)?
                                .into_iter()
                                .map(|value| {
                                    value
                                        .ok_or_else(|| {
                                            vm_error("null-pointer-exception", "null morph target")
                                        })
                                        .and_then(|guest| self.m3g_handle(guest))
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            if targets.is_empty() {
                                return self.thread_exception(
                                    "java/lang/IllegalArgumentException",
                                    Some("MorphingMesh requires at least one target"),
                                );
                            }
                            m3g::ObjectKind::MorphingMesh {
                                weights: vec![0.0; targets.len()],
                                targets,
                                mesh,
                            }
                        }
                        _ => {
                            let skeleton = self.m3g_handle(reference_argument(args, 4)?)?;
                            if !matches!(
                                self.m3g.runtime.kind(skeleton)?,
                                m3g::ObjectKind::Group { .. }
                            ) {
                                return Err(type_error());
                            }
                            m3g::ObjectKind::SkinnedMesh {
                                mesh,
                                skeleton,
                                bones: Vec::new(),
                                bind_transforms: Vec::new(),
                                influences: Vec::new(),
                            }
                        }
                    }
                }
                "javax/microedition/m3g/Sprite3D" => {
                    let image = self.m3g_handle(reference_argument(args, 2)?)?;
                    let m3g::ObjectKind::Image2D(image_state) = self.m3g.runtime.kind(image)?
                    else {
                        return Err(type_error());
                    };
                    let crop = m3g::SpriteState::image_crop(
                        image_state,
                        self.limits.m3g_max_sprite_crop_dimension,
                    );
                    let appearance = optional_reference_argument(args, 3)?
                        .map(|guest| self.m3g_handle(guest))
                        .transpose()?;
                    m3g::ObjectKind::Sprite3D(m3g::SpriteState {
                        node: m3g::NodeState::default(),
                        scaled: int_argument(args, 1)? != 0,
                        image,
                        appearance,
                        crop,
                    })
                }
                "javax/microedition/m3g/RayIntersection" => {
                    m3g::ObjectKind::RayIntersection(m3g::RayIntersectionState::default())
                }
                "javax/microedition/m3g/Graphics3D" => m3g::ObjectKind::Object,
                _ if self
                    .program
                    .is_assignable_to(class, "javax/microedition/m3g/Node") =>
                {
                    m3g::ObjectKind::Node(m3g::NodeState::default())
                }
                _ if self
                    .program
                    .is_assignable_to(class, "javax/microedition/m3g/Transformable") =>
                {
                    m3g::ObjectKind::Transformable(m3g::TransformableState::default())
                }
                _ => m3g::ObjectKind::Object,
            };
            let created = match self.m3g_allocate_native(args, |runtime| {
                runtime.create(Some(receiver.to_raw()), kind.clone())
            }) {
                Ok(created) => created,
                Err(error) => return self.error_as_call_outcome(error, args),
            };
            if class == "javax/microedition/m3g/SkinnedMesh"
                && let Err(error) =
                    self.m3g_allocate_native(args, |runtime| runtime.bind_skin_skeleton(created))
            {
                self.m3g.runtime.rollback_created(&[created]);
                return self.error_as_call_outcome(error, args);
            }
            return Ok(CallOutcome::Return(None));
        }

        match class {
            "javax/microedition/m3g/Loader"
            | "javax/microedition/m3g/Object3D"
            | "javax/microedition/m3g/VertexArray"
            | "javax/microedition/m3g/IndexBuffer"
            | "javax/microedition/m3g/VertexBuffer"
            | "javax/microedition/m3g/Image2D" => {
                self.invoke_m3g_object_native(class, name, descriptor, args)
            }
            "javax/microedition/m3g/Appearance"
            | "javax/microedition/m3g/CompositingMode"
            | "javax/microedition/m3g/PolygonMode"
            | "javax/microedition/m3g/Fog"
            | "javax/microedition/m3g/Material"
            | "javax/microedition/m3g/Texture2D" => {
                self.invoke_m3g_appearance_native(class, name, descriptor, args)
            }
            "javax/microedition/m3g/AnimationController"
            | "javax/microedition/m3g/AnimationTrack"
            | "javax/microedition/m3g/KeyframeSequence" => {
                self.invoke_m3g_animation_native(class, name, descriptor, args)
            }
            "javax/microedition/m3g/Mesh"
            | "javax/microedition/m3g/MorphingMesh"
            | "javax/microedition/m3g/Sprite3D"
            | "javax/microedition/m3g/SkinnedMesh"
            | "javax/microedition/m3g/RayIntersection" => {
                self.invoke_m3g_mesh_native(class, name, descriptor, args)
            }
            "javax/microedition/m3g/Transform"
            | "javax/microedition/m3g/Transformable"
            | "javax/microedition/m3g/Node"
            | "javax/microedition/m3g/Group" => {
                self.invoke_m3g_scene_graph_native(class, name, descriptor, args)
            }
            "javax/microedition/m3g/Camera" => {
                self.invoke_m3g_camera_native(name, descriptor, args)
            }
            "javax/microedition/m3g/World"
            | "javax/microedition/m3g/Light"
            | "javax/microedition/m3g/Background" => {
                self.invoke_m3g_world_native(class, name, descriptor, args)
            }
            "javax/microedition/m3g/Graphics3D" => {
                self.invoke_m3g_graphics3d_native(class, name, descriptor, args)
            }
            _ => self.m3g_unsupported_native(),
        }
    }

    pub(in crate::machine) fn m3g_unsupported_native(&mut self) -> Result<CallOutcome, EmuError> {
        self.thread_exception(
            "java/lang/UnsupportedOperationException",
            Some("M3G method has no executable bridge contract yet"),
        )
    }
}
