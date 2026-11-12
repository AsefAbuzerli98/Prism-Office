//! Secondary WordprocessingML parts: footnotes/endnotes, comments,
//! headers/footers, settings and the bibliography custom XML part.

use quick_xml::events::Event;
use quick_xml::Reader;

use prism_openxml::{Package, PartName, Relationships, TargetMode};

use crate::model::*;
use crate::raw::{attr, capture, escape_attr, escape_text, local, skip};
use crate::read::{parse_blocks, Ctx};
use crate::{Error, Result};

const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub(crate) const FOOTNOTES_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
pub(crate) const ENDNOTES_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
pub(crate) const COMMENTS_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
pub(crate) const SETTINGS_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
pub(crate) const CUSTOM_XML_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml";
const CUSTOM_XML_PROPS_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps";
const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const B_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/bibliography";
const DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

pub(crate) struct Parts {
    pub notes: Vec<Note>,
    pub comments: Vec<Comment>,
    pub headers: Vec<HeaderFooter>,
    pub sources: Vec<Source>,
    pub settings: DocSettings,
}

fn resolve(rels: &Relationships, main: &PartName, rel_type: &str) -> Option<PartName> {
    rels.first_of_type(rel_type)
        .filter(|r| r.target_mode == TargetMode::Internal)
        .and_then(|r| r.resolve(Some(main)))
        .and_then(|r| r.ok())
}

fn part_text(pkg: &Package, p: &PartName) -> Option<String> {
    pkg.part(p).map(|b| String::from_utf8_lossy(b).into_owned())
}

/// (attributes, block content) of each `child` element under the root.
type NoteLike = Vec<(Vec<(String, String)>, Vec<Block>)>;

fn read_note_like(xml: &str, rels: &Relationships, child: &str) -> Result<NoteLike> {
    let mut r = Reader::from_str(xml);
    let ctx = Ctx { rels };
    let mut out = Vec::new();
    let mut depth = 0;
    loop {
        match r.read_event().map_err(Error::xml)? {
            Event::Start(e) => {
                depth += 1;
                if depth == 2 && local(&e) == child {
                    let attrs = prism_openxml::xml::attrs_of(&e);
                    let blocks = parse_blocks(&mut r, &ctx)?;
                    depth -= 1;
                    out.push((attrs, blocks));
                }
            }
            Event::End(_) => depth -= 1,
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

fn get<'a>(attrs: &'a [(String, String)], k: &str) -> Option<&'a str> {
    prism_openxml::xml::attr(attrs, k)
}

fn read_notes(pkg: &Package, part: &PartName, kind: NoteKind) -> Result<Vec<Note>> {
    let Some(xml) = part_text(pkg, part) else {
        return Ok(vec![]);
    };
    let rels = pkg.relationships(Some(part))?;
    let child = if kind == NoteKind::Footnote {
        "footnote"
    } else {
        "endnote"
    };
    Ok(read_note_like(&xml, &rels, child)?
        .into_iter()
        .filter(|(a, _)| matches!(get(a, "type"), None | Some("normal")))
        .filter_map(|(a, blocks)| {
            Some(Note {
                kind,
                id: get(&a, "id")?.parse().ok()?,
                blocks,
            })
        })
        .collect())
}

fn read_comments(pkg: &Package, part: &PartName) -> Result<Vec<Comment>> {
    let Some(xml) = part_text(pkg, part) else {
        return Ok(vec![]);
    };
    let rels = pkg.relationships(Some(part))?;
    Ok(read_note_like(&xml, &rels, "comment")?
        .into_iter()
        .filter_map(|(a, blocks)| {
            Some(Comment {
                id: get(&a, "id")?.to_string(),
                author: get(&a, "author").unwrap_or_default().to_string(),
                initials: get(&a, "initials").unwrap_or_default().to_string(),
                date: get(&a, "date").map(str::to_owned),
                blocks,
            })
        })
        .collect())
}

/// Block content of a header/footer part (direct children of the root).
fn read_root_blocks(pkg: &Package, part: &PartName) -> Result<Vec<Block>> {
    let Some(xml) = part_text(pkg, part) else {
        return Ok(vec![]);
    };
    let rels = pkg.relationships(Some(part))?;
    let mut r = Reader::from_str(&xml);
    let ctx = Ctx { rels: &rels };
    loop {
        match r.read_event().map_err(Error::xml)? {
            Event::Start(_) => return parse_blocks(&mut r, &ctx),
            Event::Eof => return Ok(vec![]),
            _ => {}
        }
    }
}

fn header_refs(section: &Section) -> Vec<(HeaderKind, String, String)> {
    section
        .extra
        .iter()
        .filter_map(|x| {
            let kind = match x.name.rsplit(':').next()? {
                "headerReference" => HeaderKind::Header,
                "footerReference" => HeaderKind::Footer,
                _ => return None,
            };
            let mut r = Reader::from_str(&x.xml);
            loop {
                match r.read_event().ok()? {
                    Event::Start(e) | Event::Empty(e) => {
                        return Some((
                            kind,
                            attr(&e, "type").unwrap_or_else(|| "default".into()),
                            attr(&e, "id")?,
                        ));
                    }
                    Event::Eof => return None,
                    _ => {}
                }
            }
        })
        .collect()
}

fn flag_on(xml: &str, name: &str) -> bool {
    let mut r = Reader::from_str(xml);
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(&e) == name => {
                return crate::raw::on_off(&e)
            }
            Ok(Event::Eof) | Err(_) => return false,
            _ => {}
        }
    }
}

