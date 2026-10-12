//! Typed Java arrays, storage accounting and element access.

use super::{Allocation, Handle, Heap, HeapError, HeapValue};
use std::sync::Arc;

/// JVM array component kind.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ArrayKind {
    Boolean,
    Byte,
    Char,
    Short,
    Int,
    Long,
    Float,
    Double,
    Reference(Arc<str>),
}

impl ArrayKind {
    pub(super) fn allocation_bytes(&self, length: usize) -> Result<usize, HeapError> {
        let width = match self {
            Self::Boolean | Self::Byte => 1,
            Self::Char | Self::Short => 2,
            Self::Int | Self::Float | Self::Reference(_) => 4,
            Self::Long | Self::Double => 8,
        };
        length
            .checked_mul(width)
            .and_then(|bytes| bytes.checked_add(24))
            .ok_or(HeapError::LimitExceeded)
    }
}

/// Operand family used by JVM array bytecodes.
///
/// This intentionally differs from [`ArrayKind`]: `baload`/`bastore` are
/// shared by `boolean[]` and `byte[]`. Passing the expected family into the
/// heap lets the common interpreter path validate the handle, component kind,
/// bounds and value with one allocation lookup instead of looking the same
/// handle up twice for every element access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArrayAccessKind {
    Int,
    Long,
    Float,
    Double,
    Reference,
    ByteOrBoolean,
    Char,
    Short,
}

impl ArrayAccessKind {
    #[inline]
    fn accepts(self, kind: &ArrayKind) -> bool {
        matches!(
            (self, kind),
            (Self::Int, ArrayKind::Int)
                | (Self::Long, ArrayKind::Long)
                | (Self::Float, ArrayKind::Float)
                | (Self::Double, ArrayKind::Double)
                | (Self::Reference, ArrayKind::Reference(_))
                | (Self::ByteOrBoolean, ArrayKind::Boolean | ArrayKind::Byte)
                | (Self::Char, ArrayKind::Char)
                | (Self::Short, ArrayKind::Short)
        )
    }
}

impl Heap {
    pub fn allocate_array(&mut self, kind: ArrayKind, length: i32) -> Result<Handle, HeapError> {
        let length = usize::try_from(length).map_err(|_| HeapError::NegativeArraySize)?;
        let bytes = kind.allocation_bytes(length)?;
        let default = match kind {
            ArrayKind::Boolean
            | ArrayKind::Byte
            | ArrayKind::Char
            | ArrayKind::Short
            | ArrayKind::Int => HeapValue::Int(0),
            ArrayKind::Long => HeapValue::Long(0),
            ArrayKind::Float => HeapValue::Float(0.0),
            ArrayKind::Double => HeapValue::Double(0.0),
            ArrayKind::Reference(_) => HeapValue::Reference(None),
        };
        self.allocate_with(bytes, |names| {
            let kind = match kind {
                ArrayKind::Reference(component) => ArrayKind::Reference(names.intern(&component)?),
                other => other,
            };
            Ok(Allocation::Array {
                kind,
                elements: vec![default; length],
            })
        })
    }

    pub fn array_length(&self, handle: Handle) -> Result<usize, HeapError> {
        match self.get(handle)? {
            Allocation::Array { elements, .. } => Ok(elements.len()),
            Allocation::Object { .. } => Err(HeapError::TypeMismatch),
        }
    }

    pub fn array_kind(&self, handle: Handle) -> Result<&ArrayKind, HeapError> {
        match self.get(handle)? {
            Allocation::Array { kind, .. } => Ok(kind),
            Allocation::Object { .. } => Err(HeapError::TypeMismatch),
        }
    }

    pub fn array_get(&self, handle: Handle, index: i32) -> Result<HeapValue, HeapError> {
        let index = usize::try_from(index).map_err(|_| HeapError::Bounds)?;
        match self.get(handle)? {
            Allocation::Array { elements, .. } => {
                elements.get(index).copied().ok_or(HeapError::Bounds)
            }
            Allocation::Object { .. } => Err(HeapError::TypeMismatch),
        }
    }

