//! Experimental numeric AOT backend. Guest heap pointers and host calls never
//! enter generated code; the only unsafe boundary is in `executable`.

mod cache;
mod compiler;
mod executable;
mod verify;

pub use compiler::{Compiler, PreparationStats, RunStats};

#[cfg(test)]
#[path = "../../../tests/unit/native-code/mod.rs"]
mod tests;
