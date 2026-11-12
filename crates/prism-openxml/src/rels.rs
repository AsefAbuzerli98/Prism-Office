use crate::part_name::resolve_from_dir;
use crate::xml::{attr, escape_attr, for_each_element, XML_DECL};
use crate::{PartName, Result};

const NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

/// Well-known relationship type URIs.
pub mod rel_types {
    pub const OFFICE_DOCUMENT: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
    /// Strict (ISO 29500 Strict) variant of the officeDocument relationship.
    pub const OFFICE_DOCUMENT_STRICT: &str =
        "http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument";
    pub const CORE_PROPERTIES: &str =
        "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
    pub const EXTENDED_PROPERTIES: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";
    pub const STYLES: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
    pub const WORKSHEET: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";
    pub const SLIDE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
    pub const SLIDE_LAYOUT: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
    pub const SLIDE_MASTER: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";
    pub const THEME: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
    pub const HYPERLINK: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
    pub const IMAGE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TargetMode {
    #[default]
    Internal,
    External,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relationship {
    pub id: String,
    pub rel_type: String,
    pub target: String,
    pub target_mode: TargetMode,
}

/// The contents of one `.rels` part.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Relationships {
    items: Vec<Relationship>,
}

impl Relationships {
    pub fn parse(part: &str, data: &[u8]) -> Result<Self> {
        let mut items = Vec::new();
        for_each_element(part, data, |name, attrs| {
            if name == "Relationship" {
                if let (Some(id), Some(t), Some(target)) = (
                    attr(attrs, "Id"),
                    attr(attrs, "Type"),
                    attr(attrs, "Target"),
                ) {
                    let target_mode = match attr(attrs, "TargetMode") {
                        Some(m) if m.eq_ignore_ascii_case("External") => TargetMode::External,
                        _ => TargetMode::Internal,
                    };
                    items.push(Relationship {
                        id: id.to_string(),
                        rel_type: t.to_string(),
                        target: target.to_string(),
                        target_mode,
                    });
                }
            }
            Ok(())
        })?;
        Ok(Self { items })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Relationship> {
        self.items.iter()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn by_id(&self, id: &str) -> Option<&Relationship> {
        self.items.iter().find(|r| r.id == id)
    }

    pub fn first_of_type(&self, rel_type: &str) -> Option<&Relationship> {
        self.items.iter().find(|r| r.rel_type == rel_type)
    }

    pub fn of_type<'a>(&'a self, rel_type: &'a str) -> impl Iterator<Item = &'a Relationship> {
        self.items.iter().filter(move |r| r.rel_type == rel_type)
    }

    /// Add a relationship with the next free `rIdN` id; returns that id.
    pub fn add(&mut self, rel_type: &str, target: &str, target_mode: TargetMode) -> String {
        let mut n = self.items.len() + 1;
        let id = loop {
            let candidate = format!("rId{n}");
            if self.by_id(&candidate).is_none() {
                break candidate;
            }
            n += 1;
        };
        self.items.push(Relationship {
            id: id.clone(),
            rel_type: rel_type.to_string(),
            target: target.to_string(),
            target_mode,
        });
        id
    }

    pub fn to_xml(&self) -> String {
        let mut s = String::from(XML_DECL);
        s.push_str(&format!(r#"<Relationships xmlns="{NS}">"#));
        for r in &self.items {
            s.push_str(&format!(
                r#"<Relationship Id="{}" Type="{}" Target="{}"{}/>"#,
                escape_attr(&r.id),
                escape_attr(&r.rel_type),
                escape_attr(&r.target),
                if r.target_mode == TargetMode::External {
                    r#" TargetMode="External""#
                } else {
                    ""
                }
            ));
        }
        s.push_str("</Relationships>");
        s
    }
}

impl Relationship {
    /// Resolve the internal target to a part name. `source` is the part that
    /// owns the relationship, or `None` for package-level (`/_rels/.rels`).
    pub fn resolve(&self, source: Option<&PartName>) -> Option<Result<PartName>> {
        if self.target_mode == TargetMode::External {
            return None;
        }
        Some(match source {
            Some(src) => src.resolve(&self.target),
            None => resolve_from_dir("/", &self.target),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_serialize_roundtrip() {
        let mut r = Relationships::default();
        let a = r.add(
            rel_types::OFFICE_DOCUMENT,
            "word/document.xml",
            TargetMode::Internal,
        );
        let b = r.add(
            rel_types::HYPERLINK,
            "https://example.com/?a=1&b=2",
            TargetMode::External,
        );
        assert_eq!((a.as_str(), b.as_str()), ("rId1", "rId2"));

        let parsed = Relationships::parse("/_rels/.rels", r.to_xml().as_bytes()).unwrap();
        assert_eq!(parsed, r);
        assert_eq!(
            parsed.by_id("rId2").unwrap().target,
            "https://example.com/?a=1&b=2"
        );
        let main = parsed.first_of_type(rel_types::OFFICE_DOCUMENT).unwrap();
        assert_eq!(
            main.resolve(None).unwrap().unwrap().as_str(),
            "/word/document.xml"
        );
        assert!(parsed.by_id("rId2").unwrap().resolve(None).is_none());
    }
}
