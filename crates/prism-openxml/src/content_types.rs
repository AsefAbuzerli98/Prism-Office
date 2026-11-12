use std::collections::BTreeMap;

use crate::xml::{attr, escape_attr, for_each_element, XML_DECL};
use crate::{PartName, Result};

pub(crate) const CONTENT_TYPES_PART: &str = "[Content_Types].xml";
const NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";

pub mod ct {
    pub const RELATIONSHIPS: &str = "application/vnd.openxmlformats-package.relationships+xml";
    pub const XML: &str = "application/xml";
    pub const CORE_PROPERTIES: &str = "application/vnd.openxmlformats-package.core-properties+xml";
    pub const EXTENDED_PROPERTIES: &str =
        "application/vnd.openxmlformats-officedocument.extended-properties+xml";
}

/// The `[Content_Types].xml` stream: extension defaults plus per-part overrides.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentTypes {
    /// Lowercase extension → content type.
    defaults: BTreeMap<String, String>,
    /// Part name → content type.
    overrides: BTreeMap<PartName, String>,
}

impl ContentTypes {
    /// Content types with the two defaults every package needs.
    pub fn with_standard_defaults() -> Self {
        let mut c = Self::default();
        c.set_default("rels", ct::RELATIONSHIPS);
        c.set_default("xml", ct::XML);
        c
    }

    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut out = Self::default();
        for_each_element(CONTENT_TYPES_PART, data, |name, attrs| {
            match name {
                "Default" => {
                    if let (Some(ext), Some(ct)) =
                        (attr(attrs, "Extension"), attr(attrs, "ContentType"))
                    {
                        out.set_default(ext, ct);
                    }
                }
                "Override" => {
                    if let (Some(p), Some(ct)) =
                        (attr(attrs, "PartName"), attr(attrs, "ContentType"))
                    {
                        out.set_override(PartName::new(p)?, ct);
                    }
                }
                _ => {}
            }
            Ok(())
        })?;
        Ok(out)
    }

    pub fn set_default(&mut self, ext: &str, content_type: &str) {
        self.defaults
            .insert(ext.to_ascii_lowercase(), content_type.to_string());
    }

    pub fn set_override(&mut self, part: PartName, content_type: &str) {
        self.overrides.insert(part, content_type.to_string());
    }

    pub fn remove_override(&mut self, part: &PartName) {
        self.overrides.remove(part);
    }

    pub fn default_for(&self, ext: &str) -> Option<&str> {
        self.defaults
            .get(&ext.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// Resolve the content type of a part: override first, then extension default.
    pub fn get(&self, part: &PartName) -> Option<&str> {
        if let Some(ct) = self.overrides.get(part) {
            return Some(ct);
        }
        part.extension()
            .and_then(|e| self.defaults.get(&e))
            .map(String::as_str)
    }

    pub fn overrides(&self) -> impl Iterator<Item = (&PartName, &str)> {
        self.overrides.iter().map(|(k, v)| (k, v.as_str()))
    }

    /// Find all parts overridden with the given content type.
    pub fn parts_with<'a>(
        &'a self,
        content_type: &'a str,
    ) -> impl Iterator<Item = &'a PartName> + 'a {
        self.overrides
            .iter()
            .filter(move |(_, v)| v.as_str() == content_type)
            .map(|(k, _)| k)
    }

    pub fn to_xml(&self) -> String {
        let mut s = String::from(XML_DECL);
        s.push_str(&format!(r#"<Types xmlns="{NS}">"#));
        for (ext, ct) in &self.defaults {
            s.push_str(&format!(
                r#"<Default Extension="{}" ContentType="{}"/>"#,
                escape_attr(ext),
                escape_attr(ct)
            ));
        }
        for (part, ct) in &self.overrides {
            s.push_str(&format!(
                r#"<Override PartName="{}" ContentType="{}"/>"#,
                escape_attr(part.as_str()),
                escape_attr(ct)
            ));
        }
        s.push_str("</Types>");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_lookup() {
        let mut c = ContentTypes::with_standard_defaults();
        c.set_default("PNG", "image/png");
        let doc = PartName::new("/word/document.xml").unwrap();
        c.set_override(doc.clone(), "application/x-test+xml");

        let parsed = ContentTypes::parse(c.to_xml().as_bytes()).unwrap();
        assert_eq!(parsed, c);
        assert_eq!(parsed.get(&doc), Some("application/x-test+xml"));
        let img = PartName::new("/word/media/a.Png").unwrap();
        assert_eq!(parsed.get(&img), Some("image/png"));
        let other = PartName::new("/foo.bin").unwrap();
        assert_eq!(parsed.get(&other), None);
    }
}
