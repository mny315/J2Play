//! Symbolic class state is independent of per-process linking/cache order.

use super::{ClassState, HashMap, HashSet, Program, Value};
use crate::machine::{EmuError, Handle, parse_descriptor_type, vm_error};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ClassCheckpoint {
    static_fields: HashMap<String, Value>,
    initialized: HashSet<String>,
    initializing: HashMap<String, u64>,
    failed_initialization: HashMap<String, String>,
    initialization_waiters: HashMap<String, Vec<Handle>>,
    objects: HashMap<String, Handle>,
}

impl ClassState {
    pub(in crate::machine) fn encode_checkpoint(
        &self,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EmuError> {
        let mut static_fields = self.static_fields.synthetic.clone();
        for (key, slot) in &self.static_fields.slots_by_key {
            if let Some(value) = self.static_fields.linked[*slot] {
                static_fields.insert(key.to_string(), value);
            }
        }
        let mut initialized = self.initialized.synthetic.clone();
        initialized.extend(
            self.initialized
                .slots_by_name
                .iter()
                .filter(|(_, slot)| self.initialized.linked[**slot])
                .map(|(name, _)| name.clone()),
        );
        save_state::encode_cancellable(
            &ClassCheckpoint {
                static_fields,
                initialized,
                initializing: self.initializing.clone(),
                failed_initialization: self.failed_initialization.clone(),
                initialization_waiters: self.initialization_waiters.clone(),
                objects: self.objects.clone(),
            },
            save_state::MAX_COMPONENT_BYTES,
            cancelled,
        )
    }

    pub(in crate::machine) fn restore_checkpoint(
        program: &Program,
        bytes: &[u8],
    ) -> Result<Self, EmuError> {
        let saved: ClassCheckpoint = save_state::decode(bytes)?;
        let mut state = Self::with_program(program);
        if saved
            .initialized
            .iter()
            .chain(saved.initializing.keys())
            .chain(saved.failed_initialization.keys())
            .chain(saved.initialization_waiters.keys())
            .any(|name| !program.classes.contains_key(name))
            || saved
                .objects
                .keys()
                .any(|name| !class_object_matches_program(program, name))
        {
            return Err(vm_error(
                "checkpoint-classes",
                "Checkpoint classes do not match the game.",
            ));
        }
        let fields: HashMap<_, _> = program
            .classes
            .values()
            .flat_map(|class| &class.fields)
            .filter(|field| field.is_static)
            .map(|field| (field.key.as_ref(), field))
            .collect();
        for (key, value) in saved.static_fields {
            let field = fields.get(key.as_str()).ok_or_else(|| {
                vm_error(
                    "checkpoint-fields",
                    "Checkpoint fields do not match the game.",
                )
            })?;
            if field.kind != value.kind() {
                return Err(vm_error(
                    "checkpoint-fields",
                    "Checkpoint field types do not match the game.",
                ));
            }
            state.static_fields.insert(key, value);
        }
        for name in saved.initialized {
            state.initialized.insert(name);
        }
        state.initializing = saved.initializing;
        state.failed_initialization = saved.failed_initialization;
        state.initialization_waiters = saved.initialization_waiters;
        state.objects = saved.objects;
        Ok(state)
    }
}

fn class_object_matches_program(program: &Program, name: &str) -> bool {
    if program.classes.contains_key(name) {
        return true;
    }
    // Object.getClass() interns array mirrors, including nested arrays. These
    // have descriptors rather than ordinary class definitions in Program.
    let mut end = 0;
    if !name.starts_with('[')
        || parse_descriptor_type(name.as_bytes(), &mut end).is_err()
        || end != name.len()
    {
        return false;
    }
    name.trim_start_matches('[')
        .strip_prefix('L')
        .and_then(|component| component.strip_suffix(';'))
        .is_none_or(|component| program.classes.contains_key(component))
}

#[cfg(test)]
#[path = "../../../../../tests/unit/vm/machine/class_state/checkpoint.rs"]
mod tests;
