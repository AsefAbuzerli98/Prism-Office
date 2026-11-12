use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP container error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("XML error in {part}: {message}")]
    Xml { part: String, message: String },
    #[error("invalid part name `{0}`")]
    InvalidPartName(String),
    #[error("package is missing required part `{0}`")]
    MissingPart(String),
    #[error("package exceeds limit: {0}")]
    LimitExceeded(String),
    #[error("unsupported or unrecognised document type")]
    UnknownKind,
}

impl Error {
    pub(crate) fn xml(part: impl Into<String>, message: impl ToString) -> Self {
        Error::Xml {
            part: part.into(),
            message: message.to_string(),
        }
    }
}
