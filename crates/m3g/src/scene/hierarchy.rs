use super::{
    BTreeSet, EmuError, Handle, ObjectKind, Runtime, clone_slice_preserving_capacity, graph_error,
};

impl Runtime {
    /// Adds a child after atomically validating ownership and cycle constraints.
    pub fn add_child(&mut self, group: Handle, child: Handle) -> Result<(), EmuError> {
        if group == child {
            return Err(graph_error("graph-cycle", "a node cannot contain itself"));
        }
        let children = self
            .objects
            .get(group)?
            .kind
            .children()
            .ok_or_else(|| graph_error("not-group", "parent is not a Group"))?;
        if children.contains(&child) {
            // The maintenance release permits re-adding a node whose parent
            // is already this Group; keep one edge and succeed idempotently.
            return Ok(());
        }
        let child_node = self
            .objects
            .get(child)?
            .kind
            .node()
            .ok_or_else(|| graph_error("not-node", "child is not a Node"))?;
        if child_node.parent.is_some() {
            return Err(graph_error(
                "already-parented",
                "child already has a parent",
            ));
        }
        self.validate_parent_link(group, child)?;
        let has_spare_capacity = match &self.objects.get(group)?.kind {
            ObjectKind::Group { children, .. } | ObjectKind::World { children, .. } => {
                children.len() < children.capacity()
            }
            _ => unreachable!("validated group changed kind"),
        };
        if has_spare_capacity {
            self.objects
                .get_mut(group)?
                .kind
                .children_mut()
                .expect("validated group")
                .push(child);
            self.objects
                .get_mut(child)?
                .kind
                .node_mut()
                .expect("validated child node")
                .parent = Some(group);
            return Ok(());
        }
        let (mut next_children, previous_allocation) = {
            let (ObjectKind::Group { children, .. } | ObjectKind::World { children, .. }) =
                &self.objects.get(group)?.kind
            else {
                unreachable!("validated group changed kind");
            };
            (
                clone_slice_preserving_capacity(children, children.capacity()),
                children
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Handle>()),
            )
        };
        next_children.push(child);
        let next_allocation = next_children
            .capacity()
            .saturating_mul(std::mem::size_of::<Handle>());
        self.reaccount_capacity_change(group, previous_allocation, next_allocation)?;
        *self
            .objects
            .get_mut(group)
            .expect("reaccounted M3G group remains live")
            .kind
            .children_mut()
            .expect("validated group") = next_children;
        self.objects
            .get_mut(child)?
            .kind
            .node_mut()
            .expect("validated child node")
            .parent = Some(group);
        Ok(())
    }

    /// Checks the complete resulting depth, including an existing subtree and
    /// the implicit skeleton edge of each `SkinnedMesh`, before changing either side.
    pub(super) fn validate_parent_link(
        &self,
        parent: Handle,
        child: Handle,
    ) -> Result<(), EmuError> {
        let mut ancestor = Some(parent);
        let mut parent_depth = 0_usize;
        while let Some(handle) = ancestor {
            if handle == child {
                return Err(graph_error("graph-cycle", "operation would create a cycle"));
            }
            parent_depth = parent_depth.saturating_add(1);
            if parent_depth >= self.max_graph_depth {
                return Err(graph_error(
                    "graph-depth",
                    "operation would exceed the scene-graph depth budget",
                ));
            }
            if parent_depth > self.objects.live_objects() {
                return Err(graph_error(
                    "graph-cycle",
                    "existing parent chain is cyclic",
                ));
            }
            ancestor = self
                .objects
                .get(handle)?
                .kind
                .node()
                .and_then(|node| node.parent);
        }

        let mut visited = BTreeSet::new();
        let mut pending = vec![(child, parent_depth + 1)];
        while let Some((handle, depth)) = pending.pop() {
            if handle == parent || !visited.insert(handle) {
                return Err(graph_error(
                    "graph-cycle",
                    "child subtree contains a repeated node",
                ));
            }
            if depth > self.max_graph_depth {
                return Err(graph_error(
                    "graph-depth",
                    "operation would exceed the scene-graph depth budget",
                ));
            }
            let kind = &self.objects.get(handle)?.kind;
            let next_depth = depth.saturating_add(1);
            pending.extend(
                kind.children()
                    .unwrap_or_default()
                    .iter()
                    .map(|child| (*child, next_depth)),
            );
            if let ObjectKind::SkinnedMesh { skeleton, .. } = kind {
                pending.push((*skeleton, next_depth));
            }
        }
        Ok(())
    }

    /// Removes a direct child and clears its parent link.
    pub fn remove_child(&mut self, group: Handle, child: Handle) -> Result<bool, EmuError> {
        let children = self
            .objects
            .get_mut(group)?
            .kind
            .children_mut()
            .ok_or_else(|| graph_error("not-group", "parent is not a Group"))?;
        let Some(position) = children.iter().position(|candidate| *candidate == child) else {
            return Ok(false);
        };
        children.remove(position);
        self.objects
            .get_mut(child)?
            .kind
            .node_mut()
            .ok_or_else(|| graph_error("not-node", "child is not a Node"))?
            .parent = None;
        Ok(true)
    }

    /// Returns a node's parent.
    pub fn parent(&self, node: Handle) -> Result<Option<Handle>, EmuError> {
        self.objects
            .get(node)?
            .kind
            .node()
            .map(|state| state.parent)
            .ok_or_else(|| graph_error("not-node", "object is not a Node"))
    }

    /// Returns a group's direct children in deterministic order.
    pub fn children(&self, group: Handle) -> Result<&[Handle], EmuError> {
        self.objects
            .get(group)?
            .kind
            .children()
            .ok_or_else(|| graph_error("not-group", "object is not a Group"))
    }
}
