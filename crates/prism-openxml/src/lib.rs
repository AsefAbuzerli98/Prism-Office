//! # prism-openxml
//!
//! Open Packaging Conventions (OPC, ISO/IEC 29500-2) reader/writer used by
//! Prism Office for `.docx`, `.xlsx` and `.pptx` files.
//!
//! The crate is intentionally format-agnostic at this layer: it understands
//! ZIP containers, `[Content_Types].xml`, relationship parts and the main
//! document part. Higher-level engines (word, sheet, slide) build on top.

mod content_types;
mod error;
mod kind;
mod package;
mod part_name;
mod rels;
pub mod templates;
mod validate;
mod xml;

pub use content_types::ContentTypes;
pub use error::{Error, Result};
pub use kind::DocumentKind;
pub use package::{Limits, Package};
pub use part_name::PartName;
pub use rels::{rel_types, Relationship, Relationships, TargetMode};
pub use validate::{Issue, Severity};