fn text_until_end(r: &mut Reader<&[u8]>) -> Result<String> {
    let mut s = String::new();
    loop {
        match r.read_event().map_err(Error::xml)? {
            Event::Text(t) => s.push_str(&prism_openxml::xml::text_of(&t)),
            Event::GeneralRef(g) => s.push_str(&prism_openxml::xml::ref_text(&g)),
            Event::End(_) | Event::Eof => return Ok(s),
            _ => {}
        }
    }
}

pub(crate) fn parse_sources(xml: &str) -> Result<Option<Vec<Source>>> {
    let mut r = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut is_bib = false;
    let mut cur: Option<Source> = None;
    let (mut last, mut first) = (String::new(), String::new());
    loop {
        match r.read_event().map_err(Error::xml)? {
            Event::Start(e) => match local(&e).as_str() {
                "Sources" => is_bib = true,
                "Source" => cur = Some(Source::default()),
                name if cur.is_some() => {
                    let c = cur.as_mut().unwrap();
                    match name {
                        "Tag" => c.tag = text_until_end(&mut r)?,
                        "SourceType" => c.kind = text_until_end(&mut r)?,
                        "Title" => c.title = text_until_end(&mut r)?,
                        "Year" => c.year = text_until_end(&mut r)?,
                        "Publisher" => c.publisher = text_until_end(&mut r)?,
                        "URL" => c.url = text_until_end(&mut r)?,
                        "Corporate" => {
                            let n = text_until_end(&mut r)?;
                            c.authors.push(n);
                        }
                        "Last" => last = text_until_end(&mut r)?,
                        "First" => first = text_until_end(&mut r)?,
                        _ => {}
                    }
                }
                _ => {}
            },
            Event::End(e) => match e.local_name().into_inner() {
                "Person" => {
                    if let Some(c) = cur.as_mut() {
                        c.authors.push(if first.is_empty() {
                            last.clone()
                        } else {
                            format!("{last}, {first}")
                        });
                    }
                    last.clear();
                    first.clear();
                }
                "Source" => out.extend(cur.take()),
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(is_bib.then_some(out))
}

fn find_sources_part(pkg: &Package, main: &PartName, rels: &Relationships) -> Option<PartName> {
    rels.of_type(CUSTOM_XML_REL)
        .filter_map(|r| r.resolve(Some(main))?.ok())
        .find(|p| part_text(pkg, p).is_some_and(|x| x.contains("Sources") && x.contains(B_NS)))
}

pub(crate) fn read_parts(
    pkg: &Package,
    main: &PartName,
    rels: &Relationships,
    section: &Section,
) -> Result<Parts> {
    let mut notes = Vec::new();
    if let Some(p) = resolve(rels, main, FOOTNOTES_REL) {
        notes.extend(read_notes(pkg, &p, NoteKind::Footnote)?);
    }
    if let Some(p) = resolve(rels, main, ENDNOTES_REL) {
        notes.extend(read_notes(pkg, &p, NoteKind::Endnote)?);
    }
    let comments = match resolve(rels, main, COMMENTS_REL) {
        Some(p) => read_comments(pkg, &p)?,
        None => vec![],
    };
    let mut headers = Vec::new();
    for (kind, which, rel_id) in header_refs(section) {
        let Some(part) = rels
            .by_id(&rel_id)
            .and_then(|r| r.resolve(Some(main)))
            .and_then(|r| r.ok())
        else {
            continue;
        };
        let mut blocks = read_root_blocks(pkg, &part)?;
        tag_image_part(&mut blocks, &part);
        headers.push(HeaderFooter {
            kind,
            which,
            rel_id,
            blocks,
        });
    }
    let settings = match resolve(rels, main, SETTINGS_REL).and_then(|p| part_text(pkg, &p)) {
        Some(x) => DocSettings {
            track_revisions: flag_on(&x, "trackRevisions"),
            auto_hyphenation: flag_on(&x, "autoHyphenation"),
        },
        None => DocSettings::default(),
    };
    let sources = match find_sources_part(pkg, main, rels).and_then(|p| part_text(pkg, &p)) {
        Some(x) => parse_sources(&x)?.unwrap_or_default(),
        None => vec![],
    };
    Ok(Parts {
        notes,
        comments,
        headers,
        sources,
        settings,
    })
}

/// Images inside headers resolve against the header part's relationships;
/// record that by prefixing the rel id with the part name.
fn tag_image_part(blocks: &mut [Block], part: &PartName) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                for i in &mut p.inlines {
                    if let Inline::Image { rel_id, .. } = i {
                        if !rel_id.contains('#') {
                            *rel_id = format!("{part}#{rel_id}");
                        }
                    }
                }
            }
            Block::Table(t) => {
                for r in &mut t.rows {
                    for c in &mut r.cells {
                        tag_image_part(&mut c.blocks, part);
                    }
                }
            }
            Block::Opaque { .. } => {}
        }
    }
}

