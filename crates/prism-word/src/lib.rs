//! # prism-word
//!
//! WordprocessingML document model with lossless DOCX import/export.
//!
//! The model covers what the editor understands (paragraphs, runs, marks,
//! lists, styles, section page setup). Everything else — tables, drawings,
//! fields, tracked changes, unknown properties — is carried through as raw
//! XML (`Opaque` blocks/inlines and `extra` property lists) so that saving a
//! document never drops content the editor does not yet support.

mod model;
mod numbering;
mod order;
mod raw;
mod read;
mod styles;
mod write;

pub use model::*;
pub use read::{read_document, read_package};
pub use write::{write_document_xml, write_package};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    OpenXml(#[from] prism_openxml::Error),
    #[error("XML error: {0}")]
    Xml(String),
    #[error("package is not a word-processing document")]
    NotADocument,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn xml(e: impl ToString) -> Self {
        Error::Xml(e.to_string())
    }
}
