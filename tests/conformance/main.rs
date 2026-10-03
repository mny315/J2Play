//! Project-owned end-to-end checks, grouped by the contract they exercise.
//! Run a subsystem with `cargo test -p j2play-conformance --test conformance cldc::`.

mod bluetooth_uuid;
mod execution_diagnostics;
mod support;
mod cldc {
    mod console;
    mod data_output;
    mod exceptions;
    mod io;
    mod methods;
    mod print_stream;
    mod string_buffer;
    mod threads;
    mod throwable;
}
mod gcf {
    mod connections;
    mod file_listing;
    mod file_streams;
}
mod lcdui {
    mod canvas_modes;
    mod game_api;
    mod graphics;
    mod profile_keys;
    mod screen_heap;
    mod screens;
}
mod midlet;
mod mmapi {
    mod nokia_sound;
    mod playback;
    mod tone;
    mod volume_events;
}
mod rms;
mod vendor_compatibility;
