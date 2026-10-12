use super::{BTreeSet, Handle, ObjectKind, Runtime};

impl Runtime {
    /// Removes all native resources and guest bindings at suite termination.
    pub fn teardown(&mut self) {
        self.guest_handles.clear();
        self.objects.clear();
    }

    /// Rolls back a failed atomic loader transaction in reverse allocation order.
    pub fn rollback_created(&mut self, handles: &[Handle]) {
        let created = handles
            .iter()
            .copied()
            .filter(|handle| self.objects.get(*handle).is_ok())
            .collect::<BTreeSet<_>>();
        let mut external_children = Vec::new();
        for parent in &created {
            let Ok(object) = self.objects.get(*parent) else {
                continue;
            };
            if let Some(children) = object.kind.children() {
                external_children.extend(
                    children
                        .iter()
                        .copied()
                        .filter(|child| !created.contains(child))
                        .map(|child| (*parent, child)),
                );
            }
            if let ObjectKind::SkinnedMesh { skeleton, .. } = &object.kind
                && !created.contains(skeleton)
            {
                external_children.push((*parent, *skeleton));
            }
        }
        for (parent, child) in external_children {
            let Ok(object) = self.objects.get_mut(child) else {
                continue;
            };
            if let Some(node) = object.kind.node_mut()
                && node.parent == Some(parent)
            {
                node.parent = None;
            }
        }

        for handle in handles.iter().rev() {
            if let Ok(object) = self.objects.get(*handle)
                && let Some(reference) = object.guest_reference
            {
                self.guest_handles.remove(&reference);
            }
            let _ = self.objects.remove(*handle);
        }
    }

    /// Current and peak object/byte counters.
    #[must_use]
    pub const fn counters(&self) -> (usize, usize, usize, usize) {
        (
            self.objects.live_objects(),
            self.objects.peak_objects(),
            self.objects.live_bytes(),
            self.objects.peak_bytes(),
        )
    }
}
