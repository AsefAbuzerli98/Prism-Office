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
    if let Some(r) = m.rtl {
        add("rtl", on_off("rtl", r));
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
    if f.bidi {
        add("bidi", "<w:bidi/>".into());
    }
    if !f.tabs.is_empty() {
        let mut t = String::from("<w:tabs>");
        for tab in &f.tabs {
            let val = match tab.align {
                TabAlign::Left => "left",
                TabAlign::Center => "center",
                TabAlign::Right => "right",
                TabAlign::Decimal => "decimal",
                TabAlign::Bar => "bar",
                TabAlign::Clear => "clear",
            };
            let leader = match tab.leader {
                Some('.') => r#" w:leader="dot""#,
                Some('-') => r#" w:leader="hyphen""#,
                Some('_') => r#" w:leader="underscore""#,
                Some('·') => r#" w:leader="middleDot""#,
                Some('━') => r#" w:leader="heavy""#,
                _ => "",
            };
            t.push_str(&format!(
                r#"<w:tab w:val="{val}"{leader} w:pos="{}"/>"#,
                tab.pos
            ));
        }
        t.push_str("</w:tabs>");
        add("tabs", t);
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

fn text_run(text: &str, marks: &Marks, deleted: bool) -> String {
    let rpr = rpr_xml(marks);
    let tag = if deleted { "w:delText" } else { "w:t" };
    let mut body = String::new();
    let mut buf = String::new();
    let flush = |buf: &mut String, body: &mut String| {
        if !buf.is_empty() {
            body.push_str(&format!(
                r#"<{tag} xml:space="preserve">{}</{tag}>"#,
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

const WP_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const PIC_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";

/// Set (or add) attribute `attr` on the first tag whose name ends with `tag`.
fn set_attr(xml: &str, tag: &str, attr: &str, value: &str) -> String {
    let Some(start) = xml
        .find(&format!(":{tag} "))
        .and_then(|i| xml[..i].rfind('<'))
    else {
        return xml.to_string();
    };
    let end = start + xml[start..].find('>').unwrap_or(0);
    let tag_src = &xml[start..end];
    let needle = format!(" {attr}=\"");
    let new_tag = match tag_src.find(&needle) {
        Some(i) => {
            let vstart = i + needle.len();
            let vend = vstart + tag_src[vstart..].find('"').unwrap_or(0);
            format!(
                "{}{}{}",
                &tag_src[..vstart],
                escape_attr(value),
                &tag_src[vend..]
            )
        }
        None => {
            let insert = if tag_src.ends_with('/') {
                tag_src.len() - 1
            } else {
                tag_src.len()
            };
            format!(
                "{} {attr}=\"{}\"{}",
                tag_src[..insert].trim_end(),
                escape_attr(value),
                &tag_src[insert..]
            )
        }
    };
    format!("{}{}{}", &xml[..start], new_tag, &xml[end..])
}

static DOC_PR_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(5000);

fn image_xml(rel_id: &str, cx: u64, cy: u64, descr: &str, xml: &str) -> String {
    let rel_id = rel_id.split_once('#').map_or(rel_id, |(_, id)| id);
    if !xml.is_empty() {
        let mut x = set_attr(xml, "docPr", "descr", descr);
        x = set_attr(&x, "extent", "cx", &cx.to_string());
        x = set_attr(&x, "extent", "cy", &cy.to_string());
        if let Some(i) = x.find(":xfrm") {
            let (head, tail) = x.split_at(i);
            let tail = set_attr(
                &set_attr(tail, "ext", "cx", &cx.to_string()),
                "ext",
                "cy",
                &cy.to_string(),
            );
            x = format!("{head}{tail}");
        }
        return x;
    }
    let id = DOC_PR_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!(
        r#"<w:r><w:drawing><wp:inline xmlns:wp="{WP_NS}" distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{id}" name="Picture {id}" descr="{d}"/><wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a="{A_NS}" noChangeAspect="1"/></wp:cNvGraphicFramePr><a:graphic xmlns:a="{A_NS}"><a:graphicData uri="{PIC_NS}"><pic:pic xmlns:pic="{PIC_NS}"><pic:nvPicPr><pic:cNvPr id="0" name="Picture {id}" descr="{d}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{rel_id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#,
        d = escape_attr(descr)
    )
}

const WPS_NS: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingShape";

fn shape_xml(
    preset: &str,
    cx: u64,
    cy: u64,
    stroke: &str,
    fill: Option<&str>,
    xml: &str,
) -> String {
    if !xml.is_empty() {
        return xml.to_string();
    }
    let id = DOC_PR_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let fill = match fill {
        Some(f) => format!(
            r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#,
            escape_attr(f)
        ),
        None => "<a:noFill/>".into(),
    };
    let stroke = if stroke.is_empty() { "1F2937" } else { stroke };
    format!(
        r#"<w:r><w:drawing><wp:inline xmlns:wp="{WP_NS}" distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{id}" name="Shape {id}"/><a:graphic xmlns:a="{A_NS}"><a:graphicData uri="{WPS_NS}"><wps:wsp xmlns:wps="{WPS_NS}"><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="{p}"><a:avLst/></a:prstGeom>{fill}<a:ln w="19050"><a:solidFill><a:srgbClr val="{s}"/></a:solidFill></a:ln></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#,
        p = escape_attr(preset),
        s = escape_attr(stroke)
    )
}

fn inline_xml(i: &Inline, deleted: bool) -> String {
    match i {
        Inline::Text { text, marks } => text_run(text, marks, deleted),
        Inline::Tab { marks } => format!("<w:r>{}<w:tab/></w:r>", rpr_xml(marks)),
        Inline::Break { kind } => match kind {
            BreakKind::Line => "<w:r><w:br/></w:r>".into(),
            BreakKind::Page => r#"<w:r><w:br w:type="page"/></w:r>"#.into(),
            BreakKind::Column => r#"<w:r><w:br w:type="column"/></w:r>"#.into(),
        },
        Inline::Image {
            rel_id,
            cx,
            cy,
            descr,
            xml,
        } => image_xml(rel_id, *cx, *cy, descr, xml),
        Inline::Shape {
            preset,
            cx,
            cy,
            stroke,
            fill,
            xml,
        } => shape_xml(preset, *cx, *cy, stroke, fill.as_deref(), xml),
        Inline::Note { kind, id, xml } => {
            if !xml.is_empty() {
                return xml.clone();
            }
            let (style, el) = match kind {
                NoteKind::Footnote => ("FootnoteReference", "footnoteReference"),
                NoteKind::Endnote => ("EndnoteReference", "endnoteReference"),
            };
            format!(
                r#"<w:r><w:rPr><w:rStyle w:val="{style}"/><w:vertAlign w:val="superscript"/></w:rPr><w:{el} w:id="{id}"/></w:r>"#
            )
        }
        Inline::Comment { id, edge } => {
            let id = escape_attr(id);
            match edge {
                CommentEdge::Start => format!(r#"<w:commentRangeStart w:id="{id}"/>"#),
                CommentEdge::End => format!(r#"<w:commentRangeEnd w:id="{id}"/>"#),
                CommentEdge::Reference => format!(
                    r#"<w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:commentReference w:id="{id}"/></w:r>"#
                ),
            }
        }
        Inline::Opaque { xml, .. } => xml.clone(),
    }
}

fn marks_of(i: &Inline) -> Option<&Marks> {
    match i {
        Inline::Text { marks, .. } | Inline::Tab { marks } => Some(marks),
        _ => None,
    }
}

fn link_of(i: &Inline) -> Option<&str> {
    marks_of(i).and_then(|m| m.link.as_deref())
}

fn revision_of(i: &Inline) -> Option<&Revision> {
    marks_of(i).and_then(|m| m.revision.as_ref())
}

fn strip(i: &Inline, link: bool, revision: bool) -> Inline {
    let mut i = i.clone();
    if let Inline::Text { marks, .. } | Inline::Tab { marks } = &mut i {
        if link {
            marks.link = None;
        }
        if revision {
            marks.revision = None;
        }
    }
    i
}

/// Inlines grouped into `w:ins`/`w:del` wrappers by revision.
fn revision_groups(inlines: &[Inline]) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i < inlines.len() {
        match revision_of(&inlines[i]).cloned() {
            None => {
                s.push_str(&inline_xml(&inlines[i], false));
                i += 1;
            }
            Some(rev) => {
                let mut j = i;
                let mut inner = String::new();
                let deleted = rev.kind == RevisionKind::Delete;
                while j < inlines.len() && revision_of(&inlines[j]) == Some(&rev) {
                    inner.push_str(&inline_xml(&strip(&inlines[j], false, true), deleted));
                    j += 1;
                }
                let tag = if deleted { "w:del" } else { "w:ins" };
                let date = rev
                    .date
                    .as_ref()
                    .map(|d| format!(r#" w:date="{}""#, escape_attr(d)))
                    .unwrap_or_default();
                s.push_str(&format!(
                    r#"<{tag} w:id="{}" w:author="{}"{date}>{inner}</{tag}>"#,
                    escape_attr(&rev.id),
                    escape_attr(&rev.author)
                ));
                i = j;
            }
        }
    }
    s
}

fn paragraph_xml(p: &Paragraph, link_id: &mut dyn FnMut(&str) -> String) -> String {
    let ppr = ppr_xml(p);
    if ppr.is_empty() && p.inlines.is_empty() {
        return "<w:p/>".into();
    }
    let mut s = format!("<w:p>{ppr}");
    let mut i = 0;
    while i < p.inlines.len() {
        match link_of(&p.inlines[i]) {
            None => {
                let mut j = i;
                while j < p.inlines.len() && link_of(&p.inlines[j]).is_none() {
                    j += 1;
                }
                s.push_str(&revision_groups(&p.inlines[i..j]));
                i = j;
            }
            Some(target) => {
                let mut j = i;
                let mut inner = Vec::new();
                while j < p.inlines.len() && link_of(&p.inlines[j]) == Some(target) {
                    inner.push(strip(&p.inlines[j], true, false));
                    j += 1;
                }
                let open = match target.strip_prefix('#') {
                    Some(anchor) => format!(r#"<w:hyperlink w:anchor="{}">"#, escape_attr(anchor)),
                    None => format!(r#"<w:hyperlink r:id="{}">"#, link_id(target)),
                };
                s.push_str(&open);
                s.push_str(&revision_groups(&inner));
                s.push_str("</w:hyperlink>");
                i = j;
            }
        }
    }
    s.push_str("</w:p>");
    s
}

const TCPR: &[&str] = &[
    "cnfStyle",
    "tcW",
    "gridSpan",
    "hMerge",
    "vMerge",
    "tcBorders",
    "shd",
    "noWrap",
    "tcMar",
    "textDirection",
    "tcFitText",
    "vAlign",
    "hideMark",
    "headers",
    "cellIns",
    "cellDel",
    "cellMerge",
    "tcPrChange",
];

const NEW_TABLE_PR: &str = r#"<w:tblPr><w:tblW w:w="0" w:type="auto"/><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:right w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="auto"/></w:tblBorders><w:tblLayout w:type="fixed"/><w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/></w:tblPr>"#;

fn table_xml(t: &Table, link_id: &mut dyn FnMut(&str) -> String) -> String {
    let mut s = String::from("<w:tbl>");
    match &t.props {
        Some(p) => s.push_str(p),
        None if t.borders => s.push_str(NEW_TABLE_PR),
        None => s.push_str(
            r#"<w:tblPr><w:tblW w:w="0" w:type="auto"/><w:tblLayout w:type="fixed"/></w:tblPr>"#,
        ),
    }
    s.push_str("<w:tblGrid>");
    for w in &t.grid {
        s.push_str(&format!(r#"<w:gridCol w:w="{w}"/>"#));
    }
    s.push_str("</w:tblGrid>");
    for row in &t.rows {
        s.push_str("<w:tr>");
        let mut trpr = row.props.clone().unwrap_or_default();
        if row.header {
            trpr = if trpr.is_empty() {
                "<w:trPr><w:tblHeader/></w:trPr>".into()
            } else {
                trpr.replacen("</w:trPr>", "<w:tblHeader/></w:trPr>", 1)
            };
        }
        s.push_str(&trpr);
        for c in &row.cells {
            let mut parts: Vec<(String, String)> = c
                .extra
                .iter()
                .map(|e| (e.name.clone(), e.xml.clone()))
                .collect();
            if let Some(w) = c.width {
                parts.push(("tcW".into(), format!(r#"<w:tcW w:w="{w}" w:type="dxa"/>"#)));
            }
            if c.span > 1 {
                parts.push((
                    "gridSpan".into(),
                    format!(r#"<w:gridSpan w:val="{}"/>"#, c.span),
                ));
            }
            match c.v_merge {
                Some(VMerge::Restart) => {
                    parts.push(("vMerge".into(), r#"<w:vMerge w:val="restart"/>"#.into()))
                }
                Some(VMerge::Continue) => parts.push(("vMerge".into(), "<w:vMerge/>".into())),
                None => {}
            }
            if let Some(fill) = &c.shading {
                parts.push((
                    "shd".into(),
                    format!(
                        r#"<w:shd w:val="clear" w:color="auto" w:fill="{}"/>"#,
                        escape_attr(fill)
                    ),
                ));
            }
            s.push_str("<w:tc>");
            if !parts.is_empty() {
                s.push_str(&format!("<w:tcPr>{}</w:tcPr>", sorted(TCPR, parts)));
            }
            let mut blocks = c.blocks.clone();
            if !matches!(blocks.last(), Some(Block::Paragraph(_))) {
                blocks.push(Block::Paragraph(Paragraph::default()));
            }
            s.push_str(&blocks_xml(&blocks, link_id));
            s.push_str("</w:tc>");
        }
        s.push_str("</w:tr>");
    }
    s.push_str("</w:tbl>");
    s
}

/// Serialise block content (body, cells, notes, comments, headers).
pub(crate) fn blocks_xml(blocks: &[Block], link_id: &mut dyn FnMut(&str) -> String) -> String {
    let mut s = String::new();
    for b in blocks {
        match b {
            Block::Paragraph(p) => s.push_str(&paragraph_xml(p, link_id)),
            Block::Table(t) => s.push_str(&table_xml(t, link_id)),
            Block::Opaque { xml, .. } => s.push_str(xml),
        }
    }
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
    if let Some(c) = &sec.columns {
        let sep = if c.separator { r#" w:sep="1""# } else { "" };
        parts.push((
            "cols".into(),
            format!(
                r#"<w:cols w:num="{}" w:space="{}"{sep}/>"#,
                c.count, c.space
            ),
        ));
    }
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
    s.push_str(&blocks_xml(&doc.blocks, link_id));
    // A body must end with a paragraph before sectPr if the last block is a table.
    if matches!(
        doc.blocks.last(),
        Some(Block::Table(_)) | Some(Block::Opaque { .. })
    ) || doc.blocks.is_empty()
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

/// Every paragraph in the document, including table cells, notes, comments and headers.
fn all_paragraphs(doc: &WordDocument) -> Vec<&Paragraph> {
    fn walk<'a>(blocks: &'a [Block], out: &mut Vec<&'a Paragraph>) {
        for b in blocks {
            match b {
                Block::Paragraph(p) => out.push(p),
                Block::Table(t) => t
                    .rows
                    .iter()
                    .flat_map(|r| &r.cells)
                    .for_each(|c| walk(&c.blocks, out)),
                Block::Opaque { .. } => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&doc.blocks, &mut out);
    doc.notes.iter().for_each(|n| walk(&n.blocks, &mut out));
    doc.comments.iter().for_each(|c| walk(&c.blocks, &mut out));
    doc.headers.iter().for_each(|h| walk(&h.blocks, &mut out));
    out
}

fn ensure_numbering(pkg: &mut Package, main: &PartName, doc: &WordDocument) -> Result<()> {
    let mut used: Vec<u32> = all_paragraphs(doc)
        .into_iter()
        .filter_map(|p| p.list.map(|l| l.num_id))
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
        let used = all_paragraphs(doc)
            .iter()
            .any(|p| p.style.as_deref() == Some(id));
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
    crate::parts::write_parts(pkg, &main, doc)?;
    Ok(())
}
