use serde::Serialize;

use crate::{Package, PartName, TargetMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub severity: Severity,
    pub part: Option<String>,
    pub message: String,
}

impl Issue {
    fn new(severity: Severity, part: Option<&PartName>, message: impl Into<String>) -> Self {
        Self {
            severity,
            part: part.map(|p| p.as_str().to_string()),
            message: message.into(),
        }
    }
}

/// Map a relationships part name back to its source part.
/// `/_rels/.rels` → `None` (package root); `/a/_rels/b.xml.rels` → `/a/b.xml`.
fn rels_source(rels: &PartName) -> Option<Option<PartName>> {
    let s = rels.as_str();
    let (dir, file) = s.rsplit_once("/_rels/")?;
    let src_file = file
        .strip_suffix(".rels")
        .or_else(|| file.strip_suffix(".RELS"))?;
    if dir.is_empty() && src_file.is_empty() {
        return Some(None);
    }
    PartName::new(&format!("{dir}/{src_file}")).ok().map(Some)
}

impl Package {
    /// Structural validation per OPC: content types, relationship targets
    /// and the main document part. Returns all issues found.
    pub fn validate(&self) -> Vec<Issue> {
        let mut issues = Vec::new();

        for name in self.part_names() {
            if self.content_type(name).is_none() {
                issues.push(Issue::new(
                    Severity::Error,
                    Some(name),
                    "part has no content type",
                ));
            }
        }

        for (name, _) in self.content_types().overrides() {
            if !self.has_part(name) {
                issues.push(Issue::new(
                    Severity::Warning,
                    Some(name),
                    "content type override refers to a missing part",
                ));
            }
        }

        let rels_parts: Vec<PartName> =
            self.part_names().filter(|p| p.is_rels()).cloned().collect();
        for rels_name in rels_parts {
            let Some(source) = rels_source(&rels_name) else {
                issues.push(Issue::new(
                    Severity::Error,
                    Some(&rels_name),
                    "relationships part name is malformed",
                ));
                continue;
            };
            if let Some(src) = &source {
                if !self.has_part(src) {
                    issues.push(Issue::new(
                        Severity::Warning,
                        Some(&rels_name),
                        format!("relationships part has no source part `{src}`"),
                    ));
                }
            }
            let rels = match self.relationships(source.as_ref()) {
                Ok(r) => r,
                Err(e) => {
                    issues.push(Issue::new(Severity::Error, Some(&rels_name), e.to_string()));
                    continue;
                }
            };
            let mut seen = std::collections::HashSet::new();
            for rel in rels.iter() {
                if !seen.insert(rel.id.as_str()) {
                    issues.push(Issue::new(
                        Severity::Error,
                        Some(&rels_name),
                        format!("duplicate relationship id `{}`", rel.id),
                    ));
                }
                if rel.target_mode == TargetMode::External {
                    continue;
                }
                match rel.resolve(source.as_ref()) {
                    Some(Ok(target)) if !self.has_part(&target) => issues.push(Issue::new(
                        Severity::Error,
                        Some(&rels_name),
                        format!("relationship `{}` targets missing part `{target}`", rel.id),
                    )),
                    Some(Err(e)) => issues.push(Issue::new(
                        Severity::Error,
                        Some(&rels_name),
                        format!("relationship `{}`: {e}", rel.id),
                    )),
                    _ => {}
                }
            }
        }

        match self.main_part() {
            Err(e) => issues.push(Issue::new(Severity::Error, None, e.to_string())),
            Ok(main) => {
                if self.kind().is_err() {
                    issues.push(Issue::new(
                        Severity::Error,
                        Some(&main),
                        "main part content type is not a supported document type",
                    ));
                }
            }
        }

        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{rel_types, DocumentKind, Relationships};

    #[test]
    fn rels_source_mapping() {
        let root = PartName::new("/_rels/.rels").unwrap();
        assert_eq!(rels_source(&root), Some(None));
        let doc = PartName::new("/word/_rels/document.xml.rels").unwrap();
        assert_eq!(
            rels_source(&doc).unwrap().unwrap().as_str(),
            "/word/document.xml"
        );
    }

    #[test]
    fn detects_dangling_relationship_and_missing_content_type() {
        let mut p = Package::new();
        let doc = PartName::new("/word/document.xml").unwrap();
        p.set_part(
            doc.clone(),
            b"<x/>".to_vec(),
            Some(DocumentKind::Document.main_content_type()),
        );
        p.set_part(PartName::new("/blob.bin").unwrap(), vec![1], None);
        let mut root = Relationships::default();
        root.add(
            rel_types::OFFICE_DOCUMENT,
            "word/document.xml",
            TargetMode::Internal,
        );
        p.set_relationships(None, &root);
        let mut dr = Relationships::default();
        dr.add(rel_types::STYLES, "styles.xml", TargetMode::Internal);
        p.set_relationships(Some(&doc), &dr);

        let issues = p.validate();
        assert!(issues
            .iter()
            .any(|i| i.message.contains("missing part `/word/styles.xml`")));
        assert!(issues
            .iter()
            .any(|i| i.part.as_deref() == Some("/blob.bin") && i.message.contains("content type")));
    }
}
