//! DOCX → model.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use prism_openxml::xml::{ref_text, text_of};
use prism_openxml::{rel_types, Package, PartName, Relationships, TargetMode};

use crate::model::*;
use crate::raw::{attr, capture, local, on_off, qname, skip};
use crate::{numbering, styles, Error, Result};

pub(crate) const NUMBERING_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";

fn parse_u32(e: &BytesStart<'_>, name: &str) -> Option<u32> {
    attr(e, name).and_then(|v| v.parse().ok())
}
fn parse_i32(e: &BytesStart<'_>, name: &str) -> Option<i32> {
    attr(e, name).and_then(|v| v.parse().ok())
}

/// Parse the children of a `w:rPr` element (reader positioned after its start).
pub(crate) fn parse_rpr(reader: &mut Reader<&[u8]>, empty: bool) -> Result<Marks> {
    let mut m = Marks::default();
    if empty {
        return Ok(m);
    }
    loop {
        let (e, is_empty) = match reader.read_event().map_err(Error::xml)? {
            Event::Start(e) => (e.into_owned(), false),
            Event::Empty(e) => (e.into_owned(), true),
            Event::End(_) => return Ok(m),
            Event::Eof => return Err(Error::xml("EOF in rPr")),
            _ => continue,
        };
        let mut keep_raw = false;
        match local(&e).as_str() {
            "rStyle" => m.style = attr(&e, "val"),
            "rFonts" => match attr(&e, "ascii").or_else(|| attr(&e, "hAnsi")) {
                Some(f) => m.font = Some(f),
                None => keep_raw = true,
            },
            "b" => m.bold = Some(on_off(&e)),
            "i" => m.italic = Some(on_off(&e)),
            "strike" => m.strike = Some(on_off(&e)),
            "u" => match attr(&e, "val").as_deref() {
                Some("single") if prism_openxml::xml::attrs_of(&e).len() == 1 => {
                    m.underline = Some(true)
                }
                Some("none") => m.underline = Some(false),
                _ => keep_raw = true,
            },
            "sz" => m.size = parse_u32(&e, "val"),
            "color" => match attr(&e, "val") {
                Some(c) if c != "auto" && prism_openxml::xml::attrs_of(&e).len() == 1 => {
                    m.color = Some(c)
                }
                _ => keep_raw = true,
            },
            "highlight" => m.highlight = attr(&e, "val"),
            "rtl" => m.rtl = Some(on_off(&e)),
            "vertAlign" => match attr(&e, "val").as_deref() {
                Some("superscript") => m.vert_align = Some(VertAlign::Superscript),
                Some("subscript") => m.vert_align = Some(VertAlign::Subscript),
                _ => keep_raw = true,
            },
            _ => keep_raw = true,
        }
        if keep_raw {
            let xml = capture(reader, &e, is_empty)?;
            m.extra.push(RawXml {
                name: qname(&e),
                xml,
            });
        } else {
            skip(reader, is_empty)?;
        }
    }
}

/// (style id, formatting, list reference, preserved unknown children)
pub(crate) type ParsedPpr = (Option<String>, ParaFormat, Option<ListRef>, Vec<RawXml>);

