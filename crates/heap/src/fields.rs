//! Ordered object fields and checked Java field access.

use super::{Allocation, Handle, Heap, HeapError, HeapValue};
use std::collections::HashMap;
use std::sync::Arc;

/// Opaque identity shared by one linked Java field and its object-layout slot.
/// Pointer identity makes the common checked slot access constant-time while
/// exact field names remain available for defensive fallback paths.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FieldToken(#[serde(skip)] Arc<()>);

impl FieldToken {
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(()))
    }

    fn matches(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for FieldToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Ordered instance-field storage.
///
/// Java field layouts are fixed for the lifetime of a class. Keeping values in
/// layout order lets the interpreter use a resolved numeric slot instead of
/// hashing a fully-qualified field name for every `getfield`/`putfield` while
/// retaining name lookup for native adapters and defensive fallback paths.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ObjectFields {
    pub(super) entries: Vec<(Arc<str>, FieldToken, HeapValue)>,
}

impl ObjectFields {
    pub(super) fn unordered(fields: HashMap<String, HeapValue>) -> Self {
        let mut entries = fields
            .into_iter()
            .map(|(key, value)| (Arc::<str>::from(key), FieldToken::new(), value))
            .collect::<Vec<_>>();
        entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        Self { entries }
    }

    pub(super) fn linked(entries: Vec<(Arc<str>, FieldToken, HeapValue)>) -> Self {
        Self { entries }
    }

    #[must_use]
    pub fn get(&self, field: &str) -> Option<&HeapValue> {
        self.entries
            .iter()
            .find_map(|(key, _, value)| (key.as_ref() == field).then_some(value))
    }

    pub fn get_mut(&mut self, field: &str) -> Option<&mut HeapValue> {
        self.entries
            .iter_mut()
            .find_map(|(key, _, value)| (key.as_ref() == field).then_some(value))
    }

    fn get_at(&self, slot: usize, token: &FieldToken, field: &str) -> Option<&HeapValue> {
        self.entries
            .get(slot)
            .and_then(|(_, candidate, value)| candidate.matches(token).then_some(value))
            .or_else(|| self.get(field))
    }

    fn get_at_mut(
        &mut self,
        slot: usize,
        token: &FieldToken,
        field: &str,
    ) -> Option<&mut HeapValue> {
        if self
            .entries
            .get(slot)
            .is_some_and(|(_, candidate, _)| candidate.matches(token))
        {
            return self.entries.get_mut(slot).map(|(_, _, value)| value);
        }
        self.get_mut(field)
    }

    pub fn values(&self) -> impl Iterator<Item = &HeapValue> {
        self.entries.iter().map(|(_, _, value)| value)
    }
}

impl Heap {
    pub fn allocate_object(
        &mut self,
        class: impl AsRef<str>,
        fields: HashMap<String, HeapValue>,
    ) -> Result<Handle, HeapError> {
        let mut fields = ObjectFields::unordered(fields);
        let bytes = object_bytes(fields.values())?;
        self.allocate_with(bytes, |names| {
            let class = names.intern(class.as_ref())?;
            for (name, _, _) in &mut fields.entries {
                *name = names.intern(name)?;
            }
            Ok(Allocation::Object { class, fields })
        })
    }

    /// Allocates an object using VM-linked field identities for checked slot
    /// access. Exact-name APIs remain available for native adapters.
    /// Field names share the linked program's bounded metadata allocation.
    /// Fields are taken only on success; a rejected allocation leaves them
    /// available to root their references during collection and retry.
    pub fn allocate_object_linked(
        &mut self,
        class: impl AsRef<str>,
        fields: &mut Vec<(Arc<str>, FieldToken, HeapValue)>,
    ) -> Result<Handle, HeapError> {
        let bytes = object_bytes(fields.iter().map(|(_, _, value)| value))?;
        self.allocate_with(bytes, |names| {
            let class = names.intern(class.as_ref())?;
            Ok(Allocation::Object {
                class,
                fields: ObjectFields::linked(std::mem::take(fields)),
            })
        })
    }

    pub fn field(&self, handle: Handle, field: &str) -> Result<HeapValue, HeapError> {
        match self.get(handle)? {
            Allocation::Object { fields, .. } => {
                fields.get(field).copied().ok_or(HeapError::TypeMismatch)
            }
            Allocation::Array { .. } => Err(HeapError::TypeMismatch),
        }
    }

    /// Reads a field through a linked layout slot, falling back to its exact
    /// symbolic key if a defensive or synthetic allocation uses another
    /// layout. The fallback preserves the old checked behavior for malformed
    /// guest receiver types.
    pub fn field_at(
        &self,
        handle: Handle,
        slot: usize,
        token: &FieldToken,
        field: &str,
    ) -> Result<HeapValue, HeapError> {
        match self.get(handle)? {
            Allocation::Object { fields, .. } => fields
                .get_at(slot, token, field)
                .copied()
                .ok_or(HeapError::TypeMismatch),
            Allocation::Array { .. } => Err(HeapError::TypeMismatch),
        }
    }

    pub fn set_field(
        &mut self,
        handle: Handle,
        field: &str,
        value: HeapValue,
    ) -> Result<(), HeapError> {
        match self.get_mut(handle)? {
            Allocation::Object { fields, .. } => {
                // Do not allocate a fresh String for every putfield. Object
                // field names are fixed at allocation time, so updating the
                // existing slot is both sufficient and dramatically cheaper
                // for Java-heavy game loops.
                let slot = fields.get_mut(field).ok_or(HeapError::TypeMismatch)?;
                *slot = value;
                Ok(())
            }
            Allocation::Array { .. } => Err(HeapError::TypeMismatch),
        }
    }

    /// Updates a field through a linked layout slot with the same exact-key
    /// fallback as [`Self::field_at`].
    pub fn set_field_at(
        &mut self,
        handle: Handle,
        slot: usize,
        token: &FieldToken,
        field: &str,
        value: HeapValue,
    ) -> Result<(), HeapError> {
        match self.get_mut(handle)? {
            Allocation::Object { fields, .. } => {
                let target = fields
                    .get_at_mut(slot, token, field)
                    .ok_or(HeapError::TypeMismatch)?;
                *target = value;
                Ok(())
            }
            Allocation::Array { .. } => Err(HeapError::TypeMismatch),
        }
    }
}

pub(super) fn object_bytes<'a>(
    mut values: impl Iterator<Item = &'a HeapValue>,
) -> Result<usize, HeapError> {
    // The selected device limits describe a 32-bit CLDC heap, not the host
    // size of HeapValue: two header words plus the target field widths.
    values.try_fold(8_usize, |bytes, value| {
        let width = match value {
            HeapValue::Long(_) | HeapValue::Double(_) => 8,
            HeapValue::Int(_) | HeapValue::Float(_) | HeapValue::Reference(_) => 4,
        };
        bytes.checked_add(width).ok_or(HeapError::LimitExceeded)
    })
}
