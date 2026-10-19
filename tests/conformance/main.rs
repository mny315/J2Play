//! Project-owned end-to-end checks, grouped by the contract they exercise.
//! Run a subsystem with `cargo test -p j2play-conformance --test conformance cldc::`.

mod bluetooth_uuid;
mod descriptor_defaults;
mod execution_diagnostics;
mod support;
mod throughput;
mod cldc {
    mod console;
    mod data_output;
    mod exceptions;
    mod io;
    mod locale;
    mod methods;
    mod print_stream;
    mod reflection;
    mod string_buffer;
    mod threads;
    mod throwable;
    mod timers;
}
mod gcf {
    mod connections;
    mod file_listing;
    mod file_streams;
}
mod lcdui {
    mod canvas_modes;
    mod display_dispatch;
    mod game_api;
    mod graphics;
    mod paint_transition;
    mod profile_keys;
    mod screen_heap;
    mod screens;
    mod text;
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
