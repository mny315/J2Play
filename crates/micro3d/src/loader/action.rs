//! MTRA data and bounded decoding of actions and bone channels.

use super::{
    AffineTrans, Cursor, EmuError, LoaderLimits, TRAILER_BYTES, Vector3D, bounded_file,
    checked_mul, loader_error, validate_retained_bytes, validate_trailer,
};

mod sampling;

/// One vector-valued MTRA keyframe.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorKeyframe {
    pub frame: u16,
    pub value: Vector3D,
}

/// One scalar-valued MTRA keyframe.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScalarKeyframe {
    pub frame: u16,
    pub value: i32,
}

/// Decoded animation channels for one Figure bone.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ActionSegmentData {
    Affine(AffineTrans),
    Components {
        translation: Vec<VectorKeyframe>,
        scale: Vec<VectorKeyframe>,
        rotation: Vec<VectorKeyframe>,
        roll: Vec<ScalarKeyframe>,
    },
}

/// One independently selectable action and its per-bone channels.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ActionData {
    pub frame_count: u16,
    pub segments: Vec<ActionSegmentData>,
    /// MTRA v5 keys: frame, low mask word, high mask word. File bit zero
    /// addresses the common group, unlike the public Figure pattern mask.
    pub pattern_keys: Vec<[u16; 3]>,
}

/// Parsed MTRA action table.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ActionTableData {
    pub format_version: u16,
    pub frame_counts: Vec<u16>,
    pub actions: Vec<ActionData>,
}

impl ActionTableData {
    pub fn parse(bytes: &[u8], limits: LoaderLimits) -> Result<Self, EmuError> {
        bounded_file(bytes, limits)?;
        let mut cursor = Cursor::new(bytes);
        if cursor.take(2)? != b"MT" || bytes.starts_with(b"MThd") {
            return Err(loader_error(
                "action-magic",
                "ActionTable resource does not start with MT",
            ));
        }
        let format_version = cursor.u16()?;
        if !matches!(format_version, 4 | 5) {
            return Err(loader_error(
                "action-version",
                format!("unsupported MTRA format version {format_version}"),
            ));
        }
        let action_count = usize::from(cursor.u16()?);
        if action_count == 0 || action_count > limits.actions {
            return Err(loader_error(
                "action-count",
                "ActionTable action count exceeds its budget",
            ));
        }
        let (frame_counts, actions) =
            parse_actions(&mut cursor, action_count, limits, format_version == 5)?;
        if bytes.len() < cursor.position().saturating_add(TRAILER_BYTES) {
            return Err(loader_error(
                "action-truncated",
                "ActionTable body/trailer is truncated",
            ));
        }
        if cursor.position() != bytes.len() - TRAILER_BYTES {
            return Err(loader_error(
                "action-body-length",
                "ActionTable body does not end at its trailer",
            ));
        }
        validate_trailer(&bytes[bytes.len() - TRAILER_BYTES..])?;
        let table = Self {
            format_version,
            frame_counts,
            actions,
        };
        validate_retained_bytes(table.allocated_bytes(), limits, "ActionTable")?;
        Ok(table)
    }

    #[must_use]
    pub fn allocated_bytes(&self) -> usize {
        let decoded = self.actions.iter().fold(
            self.actions
                .capacity()
                .saturating_mul(size_of::<ActionData>()),
            |total, action| {
                total
                    .saturating_add(
                        action
                            .segments
                            .capacity()
                            .saturating_mul(size_of::<ActionSegmentData>()),
                    )
                    .saturating_add(
                        action
                            .pattern_keys
                            .capacity()
                            .saturating_mul(size_of::<[u16; 3]>()),
                    )
                    .saturating_add(action.segments.iter().fold(0_usize, |sum, segment| {
                        let ActionSegmentData::Components {
                            translation,
                            scale,
                            rotation,
                            roll,
                        } = segment
                        else {
                            return sum;
                        };
                        sum.saturating_add(
                            translation
                                .capacity()
                                .saturating_mul(size_of::<VectorKeyframe>()),
                        )
                        .saturating_add(
                            scale.capacity().saturating_mul(size_of::<VectorKeyframe>()),
                        )
                        .saturating_add(
                            rotation
                                .capacity()
                                .saturating_mul(size_of::<VectorKeyframe>()),
                        )
                        .saturating_add(roll.capacity().saturating_mul(size_of::<ScalarKeyframe>()))
                    }))
            },
        );
        self.frame_counts
            .capacity()
            .saturating_mul(size_of::<u16>())
            .saturating_add(decoded)
    }
}