/// Parse a `w:pPr` (reader positioned after its start).
pub(crate) fn parse_ppr(reader: &mut Reader<&[u8]>, empty: bool) -> Result<ParsedPpr> {
    let mut style = None;
    let mut f = ParaFormat::default();
    let mut list = None;
    let mut extra = Vec::new();
    if empty {
        return Ok((style, f, list, extra));
    }
    loop {
        let (e, is_empty) = match reader.read_event().map_err(Error::xml)? {
            Event::Start(e) => (e.into_owned(), false),
            Event::Empty(e) => (e.into_owned(), true),
            Event::End(_) => return Ok((style, f, list, extra)),
            Event::Eof => return Err(Error::xml("EOF in pPr")),
            _ => continue,
        };
        let attrs = prism_openxml::xml::attrs_of(&e);
        let only = |allowed: &[&str]| attrs.iter().all(|(k, _)| allowed.contains(&k.as_str()));
        let mut keep_raw = false;
        match local(&e).as_str() {
            "pStyle" => style = attr(&e, "val"),
            "keepNext" => f.keep_next = on_off(&e),
            "pageBreakBefore" => f.page_break_before = on_off(&e),
            "jc" => match attr(&e, "val").as_deref().and_then(Align::from_ooxml) {
                Some(a) => f.align = Some(a),
                None => keep_raw = true,
            },
            "outlineLvl" => f.outline_level = parse_u32(&e, "val").map(|v| v as u8),
            "bidi" => f.bidi = on_off(&e),
            "tabs" if !is_empty => {
                let xml = capture(reader, &e, false)?;
                match parse_tabs(&xml) {
                    Some(t) => f.tabs = t,
                    None => extra.push(RawXml {
                        name: qname(&e),
                        xml,
                    }),
                }
                continue;
            }
            "spacing"
                if only(&["before", "after", "line", "lineRule"])
                    && matches!(attr(&e, "lineRule").as_deref(), None | Some("auto")) =>
            {
                f.space_before = parse_u32(&e, "before");
                f.space_after = parse_u32(&e, "after");
                f.line = parse_u32(&e, "line");
            }
            "ind" if only(&["left", "start", "right", "end", "firstLine", "hanging"]) => {
                f.indent_left = parse_i32(&e, "left").or_else(|| parse_i32(&e, "start"));
                f.indent_right = parse_i32(&e, "right").or_else(|| parse_i32(&e, "end"));
                f.indent_first =
                    parse_i32(&e, "firstLine").or_else(|| parse_i32(&e, "hanging").map(|h| -h));
            }
            "numPr" if !is_empty => {
                let xml = capture(reader, &e, false)?;
                match parse_num_pr(&xml) {
                    Some(l) => list = Some(l),
                    None => extra.push(RawXml {
                        name: qname(&e),
                        xml,
                    }),
                }
                continue;
            }
            _ => keep_raw = true,
        }
        if keep_raw {
            let xml = capture(reader, &e, is_empty)?;
            extra.push(RawXml {
                name: qname(&e),
                xml,
            });
        } else {
            skip(reader, is_empty)?;
        }
    }
}

