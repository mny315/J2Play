//! Bounded, host-independent stack interpreter for static Java programs.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::manual_let_else,
    clippy::similar_names,
    clippy::semicolon_if_nothing_returned,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::unnested_or_patterns
)]

pub mod acceleration;
mod machine;

pub use machine::*;