fn parse_actions(
    cursor: &mut Cursor<'_>,
    action_count: usize,
    limits: LoaderLimits,
    has_pattern_keys: bool,
) -> Result<(Vec<u16>, Vec<ActionData>), EmuError> {
    let segment_count = usize::from(cursor.u16()?);
    if segment_count == 0 || segment_count > limits.bones {
        return Err(loader_error(
            "action-segment-count",
            "ActionTable segment count exceeds its budget",
        ));
    }
    let segment_total = checked_mul(action_count, segment_count, "ActionTable segment")?;
    let mut decoded_bytes = checked_mul(
        segment_total,
        size_of::<ActionSegmentData>(),
        "ActionTable decoded segment",
    )?
    .checked_add(checked_mul(
        action_count,
        size_of::<ActionData>() + size_of::<u16>(),
        "ActionTable action",
    )?)
    .ok_or_else(|| loader_error("resource-overflow", "ActionTable decoded size overflow"))?;
    if decoded_bytes > limits.decoded_bytes {
        return Err(loader_error(
            "resource-budget",
            "ActionTable decoded segments exceed the native-memory budget",
        ));
    }
    // A converter identity occupies the 20 bytes after the action and segment
    // counts. It is opaque to the runtime but part of every corpus MTRA v4/v5
    // header; the first action frame count follows it immediately.
    cursor.skip(20)?;
    let mut frame_counts = Vec::with_capacity(action_count);
    let mut actions = Vec::with_capacity(action_count);
    let mut keyframe_count = 0_usize;
    for _ in 0..action_count {
        let frame_count = cursor.u16()?;
        frame_counts.push(frame_count);
        let mut segments = Vec::with_capacity(segment_count);
        for _ in 0..segment_count {
            let segment = match cursor.u8()? {
                0 => {
                    let mut values = [0; 12];
                    for value in &mut values {
                        *value = i32::from(cursor.i16()?);
                    }
                    ActionSegmentData::Affine(AffineTrans::new(values))
                }
                1 => ActionSegmentData::Affine(AffineTrans::IDENTITY),
                2 => component_segment(
                    vector_channel(cursor, &mut keyframe_count, &mut decoded_bytes, limits)?,
                    vector_channel(cursor, &mut keyframe_count, &mut decoded_bytes, limits)?,
                    vector_channel(cursor, &mut keyframe_count, &mut decoded_bytes, limits)?,
                    scalar_channel(cursor, &mut keyframe_count, &mut decoded_bytes, limits)?,
                ),
                // Omitted channels use sampling defaults without allocating
                // synthetic keyframes or charging them against the limits.
                3 => {
                    let translation =
                        constant_vector(cursor, &mut keyframe_count, &mut decoded_bytes, limits)?;
                    let rotation =
                        vector_channel(cursor, &mut keyframe_count, &mut decoded_bytes, limits)?;
                    let roll =
                        constant_scalar(cursor, &mut keyframe_count, &mut decoded_bytes, limits)?;
                    component_segment(translation, Vec::new(), rotation, roll)
                }
                4 => component_segment(
                    Vec::new(),
                    Vec::new(),
                    vector_channel(cursor, &mut keyframe_count, &mut decoded_bytes, limits)?,
                    scalar_channel(cursor, &mut keyframe_count, &mut decoded_bytes, limits)?,
                ),
                5 => component_segment(
                    Vec::new(),
                    Vec::new(),
                    vector_channel(cursor, &mut keyframe_count, &mut decoded_bytes, limits)?,
                    Vec::new(),
                ),
                6 => component_segment(
                    vector_channel(cursor, &mut keyframe_count, &mut decoded_bytes, limits)?,
                    Vec::new(),
                    vector_channel(cursor, &mut keyframe_count, &mut decoded_bytes, limits)?,
                    scalar_channel(cursor, &mut keyframe_count, &mut decoded_bytes, limits)?,
                ),
                kind => {
                    return Err(loader_error(
                        "action-segment-type",
                        format!(
                            "unsupported MTRA segment encoding {kind} at byte {}",
                            cursor.position().saturating_sub(1)
                        ),
                    ));
                }
            };
            segments.push(segment);
        }
        let mut pattern_keys = Vec::new();
        if has_pattern_keys {
            let auxiliary_count = usize::from(cursor.u16()?);
            consume_keyframes(&mut keyframe_count, auxiliary_count, limits)?;
            consume_decoded_items::<[u16; 3]>(&mut decoded_bytes, auxiliary_count, limits)?;
            pattern_keys = Vec::with_capacity(auxiliary_count);
            for _ in 0..auxiliary_count {
                pattern_keys.push([cursor.u16()?, cursor.u16()?, cursor.u16()?]);
            }
        }
        actions.push(ActionData {
            frame_count,
            segments,
            pattern_keys,
        });
    }
    Ok((frame_counts, actions))
}

