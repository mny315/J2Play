use super::*;
use crate::{Bone, Face};

fn translating_action() -> Vec<u8> {
    let mut bytes = b"MT\x05\0\x01\0\x01\0".to_vec();
    bytes.extend_from_slice(&[0; 20]);
    bytes.extend_from_slice(&10_u16.to_le_bytes());
    bytes.push(2);
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    for values in [[0_i16, 0, 0, 0], [10, 10, 0, 0]] {
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    for value in [0_i16, 4096, 4096, 4096] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    for value in [0_i16, 0, 0, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0_i16.to_le_bytes());
    bytes.extend_from_slice(&0_i16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(b"J2PLAY-MICRO3D-TEST!");
    bytes
}

mod arena;
mod checkpoint;
mod figure_mesh;
mod figures;
mod lifecycle;
mod lighting;
mod materials;
mod primitive_batches;
mod primitives;
mod projection;
mod static_figures;