fn untag_image_part(blocks: &[Block]) -> Vec<Block> {
    let mut out = blocks.to_vec();
    fn walk(blocks: &mut [Block]) {
        for b in blocks {
            match b {
                Block::Paragraph(p) => {
                    for i in &mut p.inlines {
                        if let Inline::Image { rel_id, .. } = i {
                            if let Some((_, id)) = rel_id.split_once('#') {
                                *rel_id = id.to_string();
                            }
                        }
                    }
                }
                Block::Table(t) => t
                    .rows
                    .iter_mut()
                    .flat_map(|r| r.cells.iter_mut())
                    .for_each(|c| walk(&mut c.blocks)),
                Block::Opaque { .. } => {}
            }
        }
    }
    walk(&mut out);
    out
}

// ---- writing -------------------------------------------------------------------

/// `(root start tag incl. '>', root qname)` of an existing part.
fn root_of(xml: &str) -> Option<(String, String)> {
    let mut r = Reader::from_str(xml);
    loop {
        match r.read_event().ok()? {
            Event::Start(e) => {
                let end = r.buffer_position() as usize;
                return Some((xml[..end].to_string(), crate::raw::qname(&e)));
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

type LinkFn<'a> = dyn FnMut(&str) -> String + 'a;

fn hyperlink_resolver<'a>(rels: &'a mut Relationships) -> impl FnMut(&str) -> String + 'a {
    move |target: &str| {
        if let Some(r) = rels
            .of_type(prism_openxml::rel_types::HYPERLINK)
            .find(|r| r.target == target && r.target_mode == TargetMode::External)
        {
            return r.id.clone();
        }
        rels.add(
            prism_openxml::rel_types::HYPERLINK,
            target,
            TargetMode::External,
        )
    }
}

fn ensure_part(
    pkg: &mut Package,
    main: &PartName,
    rel_type: &str,
    name: &str,
    ct: &str,
    init: String,
) -> Result<PartName> {
    let mut rels = pkg.relationships(Some(main))?;
    if let Some(p) = resolve(&rels, main, rel_type) {
        if pkg.has_part(&p) {
            return Ok(p);
        }
    }
    let part = PartName::new(&format!("/word/{name}"))?;
    rels.add(rel_type, name, TargetMode::Internal);
    pkg.set_relationships(Some(main), &rels);
    pkg.set_part(part.clone(), init.into_bytes(), Some(ct));
    Ok(part)
}

fn rewrite_children(
    pkg: &mut Package,
    part: &PartName,
    keep: impl Fn(&str, &[(String, String)]) -> bool,
    body: &mut dyn FnMut(&mut LinkFn<'_>) -> String,
) -> Result<()> {
    let xml = part_text(pkg, part).unwrap_or_default();
    let (prefix, root) =
        root_of(&xml).ok_or_else(|| Error::xml(format!("{part}: no root element")))?;
    // keep selected existing children (e.g. note separators) verbatim
    let mut kept = String::new();
    let mut r = Reader::from_str(&xml);
    let mut depth = 0;
    loop {
        match r.read_event().map_err(Error::xml)? {
            Event::Start(e) => {
                depth += 1;
                if depth == 2 {
                    let attrs = prism_openxml::xml::attrs_of(&e);
                    if keep(&local(&e), &attrs) {
                        kept.push_str(&capture(&mut r, &e.into_owned(), false)?);
                    } else {
                        skip(&mut r, false)?;
                    }
                    depth -= 1;
                }
            }
            Event::Empty(e) if depth == 1 => {
                let attrs = prism_openxml::xml::attrs_of(&e);
                if keep(&local(&e), &attrs) {
                    kept.push_str(&capture(&mut r, &e.into_owned(), true)?);
                }
            }
            Event::End(_) => depth -= 1,
            Event::Eof => break,
            _ => {}
        }
    }
    let mut rels = pkg.relationships(Some(part))?;
    let content = {
        let mut link = hyperlink_resolver(&mut rels);
        body(&mut link)
    };
    pkg.set_relationships(Some(part), &rels);
    let out = format!("{prefix}{kept}{content}</{root}>");
    let ct = pkg.content_type(part).map(str::to_owned);
    pkg.set_part(part.clone(), out.into_bytes(), ct.as_deref());
    Ok(())
}

fn ensure_body_paragraph(blocks: &[Block]) -> Vec<Block> {
    let mut b = blocks.to_vec();
    if !matches!(b.last(), Some(Block::Paragraph(_))) {
        b.push(Block::Paragraph(Paragraph::default()));
    }
    b
}

fn note_root(kind: NoteKind) -> (&'static str, &'static str, &'static str, &'static str) {
    match kind {
        NoteKind::Footnote => (
            "footnotes",
            "footnote",
            "footnotes.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml",
        ),
        NoteKind::Endnote => (
            "endnotes",
            "endnote",
            "endnotes.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml",
        ),
    }
}

fn write_notes(
    pkg: &mut Package,
    main: &PartName,
    doc: &WordDocument,
    kind: NoteKind,
) -> Result<()> {
    let rel = if kind == NoteKind::Footnote {
        FOOTNOTES_REL
    } else {
        ENDNOTES_REL
    };
    let rels = pkg.relationships(Some(main))?;
    let existing = match resolve(&rels, main, rel) {
        Some(p) => read_notes(pkg, &p, kind)?,
        None => vec![],
    };
    let wanted: Vec<&Note> = doc.notes.iter().filter(|n| n.kind == kind).collect();
    if wanted.iter().map(|n| (*n).clone()).collect::<Vec<_>>() == existing {
        return Ok(());
    }
    let (root, child, file, ct) = note_root(kind);
    let init = format!(
        r#"{DECL}<w:{root} xmlns:w="{W_NS}" xmlns:r="{R}"><w:{child} w:type="separator" w:id="-1"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:separator/></w:r></w:p></w:{child}><w:{child} w:type="continuationSeparator" w:id="0"><w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:{child}></w:{root}>"#
    );
    let part = ensure_part(pkg, main, rel, file, ct, init)?;
    rewrite_children(
        pkg,
        &part,
        |_, attrs| matches!(get(attrs, "type"), Some(t) if t != "normal"),
        &mut |link| {
            wanted
                .iter()
                .map(|n| {
                    format!(
                        r#"<w:{child} w:id="{}">{}</w:{child}>"#,
                        n.id,
                        crate::write::blocks_xml(&ensure_body_paragraph(&n.blocks), link)
                    )
                })
                .collect()
        },
    )
}

fn write_comments(pkg: &mut Package, main: &PartName, doc: &WordDocument) -> Result<()> {
    let rels = pkg.relationships(Some(main))?;
    let existing = match resolve(&rels, main, COMMENTS_REL) {
        Some(p) => read_comments(pkg, &p)?,
        None => vec![],
    };
    if doc.comments == existing {
        return Ok(());
    }
    let init = format!(r#"{DECL}<w:comments xmlns:w="{W_NS}" xmlns:r="{R}"></w:comments>"#);
    let part = ensure_part(
        pkg,
        main,
        COMMENTS_REL,
        "comments.xml",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml",
        init,
    )?;
    rewrite_children(pkg, &part, |_, _| false, &mut |link| {
        doc.comments
            .iter()
            .map(|c| {
                let date = c
                    .date
                    .as_ref()
                    .map(|d| format!(r#" w:date="{}""#, escape_attr(d)))
                    .unwrap_or_default();
                format!(
                    r#"<w:comment w:id="{}" w:author="{}" w:initials="{}"{date}>{}</w:comment>"#,
                    escape_attr(&c.id),
                    escape_attr(&c.author),
                    escape_attr(&c.initials),
                    crate::write::blocks_xml(&ensure_body_paragraph(&c.blocks), link)
                )
            })
            .collect()
    })
}

fn write_headers(pkg: &mut Package, main: &PartName, doc: &WordDocument) -> Result<()> {
    let rels = pkg.relationships(Some(main))?;
    for hf in &doc.headers {
        let Some(part) = rels
            .by_id(&hf.rel_id)
            .and_then(|r| r.resolve(Some(main)))
            .and_then(|r| r.ok())
        else {
            continue;
        };
        let mut current = read_root_blocks(pkg, &part)?;
        tag_image_part(&mut current, &part);
        if current == hf.blocks {
            continue;
        }
        let blocks = ensure_body_paragraph(&untag_image_part(&hf.blocks));
        rewrite_children(pkg, &part, |_, _| false, &mut |link| {
            crate::write::blocks_xml(&blocks, link)
        })?;
    }
    Ok(())
}

/// Schema order of `w:settings` children (subset used for insertion).
const SETTINGS_ORDER: &[&str] = &[
    "writeProtection",
    "view",
    "zoom",
    "removePersonalInformation",
    "removeDateAndTime",
    "doNotDisplayPageBoundaries",
    "displayBackgroundShape",
    "printPostScriptOverText",
    "printFractionalCharacterWidth",
    "printFormsData",
    "embedTrueTypeFonts",
    "embedSystemFonts",
    "saveSubsetFonts",
    "saveFormsData",
    "mirrorMargins",
    "alignBordersAndEdges",
    "bordersDoNotSurroundHeader",
    "bordersDoNotSurroundFooter",
    "gutterAtTop",
    "hideSpellingErrors",
    "hideGrammaticalErrors",
    "activeWritingStyle",
    "proofState",
    "formsDesign",
    "attachedTemplate",
    "linkStyles",
    "stylePaneFormatFilter",
    "stylePaneSortMethod",
    "documentType",
    "mailMerge",
    "revisionView",
    "trackRevisions",
    "doNotTrackMoves",
    "doNotTrackFormatting",
    "documentProtection",
    "autoFormatOverride",
    "styleLockTheme",
    "styleLockQFSet",
    "defaultTabStop",
    "autoHyphenation",
];

fn set_flag(xml: &str, name: &str, on: bool) -> String {
    let mut out = xml.to_string();
    // remove existing
    for pat in [format!("<w:{name}/>"), format!("<w:{name} ")] {
        while let Some(i) = out.find(&pat) {
            let end = out[i..]
                .find("/>")
                .map(|e| i + e + 2)
                .unwrap_or(i + pat.len());
            out.replace_range(i..end, "");
        }
    }
    if !on {
        return out;
    }
    let rank = SETTINGS_ORDER
        .iter()
        .position(|n| *n == name)
        .unwrap_or(SETTINGS_ORDER.len());
    // insert before the first top-level child with a higher rank
    let mut r = Reader::from_str(&out);
    let mut depth = 0;
    let mut insert_at = None;
    loop {
        let before = r.buffer_position() as usize;
        match r.read_event() {
            Ok(Event::Start(e)) => {
                if depth == 1
                    && insert_at.is_none()
                    && crate::order::rank(SETTINGS_ORDER, &crate::raw::qname(&e)) > rank
                {
                    insert_at = Some(before);
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                if depth == 1
                    && insert_at.is_none()
                    && crate::order::rank(SETTINGS_ORDER, &crate::raw::qname(&e)) > rank
                {
                    insert_at = Some(before);
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 && insert_at.is_none() {
                    insert_at = Some(before);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    if let Some(i) = insert_at {
        out.insert_str(i, &format!("<w:{name}/>"));
    }
    out
}

fn write_settings(pkg: &mut Package, main: &PartName, doc: &WordDocument) -> Result<()> {
    let rels = pkg.relationships(Some(main))?;
    let current = match resolve(&rels, main, SETTINGS_REL).and_then(|p| part_text(pkg, &p)) {
        Some(x) => DocSettings {
            track_revisions: flag_on(&x, "trackRevisions"),
            auto_hyphenation: flag_on(&x, "autoHyphenation"),
        },
        None => DocSettings::default(),
    };
    if current == doc.settings {
        return Ok(());
    }
    let init = format!(r#"{DECL}<w:settings xmlns:w="{W_NS}"></w:settings>"#);
    let part = ensure_part(
        pkg,
        main,
        SETTINGS_REL,
        "settings.xml",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml",
        init,
    )?;
    let mut xml = part_text(pkg, &part).unwrap_or_default();
    xml = set_flag(&xml, "trackRevisions", doc.settings.track_revisions);
    xml = set_flag(&xml, "autoHyphenation", doc.settings.auto_hyphenation);
    let ct = pkg.content_type(&part).map(str::to_owned);
    pkg.set_part(part, xml.into_bytes(), ct.as_deref());
    Ok(())
}

fn source_xml(s: &Source, n: usize) -> String {
    let t = |name: &str, v: &str| {
        if v.is_empty() {
            String::new()
        } else {
            format!("<b:{name}>{}</b:{name}>", escape_text(v))
        }
    };
    let people: String = s
        .authors
        .iter()
        .map(|a| match a.split_once(", ") {
            Some((last, first)) => format!(
                "<b:Person><b:Last>{}</b:Last><b:First>{}</b:First></b:Person>",
                escape_text(last),
                escape_text(first)
            ),
            None => format!("<b:Person><b:Last>{}</b:Last></b:Person>", escape_text(a)),
        })
        .collect();
    let authors = if people.is_empty() {
        String::new()
    } else {
        format!("<b:Author><b:Author><b:NameList>{people}</b:NameList></b:Author></b:Author>")
    };
    format!(
        "<b:Source>{}{}<b:Guid>{{{:08X}-0000-4000-8000-{:012X}}}</b:Guid>{authors}{}{}{}{}</b:Source>",
        t("Tag", &s.tag),
        t("SourceType", if s.kind.is_empty() { "Misc" } else { &s.kind }),
        n + 1,
        n + 1,
        t("Title", &s.title),
        t("Year", &s.year),
        t("Publisher", &s.publisher),
        t("URL", &s.url)
    )
}

fn write_sources(pkg: &mut Package, main: &PartName, doc: &WordDocument) -> Result<()> {
    let mut rels = pkg.relationships(Some(main))?;
    let existing_part = find_sources_part(pkg, main, &rels);
    let existing = existing_part
        .as_ref()
        .and_then(|p| part_text(pkg, p))
        .map(|x| parse_sources(&x))
        .transpose()?
        .flatten()
        .unwrap_or_default();
    if existing == doc.sources {
        return Ok(());
    }
    let body: String = doc
        .sources
        .iter()
        .enumerate()
        .map(|(i, s)| source_xml(s, i))
        .collect();
    let xml = format!(
        r#"{DECL}<b:Sources xmlns:b="{B_NS}" xmlns="{B_NS}" SelectedStyle="\APASixthEditionOfficeOnline.xsl" StyleName="APA" Version="6">{body}</b:Sources>"#
    );
    let part = match existing_part {
        Some(p) => p,
        None => {
            let mut n = 1;
            while pkg.has_part(&PartName::new(&format!("/customXml/item{n}.xml"))?) {
                n += 1;
            }
            let item = PartName::new(&format!("/customXml/item{n}.xml"))?;
            let props = PartName::new(&format!("/customXml/itemProps{n}.xml"))?;
            pkg.set_part(
                props.clone(),
                format!(r#"{DECL}<ds:datastoreItem ds:itemID="{{6F1C3A52-1B2D-4C3E-9A10-{n:012X}}}" xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml"><ds:schemaRefs><ds:schemaRef ds:uri="{B_NS}"/></ds:schemaRefs></ds:datastoreItem>"#).into_bytes(),
                Some("application/vnd.openxmlformats-officedocument.customXmlProperties+xml"),
            );
            let mut item_rels = Relationships::default();
            item_rels.add(
                CUSTOM_XML_PROPS_REL,
                &format!("itemProps{n}.xml"),
                TargetMode::Internal,
            );
            pkg.set_part(item.clone(), Vec::new(), None);
            pkg.set_relationships(Some(&item), &item_rels);
            rels.add(
                CUSTOM_XML_REL,
                &format!("../customXml/item{n}.xml"),
                TargetMode::Internal,
            );
            pkg.set_relationships(Some(main), &rels);
            item
        }
    };
    pkg.set_part(part, xml.into_bytes(), None);
    Ok(())
}

pub(crate) fn write_parts(pkg: &mut Package, main: &PartName, doc: &WordDocument) -> Result<()> {
    write_notes(pkg, main, doc, NoteKind::Footnote)?;
    write_notes(pkg, main, doc, NoteKind::Endnote)?;
    write_comments(pkg, main, doc)?;
    write_headers(pkg, main, doc)?;
    write_settings(pkg, main, doc)?;
    write_sources(pkg, main, doc)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_flag_insert_and_remove_in_order() {
        let xml = r#"<w:settings xmlns:w="x"><w:zoom w:percent="100"/><w:defaultTabStop w:val="708"/></w:settings>"#;
        let on = set_flag(xml, "trackRevisions", true);
        assert!(on.find("<w:zoom").unwrap() < on.find("<w:trackRevisions/>").unwrap());
        assert!(on.find("<w:trackRevisions/>").unwrap() < on.find("<w:defaultTabStop").unwrap());
        let off = set_flag(&on, "trackRevisions", false);
        assert_eq!(off, xml);
        let empty = set_flag(
            r#"<w:settings xmlns:w="x"></w:settings>"#,
            "autoHyphenation",
            true,
        );
        assert!(empty.contains("<w:autoHyphenation/></w:settings>"));
    }

    #[test]
    fn sources_roundtrip() {
        let s = vec![Source {
            tag: "Smi20".into(),
            kind: "Book".into(),
            title: "Rust & XML".into(),
            authors: vec!["Smith, Jane".into(), "ACME Corp".into()],
            year: "2020".into(),
            publisher: "Pub".into(),
            url: String::new(),
        }];
        let xml = format!(
            r#"<b:Sources xmlns:b="{B_NS}">{}</b:Sources>"#,
            source_xml(&s[0], 0)
        );
        assert_eq!(parse_sources(&xml).unwrap().unwrap(), s);
        assert!(parse_sources("<root/>").unwrap().is_none());
    }
}