    /// Reads an array element while validating the JVM opcode family in the
    /// same checked allocation lookup.
    #[inline]
    pub fn array_get_typed(
        &self,
        handle: Handle,
        index: i32,
        access: ArrayAccessKind,
    ) -> Result<HeapValue, HeapError> {
        let index = usize::try_from(index).map_err(|_| HeapError::Bounds)?;
        match self.get(handle)? {
            Allocation::Array { kind, elements } if access.accepts(kind) => {
                elements.get(index).copied().ok_or(HeapError::Bounds)
            }
            Allocation::Array { .. } | Allocation::Object { .. } => Err(HeapError::TypeMismatch),
        }
    }

    pub fn array_set(
        &mut self,
        handle: Handle,
        index: i32,
        value: HeapValue,
    ) -> Result<(), HeapError> {
        let index = usize::try_from(index).map_err(|_| HeapError::Bounds)?;
        match self.get_mut(handle)? {
            Allocation::Array { kind, elements } => {
                let value = normalize(kind, value).ok_or(HeapError::TypeMismatch)?;
                *elements.get_mut(index).ok_or(HeapError::Bounds)? = value;
                Ok(())
            }
            Allocation::Object { .. } => Err(HeapError::TypeMismatch),
        }
    }

    /// Stores an array element while validating the JVM opcode family in the
    /// same checked mutable allocation lookup.
    #[inline]
    pub fn array_set_typed(
        &mut self,
        handle: Handle,
        index: i32,
        access: ArrayAccessKind,
        value: HeapValue,
    ) -> Result<(), HeapError> {
        let index = usize::try_from(index).map_err(|_| HeapError::Bounds)?;
        match self.get_mut(handle)? {
            Allocation::Array { kind, elements } if access.accepts(kind) => {
                let value = normalize(kind, value).ok_or(HeapError::TypeMismatch)?;
                *elements.get_mut(index).ok_or(HeapError::Bounds)? = value;
                Ok(())
            }
            Allocation::Array { .. } | Allocation::Object { .. } => Err(HeapError::TypeMismatch),
        }
    }

    /// Fills a checked primitive-array range with one JVM value.
    ///
    /// The range is validated before mutation, so callers can decline a
    /// speculative interpreter fast path without committing a partial write.
    #[inline]
    pub fn array_fill_typed(
        &mut self,
        handle: Handle,
        start: i32,
        length: usize,
        access: ArrayAccessKind,
        value: HeapValue,
    ) -> Result<(), HeapError> {
        let start = usize::try_from(start).map_err(|_| HeapError::Bounds)?;
        match self.get_mut(handle)? {
            Allocation::Array { kind, elements } if access.accepts(kind) => {
                let value = normalize(kind, value).ok_or(HeapError::TypeMismatch)?;
                let end = start.checked_add(length).ok_or(HeapError::Bounds)?;
                elements
                    .get_mut(start..end)
                    .ok_or(HeapError::Bounds)?
                    .fill(value);
                Ok(())
            }
            Allocation::Array { .. } | Allocation::Object { .. } => Err(HeapError::TypeMismatch),
        }
    }
}

pub(super) fn normalize(kind: &ArrayKind, value: HeapValue) -> Option<HeapValue> {
    match (kind, value) {
        (ArrayKind::Boolean, HeapValue::Int(value)) => Some(HeapValue::Int(value & 1)),
        (ArrayKind::Byte, HeapValue::Int(value)) => Some(HeapValue::Int(i32::from(value as i8))),
        (ArrayKind::Char, HeapValue::Int(value)) => Some(HeapValue::Int(value & 0xffff)),
        (ArrayKind::Short, HeapValue::Int(value)) => Some(HeapValue::Int(i32::from(value as i16))),
        (ArrayKind::Int, value @ HeapValue::Int(_))
        | (ArrayKind::Long, value @ HeapValue::Long(_))
        | (ArrayKind::Float, value @ HeapValue::Float(_))
        | (ArrayKind::Double, value @ HeapValue::Double(_))
        | (ArrayKind::Reference(_), value @ HeapValue::Reference(_)) => Some(value),
        _ => None,
    }
}
