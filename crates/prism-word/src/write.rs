//! Model → DOCX.

use quick_xml::events::Event;
use quick_xml::Reader;

use prism_openxml::{rel_types, Package, PartName, TargetMode};

use crate::model::*;
use crate::order::{sorted, PPR, RPR, SECTPR};
use crate::raw::{escape_attr, escape_text, local};
use crate::read::NUMBERING_REL;
use crate::{numbering, styles, Error, Result};

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DEFAULT_ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>"#;

/// Everything up to and including the `<w:body>` start tag of `original`,
/// with `w`/`r` namespace declarations guaranteed on the root element.
fn document_prefix(original: Option<&[u8]>) -> String {
    let Some(orig) = original.and_then(|o| std::str::from_utf8(o).ok()) else {
        return DEFAULT_ROOT.to_string();
    };
    let mut r = Reader::from_str(orig);
    let mut root_end = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let name = local(&e);
                if root_end.is_none() {
                    root_end = Some(r.buffer_position() as usize);
                }
                if name == "body" {
                    let mut prefix = orig[..r.buffer_position() as usize].to_string();
                    let root_close = root_end.unwrap() - 1; // index of '>' of root tag
                    let root_tag = &prefix[..root_close];
                    let mut inject = String::new();
                    if !root_tag.contains("xmlns:w=") {
                        inject.push_str(&format!(r#" xmlns:w="{W_NS}""#));
                    }
                    if !root_tag.contains("xmlns:r=") {
                        inject.push_str(&format!(r#" xmlns:r="{R_NS}""#));
                    }
                    prefix.insert_str(root_close, &inject);
                    return prefix;
                }
            }
            Ok(Event::Empty(e)) if local(&e) == "body" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    DEFAULT_ROOT.to_string()
}

fn on_off(name: &str, v: bool) -> String {
    if v {
        format!("<w:{name}/>")
    } else {
        format!(r#"<w:{name} w:val="0"/>"#)
    }
}

pub(crate) fn rpr_xml(m: &Marks) -> String {
    let mut parts: Vec<(String, String)> = Vec::new();
    let mut add = |n: &str, x: String| parts.push((n.to_string(), x));
    if let Some(s) = &m.style {
        add(
            "rStyle",
            format!(r#"<w:rStyle w:val="{}"/>"#, escape_attr(s)),
        );
    }
    if let Some(f) = &m.font {
        let f = escape_attr(f);
        add(
            "rFonts",
            format!(r#"<w:rFonts w:ascii="{f}" w:hAnsi="{f}" w:eastAsia="{f}" w:cs="{f}"/>"#),
        );
    }
    if let Some(b) = m.bold {
        add("b", on_off("b", b));
    }
    if let Some(i) = m.italic {
        add("i", on_off("i", i));
    }
    if let Some(s) = m.strike {
        add("strike", on_off("strike", s));
    }
    if let Some(c) = &m.color {
        add("color", format!(r#"<w:color w:val="{}"/>"#, escape_attr(c)));
    }
    if let Some(sz) = m.size {
        add("sz", format!(r#"<w:sz w:val="{sz}"/>"#));
    }
    if let Some(h) = &m.highlight {
        add(
            "highlight",
            format!(r#"<w:highlight w:val="{}"/>"#, escape_attr(h)),
        );
    }
    if let Some(u) = m.underline {
        add(
            "u",
            format!(r#"<w:u w:val="{}"/>"#, if u { "single" } else { "none" }),
        );
    }
    if let Some(v) = m.vert_align {
        let v = match v {
            VertAlign::Superscript => "superscript",
            VertAlign::Subscript => "subscript",
        };
        add("vertAlign", format!(r#"<w:vertAlign w:val="{v}"/>"#));
    }
    for e in &m.extra {
        parts.push((e.name.clone(), e.xml.clone()));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{}</w:rPr>", sorted(RPR, parts))
    }
}

fn ppr_xml(p: &Paragraph) -> String {
    let f = &p.format;
    let mut parts: Vec<(String, String)> = Vec::new();
    let mut add = |n: &str, x: String| parts.push((n.to_string(), x));
    if let Some(s) = &p.style {
        add(
            "pStyle",
            format!(r#"<w:pStyle w:val="{}"/>"#, escape_attr(s)),
        );
    }
    if f.keep_next {
        add("keepNext", "<w:keepNext/>".into());
    }
    if f.page_break_before {
        add("pageBreakBefore", "<w:pageBreakBefore/>".into());
    }
    if let Some(l) = p.list {
        add(
            "numPr",
            format!(
                r#"<w:numPr><w:ilvl w:val="{}"/><w:numId w:val="{}"/></w:numPr>"#,
                l.level, l.num_id
            ),
        );
    }
    if f.space_before.is_some() || f.space_after.is_some() || f.line.is_some() {
        let mut s = String::from("<w:spacing");
        if let Some(v) = f.space_before {
            s.push_str(&format!(r#" w:before="{v}""#));
        }
        if let Some(v) = f.space_after {
            s.push_str(&format!(r#" w:after="{v}""#));
        }
        if let Some(v) = f.line {
            s.push_str(&format!(r#" w:line="{v}" w:lineRule="auto""#));
        }
        s.push_str("/>");
        add("spacing", s);
    }
    if f.indent_left.is_some() || f.indent_right.is_some() || f.indent_first.is_some() {
        let mut s = String::from("<w:ind");
        if let Some(v) = f.indent_left {
            s.push_str(&format!(r#" w:left="{v}""#));
        }
        if let Some(v) = f.indent_right {
            s.push_str(&format!(r#" w:right="{v}""#));
        }
        match f.indent_first {
            Some(v) if v >= 0 => s.push_str(&format!(r#" w:firstLine="{v}""#)),
            Some(v) => s.push_str(&format!(r#" w:hanging="{}""#, -v)),
            None => {}
        }
        s.push_str("/>");
        add("ind", s);
    }
    if let Some(a) = f.align {
        add("jc", format!(r#"<w:jc w:val="{}"/>"#, a.to_ooxml()));
    }
    if let Some(o) = f.outline_level {
        add("outlineLvl", format!(r#"<w:outlineLvl w:val="{o}"/>"#));
    }
    for e in &p.extra {
        parts.push((e.name.clone(), e.xml.clone()));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("<w:pPr>{}</w:pPr>", sorted(PPR, parts))
    }
}

fn text_run(text: &str, marks: &Marks) -> String {
    let rpr = rpr_xml(marks);
    let mut body = String::new();
    let mut buf = String::new();
    let flush = |buf: &mut String, body: &mut String| {
        if !buf.is_empty() {
            body.push_str(&format!(
                r#"<w:t xml:space="preserve">{}</w:t>"#,
                escape_text(buf)
            ));
            buf.clear();
        }
    };
    for c in text.chars() {
        match c {
            '\t' => {
                flush(&mut buf, &mut body);
                body.push_str("<w:tab/>");
            }
            '\n' => {
                flush(&mut buf, &mut body);
                body.push_str("<w:br/>");
            }
            '\u{2011}' => {
                flush(&mut buf, &mut body);
                body.push_str("<w:noBreakHyphen/>");
            }
            '\u{00AD}' => {
                flush(&mut buf, &mut body);
                body.push_str("<w:softHyphen/>");
            }
            // Characters not allowed in XML 1.0.
            c if (c as u32) < 0x20 => {}
            c => buf.push(c),
        }
    }
    flush(&mut buf, &mut body);
    format!("<w:r>{rpr}{body}</w:r>")
}

fn inline_xml(i: &Inline) -> String {
    match i {
        Inline::Text { text, marks } => text_run(text, marks),
        Inline::Tab { marks } => format!("<w:r>{}<w:tab/></w:r>", rpr_xml(marks)),
        Inline::Break { kind } => match kind {
            BreakKind::Line => "<w:r><w:br/></w:r>".into(),
            BreakKind::Page => r#"<w:r><w:br w:type="page"/></w:r>"#.into(),
            BreakKind::Column => r#"<w:r><w:br w:type="column"/></w:r>"#.into(),
        },
        Inline::Opaque { xml, .. } => xml.clone(),
    }
}

fn link_of(i: &Inline) -> Option<&str> {
    match i {
        Inline::Text { marks, .. } | Inline::Tab { marks } => marks.link.as_deref(),
        _ => None,
    }
}

fn strip_link(i: &Inline) -> Inline {
    let mut i = i.clone();
    if let Inline::Text { marks, .. } | Inline::Tab { marks } = &mut i {
        marks.link = None;
    }
    i
}

fn paragraph_xml(p: &Paragraph, link_id: &mut dyn FnMut(&str) -> String) -> String {
    let mut s = format!("<w:p>{}", ppr_xml(p));
    let mut i = 0;
    while i < p.inlines.len() {
        match link_of(&p.inlines[i]) {
            None => {
                s.push_str(&inline_xml(&p.inlines[i]));
                i += 1;
            }
            Some(target) => {
                let mut j = i;
                let mut inner = String::new();
                while j < p.inlines.len() && link_of(&p.inlines[j]) == Some(target) {
                    inner.push_str(&inline_xml(&strip_link(&p.inlines[j])));
                    j += 1;
                }
                let open = match target.strip_prefix('#') {
                    Some(anchor) => format!(r#"<w:hyperlink w:anchor="{}">"#, escape_attr(anchor)),
                    None => format!(r#"<w:hyperlink r:id="{}">"#, link_id(target)),
                };
                s.push_str(&open);
                s.push_str(&inner);
                s.push_str("</w:hyperlink>");
                i = j;
            }
        }
    }
    s.push_str("</w:p>");
    s
}

fn sect_xml(sec: &Section) -> String {
    let gutter = sec
        .extra
        .iter()
        .find(|e| e.name == "w:gutter")
        .map(|e| e.xml.as_str())
        .unwrap_or("0");
    let mut parts: Vec<(String, String)> = sec
        .extra
        .iter()
        .filter(|e| e.name != "w:gutter")
        .map(|e| (e.name.clone(), e.xml.clone()))
        .collect();
    let orient = if sec.landscape {
        r#" w:orient="landscape""#
    } else {
        ""
    };
    parts.push((
        "pgSz".into(),
        format!(
            r#"<w:pgSz w:w="{}" w:h="{}"{orient}/>"#,
            sec.page_width, sec.page_height
        ),
    ));
    parts.push((
        "pgMar".into(),
        format!(
            r#"<w:pgMar w:top="{}" w:right="{}" w:bottom="{}" w:left="{}" w:header="{}" w:footer="{}" w:gutter="{gutter}"/>"#,
            sec.margin_top, sec.margin_right, sec.margin_bottom, sec.margin_left, sec.margin_header, sec.margin_footer
        ),
    ));
    format!("<w:sectPr>{}</w:sectPr>", sorted(SECTPR, parts))
}

/// Serialise `doc` as `word/document.xml`, reusing the root element and any
/// pre-body content (e.g. `w:background`) of `original`.
pub fn write_document_xml(
    original: Option<&[u8]>,
    doc: &WordDocument,
    link_id: &mut dyn FnMut(&str) -> String,
) -> String {
    let mut s = document_prefix(original);
    for b in &doc.blocks {
        match b {
            Block::Paragraph(p) => s.push_str(&paragraph_xml(p, link_id)),
            Block::Opaque { xml, .. } => s.push_str(xml),
        }
    }
    // A body must end with a paragraph before sectPr if the last block is a table.
    if matches!(doc.blocks.last(), Some(Block::Opaque { label, .. }) if label == "Table")
        || doc.blocks.is_empty()
    {
        s.push_str("<w:p/>");
    }
    s.push_str(&sect_xml(&doc.section));
    s.push_str("</w:body></w:document>");
    s
}

/// Byte offsets: (before first top-level child named `child`, before root end tag).
fn insertion_points(xml: &str, child: &str) -> Option<(Option<usize>, usize)> {
    let mut r = Reader::from_str(xml);
    let mut depth = 0usize;
    let mut first_child = None;
    loop {
        let before = r.buffer_position() as usize;
        match r.read_event().ok()? {
            Event::Start(e) => {
                if depth == 1 && first_child.is_none() && local(&e) == child {
                    first_child = Some(before);
                }
                depth += 1;
            }
            Event::Empty(e) => {
                if depth == 1 && first_child.is_none() && local(&e) == child {
                    first_child = Some(before);
                }
            }
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    return Some((first_child, before));
                }
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

fn ensure_numbering(pkg: &mut Package, main: &PartName, doc: &WordDocument) -> Result<()> {
    let mut used: Vec<u32> = doc
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Paragraph(p) => p.list.map(|l| l.num_id),
            _ => None,
        })
        .filter(|id| numbering::builtin_xml(*id).is_some())
        .collect();
    used.sort_unstable();
    used.dedup();
    if used.is_empty() {
        return Ok(());
    }
    let mut rels = pkg.relationships(Some(main))?;
    let part = match rels
        .first_of_type(NUMBERING_REL)
        .and_then(|r| r.resolve(Some(main)))
        .transpose()?
    {
        Some(p) => p,
        None => {
            let p = PartName::new("/word/numbering.xml")?;
            rels.add(NUMBERING_REL, "numbering.xml", TargetMode::Internal);
            pkg.set_relationships(Some(main), &rels);
            pkg.set_part(
                p.clone(),
                format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="{W_NS}"></w:numbering>"#).into_bytes(),
                Some("application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"),
            );
            p
        }
    };
    let mut xml = String::from_utf8_lossy(pkg.part(&part).unwrap_or_default()).into_owned();
    let existing = numbering::parse_numbering(xml.as_bytes())?;
    let missing: Vec<u32> = used
        .into_iter()
        .filter(|id| !existing.iter().any(|d| d.num_id == *id))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let (first_num, end) =
        insertion_points(&xml, "num").ok_or_else(|| Error::xml("malformed numbering.xml"))?;
    let (mut abs_all, mut num_all) = (String::new(), String::new());
    for id in missing {
        let (a, n) = numbering::builtin_xml(id).unwrap();
        abs_all.push_str(&a);
        num_all.push_str(&n);
    }
    // abstractNum elements must precede all num elements.
    xml.insert_str(end, &num_all);
    xml.insert_str(first_num.unwrap_or(end), &abs_all);
    let ct = pkg.content_type(&part).map(str::to_owned);
    pkg.set_part(part, xml.into_bytes(), ct.as_deref());
    Ok(())
}

fn ensure_styles(pkg: &mut Package, main: &PartName, doc: &WordDocument) -> Result<()> {
    let rels = pkg.relationships(Some(main))?;
    let Some(part) = rels
        .first_of_type(rel_types::STYLES)
        .and_then(|r| r.resolve(Some(main)))
        .transpose()?
    else {
        return Ok(());
    };
    let Some(data) = pkg.part(&part) else {
        return Ok(());
    };
    let mut xml = String::from_utf8_lossy(data).into_owned();
    let (_, _, existing) = styles::parse_styles(xml.as_bytes())?;
    let mut add = String::new();
    for id in styles::BUILTIN_IDS {
        let used = doc
            .blocks
            .iter()
            .any(|b| matches!(b, Block::Paragraph(p) if p.style.as_deref() == Some(id)));
        if used && !existing.iter().any(|s| s.id == *id) {
            add.push_str(&styles::builtin_style_xml(id).unwrap());
        }
    }
    if add.is_empty() {
        return Ok(());
    }
    let (_, end) =
        insertion_points(&xml, "style").ok_or_else(|| Error::xml("malformed styles.xml"))?;
    xml.insert_str(end, &add);
    let ct = pkg.content_type(&part).map(str::to_owned);
    pkg.set_part(part, xml.into_bytes(), ct.as_deref());
    Ok(())
}

/// Write `doc` into `pkg`, replacing the main document part and adding any
/// list/style definitions and hyperlink relationships it needs.
pub fn write_package(pkg: &mut Package, doc: &WordDocument) -> Result<()> {
    let main = pkg.main_part()?;
    let mut rels = pkg.relationships(Some(&main))?;
    let original = pkg.part(&main).map(<[u8]>::to_vec);
    let xml = write_document_xml(original.as_deref(), doc, &mut |target: &str| {
        if let Some(r) = rels
            .of_type(rel_types::HYPERLINK)
            .find(|r| r.target == target && r.target_mode == TargetMode::External)
        {
            return r.id.clone();
        }
        rels.add(rel_types::HYPERLINK, target, TargetMode::External)
    });
    pkg.set_relationships(Some(&main), &rels);
    let ct = pkg.content_type(&main).map(str::to_owned);
    pkg.set_part(main.clone(), xml.into_bytes(), ct.as_deref());
    ensure_numbering(pkg, &main, doc)?;
    ensure_styles(pkg, &main, doc)?;
    Ok(())
}
