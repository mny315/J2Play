use super::{EmuError, Handle, Runtime, clone_slice_preserving_capacity, graph_error};

impl Runtime {
    /// Sets the JSR user identifier.
    pub fn set_user_id(&mut self, handle: Handle, user_id: i32) -> Result<(), EmuError> {
        self.objects.get_mut(handle)?.base.user_id = user_id;
        Ok(())
    }

    /// Returns the JSR user identifier.
    pub fn user_id(&self, handle: Handle) -> Result<i32, EmuError> {
        Ok(self.objects.get(handle)?.base.user_id)
    }

    /// Stores the guest-side user object reference without treating it as scene ownership.
    pub fn set_user_object(
        &mut self,
        handle: Handle,
        user_object: Option<u64>,
    ) -> Result<(), EmuError> {
        self.objects.get_mut(handle)?.base.user_object = user_object;
        Ok(())
    }

    /// Returns the guest-side user object reference.
    pub fn user_object(&self, handle: Handle) -> Result<Option<u64>, EmuError> {
        Ok(self.objects.get(handle)?.base.user_object)
    }

    /// Ordered animation tracks directly attached to an object.
    pub fn animation_tracks(&self, handle: Handle) -> Result<&[Handle], EmuError> {
        Ok(&self.objects.get(handle)?.base.animation_tracks)
    }

    /// Adds an animation track once, preserving insertion order.
    pub fn add_animation_track(&mut self, object: Handle, track: Handle) -> Result<(), EmuError> {
        let (property, components) = self.animation_track_shape(self.kind(track)?)?;
        self.kind(object)?.validate_animation_target(property)?;
        let has_spare_capacity = {
            let tracks = &self.objects.get(object)?.base.animation_tracks;
            for previous in tracks {
                if *previous == track {
                    return Err(graph_error(
                        "duplicate-reference",
                        "animation track is already attached",
                    ));
                }
                let (previous_property, previous_components) =
                    self.animation_track_shape(self.kind(*previous)?)?;
                if property == previous_property && components != previous_components {
                    return Err(graph_error(
                        "invalid-animation-components",
                        "tracks for one property have different keyframe sizes",
                    ));
                }
            }
            tracks.len() < tracks.capacity()
        };
        if has_spare_capacity {
            self.objects
                .get_mut(object)?
                .base
                .animation_tracks
                .push(track);
            return Ok(());
        }
        let (mut tracks, previous_allocation) = {
            let tracks = &self.objects.get(object)?.base.animation_tracks;
            (
                clone_slice_preserving_capacity(tracks, tracks.capacity()),
                tracks
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Handle>()),
            )
        };
        tracks.push(track);
        let next_allocation = tracks
            .capacity()
            .saturating_mul(std::mem::size_of::<Handle>());
        self.reaccount_capacity_change(object, previous_allocation, next_allocation)?;
        self.objects
            .get_mut(object)
            .expect("reaccounted M3G object remains live")
            .base
            .animation_tracks = tracks;
        Ok(())
    }

    /// Removes an animation track, returning whether it was present.
    pub fn remove_animation_track(
        &mut self,
        object: Handle,
        track: Handle,
    ) -> Result<bool, EmuError> {
        let tracks = &mut self.objects.get_mut(object)?.base.animation_tracks;
        let Some(position) = tracks.iter().position(|candidate| *candidate == track) else {
            return Ok(false);
        };
        tracks.remove(position);
        Ok(true)
    }
}
