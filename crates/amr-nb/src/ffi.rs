//! `OpenCORE` state ownership and calls after storage-frame validation.
#![allow(unsafe_code)]

use super::DecodeError;
use std::ffi::c_void;
use std::ptr::NonNull;

#[link(name = "opencore-amrnb")]
unsafe extern "C" {
    fn Decoder_Interface_init() -> *mut c_void;
    fn Decoder_Interface_exit(state: *mut c_void);
    fn Decoder_Interface_Decode(
        state: *mut c_void,
        input: *const u8,
        output: *mut i16,
        bad_frame: i32,
    );
}

pub(super) struct Decoder(NonNull<c_void>);

impl Decoder {
    pub(super) fn new() -> Result<Self, DecodeError> {
        // SAFETY: initialization takes no arguments and returns either an
        // owned opaque state pointer or null on allocation failure.
        NonNull::new(unsafe { Decoder_Interface_init() })
            .map(Self)
            .ok_or(DecodeError::DecoderInitialization)
    }

    pub(super) fn decode(&mut self, input: &[u8], output: &mut [i16; 160], bad_frame: bool) {
        // SAFETY: validated_sample_count guarantees a complete RFC 4867 frame.
        // OpenCORE writes exactly 160 i16 samples for every AMR-NB frame;
        // state is live and exclusively borrowed for the duration.
        unsafe {
            Decoder_Interface_Decode(
                self.0.as_ptr(),
                input.as_ptr(),
                output.as_mut_ptr(),
                i32::from(bad_frame),
            );
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: the pointer came from Decoder_Interface_init, remains
        // owned by this value, and Drop runs exactly once.
        unsafe { Decoder_Interface_exit(self.0.as_ptr()) };
    }
}
