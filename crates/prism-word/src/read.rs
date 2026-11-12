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

struct Ctx<'a> {
    rels: &'a Relationships,
}

fn label_for(name: &str) -> &'static str {
    match name {
        "drawing" | "pict" => "Image",
        "object" => "Object",
        "tbl" => "Table",
        "sdt" => "Content control",
        "ins" => "Insertion",
        "del" => "Deletion",
        "fldSimple" => "Field",
        "oMath" | "oMathPara" => "Equation",
        "footnoteReference" => "Footnote",
        "endnoteReference" => "Endnote",
        "commentReference" => "Comment",
        "sym" => "Symbol",
        "smartTag" | "customXml" => "Tagged text",
        _ => "Element",
    }
}

/// Elements that never render anything.
fn invisible(name: &str) -> bool {
    matches!(
        name,
        "bookmarkStart"
            | "bookmarkEnd"
            | "commentRangeStart"
            | "commentRangeEnd"
            | "permStart"
            | "permEnd"
            | "lastRenderedPageBreak"
            | "moveFromRangeStart"
            | "moveFromRangeEnd"
            | "moveToRangeStart"
            | "moveToRangeEnd"
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

/// Parse a `w:r`. If it contains anything beyond text/tab/break, the whole
/// run is preserved as one opaque inline.
fn parse_run(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart<'_>,
    empty: bool,
    link: Option<&str>,
    out: &mut Vec<Inline>,
) -> Result<()> {
    if empty {
        return Ok(());
    }
    let raw = capture(reader, start, false)?;
    let mut r = Reader::from_str(&raw);
    r.read_event().map_err(Error::xml)?; // <w:r>
    let mut marks = Marks {
        link: link.map(str::to_owned),
        ..Default::default()
    };
    let mut items: Vec<Inline> = Vec::new();
    let mut opaque: Option<&'static str> = None;
    loop {
        let (e, is_empty) = match r.read_event().map_err(Error::xml)? {
            Event::Start(e) => (e.into_owned(), false),
            Event::Empty(e) => (e.into_owned(), true),
            Event::End(_) | Event::Eof => break,
            _ => continue,
        };
        match local(&e).as_str() {
            "rPr" => {
                marks = parse_rpr(&mut r, is_empty)?;
                if let Some(l) = link {
                    marks.link = Some(l.to_string());
                }
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
            "t" => {}
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
            other => {
                opaque = Some(label_for(other));
                skip(&mut r, is_empty)?;
            }
        }
    }
    if let Some(label) = opaque {
        out.push(Inline::Opaque {
            label: label.to_string(),
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
    parse_paragraph_children(reader, ctx, &mut p, None)?;
    Ok(p)
}

/// Shared by `w:p` and `w:hyperlink` (which nests runs).
fn parse_paragraph_children(
    reader: &mut Reader<&[u8]>,
    ctx: &Ctx<'_>,
    p: &mut Paragraph,
    link: Option<&str>,
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
            "pPr" if link.is_none() => {
                let (style, format, list, extra) = parse_ppr(reader, is_empty)?;
                p.style = style;
                p.format = format;
                p.list = list;
                p.extra = extra;
            }
            "r" => parse_run(reader, &e, is_empty, link, &mut p.inlines)?,
            "proofErr" => skip(reader, is_empty)?,
            "hyperlink" if link.is_none() && !is_empty => {
                // Only simple hyperlinks are modelled; anything with extra
                // attributes (tooltip, history…) stays opaque.
                let attrs = prism_openxml::xml::attrs_of(&e);
                let simple = attrs
                    .iter()
                    .all(|(k, _)| k == "id" || k == "anchor" || k == "history");
                let target = attr(&e, "id")
                    .and_then(|id| ctx.rels.by_id(&id).map(|r| r.target.clone()))
                    .or_else(|| attr(&e, "anchor").map(|a| format!("#{a}")));
                match (simple, target) {
                    (true, Some(t)) => parse_paragraph_children(reader, ctx, p, Some(&t))?,
                    _ => {
                        let xml = capture(reader, &e, false)?;
                        p.inlines.push(Inline::Opaque {
                            label: "Link".into(),
                            xml,
                        });
                    }
                }
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
        match local(&e).as_str() {
            "pgSz"
                if prism_openxml::xml::attrs_of(&e)
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
        let name = local(&e);
        if !in_body {
            if name == "body" {
                in_body = true;
            }
            continue;
        }
        match name.as_str() {
            "p" => blocks.push(Block::Paragraph(parse_paragraph(
                &mut reader,
                is_empty,
                &ctx,
            )?)),
            "sectPr" => section = parse_sect_pr(&mut reader, is_empty)?,
            other => {
                let label = label_for(other).to_string();
                let xml = capture(&mut reader, &e, is_empty)?;
                blocks.push(Block::Opaque { label, xml });
            }
        }
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

    Ok(WordDocument {
        blocks,
        section,
        styles,
        default_run,
        default_para,
        numbering,
    })
}
