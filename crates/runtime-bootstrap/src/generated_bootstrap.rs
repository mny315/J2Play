//! Canonical Rust-owned class metadata and bytecode. `SourceFile` constants
//! identify this module; no external Java sources or toolchain are required.
#![allow(clippy::manual_string_new, clippy::unreadable_literal)]

use classfile::{Attribute, ClassFile, CodeAttribute, Constant, ExceptionHandler, Member};

mod choice_flags;
mod cldc_io;
mod cldc_lang;
mod cldc_util;
mod game;
mod gcf;
mod lcdui_core;
mod lcdui_graphics;
mod midlet;
mod mmapi;
mod rms;

use cldc_io::append_generated_cldc_io;
use cldc_lang::append_generated_cldc_lang;
use cldc_util::append_generated_cldc_util;
use game::append_generated_game;
use gcf::append_generated_gcf;
use lcdui_core::append_generated_lcdui_core;
use lcdui_graphics::append_generated_lcdui_graphics;
use midlet::append_generated_midlet;
use mmapi::append_generated_mmapi;
use rms::append_generated_rms;

// Each constructor uses its own stack frame, avoiding stack space for the
// entire bootstrap inventory in a single aggregate expression.
#[inline(never)]
fn build_bootstrap_class(constructor: fn() -> ClassFile) -> ClassFile {
    constructor()
}

macro_rules! append_bootstrap_classes {
    ($classes:expr; $($class:expr),* $(,)?) => {
        $($classes.push(build_bootstrap_class(|| $class));)*
    };
}

pub(super) use append_bootstrap_classes;

const GENERATED_BOOTSTRAP_CLASS_COUNT: usize = 116;

pub(crate) fn generated_bootstrap_classes() -> Vec<ClassFile> {
    let mut classes = Vec::with_capacity(GENERATED_BOOTSTRAP_CLASS_COUNT);
    append_generated_cldc_io(&mut classes);
    append_generated_cldc_lang(&mut classes);
    append_generated_cldc_util(&mut classes);
    append_generated_gcf(&mut classes);
    append_generated_lcdui_core(&mut classes);
    append_generated_lcdui_graphics(&mut classes);
    append_generated_game(&mut classes);
    append_generated_mmapi(&mut classes);
    append_generated_midlet(&mut classes);
    append_generated_rms(&mut classes);
    debug_assert_eq!(classes.len(), GENERATED_BOOTSTRAP_CLASS_COUNT);
    classes
}
