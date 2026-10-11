//! Manim Core AST. Mirrors docs/spec/manim-core.md §3–§4; no behaviour lives here.

mod ast;
mod build;
mod color;
mod ids;
mod sym;
mod time;
mod walk;

pub use ast::*;
pub use build::{SlideBuilder, UNBOUND};
pub use color::Rgba;
pub use ids::{NodeId, SlideId, VarId};
pub use sym::Sym;
pub use time::{FLICKS_PER_SECOND, Time};
