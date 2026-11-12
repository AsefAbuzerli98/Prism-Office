use serde::{Deserialize, Serialize};

/// The three Prism Office modalities and their OpenXML file families.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentKind {
    /// Word processing (`.docx`, `.docm`, `.dotx`, `.dotm`).
    Document,
    /// Spreadsheet (`.xlsx`, `.xlsm`, `.xltx`, `.xltm`).
    Workbook,
    /// Presentation (`.pptx`, `.pptm`, `.potx`, `.potm`, `.ppsx`).
    Presentation,
}

const DOCUMENT_TYPES: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml",
    "application/vnd.ms-word.document.macroEnabled.main+xml",
    "application/vnd.ms-word.template.macroEnabledTemplate.main+xml",
];
const WORKBOOK_TYPES: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml",
    "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
    "application/vnd.ms-excel.template.macroEnabled.main+xml",
];
const PRESENTATION_TYPES: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
    "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml",
    "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml",
    "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml",
    "application/vnd.ms-powerpoint.template.macroEnabled.main+xml",
    "application/vnd.ms-powerpoint.slideshow.macroEnabled.main+xml",
];

impl DocumentKind {
    pub const ALL: [DocumentKind; 3] = [Self::Document, Self::Workbook, Self::Presentation];

    /// Classify by the main part's content type.
    pub fn from_main_content_type(ct: &str) -> Option<Self> {
        let ct = ct.trim();
        if DOCUMENT_TYPES.contains(&ct) {
            Some(Self::Document)
        } else if WORKBOOK_TYPES.contains(&ct) {
            Some(Self::Workbook)
        } else if PRESENTATION_TYPES.contains(&ct) {
            Some(Self::Presentation)
        } else {
            None
        }
    }

    /// Classify by file extension (without dot, any case).
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "docx" | "docm" | "dotx" | "dotm" => Some(Self::Document),
            "xlsx" | "xlsm" | "xltx" | "xltm" => Some(Self::Workbook),
            "pptx" | "pptm" | "potx" | "potm" | "ppsx" | "ppsm" => Some(Self::Presentation),
            _ => None,
        }
    }

    /// Canonical (non-macro, non-template) main content type.
    pub fn main_content_type(self) -> &'static str {
        match self {
            Self::Document => DOCUMENT_TYPES[0],
            Self::Workbook => WORKBOOK_TYPES[0],
            Self::Presentation => PRESENTATION_TYPES[0],
        }
    }

    pub fn default_extension(self) -> &'static str {
        match self {
            Self::Document => "docx",
            Self::Workbook => "xlsx",
            Self::Presentation => "pptx",
        }
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Document => &["docx", "docm", "dotx", "dotm"],
            Self::Workbook => &["xlsx", "xlsm", "xltx", "xltm"],
            Self::Presentation => &["pptx", "pptm", "potx", "potm", "ppsx", "ppsm"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies() {
        for k in DocumentKind::ALL {
            assert_eq!(
                DocumentKind::from_main_content_type(k.main_content_type()),
                Some(k)
            );
            assert_eq!(DocumentKind::from_extension(k.default_extension()), Some(k));
        }
        assert_eq!(
            DocumentKind::from_extension("PPTM"),
            Some(DocumentKind::Presentation)
        );
        assert_eq!(DocumentKind::from_extension("pdf"), None);
        assert_eq!(DocumentKind::from_main_content_type("text/plain"), None);
    }
}
