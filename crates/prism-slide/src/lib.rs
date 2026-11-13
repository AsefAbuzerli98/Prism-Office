//! # prism-slide
//!
//! PresentationML (PPTX) engine: a JSON-friendly presentation model
//! (slides, layouts, masters, themes, notes, transitions) read from and
//! written back to the package. Unchanged XML is preserved; picture and
//! hyperlink relationships are expressed as part names / URLs in the model
//! so shapes can move between slides freely.

pub mod anim;

use std::collections::HashMap;

use prism_drawingml::color::Theme;
use prism_drawingml::dom::{El, XNode};
use prism_drawingml::fill::Fill;
use prism_drawingml::shape::{read_tree, write_tree, Node, Originals};
use prism_drawingml::text::{list_style, ListStyle, TextBody};
use prism_openxml::{rel_types, DocumentKind, Package, PartName, Relationships, TargetMode};
use serde::{Deserialize, Serialize};

pub use anim::{Timing, Transition};

#[derive(Debug, thiserror::Error)]
pub enum SlideError {
    #[error(transparent)]
    Package(#[from] prism_openxml::Error),
    #[error("malformed {0}")]
    Malformed(String),
}

type Result<T> = std::result::Result<T, SlideError>;

fn none<T>(o: &Option<T>) -> bool {
    o.is_none()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Background {
    #[serde(default, skip_serializing_if = "none")]
    pub fill: Option<Fill>,
    /// `p:bgRef`: theme background style index and colour.
    #[serde(default, skip_serializing_if = "none")]
    pub style_ref: Option<(u32, prism_drawingml::color::Color)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TextStyles {
    pub title: ListStyle,
    pub body: ListStyle,
    pub other: ListStyle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// Part name, e.g. `/ppt/slideLayouts/slideLayout2.xml`.
    pub part: String,
    pub name: String,
    /// title, obj, twoObj, secHead, blank …
    #[serde(default, skip_serializing_if = "none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "none")]
    pub background: Option<Background>,
    pub shapes: Vec<Node>,
    #[serde(default = "yes")]
    pub show_master_shapes: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Master {
    pub part: String,
    pub theme: Theme,
    pub clr_map: Vec<(String, String)>,
    #[serde(default, skip_serializing_if = "none")]
    pub background: Option<Background>,
    pub shapes: Vec<Node>,
    pub text_styles: TextStyles,
    pub layouts: Vec<Layout>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Slide {
    /// Part name in the source package (`None` for slides created in the editor).
    #[serde(default, skip_serializing_if = "none")]
    pub part: Option<String>,
    /// Layout part name.
    pub layout: String,
    pub shapes: Vec<Node>,
    #[serde(default, skip_serializing_if = "none")]
    pub background: Option<Background>,
    /// Speaker notes (plain text, paragraphs separated by `\n`).
    #[serde(default)]
    pub notes: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "none")]
    pub transition: Option<Transition>,
    #[serde(default, skip_serializing_if = "none")]
    pub timing: Option<Timing>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentationModel {
    /// Slide size (EMU).
    pub width: i64,
    pub height: i64,
    pub masters: Vec<Master>,
    pub slides: Vec<Slide>,
    #[serde(default)]
    pub default_text_style: ListStyle,
}

/// Original package of an imported presentation (lossless save).
#[derive(Clone, Debug)]
pub struct SlideSource {
    pub package: Package,
    pres_part: PartName,
    /// Per slide part: original shape elements by id.
    originals: HashMap<String, Originals>,
    notes_text: HashMap<String, String>,
}

// ---- helpers --------------------------------------------------------------------------------

const P_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
const REL_NOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
const REL_NOTES_MASTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster";
const REL_CHART: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const CT_SLIDE: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
const CT_NOTES: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml";
const CT_NOTES_MASTER: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml";

fn part_el(pkg: &Package, p: &PartName) -> Result<El> {
    let bytes = pkg
        .part(p)
        .ok_or_else(|| SlideError::Malformed(format!("missing {p}")))?;
    El::parse(&String::from_utf8_lossy(bytes)).ok_or_else(|| SlideError::Malformed(p.to_string()))
}

fn rels_of(pkg: &Package, p: &PartName) -> Relationships {
    pkg.relationships(Some(p)).unwrap_or_default()
}

fn target(rels: &Relationships, from: &PartName, rel_type: &str) -> Option<PartName> {
    rels.first_of_type(rel_type)
        .and_then(|r| from.resolve(&r.target).ok())
}

fn background(csld: &El) -> Option<Background> {
    let bg = csld.find("bg")?;
    if let Some(pr) = bg.find("bgPr") {
        return Some(Background {
            fill: Fill::from_parent(pr),
            style_ref: None,
        });
    }
    let r = bg.find("bgRef")?;
    Some(Background {
        fill: None,
        style_ref: prism_drawingml::color::Color::from_parent(r)
            .map(|c| (r.attr_i64("idx").unwrap_or(1001) as u32, c)),
    })
}

fn background_el(b: &Background) -> El {
    let mut bg = El::new("p:bg");
    if let Some(f) = &b.fill {
        bg.push(
            El::new("p:bgPr")
                .child(f.to_el())
                .child(El::new("a:effectLst")),
        );
    } else if let Some((idx, c)) = &b.style_ref {
        bg.push(El::new("p:bgRef").with("idx", idx).child(c.to_el()));
    }
    bg
}

fn clr_map(e: Option<&El>) -> Vec<(String, String)> {
    e.map(|m| {
        m.attrs
            .iter()
            .filter(|(k, _)| !k.starts_with("xmlns"))
            .cloned()
            .collect()
    })
    .unwrap_or_default()
}

/// Map picture / chart / hyperlink relationship ids to part names / URLs.
fn rels_to_parts(nodes: &mut [Node], rels: &Relationships, from: &PartName) {
    let resolve = |id: &str| -> Option<String> {
        let r = rels.by_id(id)?;
        if r.target_mode == TargetMode::External {
            return Some(format!("url:{}", r.target));
        }
        from.resolve(&r.target).ok().map(|p| p.as_str().to_string())
    };
    fn walk(nodes: &mut [Node], f: &dyn Fn(&str) -> Option<String>) {
        for n in nodes {
            match n {
                Node::Picture(p) => {
                    if let Fill::Image { rel_id, .. } = &mut p.image {
                        if let Some(t) = f(rel_id) {
                            *rel_id = t;
                        }
                    }
                }
                Node::Shape(s) | Node::Connector(s) => {
                    if let Some(Fill::Image { rel_id, .. }) = &mut s.fill {
                        if let Some(t) = f(rel_id) {
                            *rel_id = t;
                        }
                    }
                    if let Some(t) = &mut s.text {
                        for p in &mut t.paragraphs {
                            for r in &mut p.runs {
                                if let prism_drawingml::text::Run::Text { props, .. } = r {
                                    if let Some(l) = &props.link {
                                        props.link = f(l);
                                    }
                                }
                            }
                        }
                    }
                }
                Node::Frame(fr) => {
                    if let Some(id) = &fr.rel_id {
                        fr.rel_id = f(id);
                    }
                }
                Node::Group(g) => walk(&mut g.children, f),
                Node::Other(_) => {}
            }
        }
    }
    walk(nodes, &resolve);
}

/// Inverse of [`rels_to_parts`]: part names / URLs back to relationship ids
/// of `from`, adding relationships as needed.
fn parts_to_rels(nodes: &mut [Node], rels: &mut Relationships, from: &PartName) {
    fn rel_for(rels: &mut Relationships, from: &PartName, target: &str, kind: &str) -> String {
        if let Some(url) = target.strip_prefix("url:") {
            if let Some(r) = rels
                .iter()
                .find(|r| r.target_mode == TargetMode::External && r.target == url)
            {
                return r.id.clone();
            }
            return rels.add(rel_types::HYPERLINK, url, TargetMode::External);
        }
        let Ok(p) = PartName::new(target) else {
            return target.to_string();
        };
        if let Some(r) = rels.iter().find(|r| {
            r.target_mode == TargetMode::Internal
                && from.resolve(&r.target).ok().as_ref() == Some(&p)
        }) {
            return r.id.clone();
        }
        rels.add(kind, &relative(from, &p), TargetMode::Internal)
    }
    fn walk(nodes: &mut [Node], rels: &mut Relationships, from: &PartName) {
        for n in nodes {
            match n {
                Node::Picture(p) => {
                    if let Fill::Image { rel_id, .. } = &mut p.image {
                        *rel_id = rel_for(rels, from, rel_id, rel_types::IMAGE);
                    }
                }
                Node::Shape(s) | Node::Connector(s) => {
                    if let Some(Fill::Image { rel_id, .. }) = &mut s.fill {
                        *rel_id = rel_for(rels, from, rel_id, rel_types::IMAGE);
                    }
                    if let Some(t) = &mut s.text {
                        for p in &mut t.paragraphs {
                            for r in &mut p.runs {
                                if let prism_drawingml::text::Run::Text { props, .. } = r {
                                    if let Some(l) = props.link.clone() {
                                        props.link =
                                            Some(rel_for(rels, from, &l, rel_types::HYPERLINK));
                                    }
                                }
                            }
                        }
                    }
                }
                Node::Frame(fr) => {
                    if let Some(t) = fr.rel_id.clone() {
                        fr.rel_id = Some(rel_for(rels, from, &t, REL_CHART));
                    }
                }
                Node::Group(g) => walk(&mut g.children, rels, from),
                Node::Other(_) => {}
            }
        }
    }
    walk(nodes, rels, from);
}

/// Relationship target of `to` relative to `from`'s folder.
pub fn relative(from: &PartName, to: &PartName) -> String {
    let a: Vec<&str> = from
        .directory()
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let b: Vec<&str> = to.as_str().trim_start_matches('/').split('/').collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<String> = vec!["..".to_string(); a.len() - common];
    parts.extend(b[common..].iter().map(|s| s.to_string()));
    parts.join("/")
}

fn tree_of(csld: Option<&El>) -> Option<&El> {
    csld?.find("spTree")
}

fn notes_text(pkg: &Package, slide: &PartName) -> String {
    let rels = rels_of(pkg, slide);
    let Some(np) = target(&rels, slide, REL_NOTES) else {
        return String::new();
    };
    let Ok(n) = part_el(pkg, &np) else {
        return String::new();
    };
    let Some(tree) = tree_of(n.find("cSld")) else {
        return String::new();
    };
    let (nodes, _) = read_tree(tree);
    nodes
        .iter()
        .find(|n| {
            n.common()
                .ph
                .as_ref()
                .is_some_and(|p| p.kind.as_deref() == Some("body"))
        })
        .and_then(|n| n.text())
        .map(|t| t.plain_text())
        .unwrap_or_default()
}

// ---- reading ----------------------------------------------------------------------------------

/// Read a `.pptx` package.
pub fn read_package(pkg: Package) -> Result<(PresentationModel, SlideSource)> {
    if pkg.kind()? != DocumentKind::Presentation {
        return Err(SlideError::Malformed("not a presentation".into()));
    }
    let pres_part = pkg.main_part()?;
    let pres = part_el(&pkg, &pres_part)?;
    let prels = rels_of(&pkg, &pres_part);
    let size = pres.find("sldSz");
    let mut model = PresentationModel {
        width: size.and_then(|s| s.attr_i64("cx")).unwrap_or(12192000),
        height: size.and_then(|s| s.attr_i64("cy")).unwrap_or(6858000),
        masters: vec![],
        slides: vec![],
        default_text_style: pres
            .find("defaultTextStyle")
            .map(list_style)
            .unwrap_or_default(),
    };
    let mut originals = HashMap::new();
    for id in pres
        .find("sldMasterIdLst")
        .map(|l| l.find_all("sldMasterId").collect::<Vec<_>>())
        .unwrap_or_default()
    {
        let Some(mp) = id
            .attr("r:id")
            .and_then(|r| prels.by_id(r))
            .and_then(|r| pres_part.resolve(&r.target).ok())
        else {
            continue;
        };
        let m = part_el(&pkg, &mp)?;
        let mrels = rels_of(&pkg, &mp);
        let theme = target(&mrels, &mp, rel_types::THEME)
            .and_then(|t| {
                pkg.part(&t)
                    .map(|b| Theme::parse(&String::from_utf8_lossy(b)))
            })
            .unwrap_or_default();
        let csld = m.find("cSld");
        let (mut shapes, _) = tree_of(csld).map(read_tree).unwrap_or_default();
        rels_to_parts(&mut shapes, &mrels, &mp);
        let ts = m.find("txStyles");
        let mut layouts = Vec::new();
        for lid in m
            .find("sldLayoutIdLst")
            .map(|l| l.find_all("sldLayoutId").collect::<Vec<_>>())
            .unwrap_or_default()
        {
            let Some(lp) = lid
                .attr("r:id")
                .and_then(|r| mrels.by_id(r))
                .and_then(|r| mp.resolve(&r.target).ok())
            else {
                continue;
            };
            let l = part_el(&pkg, &lp)?;
            let lrels = rels_of(&pkg, &lp);
            let lcsld = l.find("cSld");
            let (mut lshapes, _) = tree_of(lcsld).map(read_tree).unwrap_or_default();
            rels_to_parts(&mut lshapes, &lrels, &lp);
            layouts.push(Layout {
                part: lp.as_str().to_string(),
                name: lcsld
                    .and_then(|c| c.attr("name"))
                    .unwrap_or_default()
                    .to_string(),
                kind: l.attr("type").map(String::from),
                background: lcsld.and_then(background),
                shapes: lshapes,
                show_master_shapes: l.attr_bool("showMasterSp").unwrap_or(true),
            });
        }
        model.masters.push(Master {
            part: mp.as_str().to_string(),
            theme,
            clr_map: clr_map(m.find("clrMap")),
            background: csld.and_then(background),
            shapes,
            text_styles: TextStyles {
                title: ts
                    .and_then(|t| t.find("titleStyle"))
                    .map(list_style)
                    .unwrap_or_default(),
                body: ts
                    .and_then(|t| t.find("bodyStyle"))
                    .map(list_style)
                    .unwrap_or_default(),
                other: ts
                    .and_then(|t| t.find("otherStyle"))
                    .map(list_style)
                    .unwrap_or_default(),
            },
            layouts,
        });
    }
    let mut notes = HashMap::new();
    for id in pres
        .find("sldIdLst")
        .map(|l| l.find_all("sldId").collect::<Vec<_>>())
        .unwrap_or_default()
    {
        let Some(sp) = id
            .attr("r:id")
            .and_then(|r| prels.by_id(r))
            .and_then(|r| pres_part.resolve(&r.target).ok())
        else {
            continue;
        };
        let s = part_el(&pkg, &sp)?;
        let srels = rels_of(&pkg, &sp);
        let csld = s.find("cSld");
        let (mut shapes, orig) = tree_of(csld).map(read_tree).unwrap_or_default();
        rels_to_parts(&mut shapes, &srels, &sp);
        let layout = target(&srels, &sp, rel_types::SLIDE_LAYOUT)
            .map(|p| p.as_str().to_string())
            .unwrap_or_default();
        let text = notes_text(&pkg, &sp);
        notes.insert(sp.as_str().to_string(), text.clone());
        originals.insert(sp.as_str().to_string(), orig);
        model.slides.push(Slide {
            part: Some(sp.as_str().to_string()),
            layout,
            shapes,
            background: csld.and_then(background),
            notes: text,
            hidden: s.attr("show") == Some("0"),
            transition: anim::read_transition(&s),
            timing: s.find("timing").map(anim::read_timing),
        });
    }
    Ok((
        model,
        SlideSource {
            package: pkg,
            pres_part,
            originals,
            notes_text: notes,
        },
    ))
}

pub fn read_pptx(bytes: &[u8]) -> Result<(PresentationModel, SlideSource)> {
    read_package(Package::from_bytes(bytes)?)
}

// ---- writing ------------------------------------------------------------------------------------

fn free_part(pkg: &Package, stem: &str) -> PartName {
    (1..)
        .map(|n| PartName::new(&format!("{stem}{n}.xml")).expect("static"))
        .find(|p| !pkg.has_part(p))
        .unwrap()
}

/// Replace the shapes of a `p:cSld` keeping its group header and other children.
fn set_tree(csld: &mut El, nodes: Vec<El>) {
    for c in csld.children.iter_mut() {
        if let XNode::El(t) = c {
            if t.local() == "spTree" {
                let head: Vec<XNode> = t.children.iter().filter(|x| matches!(x, XNode::El(e) if matches!(e.local(), "nvGrpSpPr" | "grpSpPr"))).cloned().collect();
                let tail: Vec<XNode> = t
                    .children
                    .iter()
                    .filter(|x| matches!(x, XNode::El(e) if e.local() == "extLst"))
                    .cloned()
                    .collect();
                t.children = head;
                t.children.extend(nodes.into_iter().map(XNode::El));
                t.children.extend(tail);
                return;
            }
        }
    }
}

fn set_child(parent: &mut El, name_local: &str, new: Option<El>, before: &[&str]) {
    parent
        .children
        .retain(|c| !matches!(c, XNode::El(e) if e.local() == name_local));
    if let Some(n) = new {
        let pos = parent
            .children
            .iter()
            .position(|c| matches!(c, XNode::El(e) if before.contains(&e.local())))
            .unwrap_or(parent.children.len());
        parent.children.insert(pos, XNode::El(n));
    }
}

fn empty_slide() -> El {
    El::parse(&format!(
        r#"<p:sld xmlns:a="{A_NS}" xmlns:r="{R_NS}" xmlns:p="{P_NS}"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
    ))
    .expect("static slide")
}

fn notes_paragraphs(text: &str) -> TextBody {
    TextBody {
        paragraphs: text
            .split('\n')
            .map(prism_drawingml::text::Paragraph::plain)
            .collect(),
        ..Default::default()
    }
}

fn ensure_notes_master(
    pkg: &mut Package,
    pres_part: &PartName,
    prels: &mut Relationships,
    theme_part: Option<PartName>,
) -> PartName {
    if let Some(p) = target(prels, pres_part, REL_NOTES_MASTER) {
        return p;
    }
    let p = free_part(pkg, "/ppt/notesMasters/notesMaster");
    let xml = format!(
        r#"{DECL}<p:notesMaster xmlns:a="{A_NS}" xmlns:r="{R_NS}" xmlns:p="{P_NS}"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Notes Placeholder"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="4400550"/><a:ext cx="5486400" cy="3600450"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></p:txBody></p:sp></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/></p:notesMaster>"#
    );
    pkg.set_part(p.clone(), xml.into_bytes(), Some(CT_NOTES_MASTER));
    if let Some(t) = theme_part {
        let mut r = Relationships::default();
        r.add(rel_types::THEME, &relative(&p, &t), TargetMode::Internal);
        pkg.set_relationships(Some(&p), &r);
    }
    let rid = prels.add(
        REL_NOTES_MASTER,
        &relative(pres_part, &p),
        TargetMode::Internal,
    );
    // register in presentation.xml
    if let Ok(mut pres) = part_el(pkg, pres_part) {
        let lst =
            El::new("p:notesMasterIdLst").child(El::new("p:notesMasterId").with("r:id", &rid));
        set_child(
            &mut pres,
            "notesMasterIdLst",
            Some(lst),
            &["handoutMasterIdLst", "sldIdLst", "sldSz"],
        );
        pkg.set_part(
            pres_part.clone(),
            format!("{DECL}{}", pres.to_xml()).into_bytes(),
            None,
        );
    }
    p
}

fn write_notes(pkg: &mut Package, slide: &PartName, text: &str, notes_master: &PartName) {
    let mut srels = rels_of(pkg, slide);
    let body = notes_paragraphs(text);
    match target(&srels, slide, REL_NOTES) {
        Some(np) => {
            let Ok(mut n) = part_el(pkg, &np) else { return };
            for c in n.children.iter_mut() {
                if let XNode::El(csld) = c {
                    if csld.local() != "cSld" {
                        continue;
                    }
                    for t in csld.children.iter_mut() {
                        if let XNode::El(tree) = t {
                            if tree.local() != "spTree" {
                                continue;
                            }
                            for sp in tree.children.iter_mut() {
                                if let XNode::El(sp) = sp {
                                    let is_body = sp
                                        .path(&["nvSpPr", "nvPr", "ph"])
                                        .is_some_and(|p| p.attr("type") == Some("body"));
                                    if is_body {
                                        let tx = body.to_el("p:txBody");
                                        set_child(sp, "txBody", Some(tx), &["extLst"]);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            pkg.set_part(np, format!("{DECL}{}", n.to_xml()).into_bytes(), None);
        }
        None => {
            let np = free_part(pkg, "/ppt/notesSlides/notesSlide");
            let tx = body.to_el("p:txBody").to_xml();
            let xml = format!(
                r#"{DECL}<p:notes xmlns:a="{A_NS}" xmlns:r="{R_NS}" xmlns:p="{P_NS}"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Notes Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/>{tx}</p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"#
            );
            pkg.set_part(np.clone(), xml.into_bytes(), Some(CT_NOTES));
            let mut nrels = Relationships::default();
            nrels.add(
                REL_NOTES_MASTER,
                &relative(&np, notes_master),
                TargetMode::Internal,
            );
            nrels.add(
                rel_types::SLIDE,
                &relative(&np, slide),
                TargetMode::Internal,
            );
            pkg.set_relationships(Some(&np), &nrels);
            srels.add(REL_NOTES, &relative(slide, &np), TargetMode::Internal);
            pkg.set_relationships(Some(slide), &srels);
        }
    }
}

/// Build the output package for a model.
pub fn write_package(model: &PresentationModel, source: &SlideSource) -> Result<Package> {
    let mut pkg = source.package.clone();
    let pres_part = source.pres_part.clone();
    let mut pres = part_el(&pkg, &pres_part)?;
    let mut prels = rels_of(&pkg, &pres_part);
    let empty = Originals::new();
    // slides that disappeared
    let kept: Vec<&str> = model
        .slides
        .iter()
        .filter_map(|s| s.part.as_deref())
        .collect();
    let old_ids: Vec<(String, PartName)> = prels
        .of_type(rel_types::SLIDE)
        .filter_map(|r| pres_part.resolve(&r.target).ok().map(|p| (r.id.clone(), p)))
        .collect();
    for (rid, p) in &old_ids {
        if !kept.contains(&p.as_str()) {
            if let Some(np) = target(&rels_of(&pkg, p), p, REL_NOTES) {
                pkg.remove_part(&np);
                pkg.remove_part(&np.rels_part());
            }
            pkg.remove_part(p);
            pkg.remove_part(&p.rels_part());
            prels.remove(rid);
        }
    }
    let theme_part = model
        .masters
        .first()
        .and_then(|m| PartName::new(&m.part).ok())
        .and_then(|mp| target(&rels_of(&pkg, &mp), &mp, rel_types::THEME));
    let needs_notes = model.slides.iter().any(|s| !s.notes.is_empty());
    let notes_master = if needs_notes {
        Some(ensure_notes_master(
            &mut pkg, &pres_part, &mut prels, theme_part,
        ))
    } else {
        None
    };
    if needs_notes {
        pres = part_el(&pkg, &pres_part)?;
    }
    let mut sld_ids = Vec::new();
    let mut next_id = 256u32.max(
        pres.find("sldIdLst")
            .map(|l| {
                l.find_all("sldId")
                    .filter_map(|s| s.attr_i64("id"))
                    .max()
                    .unwrap_or(255) as u32
                    + 1
            })
            .unwrap_or(256),
    );
    for slide in &model.slides {
        let (part, mut el, orig) = match slide
            .part
            .as_deref()
            .and_then(|p| PartName::new(p).ok())
            .filter(|p| pkg.has_part(p))
        {
            Some(p) => {
                let el = part_el(&pkg, &p)?;
                let o = source.originals.get(p.as_str()).unwrap_or(&empty);
                (p, el, o)
            }
            None => {
                let p = free_part(&pkg, "/ppt/slides/slide");
                let mut r = Relationships::default();
                if let Ok(lp) = PartName::new(&slide.layout) {
                    r.add(
                        rel_types::SLIDE_LAYOUT,
                        &relative(&p, &lp),
                        TargetMode::Internal,
                    );
                }
                pkg.set_relationships(Some(&p), &r);
                (p, empty_slide(), &empty)
            }
        };
        let mut srels = rels_of(&pkg, &part);
        // layout change
        if let Ok(lp) = PartName::new(&slide.layout) {
            let cur = target(&srels, &part, rel_types::SLIDE_LAYOUT);
            if cur.as_ref() != Some(&lp) {
                if let Some(id) = srels
                    .first_of_type(rel_types::SLIDE_LAYOUT)
                    .map(|r| r.id.clone())
                {
                    srels.remove(&id);
                }
                srels.add(
                    rel_types::SLIDE_LAYOUT,
                    &relative(&part, &lp),
                    TargetMode::Internal,
                );
            }
        }
        let mut shapes = slide.shapes.clone();
        parts_to_rels(&mut shapes, &mut srels, &part);
        let nodes = write_tree("p", &shapes, orig);
        for c in el.children.iter_mut() {
            if let XNode::El(csld) = c {
                if csld.local() == "cSld" {
                    set_tree(csld, nodes.clone());
                    let mut bg = slide.background.clone();
                    if let Some(Background { fill: Some(f), .. }) = &mut bg {
                        if let Fill::Image { rel_id, .. } = f {
                            let mut tmp = vec![Node::Picture(prism_drawingml::shape::Picture {
                                common: Default::default(),
                                image: Fill::Image {
                                    rel_id: rel_id.clone(),
                                    crop: [0; 4],
                                    tile: false,
                                },
                                geom: None,
                                line: None,
                                effects: None,
                                media: None,
                            })];
                            parts_to_rels(&mut tmp, &mut srels, &part);
                            if let Node::Picture(p) = &tmp[0] {
                                *f = p.image.clone();
                            }
                        }
                    }
                    let current = background(csld);
                    if current != slide.background {
                        set_child(csld, "bg", bg.as_ref().map(background_el), &["spTree"]);
                    }
                }
            }
        }
        if slide.hidden {
            el.set("show", "0");
        } else {
            el.attrs.retain(|(k, _)| k != "show");
        }
        if anim::read_transition(&el) != slide.transition {
            el.children.retain(|c| !matches!(c, XNode::El(e) if e.local() == "transition" || (e.local() == "AlternateContent" && e.to_xml().contains("transition"))));
            if let Some(t) = &slide.transition {
                set_child(
                    &mut el,
                    "transition",
                    Some(anim::transition_el(t)),
                    &["timing", "extLst"],
                );
            }
        }
        if el.find("timing").map(anim::read_timing) != slide.timing {
            set_child(
                &mut el,
                "timing",
                slide.timing.as_ref().map(anim::timing_el),
                &["extLst"],
            );
        }
        pkg.set_part(
            part.clone(),
            format!("{DECL}{}", el.to_xml()).into_bytes(),
            Some(CT_SLIDE),
        );
        pkg.set_relationships(Some(&part), &srels);
        // notes
        let old_notes = slide
            .part
            .as_ref()
            .and_then(|p| source.notes_text.get(p))
            .cloned()
            .unwrap_or_default();
        if slide.notes != old_notes {
            if let Some(nm) = &notes_master {
                write_notes(&mut pkg, &part, &slide.notes, nm);
            }
        }
        let existing = prels
            .iter()
            .find(|r| pres_part.resolve(&r.target).ok().as_ref() == Some(&part))
            .map(|r| r.id.clone());
        let rid = match existing {
            Some(id) => id,
            None => prels.add(
                rel_types::SLIDE,
                &relative(&pres_part, &part),
                TargetMode::Internal,
            ),
        };
        let id = pres
            .find("sldIdLst")
            .and_then(|l| {
                l.find_all("sldId")
                    .find(|s| s.attr("r:id") == Some(rid.as_str()))
                    .and_then(|s| s.attr_i64("id"))
            })
            .map(|v| v as u32)
            .unwrap_or_else(|| {
                let v = next_id;
                next_id += 1;
                v
            });
        sld_ids.push(El::new("p:sldId").with("id", id).with("r:id", rid));
    }
    let mut lst = El::new("p:sldIdLst");
    for s in sld_ids {
        lst.push(s);
    }
    set_child(
        &mut pres,
        "sldIdLst",
        Some(lst),
        &[
            "sldSz",
            "notesSz",
            "smartTags",
            "embeddedFontLst",
            "custShowLst",
            "photoAlbum",
            "custDataLst",
            "kinsoku",
            "defaultTextStyle",
            "modifyVerifier",
            "extLst",
        ],
    );
    if let Some(sz) = pres.children.iter_mut().find_map(|c| {
        if let XNode::El(e) = c {
            (e.local() == "sldSz").then_some(e)
        } else {
            None
        }
    }) {
        sz.set("cx", model.width).set("cy", model.height);
    }
    pkg.set_part(
        pres_part.clone(),
        format!("{DECL}{}", pres.to_xml()).into_bytes(),
        None,
    );
    pkg.set_relationships(Some(&pres_part), &prels);
    Ok(pkg)
}

pub fn write_pptx(model: &PresentationModel, source: &SlideSource) -> Result<Vec<u8>> {
    Ok(write_package(model, source)?.to_bytes()?)
}

impl SlideSource {
    /// Add a picture to the package; returns its part name for `Fill::Image`.
    pub fn add_image(&mut self, bytes: &[u8]) -> String {
        let (ext, ct) = match bytes {
            [0x89, b'P', ..] => ("png", "image/png"),
            [0xFF, 0xD8, ..] => ("jpeg", "image/jpeg"),
            [b'G', b'I', b'F', ..] => ("gif", "image/gif"),
            [b'<', ..] => ("svg", "image/svg+xml"),
            _ => ("png", "image/png"),
        };
        let p = (1..)
            .map(|n| PartName::new(&format!("/ppt/media/image{n}.{ext}")).expect("static"))
            .find(|p| !self.package.has_part(p))
            .unwrap();
        self.package.set_part(p.clone(), bytes.to_vec(), Some(ct));
        p.as_str().to_string()
    }

    /// Bytes and content type of a part (pictures for the renderer).
    pub fn part_bytes(&self, part: &str) -> Option<(String, Vec<u8>)> {
        let p = PartName::new(part).ok()?;
        let ct = self
            .package
            .content_type(&p)
            .unwrap_or("application/octet-stream")
            .to_string();
        Some((ct, self.package.part(&p)?.to_vec()))
    }
}
