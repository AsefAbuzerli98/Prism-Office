//! Text bodies (`p:txBody` / `a:txBody`): body properties, list styles,
//! paragraphs, runs, fields and bullets.

use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::dom::El;
use crate::fill::Fill;

fn none<T>(o: &Option<T>) -> bool {
    o.is_none()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunProps {
    /// Hundredths of a point.
    #[serde(default, skip_serializing_if = "none")]
    pub size: Option<i64>,
    #[serde(default, skip_serializing_if = "none")]
    pub bold: Option<bool>,
    #[serde(default, skip_serializing_if = "none")]
    pub italic: Option<bool>,
    /// sng, dbl, none …
    #[serde(default, skip_serializing_if = "none")]
    pub underline: Option<String>,
    /// sngStrike, dblStrike, noStrike
    #[serde(default, skip_serializing_if = "none")]
    pub strike: Option<String>,
    #[serde(default, skip_serializing_if = "none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "none")]
    pub fill: Option<Fill>,
    /// Superscript (>0) / subscript (<0) in 1000ths of a percent.
    #[serde(default, skip_serializing_if = "none")]
    pub baseline: Option<i64>,
    /// all, small, none
    #[serde(default, skip_serializing_if = "none")]
    pub cap: Option<String>,
    /// Character spacing in hundredths of a point.
    #[serde(default, skip_serializing_if = "none")]
    pub spacing: Option<i64>,
    #[serde(default, skip_serializing_if = "none")]
    pub highlight: Option<Color>,
    /// Hyperlink relationship id.
    #[serde(default, skip_serializing_if = "none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "none")]
    pub lang: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum Spacing {
    /// 1000ths of a percent of line height.
    Pct { v: i64 },
    /// Hundredths of a point.
    Pts { v: i64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum Bullet {
    None,
    #[serde(rename_all = "camelCase")]
    Char {
        ch: String,
        #[serde(default, skip_serializing_if = "none")]
        font: Option<String>,
        #[serde(default, skip_serializing_if = "none")]
        color: Option<Color>,
        /// Size in 1000ths of a percent of the text.
        #[serde(default, skip_serializing_if = "none")]
        size: Option<i64>,
    },
    #[serde(rename_all = "camelCase")]
    AutoNum {
        scheme: String,
        #[serde(default, skip_serializing_if = "none")]
        start: Option<i64>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ParaProps {
    #[serde(default, skip_serializing_if = "none")]
    pub level: Option<u8>,
    /// l, ctr, r, just, dist
    #[serde(default, skip_serializing_if = "none")]
    pub align: Option<String>,
    /// EMU
    #[serde(default, skip_serializing_if = "none")]
    pub margin_left: Option<i64>,
    #[serde(default, skip_serializing_if = "none")]
    pub indent: Option<i64>,
    #[serde(default, skip_serializing_if = "none")]
    pub line_spacing: Option<Spacing>,
    #[serde(default, skip_serializing_if = "none")]
    pub space_before: Option<Spacing>,
    #[serde(default, skip_serializing_if = "none")]
    pub space_after: Option<Spacing>,
    #[serde(default, skip_serializing_if = "none")]
    pub bullet: Option<Bullet>,
    #[serde(default, skip_serializing_if = "none")]
    pub rtl: Option<bool>,
    /// Default run properties (in list styles).
    #[serde(default, skip_serializing_if = "none")]
    pub run: Option<RunProps>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum Run {
    Text {
        text: String,
        #[serde(default)]
        props: RunProps,
    },
    Break {
        #[serde(default)]
        props: RunProps,
    },
    /// slidenum, datetime …
    #[serde(rename_all = "camelCase")]
    Field {
        kind: String,
        id: String,
        text: String,
        #[serde(default)]
        props: RunProps,
    },
}

impl Run {
    pub fn text(&self) -> &str {
        match self {
            Run::Text { text, .. } | Run::Field { text, .. } => text,
            Run::Break { .. } => "\n",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Paragraph {
    #[serde(default)]
    pub props: ParaProps,
    #[serde(default)]
    pub runs: Vec<Run>,
    #[serde(default, skip_serializing_if = "none")]
    pub end: Option<RunProps>,
}

impl Paragraph {
    pub fn plain(text: &str) -> Self {
        Paragraph {
            props: ParaProps::default(),
            runs: vec![Run::Text {
                text: text.into(),
                props: RunProps::default(),
            }],
            end: None,
        }
    }

    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text()).collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum Autofit {
    None,
    /// Shrink text: font scale and line-spacing reduction (1000ths of a percent).
    #[serde(rename_all = "camelCase")]
    Normal {
        font_scale: Option<i64>,
        line_reduction: Option<i64>,
    },
    /// Resize shape to fit text.
    Shape,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BodyProps {
    /// square / none
    #[serde(default, skip_serializing_if = "none")]
    pub wrap: Option<String>,
    /// t, ctr, b
    #[serde(default, skip_serializing_if = "none")]
    pub anchor: Option<String>,
    #[serde(default, skip_serializing_if = "none")]
    pub anchor_ctr: Option<bool>,
    /// Insets (EMU): l, t, r, b.
    #[serde(default, skip_serializing_if = "none")]
    pub insets: Option<[Option<i64>; 4]>,
    #[serde(default, skip_serializing_if = "none")]
    pub autofit: Option<Autofit>,
    /// horz, vert, vert270, wordArtVert …
    #[serde(default, skip_serializing_if = "none")]
    pub vert: Option<String>,
    #[serde(default, skip_serializing_if = "none")]
    pub rot: Option<i64>,
    #[serde(default, skip_serializing_if = "none")]
    pub columns: Option<u32>,
}

/// Levels 1–9 of a list style (index 0 = lvl1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListStyle {
    #[serde(default, skip_serializing_if = "none")]
    pub default: Option<ParaProps>,
    #[serde(default)]
    pub levels: Vec<Option<ParaProps>>,
}

impl ListStyle {
    pub fn is_empty(&self) -> bool {
        self.default.is_none() && self.levels.iter().all(|l| l.is_none())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TextBody {
    #[serde(default)]
    pub body: BodyProps,
    #[serde(default, skip_serializing_if = "ListStyle::is_empty")]
    pub list_style: ListStyle,
    #[serde(default)]
    pub paragraphs: Vec<Paragraph>,
}

// ---- reading -------------------------------------------------------------------------

fn spacing(e: Option<&El>) -> Option<Spacing> {
    let e = e?;
    if let Some(p) = e.find("spcPct") {
        return Some(Spacing::Pct {
            v: p.attr_i64("val")?,
        });
    }
    e.find("spcPts").and_then(|p| {
        Some(Spacing::Pts {
            v: p.attr_i64("val")?,
        })
    })
}

pub fn run_props(e: &El) -> RunProps {
    RunProps {
        size: e.attr_i64("sz"),
        bold: e.attr_bool("b"),
        italic: e.attr_bool("i"),
        underline: e.attr("u").map(String::from),
        strike: e.attr("strike").map(String::from),
        font: e
            .find("latin")
            .and_then(|l| l.attr("typeface"))
            .map(String::from),
        fill: Fill::from_parent(e),
        baseline: e.attr_i64("baseline"),
        cap: e.attr("cap").map(String::from),
        spacing: e.attr_i64("spc"),
        highlight: e.find("highlight").and_then(Color::from_parent),
        link: e
            .find("hlinkClick")
            .and_then(|h| h.attr("r:id").or(h.attr("id")))
            .map(String::from),
        lang: e.attr("lang").map(String::from),
    }
}

pub fn para_props(e: &El) -> ParaProps {
    let bullet = if e.find("buNone").is_some() {
        Some(Bullet::None)
    } else if let Some(c) = e.find("buChar") {
        Some(Bullet::Char {
            ch: c.attr("char").unwrap_or("•").to_string(),
            font: e
                .find("buFont")
                .and_then(|f| f.attr("typeface"))
                .map(String::from),
            color: e.find("buClr").and_then(Color::from_parent),
            size: e.find("buSzPct").and_then(|s| s.attr_i64("val")),
        })
    } else {
        e.find("buAutoNum").map(|a| Bullet::AutoNum {
            scheme: a.attr("type").unwrap_or("arabicPeriod").to_string(),
            start: a.attr_i64("startAt"),
        })
    };
    ParaProps {
        level: e.attr_i64("lvl").map(|l| l as u8),
        align: e.attr("algn").map(String::from),
        margin_left: e.attr_i64("marL"),
        indent: e.attr_i64("indent"),
        line_spacing: spacing(e.find("lnSpc")),
        space_before: spacing(e.find("spcBef")),
        space_after: spacing(e.find("spcAft")),
        bullet,
        rtl: e.attr_bool("rtl"),
        run: e.find("defRPr").map(run_props),
    }
}

pub fn list_style(e: &El) -> ListStyle {
    let mut levels = vec![None; 9];
    for (i, slot) in levels.iter_mut().enumerate() {
        if let Some(l) = e.find(&format!("lvl{}pPr", i + 1)) {
            *slot = Some(para_props(l));
        }
    }
    if levels.iter().all(|l| l.is_none()) {
        levels.clear();
    }
    ListStyle {
        default: e.find("defPPr").map(para_props),
        levels,
    }
}

pub fn body_props(e: &El) -> BodyProps {
    let ins = ["lIns", "tIns", "rIns", "bIns"].map(|k| e.attr_i64(k));
    BodyProps {
        wrap: e.attr("wrap").map(String::from),
        anchor: e.attr("anchor").map(String::from),
        anchor_ctr: e.attr_bool("anchorCtr"),
        insets: ins.iter().any(|i| i.is_some()).then_some(ins),
        autofit: if let Some(n) = e.find("normAutofit") {
            Some(Autofit::Normal {
                font_scale: n.attr_i64("fontScale"),
                line_reduction: n.attr_i64("lnSpcReduction"),
            })
        } else if e.find("spAutoFit").is_some() {
            Some(Autofit::Shape)
        } else if e.find("noAutofit").is_some() {
            Some(Autofit::None)
        } else {
            None
        },
        vert: e.attr("vert").map(String::from),
        rot: e.attr_i64("rot"),
        columns: e.attr_i64("numCol").map(|n| n as u32),
    }
}

impl TextBody {
    pub fn from_el(tx: &El) -> TextBody {
        let paragraphs = tx
            .find_all("p")
            .map(|p| {
                let mut runs = Vec::new();
                for c in p.elements() {
                    match c.local() {
                        "r" => runs.push(Run::Text {
                            text: c.find("t").map(|t| t.text()).unwrap_or_default(),
                            props: c.find("rPr").map(run_props).unwrap_or_default(),
                        }),
                        "br" => runs.push(Run::Break {
                            props: c.find("rPr").map(run_props).unwrap_or_default(),
                        }),
                        "fld" => runs.push(Run::Field {
                            kind: c.attr("type").unwrap_or_default().to_string(),
                            id: c.attr("id").unwrap_or_default().to_string(),
                            text: c.find("t").map(|t| t.text()).unwrap_or_default(),
                            props: c.find("rPr").map(run_props).unwrap_or_default(),
                        }),
                        _ => {}
                    }
                }
                Paragraph {
                    props: p.find("pPr").map(para_props).unwrap_or_default(),
                    runs,
                    end: p.find("endParaRPr").map(run_props),
                }
            })
            .collect();
        TextBody {
            body: tx.find("bodyPr").map(body_props).unwrap_or_default(),
            list_style: tx.find("lstStyle").map(list_style).unwrap_or_default(),
            paragraphs,
        }
    }

    pub fn plain_text(&self) -> String {
        self.paragraphs
            .iter()
            .map(|p| p.text())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ---- writing ---------------------------------------------------------------------------

fn spacing_el(name: &str, s: &Spacing) -> El {
    match s {
        Spacing::Pct { v } => El::new(name).child(El::new("a:spcPct").with("val", v)),
        Spacing::Pts { v } => El::new(name).child(El::new("a:spcPts").with("val", v)),
    }
}

fn b(v: bool) -> &'static str {
    if v {
        "1"
    } else {
        "0"
    }
}

pub fn run_props_el(name: &str, p: &RunProps) -> El {
    let mut e = El::new(name);
    if let Some(l) = &p.lang {
        e.set("lang", l);
    }
    if let Some(v) = p.size {
        e.set("sz", v);
    }
    if let Some(v) = p.bold {
        e.set("b", b(v));
    }
    if let Some(v) = p.italic {
        e.set("i", b(v));
    }
    if let Some(v) = &p.underline {
        e.set("u", v);
    }
    if let Some(v) = &p.strike {
        e.set("strike", v);
    }
    if let Some(v) = p.spacing {
        e.set("spc", v);
    }
    if let Some(v) = p.baseline {
        e.set("baseline", v);
    }
    if let Some(v) = &p.cap {
        e.set("cap", v);
    }
    // children in schema order: ln, fill, effect, highlight, …, latin, ea, cs, sym, hlinkClick
    if let Some(f) = &p.fill {
        e.push(f.to_el());
    }
    if let Some(h) = &p.highlight {
        e.push(El::new("a:highlight").child(h.to_el()));
    }
    if let Some(f) = &p.font {
        e.push(El::new("a:latin").with("typeface", f));
    }
    if let Some(l) = &p.link {
        e.push(El::new("a:hlinkClick").with("r:id", l));
    }
    e
}

pub fn para_props_el(name: &str, p: &ParaProps) -> El {
    let mut e = El::new(name);
    if let Some(v) = p.margin_left {
        e.set("marL", v);
    }
    if let Some(v) = p.level {
        e.set("lvl", v);
    }
    if let Some(v) = p.indent {
        e.set("indent", v);
    }
    if let Some(v) = &p.align {
        e.set("algn", v);
    }
    if let Some(v) = p.rtl {
        e.set("rtl", b(v));
    }
    if let Some(s) = &p.line_spacing {
        e.push(spacing_el("a:lnSpc", s));
    }
    if let Some(s) = &p.space_before {
        e.push(spacing_el("a:spcBef", s));
    }
    if let Some(s) = &p.space_after {
        e.push(spacing_el("a:spcAft", s));
    }
    match &p.bullet {
        Some(Bullet::None) => e.push(El::new("a:buNone")),
        Some(Bullet::Char {
            ch,
            font,
            color,
            size,
        }) => {
            if let Some(c) = color {
                e.push(El::new("a:buClr").child(c.to_el()));
            }
            if let Some(s) = size {
                e.push(El::new("a:buSzPct").with("val", s));
            }
            if let Some(f) = font {
                e.push(El::new("a:buFont").with("typeface", f));
            }
            e.push(El::new("a:buChar").with("char", ch));
        }
        Some(Bullet::AutoNum { scheme, start }) => {
            let mut a = El::new("a:buAutoNum").with("type", scheme);
            if let Some(s) = start {
                a.set("startAt", s);
            }
            e.push(a);
        }
        None => {}
    }
    if let Some(r) = &p.run {
        e.push(run_props_el("a:defRPr", r));
    }
    e
}

pub fn list_style_el(l: &ListStyle) -> El {
    let mut e = El::new("a:lstStyle");
    if let Some(d) = &l.default {
        e.push(para_props_el("a:defPPr", d));
    }
    for (i, lv) in l.levels.iter().enumerate() {
        if let Some(p) = lv {
            e.push(para_props_el(&format!("a:lvl{}pPr", i + 1), p));
        }
    }
    e
}

pub fn body_props_el(p: &BodyProps) -> El {
    let mut e = El::new("a:bodyPr");
    if let Some(v) = p.rot {
        e.set("rot", v);
    }
    if let Some(v) = &p.vert {
        e.set("vert", v);
    }
    if let Some(v) = &p.wrap {
        e.set("wrap", v);
    }
    if let Some(ins) = &p.insets {
        for (k, v) in ["lIns", "tIns", "rIns", "bIns"].iter().zip(ins) {
            if let Some(v) = v {
                e.set(k, v);
            }
        }
    }
    if let Some(v) = p.columns {
        e.set("numCol", v);
    }
    if let Some(v) = &p.anchor {
        e.set("anchor", v);
    }
    if let Some(v) = p.anchor_ctr {
        e.set("anchorCtr", b(v));
    }
    match &p.autofit {
        Some(Autofit::None) => e.push(El::new("a:noAutofit")),
        Some(Autofit::Shape) => e.push(El::new("a:spAutoFit")),
        Some(Autofit::Normal {
            font_scale,
            line_reduction,
        }) => {
            let mut n = El::new("a:normAutofit");
            if let Some(f) = font_scale {
                n.set("fontScale", f);
            }
            if let Some(l) = line_reduction {
                n.set("lnSpcReduction", l);
            }
            e.push(n);
        }
        None => {}
    }
    e
}

impl TextBody {
    /// Serialize with the container name (`p:txBody`, `a:txBody`, `wps:txbx` …).
    pub fn to_el(&self, name: &str) -> El {
        let mut tx = El::new(name)
            .child(body_props_el(&self.body))
            .child(list_style_el(&self.list_style));
        for p in &self.paragraphs {
            let mut pe = El::new("a:p");
            if p.props != ParaProps::default() {
                pe.push(para_props_el("a:pPr", &p.props));
            }
            for r in &p.runs {
                match r {
                    Run::Text { text, props } => pe.push(
                        El::new("a:r")
                            .child(run_props_el("a:rPr", props))
                            .child(El::new("a:t").text_child(text)),
                    ),
                    Run::Break { props } => {
                        pe.push(El::new("a:br").child(run_props_el("a:rPr", props)))
                    }
                    Run::Field {
                        kind,
                        id,
                        text,
                        props,
                    } => pe.push(
                        El::new("a:fld")
                            .with("id", id)
                            .with("type", kind)
                            .child(run_props_el("a:rPr", props))
                            .child(El::new("a:t").text_child(text)),
                    ),
                }
            }
            if let Some(e) = &p.end {
                pe.push(run_props_el("a:endParaRPr", e));
            }
            tx.push(pe);
        }
        tx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_body_round_trip() {
        let xml = r#"<p:txBody xmlns:p="urn:p" xmlns:a="urn:a"><a:bodyPr anchor="ctr" lIns="91440"><a:normAutofit fontScale="90000"/></a:bodyPr><a:lstStyle><a:lvl1pPr algn="ctr"><a:defRPr sz="4400"/></a:lvl1pPr></a:lstStyle><a:p><a:pPr lvl="1" algn="r"><a:lnSpc><a:spcPct val="90000"/></a:lnSpc><a:buFont typeface="Arial"/><a:buChar char="–"/></a:pPr><a:r><a:rPr lang="en-US" sz="2400" b="1"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill><a:latin typeface="Georgia"/></a:rPr><a:t>Hello</a:t></a:r><a:br><a:rPr lang="en-US"/></a:br><a:fld id="{1}" type="slidenum"><a:rPr lang="en-US"/><a:t>3</a:t></a:fld><a:endParaRPr lang="en-US"/></a:p></p:txBody>"#;
        let t = TextBody::from_el(&El::parse(xml).unwrap());
        assert_eq!(t.plain_text(), "Hello\n3");
        assert_eq!(t.body.anchor.as_deref(), Some("ctr"));
        assert!(matches!(
            t.body.autofit,
            Some(Autofit::Normal {
                font_scale: Some(90000),
                ..
            })
        ));
        assert_eq!(
            t.list_style.levels[0]
                .as_ref()
                .unwrap()
                .run
                .as_ref()
                .unwrap()
                .size,
            Some(4400)
        );
        let p = &t.paragraphs[0].props;
        assert_eq!((p.level, p.align.as_deref()), (Some(1), Some("r")));
        assert!(
            matches!(&p.bullet, Some(Bullet::Char { ch, font: Some(f), .. }) if ch == "–" && f == "Arial")
        );
        let again = TextBody::from_el(&t.to_el("p:txBody"));
        assert_eq!(again, t);
    }
}
