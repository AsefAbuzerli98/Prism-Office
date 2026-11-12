//! `word/styles.xml`: parse styles and resolve `basedOn` inheritance; add
//! missing built-in styles on export.

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::model::*;
use crate::raw::{attr, local, skip};
use crate::read::{parse_ppr, parse_rpr};
use crate::{Error, Result};

struct RawStyle {
    info: StyleInfo,
}

pub(crate) fn parse_styles(data: &[u8]) -> Result<(Marks, ParaFormat, Vec<StyleInfo>)> {
    let text = std::str::from_utf8(data).map_err(Error::xml)?;
    let mut r = Reader::from_str(text);
    let mut default_run = Marks::default();
    let mut default_para = ParaFormat::default();
    let mut styles: Vec<RawStyle> = Vec::new();
    let mut cur: Option<RawStyle> = None;
    let mut in_rpr_default = false;
    let mut in_ppr_default = false;

    loop {
        let (e, empty) = match r.read_event().map_err(Error::xml)? {
            Event::Start(e) => (e.into_owned(), false),
            Event::Empty(e) => (e.into_owned(), true),
            Event::End(e) => {
                match e.local_name().into_inner() {
                    "style" => styles.extend(cur.take()),
                    "rPrDefault" => in_rpr_default = false,
                    "pPrDefault" => in_ppr_default = false,
                    _ => {}
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        match local(&e).as_str() {
            "rPrDefault" => in_rpr_default = !empty,
            "pPrDefault" => in_ppr_default = !empty,
            "rPr" if in_rpr_default => default_run = parse_rpr(&mut r, empty)?,
            "pPr" if in_ppr_default => default_para = parse_ppr(&mut r, empty)?.1,
            "style" => {
                let kind = match attr(&e, "type").as_deref() {
                    Some("character") => StyleKind::Character,
                    Some("table") => StyleKind::Table,
                    Some("numbering") => StyleKind::Numbering,
                    _ => StyleKind::Paragraph,
                };
                let id = attr(&e, "styleId").unwrap_or_default();
                let s = RawStyle {
                    info: StyleInfo {
                        name: id.clone(),
                        id,
                        kind,
                        based_on: None,
                        next: None,
                        is_default: matches!(attr(&e, "default").as_deref(), Some("1" | "true")),
                        para: ParaFormat::default(),
                        run: Marks::default(),
                    },
                };
                if empty {
                    styles.push(s);
                } else {
                    cur = Some(s);
                }
            }
            "name" if cur.is_some() => {
                if let (Some(c), Some(v)) = (cur.as_mut(), attr(&e, "val")) {
                    c.info.name = v;
                }
                skip(&mut r, empty)?;
            }
            "basedOn" if cur.is_some() => {
                cur.as_mut().unwrap().info.based_on = attr(&e, "val");
                skip(&mut r, empty)?;
            }
            "next" if cur.is_some() => {
                cur.as_mut().unwrap().info.next = attr(&e, "val");
                skip(&mut r, empty)?;
            }
            "pPr" if cur.is_some() => {
                cur.as_mut().unwrap().info.para = parse_ppr(&mut r, empty)?.1;
            }
            "rPr" if cur.is_some() => {
                let mut m = parse_rpr(&mut r, empty)?;
                m.extra.clear();
                cur.as_mut().unwrap().info.run = m;
            }
            _ if cur.is_some() => skip(&mut r, empty)?,
            _ => {}
        }
    }
    default_run.extra.clear();

    // Resolve inheritance (cycle-safe, depth-limited).
    let by_id: HashMap<String, StyleInfo> = styles
        .iter()
        .map(|s| (s.info.id.clone(), s.info.clone()))
        .collect();
    let resolve = |s: &StyleInfo| -> StyleInfo {
        let mut chain = vec![s.clone()];
        let mut cur = s.based_on.clone();
        while let Some(id) = cur {
            if chain.len() > 16 || chain.iter().any(|c| c.id == id) {
                break;
            }
            match by_id.get(&id) {
                Some(p) => {
                    cur = p.based_on.clone();
                    chain.push(p.clone());
                }
                None => break,
            }
        }
        let mut para = ParaFormat::default();
        let mut run = Marks::default();
        for c in chain.iter().rev() {
            para = para.overlay(&c.para);
            run = run.overlay(&c.run);
        }
        StyleInfo {
            para,
            run,
            ..s.clone()
        }
    };
    let resolved = styles.iter().map(|s| resolve(&s.info)).collect();
    Ok((default_run, default_para, resolved))
}

/// Definitions for built-in paragraph styles Prism can apply. Added to
/// `styles.xml` on export when a document references one that is missing.
pub(crate) fn builtin_style_xml(id: &str) -> Option<String> {
    let (name, ppr, rpr) = match id {
        "Title" => (
            "Title",
            r#"<w:spacing w:after="80"/><w:contextualSpacing/>"#,
            r#"<w:sz w:val="56"/>"#,
        ),
        "Heading1" => (
            "heading 1",
            r#"<w:keepNext/><w:spacing w:before="360" w:after="80"/><w:outlineLvl w:val="0"/>"#,
            r#"<w:b/><w:sz w:val="40"/>"#,
        ),
        "Heading2" => (
            "heading 2",
            r#"<w:keepNext/><w:spacing w:before="160" w:after="80"/><w:outlineLvl w:val="1"/>"#,
            r#"<w:b/><w:sz w:val="32"/>"#,
        ),
        "Heading3" => (
            "heading 3",
            r#"<w:keepNext/><w:spacing w:before="160" w:after="80"/><w:outlineLvl w:val="2"/>"#,
            r#"<w:b/><w:sz w:val="28"/>"#,
        ),
        "Heading4" => (
            "heading 4",
            r#"<w:keepNext/><w:spacing w:before="80" w:after="40"/><w:outlineLvl w:val="3"/>"#,
            r#"<w:b/><w:i/>"#,
        ),
        "Quote" => (
            "Quote",
            r#"<w:spacing w:before="160" w:after="160"/><w:ind w:left="864" w:right="864"/>"#,
            r#"<w:i/><w:color w:val="404040"/>"#,
        ),
        "Code" => (
            "Code",
            r#"<w:spacing w:after="0" w:line="240" w:lineRule="auto"/>"#,
            r#"<w:rFonts w:ascii="Consolas" w:hAnsi="Consolas" w:cs="Consolas"/><w:sz w:val="20"/>"#,
        ),
        "ListParagraph" => (
            "List Paragraph",
            r#"<w:ind w:left="720"/><w:contextualSpacing/>"#,
            "",
        ),
        _ => return None,
    };
    let next = if id.starts_with("Heading") || id == "Title" {
        r#"<w:next w:val="Normal"/>"#
    } else {
        ""
    };
    Some(format!(
        r#"<w:style w:type="paragraph" w:styleId="{id}"><w:name w:val="{name}"/><w:basedOn w:val="Normal"/>{next}<w:qFormat/><w:pPr>{ppr}</w:pPr><w:rPr>{rpr}</w:rPr></w:style>"#
    ))
}

pub(crate) const BUILTIN_IDS: &[&str] = &[
    "Title",
    "Heading1",
    "Heading2",
    "Heading3",
    "Heading4",
    "Quote",
    "Code",
    "ListParagraph",
];

/// Resolved built-in style infos (for documents whose styles.xml lacks them).
pub(crate) fn builtin_infos(existing: &[StyleInfo]) -> Vec<StyleInfo> {
    let mut wrapped = String::from(
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    );
    for id in BUILTIN_IDS {
        if !existing.iter().any(|s| s.id == *id) {
            wrapped.push_str(&builtin_style_xml(id).unwrap());
        }
    }
    wrapped.push_str("</w:styles>");
    parse_styles(wrapped.as_bytes())
        .map(|(_, _, s)| s)
        .unwrap_or_default()
}