fn parse_tabs(xml: &str) -> Option<Vec<TabStop>> {
    let mut r = Reader::from_str(xml);
    let mut out = Vec::new();
    loop {
        match r.read_event().ok()? {
            Event::Start(e) | Event::Empty(e) if local(&e) == "tab" => {
                let attrs = prism_openxml::xml::attrs_of(&e);
                if attrs
                    .iter()
                    .any(|(k, _)| !["val", "pos", "leader"].contains(&k.as_str()))
                {
                    return None;
                }
                let align = match attr(&e, "val")?.as_str() {
                    "left" | "start" => TabAlign::Left,
                    "center" => TabAlign::Center,
                    "right" | "end" => TabAlign::Right,
                    "decimal" => TabAlign::Decimal,
                    "bar" => TabAlign::Bar,
                    "clear" => TabAlign::Clear,
                    _ => return None,
                };
                let leader = match attr(&e, "leader").as_deref() {
                    None | Some("none") => None,
                    Some("dot") => Some('.'),
                    Some("hyphen") => Some('-'),
                    Some("underscore") => Some('_'),
                    Some("middleDot") => Some('·'),
                    Some("heavy") => Some('━'),
                    Some(_) => return None,
                };
                out.push(TabStop {
                    pos: attr(&e, "pos")?.parse().ok()?,
                    align,
                    leader,
                });
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Some(out)
}

fn parse_num_pr(xml: &str) -> Option<ListRef> {
    let mut r = Reader::from_str(xml);
    let (mut level, mut num) = (0u8, None);
    loop {
        match r.read_event().ok()? {
            Event::Start(e) | Event::Empty(e) => match local(&e).as_str() {
                "ilvl" => level = attr(&e, "val")?.parse().ok()?,
                "numId" => num = attr(&e, "val")?.parse::<u32>().ok(),
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    num.filter(|&n| n != 0)
        .map(|num_id| ListRef { num_id, level })
}

pub(crate) struct Ctx<'a> {
    pub rels: &'a Relationships,
}

#[derive(Clone, Default)]
struct RunCtx {
    link: Option<String>,
    revision: Option<Revision>,
}

fn label_for(name: &str) -> &'static str {
    match name {
        "drawing" | "pict" => "Image",
        "object" => "Object",
        "tbl" => "Table",
        "sdt" => "Content control",
        "fldSimple" => "Field",
        "oMath" | "oMathPara" => "Equation",
        "sym" => "Symbol",
        "smartTag" | "customXml" => "Tagged text",
        "moveFrom" | "moveTo" => "Moved text",
        _ => "Element",
    }
}

/// Elements that never render anything.
fn invisible(name: &str) -> bool {
    matches!(
        name,
        "bookmarkStart"
            | "bookmarkEnd"
            | "permStart"
            | "permEnd"
            | "lastRenderedPageBreak"
            | "moveFromRangeStart"
            | "moveFromRangeEnd"
            | "moveToRangeStart"
            | "moveToRangeEnd"
            | "fldChar"
            | "instrText"
            | "footnoteRef"
            | "endnoteRef"
            | "separator"
            | "continuationSeparator"
            | "annotationRef"
            | "delInstrText"
    )
}

fn push_text(inlines: &mut Vec<Inline>, text: &str, marks: &Marks) {
    if text.is_empty() {
        return;
    }
    if let Some(Inline::Text { text: t, marks: m }) = inlines.last_mut() {
        if m == marks {
            t.push_str(text);
            return;
        }
    }
    inlines.push(Inline::Text {
        text: text.to_string(),
        marks: marks.clone(),
    });
}

/// Inline picture details from a run containing `w:drawing/wp:inline`.
pub(crate) fn image_from_run(raw: &str) -> Option<(String, u64, u64, String)> {
    let mut r = Reader::from_str(raw);
    let (mut inline, mut embed, mut cx, mut cy, mut descr, mut blips) =
        (false, None, 0u64, 0u64, String::new(), 0);
    loop {
        match r.read_event().ok()? {
            Event::Start(e) | Event::Empty(e) => match local(&e).as_str() {
                "inline" => inline = true,
                "anchor" => return None,
                "extent" if cx == 0 => {
                    cx = attr(&e, "cx")?.parse().ok()?;
                    cy = attr(&e, "cy")?.parse().ok()?;
                }
                "docPr" => descr = attr(&e, "descr").unwrap_or_default(),
                "blip" => {
                    blips += 1;
                    embed = attr(&e, "embed");
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    (inline && blips == 1).then_some(())?;
    Some((embed?, cx, cy, descr))
}

/// Inline preset shape from a run containing `wp:inline/…/wps:wsp`.
fn shape_from_run(raw: &str) -> Option<(String, u64, u64, String, Option<String>)> {
    let mut r = Reader::from_str(raw);
    let (mut inline, mut wsp, mut preset, mut cx, mut cy) = (false, false, None, 0u64, 0u64);
    let (mut stroke, mut fill) = (String::new(), None);
    let mut in_ln = 0;
    loop {
        match r.read_event().ok()? {
            Event::Start(e) => match local(&e).as_str() {
                "inline" => inline = true,
                "anchor" => return None,
                "wsp" => wsp = true,
                "ln" => in_ln += 1,
                "prstGeom" => preset = attr(&e, "prst"),
                _ => {}
            },
            Event::Empty(e) => match local(&e).as_str() {
                "extent" if cx == 0 => {
                    cx = attr(&e, "cx")?.parse().ok()?;
                    cy = attr(&e, "cy")?.parse().ok()?;
                }
                "srgbClr" if in_ln > 0 => stroke = attr(&e, "val").unwrap_or_default(),
                "srgbClr" if fill.is_none() => fill = attr(&e, "val"),
                _ => {}
            },
            Event::End(e) if e.local_name().into_inner() == "ln" => in_ln -= 1,
            Event::Eof => break,
            _ => {}
        }
    }
    (inline && wsp).then_some(())?;
    Some((preset?, cx, cy, stroke, fill))
}

/// Parse a `w:r`. Runs with text/tab/break become text inlines; recognised
/// special runs (pictures, note references, field codes) map to dedicated
/// inlines; anything else is preserved as one opaque inline.
fn parse_run(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart<'_>,
    empty: bool,
    rc: &RunCtx,
    out: &mut Vec<Inline>,
) -> Result<()> {
    if empty {
        return Ok(());
    }
    let raw = capture(reader, start, false)?;
    let mut r = Reader::from_str(&raw);
    r.read_event().map_err(Error::xml)?; // <w:r>
    let mut marks = Marks {
        link: rc.link.clone(),
        revision: rc.revision.clone(),
        ..Default::default()
    };
    let mut items: Vec<Inline> = Vec::new();
    let mut opaque: Option<&'static str> = None;
    let mut special: Option<Inline> = None;
    let mut hidden_only = true;
    loop {
        let (e, is_empty) = match r.read_event().map_err(Error::xml)? {
            Event::Start(e) => (e.into_owned(), false),
            Event::Empty(e) => (e.into_owned(), true),
            Event::End(_) | Event::Eof => break,
            _ => continue,
        };
        let name = local(&e);
        if name != "rPr" && !invisible(&name) {
            hidden_only = false;
        }
        match name.as_str() {
            "rPr" => {
                let m = parse_rpr(&mut r, is_empty)?;
                marks = Marks {
                    link: rc.link.clone(),
                    revision: rc.revision.clone(),
                    ..m
                };
            }
            "t" | "delText" if !is_empty => {
                let mut s = String::new();
                loop {
                    match r.read_event().map_err(Error::xml)? {
                        Event::Text(t) => s.push_str(&text_of(&t)),
                        Event::GeneralRef(g) => s.push_str(&ref_text(&g)),
                        Event::CData(c) => s.push_str(&c.into_inner()),
                        Event::End(_) => break,
                        Event::Eof => return Err(Error::xml("EOF in w:t")),
                        _ => {}
                    }
                }
                push_text(&mut items, &s, &marks);
            }
            "t" | "delText" => {}
            "tab" => {
                items.push(Inline::Tab {
                    marks: marks.clone(),
                });
                skip(&mut r, is_empty)?;
            }
            "br" => {
                let kind = match attr(&e, "type").as_deref() {
                    Some("page") => BreakKind::Page,
                    Some("column") => BreakKind::Column,
                    _ => BreakKind::Line,
                };
                if attr(&e, "clear").is_some() {
                    opaque = Some("Element");
                }
                items.push(Inline::Break { kind });
                skip(&mut r, is_empty)?;
            }
            "cr" => {
                items.push(Inline::Break {
                    kind: BreakKind::Line,
                });
                skip(&mut r, is_empty)?;
            }
            "noBreakHyphen" => push_text(&mut items, "\u{2011}", &marks),
            "softHyphen" => push_text(&mut items, "\u{00AD}", &marks),
            "lastRenderedPageBreak" => skip(&mut r, is_empty)?,
            "footnoteReference" | "endnoteReference" => {
                let kind = if name == "footnoteReference" {
                    NoteKind::Footnote
                } else {
                    NoteKind::Endnote
                };
                match attr(&e, "id").and_then(|i| i.parse().ok()) {
                    Some(id) => {
                        special = Some(Inline::Note {
                            kind,
                            id,
                            xml: String::new(),
                        })
                    }
                    None => opaque = Some("Element"),
                }
                skip(&mut r, is_empty)?;
            }
            "commentReference" => {
                match attr(&e, "id") {
                    Some(id) => {
                        special = Some(Inline::Comment {
                            id,
                            edge: CommentEdge::Reference,
                        })
                    }
                    None => opaque = Some("Element"),
                }
                skip(&mut r, is_empty)?;
            }
            "drawing" => {
                opaque = Some("Image");
                skip(&mut r, is_empty)?;
            }
            n if invisible(n) => skip(&mut r, is_empty)?,
            other => {
                opaque = Some(label_for(other));
                skip(&mut r, is_empty)?;
            }
        }
    }
    let plain = items.is_empty();
    if opaque == Some("Image") && plain && special.is_none() && rc.revision.is_none() {
        if let Some((rel_id, cx, cy, descr)) = image_from_run(&raw) {
            out.push(Inline::Image {
                rel_id,
                cx,
                cy,
                descr,
                xml: raw,
            });
            return Ok(());
        }
        if let Some((preset, cx, cy, stroke, fill)) = shape_from_run(&raw) {
            out.push(Inline::Shape {
                preset,
                cx,
                cy,
                stroke,
                fill,
                xml: raw,
            });
            return Ok(());
        }
    }
    if let Some(sp) = special.filter(|_| opaque.is_none() && plain) {
        out.push(match sp {
            Inline::Note { kind, id, .. } => Inline::Note { kind, id, xml: raw },
            other => other,
        });
        return Ok(());
    }
    if let Some(label) = opaque {
        out.push(Inline::Opaque {
            label: label.to_string(),
            xml: raw,
        });
    } else if hidden_only && plain {
        out.push(Inline::Opaque {
            label: String::new(),
            xml: raw,
        });
    } else {
        for it in items {
            match it {
                Inline::Text { text, marks } => push_text(out, &text, &marks),
                other => out.push(other),
            }
        }
    }
    Ok(())
}

fn parse_paragraph(reader: &mut Reader<&[u8]>, empty: bool, ctx: &Ctx<'_>) -> Result<Paragraph> {
    let mut p = Paragraph::default();
    if empty {
        return Ok(p);
    }
    parse_paragraph_children(reader, ctx, &mut p, &RunCtx::default(), true)?;
    Ok(p)
}

fn revision_of(e: &BytesStart<'_>, kind: RevisionKind) -> Option<Revision> {
    let attrs = prism_openxml::xml::attrs_of(e);
    if attrs
        .iter()
        .any(|(k, _)| !["id", "author", "date"].contains(&k.as_str()))
    {
        return None;
    }
    Some(Revision {
        kind,
        id: attr(e, "id")?,
        author: attr(e, "author").unwrap_or_default(),
        date: attr(e, "date"),
    })
}

/// Shared by `w:p`, `w:hyperlink`, `w:ins` and `w:del`.
fn parse_paragraph_children(
    reader: &mut Reader<&[u8]>,
    ctx: &Ctx<'_>,
    p: &mut Paragraph,
    rc: &RunCtx,
    top: bool,
) -> Result<()> {
    loop {
        let (e, is_empty) = match reader.read_event().map_err(Error::xml)? {
            Event::Start(e) => (e.into_owned(), false),
            Event::Empty(e) => (e.into_owned(), true),
            Event::End(_) => return Ok(()),
            Event::Eof => return Err(Error::xml("EOF in paragraph")),
            _ => continue,
        };
        let name = local(&e);
        match name.as_str() {
            "pPr" if top => {
                let (style, format, list, extra) = parse_ppr(reader, is_empty)?;
                p.style = style;
                p.format = format;
                p.list = list;
                p.extra = extra;
            }
            "r" => parse_run(reader, &e, is_empty, rc, &mut p.inlines)?,
            "proofErr" => skip(reader, is_empty)?,
            "hyperlink" if rc.link.is_none() && rc.revision.is_none() && !is_empty => {
                let attrs = prism_openxml::xml::attrs_of(&e);
                let simple = attrs
                    .iter()
                    .all(|(k, _)| k == "id" || k == "anchor" || k == "history");
                let target = attr(&e, "id")
                    .and_then(|id| ctx.rels.by_id(&id).map(|r| r.target.clone()))
                    .or_else(|| attr(&e, "anchor").map(|a| format!("#{a}")));
                match (simple, target) {
                    (true, Some(t)) => {
                        let sub = RunCtx {
                            link: Some(t),
                            ..rc.clone()
                        };
                        parse_paragraph_children(reader, ctx, p, &sub, false)?
                    }
                    _ => {
                        let xml = capture(reader, &e, false)?;
                        p.inlines.push(Inline::Opaque {
                            label: "Link".into(),
                            xml,
                        });
                    }
                }
            }
            "ins" | "del" if rc.revision.is_none() && !is_empty => {
                let kind = if name == "ins" {
                    RevisionKind::Insert
                } else {
                    RevisionKind::Delete
                };
                match revision_of(&e, kind) {
                    Some(rev) => {
                        let sub = RunCtx {
                            revision: Some(rev),
                            ..rc.clone()
                        };
                        parse_paragraph_children(reader, ctx, p, &sub, false)?
                    }
                    None => {
                        let xml = capture(reader, &e, false)?;
                        p.inlines.push(Inline::Opaque {
                            label: "Tracked change".into(),
                            xml,
                        });
                    }
                }
            }
            "commentRangeStart" | "commentRangeEnd"
                if attr(&e, "id").is_some() && prism_openxml::xml::attrs_of(&e).len() == 1 =>
            {
                let edge = if name == "commentRangeStart" {
                    CommentEdge::Start
                } else {
                    CommentEdge::End
                };
                p.inlines.push(Inline::Comment {
                    id: attr(&e, "id").unwrap(),
                    edge,
                });
                skip(reader, is_empty)?;
            }
            n if invisible(n) => {
                let xml = capture(reader, &e, is_empty)?;
                p.inlines.push(Inline::Opaque {
                    label: String::new(),
                    xml,
                });
            }
            other => {
                let label = label_for(other).to_string();
                let xml = capture(reader, &e, is_empty)?;
                p.inlines.push(Inline::Opaque { label, xml });
            }
        }
    }
}

fn parse_sect_pr(reader: &mut Reader<&[u8]>, empty: bool) -> Result<Section> {
    let mut s = Section::default();
    if empty {
        return Ok(s);
    }
    loop {
        let (e, is_empty) = match reader.read_event().map_err(Error::xml)? {
            Event::Start(e) => (e.into_owned(), false),
            Event::Empty(e) => (e.into_owned(), true),
            Event::End(_) => return Ok(s),
            Event::Eof => return Err(Error::xml("EOF in sectPr")),
            _ => continue,
        };
        let attrs = prism_openxml::xml::attrs_of(&e);
        match local(&e).as_str() {
            "pgSz"
                if attrs
                    .iter()
                    .all(|(k, _)| ["w", "h", "orient"].contains(&k.as_str())) =>
            {
                s.page_width = parse_u32(&e, "w").unwrap_or(s.page_width);
                s.page_height = parse_u32(&e, "h").unwrap_or(s.page_height);
                s.landscape = attr(&e, "orient").as_deref() == Some("landscape");
                skip(reader, is_empty)?;
            }
            "pgMar" => {
                s.margin_top = parse_i32(&e, "top").unwrap_or(s.margin_top);
                s.margin_bottom = parse_i32(&e, "bottom").unwrap_or(s.margin_bottom);
                s.margin_left = parse_u32(&e, "left").unwrap_or(s.margin_left);
                s.margin_right = parse_u32(&e, "right").unwrap_or(s.margin_right);
                s.margin_header = parse_u32(&e, "header").unwrap_or(s.margin_header);
                s.margin_footer = parse_u32(&e, "footer").unwrap_or(s.margin_footer);
                if let Some(g) = attr(&e, "gutter").filter(|g| g != "0") {
                    s.extra.push(RawXml {
                        name: "w:gutter".into(),
                        xml: g,
                    });
                }
                skip(reader, is_empty)?;
            }
            "cols"
                if is_empty
                    && attrs.iter().all(|(k, _)| {
                        ["num", "space", "sep", "equalWidth"].contains(&k.as_str())
                    })
                    && attr(&e, "equalWidth").as_deref() != Some("0") =>
            {
                let count = parse_u32(&e, "num").unwrap_or(1).max(1);
                let space = parse_u32(&e, "space").unwrap_or(720);
                let separator = matches!(attr(&e, "sep").as_deref(), Some("1" | "true" | "on"));
                s.columns = Some(Columns {
                    count,
                    space,
                    separator,
                });
            }
            _ => {
                let xml = capture(reader, &e, is_empty)?;
                s.extra.push(RawXml {
                    name: qname(&e),
                    xml,
                });
            }
        }
    }
}

fn parse_table(reader: &mut Reader<&[u8]>, ctx: &Ctx<'_>) -> Result<Table> {
    let mut t = Table::default();
    loop {
        let (e, is_empty) = match reader.read_event().map_err(Error::xml)? {
            Event::Start(e) => (e.into_owned(), false),
            Event::Empty(e) => (e.into_owned(), true),
            Event::End(_) => break,
            Event::Eof => return Err(Error::xml("EOF in table")),
            _ => continue,
        };
        match local(&e).as_str() {
            "tblPr" => {
                let xml = capture(reader, &e, is_empty)?;
                t.borders = table_has_borders(&xml);
                t.props = Some(xml);
            }
            "tblGrid" if !is_empty => loop {
                match reader.read_event().map_err(Error::xml)? {
                    Event::Start(g) | Event::Empty(g) if local(&g) == "gridCol" => {
                        t.grid.push(parse_u32(&g, "w").unwrap_or(0));
                    }
                    Event::End(g) if g.local_name().into_inner() == "tblGrid" => break,
                    Event::Eof => return Err(Error::xml("EOF in tblGrid")),
                    _ => {}
                }
            },
            "tr" if !is_empty => t.rows.push(parse_row(reader, ctx)?),
            _ => skip(reader, is_empty)?,
        }
    }
    Ok(t)
}

fn table_has_borders(tbl_pr: &str) -> bool {
    if tbl_pr.contains("TableGrid") || tbl_pr.contains("Grid") {
        return true;
    }
    let Some(start) = tbl_pr.find("tblBorders") else {
        return false;
    };
    let section = &tbl_pr[start..];
    section.contains("w:val=\"single\"")
        || section.contains("w:val=\"thick\"")
        || section.contains("w:val=\"double\"")
}

fn parse_row(reader: &mut Reader<&[u8]>, ctx: &Ctx<'_>) -> Result<TableRow> {
    let mut row = TableRow::default();
    loop {
        let (e, is_empty) = match reader.read_event().map_err(Error::xml)? {
            Event::Start(e) => (e.into_owned(), false),
            Event::Empty(e) => (e.into_owned(), true),
            Event::End(_) => break,
            Event::Eof => return Err(Error::xml("EOF in row")),
            _ => continue,
        };
        match local(&e).as_str() {
            "trPr" => {
                let xml = capture(reader, &e, is_empty)?;
                row.header = xml.contains("tblHeader")
                    && !xml.contains("tblHeader w:val=\"0\"")
                    && !xml.contains("tblHeader w:val=\"false\"");
                let stripped = strip_element(&xml, "tblHeader");
                if stripped.matches('<').count() > 2 {
                    row.props = Some(stripped);
                }
            }
            "tc" => row.cells.push(parse_cell(reader, ctx, is_empty)?),
            _ => skip(reader, is_empty)?,
        }
    }
    Ok(row)
}

/// Remove self-closing `<w:{name} .../>` occurrences from a fragment.
fn strip_element(xml: &str, name: &str) -> String {
    let mut out = xml.to_string();
    let needle = format!("<w:{name}");
    while let Some(i) = out.find(&needle) {
        let Some(end) = out[i..].find("/>") else {
            break;
        };
        out.replace_range(i..i + end + 2, "");
    }
    out
}

fn parse_cell(reader: &mut Reader<&[u8]>, ctx: &Ctx<'_>, empty: bool) -> Result<TableCell> {
    let mut cell = TableCell {
        blocks: Vec::new(),
        ..Default::default()
    };
    if empty {
        cell.blocks.push(Block::Paragraph(Paragraph::default()));
        return Ok(cell);
    }
    let mut blocks = Vec::new();
    loop {
        let (e, is_empty) = match reader.read_event().map_err(Error::xml)? {
            Event::Start(e) => (e.into_owned(), false),
            Event::Empty(e) => (e.into_owned(), true),
            Event::End(_) => break,
            Event::Eof => return Err(Error::xml("EOF in cell")),
            _ => continue,
        };
        if local(&e) == "tcPr" {
            if is_empty {
                continue;
            }
            loop {
                let (c, ce) = match reader.read_event().map_err(Error::xml)? {
                    Event::Start(c) => (c.into_owned(), false),
                    Event::Empty(c) => (c.into_owned(), true),
                    Event::End(_) => break,
                    Event::Eof => return Err(Error::xml("EOF in tcPr")),
                    _ => continue,
                };
                let attrs = prism_openxml::xml::attrs_of(&c);
                match local(&c).as_str() {
                    "gridSpan" => {
                        cell.span = parse_u32(&c, "val").unwrap_or(1).max(1);
                        skip(reader, ce)?;
                    }
                    "vMerge" => {
                        cell.v_merge = Some(if attr(&c, "val").as_deref() == Some("restart") {
                            VMerge::Restart
                        } else {
                            VMerge::Continue
                        });
                        skip(reader, ce)?;
                    }
                    "tcW" if attr(&c, "type").as_deref() == Some("dxa") => {
                        cell.width = parse_u32(&c, "w");
                        skip(reader, ce)?;
                    }
                    "shd"
                        if attrs
                            .iter()
                            .all(|(k, _)| ["val", "color", "fill"].contains(&k.as_str()))
                            && attr(&c, "val").as_deref() == Some("clear")
                            && attr(&c, "fill").is_some_and(|f| f != "auto") =>
                    {
                        cell.shading = attr(&c, "fill");
                        skip(reader, ce)?;
                    }
                    _ => {
                        let xml = capture(reader, &c, ce)?;
                        cell.extra.push(RawXml {
                            name: qname(&c),
                            xml,
                        });
                    }
                }
            }
            continue;
        }
        parse_block(reader, &e, is_empty, ctx, &mut blocks, None)?;
    }
    if blocks.is_empty() {
        blocks.push(Block::Paragraph(Paragraph::default()));
    }
    cell.blocks = blocks;
    Ok(cell)
}

/// Parse one block-level element into `out`.
fn parse_block(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart<'_>,
    is_empty: bool,
    ctx: &Ctx<'_>,
    out: &mut Vec<Block>,
    section: Option<&mut Section>,
) -> Result<()> {
    match local(e).as_str() {
        "p" => out.push(Block::Paragraph(parse_paragraph(reader, is_empty, ctx)?)),
        "tbl" if !is_empty => out.push(Block::Table(parse_table(reader, ctx)?)),
        "sectPr" if section.is_some() => *section.unwrap() = parse_sect_pr(reader, is_empty)?,
        other => {
            let label = label_for(other).to_string();
            let xml = capture(reader, e, is_empty)?;
            out.push(Block::Opaque { label, xml });
        }
    }
    Ok(())
}

/// Parse the block children of the current element until its end tag.
pub(crate) fn parse_blocks(reader: &mut Reader<&[u8]>, ctx: &Ctx<'_>) -> Result<Vec<Block>> {
    let mut out = Vec::new();
    loop {
        let (e, is_empty) = match reader.read_event().map_err(Error::xml)? {
            Event::Start(e) => (e.into_owned(), false),
            Event::Empty(e) => (e.into_owned(), true),
            Event::End(_) | Event::Eof => return Ok(out),
            _ => continue,
        };
        parse_block(reader, &e, is_empty, ctx, &mut out, None)?;
    }
}

/// Parse `word/document.xml` body content.
pub fn read_document(xml: &[u8], rels: &Relationships) -> Result<(Vec<Block>, Section)> {
    let text = std::str::from_utf8(xml).map_err(Error::xml)?;
    let mut reader = Reader::from_str(text);
    let ctx = Ctx { rels };
    let mut blocks = Vec::new();
    let mut section = Section::default();
    let mut in_body = false;
    loop {
        let (e, is_empty) = match reader.read_event().map_err(Error::xml)? {
            Event::Start(e) => (e.into_owned(), false),
            Event::Empty(e) => (e.into_owned(), true),
            Event::End(e) if in_body && e.local_name().into_inner() == "body" => break,
            Event::Eof => break,
            _ => continue,
        };
        if !in_body {
            if local(&e) == "body" {
                in_body = true;
            }
            continue;
        }
        parse_block(
            &mut reader,
            &e,
            is_empty,
            &ctx,
            &mut blocks,
            Some(&mut section),
        )?;
    }
    Ok((blocks, section))
}

/// Read the complete model from a DOCX package.
pub fn read_package(pkg: &Package) -> Result<WordDocument> {
    if pkg.kind()? != prism_openxml::DocumentKind::Document {
        return Err(Error::NotADocument);
    }
    let main = pkg.main_part()?;
    let rels = pkg.relationships(Some(&main))?;
    let xml = pkg
        .part(&main)
        .ok_or_else(|| prism_openxml::Error::MissingPart(main.to_string()))?;
    let (blocks, section) = read_document(xml, &rels)?;

    let part_for = |rel_type: &str| -> Option<PartName> {
        rels.first_of_type(rel_type)
            .filter(|r| r.target_mode == TargetMode::Internal)
            .and_then(|r| r.resolve(Some(&main)))
            .and_then(|r| r.ok())
    };
    let (default_run, default_para, mut styles) =
        match part_for(rel_types::STYLES).and_then(|p| pkg.part(&p)) {
            Some(data) => styles::parse_styles(data)?,
            None => (Marks::default(), ParaFormat::default(), Vec::new()),
        };
    styles.extend(styles::builtin_infos(&styles));
    let mut numbering = match part_for(NUMBERING_REL).and_then(|p| pkg.part(&p)) {
        Some(data) => numbering::parse_numbering(data)?,
        None => Vec::new(),
    };
    numbering::add_builtin_defs(&mut numbering);

    let parts = crate::parts::read_parts(pkg, &main, &rels, &section)?;

    Ok(WordDocument {
        blocks,
        section,
        styles,
        default_run,
        default_para,
        numbering,
        notes: parts.notes,
        comments: parts.comments,
        headers: parts.headers,
        sources: parts.sources,
        settings: parts.settings,
    })
}
