//! The type system of Manim Core (docs/spec/manim-core.md §5): kinds, presence typestate,
//! animation effects and static timing.

mod check;
mod diag;
mod env;
pub mod timing;

pub use check::{Checked, Note, ObjectState, SlideReport, check};
pub use diag::{Diagnostic, Severity};
pub use env::State;