fn vector_channel(
    cursor: &mut Cursor<'_>,
    keyframe_count: &mut usize,
    decoded_bytes: &mut usize,
    limits: LoaderLimits,
) -> Result<Vec<VectorKeyframe>, EmuError> {
    let count = usize::from(cursor.u16()?);
    consume_keyframes(keyframe_count, count, limits)?;
    consume_decoded_items::<VectorKeyframe>(decoded_bytes, count, limits)?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(VectorKeyframe {
            frame: cursor.u16()?,
            value: Vector3D::new(
                i32::from(cursor.i16()?),
                i32::from(cursor.i16()?),
                i32::from(cursor.i16()?),
            ),
        });
    }
    Ok(values)
}

fn scalar_channel(
    cursor: &mut Cursor<'_>,
    keyframe_count: &mut usize,
    decoded_bytes: &mut usize,
    limits: LoaderLimits,
) -> Result<Vec<ScalarKeyframe>, EmuError> {
    let count = usize::from(cursor.u16()?);
    consume_keyframes(keyframe_count, count, limits)?;
    consume_decoded_items::<ScalarKeyframe>(decoded_bytes, count, limits)?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(ScalarKeyframe {
            frame: cursor.u16()?,
            value: i32::from(cursor.i16()?),
        });
    }
    Ok(values)
}

fn constant_vector(
    cursor: &mut Cursor<'_>,
    keyframe_count: &mut usize,
    decoded_bytes: &mut usize,
    limits: LoaderLimits,
) -> Result<Vec<VectorKeyframe>, EmuError> {
    consume_keyframes(keyframe_count, 1, limits)?;
    consume_decoded_items::<VectorKeyframe>(decoded_bytes, 1, limits)?;
    Ok(vec![VectorKeyframe {
        frame: 0,
        value: Vector3D::new(
            i32::from(cursor.i16()?),
            i32::from(cursor.i16()?),
            i32::from(cursor.i16()?),
        ),
    }])
}

fn constant_scalar(
    cursor: &mut Cursor<'_>,
    keyframe_count: &mut usize,
    decoded_bytes: &mut usize,
    limits: LoaderLimits,
) -> Result<Vec<ScalarKeyframe>, EmuError> {
    consume_keyframes(keyframe_count, 1, limits)?;
    consume_decoded_items::<ScalarKeyframe>(decoded_bytes, 1, limits)?;
    Ok(vec![ScalarKeyframe {
        frame: 0,
        value: i32::from(cursor.i16()?),
    }])
}

fn consume_keyframes(
    keyframe_count: &mut usize,
    count: usize,
    limits: LoaderLimits,
) -> Result<(), EmuError> {
    *keyframe_count = keyframe_count
        .checked_add(count)
        .filter(|total| *total <= limits.keyframes)
        .ok_or_else(|| {
            loader_error(
                "action-keyframe-count",
                "ActionTable keyframe count exceeds its budget",
            )
        })?;
    Ok(())
}

fn consume_decoded_items<T>(
    decoded_bytes: &mut usize,
    count: usize,
    limits: LoaderLimits,
) -> Result<(), EmuError> {
    let bytes = checked_mul(count, size_of::<T>(), "ActionTable decoded item")?;
    *decoded_bytes = decoded_bytes
        .checked_add(bytes)
        .filter(|total| *total <= limits.decoded_bytes)
        .ok_or_else(|| {
            loader_error(
                "resource-budget",
                "ActionTable decoded data exceeds the native-memory budget",
            )
        })?;
    Ok(())
}

fn component_segment(
    translation: Vec<VectorKeyframe>,
    scale: Vec<VectorKeyframe>,
    rotation: Vec<VectorKeyframe>,
    roll: Vec<ScalarKeyframe>,
) -> ActionSegmentData {
    ActionSegmentData::Components {
        translation,
        scale,
        rotation,
        roll,
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/micro3d/loader/action.rs"]
mod tests;
