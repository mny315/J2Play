//! Shared animation-track contracts for construction, binding and checkpoints.

use super::{BTreeSet, EmuError, Object, ObjectKind, Runtime, graph_error};

impl Runtime {
    /// Checks the target type and keyframe size before applying an animation value.
    pub fn validate_animation_property(
        &self,
        object: super::Handle,
        property: i32,
        components: usize,
    ) -> Result<(), EmuError> {
        validate_components(property, components)?;
        self.kind(object)?.validate_animation_target(property)
    }

    pub(super) fn animation_track_shape(
        &self,
        kind: &ObjectKind,
    ) -> Result<(i32, usize), EmuError> {
        let ObjectKind::AnimationTrack {
            sequence,
            controller,
            property,
        } = kind
        else {
            return Err(graph_error(
                "invalid-animation-track",
                "object is not an animation track",
            ));
        };
        let ObjectKind::KeyframeSequence(sequence) = self.kind(*sequence)? else {
            return Err(graph_error(
                "invalid-animation-track",
                "animation track requires a keyframe sequence",
            ));
        };
        if let Some(controller) = controller
            && !matches!(self.kind(*controller)?, ObjectKind::AnimationController(_))
        {
            return Err(graph_error(
                "invalid-animation-track",
                "animation controller has the wrong type",
            ));
        }
        let components = sequence.component_count();
        validate_components(*property, components)?;
        Ok((*property, components))
    }

    pub(super) fn validate_animation_tracks(&self, object: &Object) -> Result<(), EmuError> {
        let mut tracks = BTreeSet::new();
        let mut components = [None; 21];
        for track in &object.base.animation_tracks {
            if !tracks.insert(track) {
                return Err(graph_error(
                    "duplicate-reference",
                    "animation track is already attached",
                ));
            }
            let (property, count) = self.animation_track_shape(self.kind(*track)?)?;
            object.kind.validate_animation_target(property)?;
            let previous = &mut components[(property - 256) as usize];
            if previous.is_some_and(|previous| previous != count) {
                return Err(graph_error(
                    "invalid-animation-components",
                    "tracks for one property have different keyframe sizes",
                ));
            }
            *previous = Some(count);
        }
        Ok(())
    }
}

impl ObjectKind {
    pub(super) fn validate_animation_target(&self, property: i32) -> Result<(), EmuError> {
        let compatible = match property {
            256 => {
                self.node().is_some()
                    || matches!(
                        self,
                        Self::Background(_) | Self::VertexBuffer { .. } | Self::Material(_)
                    )
            }
            257 | 261 | 262 | 271 | 272 => matches!(self, Self::Material(_)),
            258 => matches!(
                self,
                Self::Background(_)
                    | Self::Fog(_)
                    | Self::Light { .. }
                    | Self::Texture2D(_)
                    | Self::VertexBuffer { .. }
            ),
            259 => matches!(self, Self::Background(_) | Self::Sprite3D(_)),
            260 => matches!(self, Self::Fog(_)),
            263 | 267 => matches!(self, Self::Camera { .. } | Self::Fog(_)),
            264 => matches!(self, Self::Camera { .. }),
            265 | 273 | 274 => matches!(self, Self::Light { .. }),
            266 => matches!(self, Self::MorphingMesh { .. }),
            268 | 270 | 275 => self.transformable().is_some(),
            269 | 276 => self.node().is_some(),
            _ => false,
        };
        if compatible {
            Ok(())
        } else {
            Err(graph_error(
                "invalid-animation-target",
                "animation property is incompatible with this object",
            ))
        }
    }
}

fn validate_components(property: i32, components: usize) -> Result<(), EmuError> {
    let valid = match property {
        257 | 258 | 261 | 262 | 272 | 275 => components == 3,
        259 => matches!(components, 2 | 4),
        266 => (1..=16).contains(&components),
        268 => components == 4,
        270 => matches!(components, 1 | 3),
        256 | 260 | 263 | 264 | 265 | 267 | 269 | 271 | 273 | 274 | 276 => components == 1,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(graph_error(
            "invalid-animation-components",
            "keyframe size is incompatible with the animation property",
        ))
    }
}
