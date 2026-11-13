//! # prism-drawingml
//!
//! Shared DrawingML engine used by the Word, Sheet and Slide engines:
//! preset/custom geometry, colours and themes, fills, lines, effects,
//! shape trees and text bodies, with lossless XML read/write.

pub mod color;
pub mod dom;
pub mod fill;
pub mod geom;
pub mod shape;
pub mod text;
#[cfg(feature = "wasm")]
pub mod wasm;
