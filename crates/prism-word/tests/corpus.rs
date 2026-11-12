//! Round-trip every document in tests/corpus (P2-03).

use std::collections::BTreeSet;

use prism_openxml::{Package, Severity};
use prism_word::*;

fn errors(pkg: &Package) -> BTreeSet<String> {
    pkg.validate()
        .into_iter()
        .filter(|i| i.severity == Severity::Error)
        .map(|i| format!("{:?}: {}", i.part, i.message))
        .collect()
}

#[test]
fn corpus_round_trips() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "docx"))
        .collect();
    files.sort();
    assert!(files.len() >= 10, "corpus missing");
    let mut report = Vec::new();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let original = Package::open(&path).unwrap_or_else(|e| panic!("{name}: open: {e}"));
        let before_errors = errors(&original);
        let doc = read_package(&original).unwrap_or_else(|e| panic!("{name}: read: {e}"));

        let mut pkg = original.clone();
        write_package(&mut pkg, &doc).unwrap_or_else(|e| panic!("{name}: write: {e}"));
        let new_errors: Vec<_> = errors(&pkg).difference(&before_errors).cloned().collect();
        assert!(
            new_errors.is_empty(),
            "{name}: new validation errors {new_errors:#?}"
        );

        // untouched parts stay byte-identical
        let main = pkg.main_part().unwrap();
        for part in original.part_names() {
            if *part == main || part.as_str().ends_with("document.xml.rels") {
                continue;
            }
            assert_eq!(
                pkg.part(part),
                original.part(part),
                "{name}: {part} changed although nothing was edited"
            );
        }

        let bytes = pkg.to_bytes().unwrap();
        let back = read_package(&Package::from_bytes(&bytes).unwrap())
            .unwrap_or_else(|e| panic!("{name}: re-read: {e}"));
        assert_eq!(
            back.blocks, doc.blocks,
            "{name}: body changed on round-trip"
        );
        assert_eq!(back.section, doc.section, "{name}: section changed");
        assert_eq!(back.notes, doc.notes, "{name}: notes changed");
        assert_eq!(back.comments, doc.comments, "{name}: comments changed");
        assert_eq!(back.headers, doc.headers, "{name}: headers changed");
        assert_eq!(back.plain_text(), doc.plain_text());

        let mut again = Package::from_bytes(&bytes).unwrap();
        write_package(&mut again, &back).unwrap();
        assert_eq!(
            again.part(&main),
            pkg.part(&main),
            "{name}: second export not idempotent"
        );

        let opaque = count_opaque(&doc.blocks);
        report.push(format!(
            "{name}: {} blocks, {} chars, {} opaque",
            doc.blocks.len(),
            doc.plain_text().len(),
            opaque
        ));
    }
    println!("{}", report.join("\n"));
}

fn count_opaque(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .map(|b| match b {
            Block::Opaque { .. } => 1,
            Block::Table(t) => t
                .rows
                .iter()
                .flat_map(|r| &r.cells)
                .map(|c| count_opaque(&c.blocks))
                .sum(),
            Block::Paragraph(p) => p
                .inlines
                .iter()
                .filter(|i| matches!(i, Inline::Opaque { label, .. } if !label.is_empty()))
                .count(),
        })
        .sum()
}
