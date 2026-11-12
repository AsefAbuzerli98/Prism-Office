use std::fmt;

use crate::{Error, Result};

/// A normalised OPC part name, e.g. `/word/document.xml`.
///
/// Part names are absolute, use `/` separators, never contain `.`/`..`
/// segments and are compared case-insensitively (ISO/IEC 29500-2 §9.1.1.1).
/// We store the original spelling and compare on the ASCII-lowercased form.
#[derive(Clone, Debug, Eq)]
pub struct PartName {
    name: String,
    key: String,
}

impl PartName {
    /// Parse and validate a part name. Accepts names with or without the
    /// leading slash (ZIP entry names have none).
    pub fn new(raw: &str) -> Result<Self> {
        let trimmed = raw.trim_start_matches('/');
        if trimmed.is_empty() || trimmed.ends_with('/') || raw.contains('\\') {
            return Err(Error::InvalidPartName(raw.to_string()));
        }
        for seg in trimmed.split('/') {
            if seg.is_empty() || seg == "." || seg == ".." || seg.ends_with('.') {
                return Err(Error::InvalidPartName(raw.to_string()));
            }
        }
        let name = format!("/{trimmed}");
        let key = name.to_ascii_lowercase();
        Ok(Self { name, key })
    }

    pub fn as_str(&self) -> &str {
        &self.name
    }

    /// Name used inside the ZIP container (no leading slash).
    pub fn zip_name(&self) -> &str {
        &self.name[1..]
    }

    /// Case-insensitive comparison key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// File extension (lowercase) without the dot.
    pub fn extension(&self) -> Option<String> {
        let file = self.name.rsplit('/').next()?;
        file.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())
    }

    /// Directory portion including trailing slash, e.g. `/word/`.
    pub fn directory(&self) -> &str {
        let idx = self.name.rfind('/').unwrap_or(0);
        &self.name[..=idx]
    }

    /// The relationships part for this part, e.g.
    /// `/word/document.xml` → `/word/_rels/document.xml.rels`.
    pub fn rels_part(&self) -> PartName {
        let file = &self.name[self.directory().len()..];
        PartName::new(&format!("{}_rels/{file}.rels", self.directory()))
            .expect("rels part name derived from valid part name is valid")
    }

    /// Whether this part is itself a relationships part.
    pub fn is_rels(&self) -> bool {
        self.key.ends_with(".rels") && self.key.contains("/_rels/")
    }

    /// Resolve a relative relationship target against this part as source.
    pub fn resolve(&self, target: &str) -> Result<PartName> {
        resolve_from_dir(self.directory(), target)
    }
}

/// Resolve `target` (relative or absolute) against base directory `dir`
/// (which must start and end with `/`).
pub(crate) fn resolve_from_dir(dir: &str, target: &str) -> Result<PartName> {
    let target = target.split(['#', '?']).next().unwrap_or("");
    let joined = if target.starts_with('/') {
        target.to_string()
    } else {
        format!("{dir}{target}")
    };
    let mut stack: Vec<&str> = Vec::new();
    for seg in joined.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if stack.pop().is_none() {
                    return Err(Error::InvalidPartName(target.to_string()));
                }
            }
            s => stack.push(s),
        }
    }
    PartName::new(&stack.join("/"))
}

impl PartialEq for PartName {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl std::hash::Hash for PartName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key.hash(state)
    }
}

impl PartialOrd for PartName {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PartName {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}

impl fmt::Display for PartName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_and_compares_case_insensitively() {
        let a = PartName::new("word/Document.xml").unwrap();
        let b = PartName::new("/WORD/document.xml").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.as_str(), "/word/Document.xml");
        assert_eq!(a.zip_name(), "word/Document.xml");
    }

    #[test]
    fn rejects_bad_names() {
        for bad in ["", "/", "a//b", "../x", "a/./b", "a/", "a\\b", "x."] {
            assert!(PartName::new(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn rels_part_and_resolve() {
        let p = PartName::new("/ppt/slides/slide1.xml").unwrap();
        assert_eq!(p.rels_part().as_str(), "/ppt/slides/_rels/slide1.xml.rels");
        assert_eq!(
            p.resolve("../slideLayouts/slideLayout1.xml")
                .unwrap()
                .as_str(),
            "/ppt/slideLayouts/slideLayout1.xml"
        );
        assert_eq!(
            p.resolve("/docProps/app.xml").unwrap().as_str(),
            "/docProps/app.xml"
        );
        assert!(p.resolve("../../../escape.xml").is_err());
    }
}
