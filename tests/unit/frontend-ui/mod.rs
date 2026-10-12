#![allow(clippy::wildcard_imports)]

use super::*;
use crate::controls::*;
use crate::layout::{frame_scale_for_fit, keypad_slot_dimensions};

#[path = "../../support/storage.rs"]
pub(crate) mod test_storage;

mod app_settings;
mod controls;
mod desktop;
mod dialogs;
mod document_browser;
mod fullscreen;
mod gameplay;
mod import_errors;
mod import_flow;
mod layout;
mod library;
mod navigation;
mod pointer_input;
mod recovery;
mod resume;
mod runtime_events;
mod scrolling;
mod settings;
mod theme;
