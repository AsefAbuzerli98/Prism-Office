//! Shape trees (`p:spTree`, group shapes): shapes, pictures, connectors,
//! groups and graphic frames (tables, charts). Reading keeps every original
//! element so unchanged nodes are written back byte-for-byte; edited nodes
//! are regenerated, keeping unknown children (`extLst`, 3-D, …).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::dom::{El, XNode};
use crate::fill::{Effects, Fill, Line};
use crate::geom::{GeomDef, Guide, PathCmd, PathDef, PathFill};
use crate::text::TextBody;

fn none<T>(o: &Option<T>) -> bool {
    o.is_none()
}
fn no(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Xfrm {
    pub x: i64,
    pub y: i64,
    pub cx: i64,
    pub cy: i64,
    /// 60000ths of a degree, clockwise.
    #[serde(default)]
    pub rot: i64,
    #[serde(default, skip_serializing_if = "no")]
    pub flip_h: bool,
    #[serde(default, skip_serializing_if = "no")]
    pub flip_v: bool,
    /// Group child space: chOff x, y, chExt cx, cy.
    #[serde(default, skip_serializing_if = "none")]
    pub child: Option<[i64; 4]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Placeholder {
    /// title, body, ctrTitle, subTitle, dt, ftr, sldNum, pic, chart, tbl, obj …
    #[serde(default, skip_serializing_if = "none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "none")]
    pub idx: Option<u32>,
    #[serde(default, skip_serializing_if = "none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "none")]
    pub orient: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum Geometry {
    Preset {
        name: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        adj: Vec<(String, f64)>,
    },
    Custom {
        def: GeomDef,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StyleRef {
    pub idx: u32,
    #[serde(default, skip_serializing_if = "none")]
    pub color: Option<Color>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct ShapeStyle {
    #[serde(default, skip_serializing_if = "none")]
    pub line: Option<StyleRef>,
    #[serde(default, skip_serializing_if = "none")]
    pub fill: Option<StyleRef>,
    #[serde(default, skip_serializing_if = "none")]
    pub effect: Option<StyleRef>,
    /// major / minor / none and colour.
    #[serde(default, skip_serializing_if = "none")]
    pub font: Option<(String, Option<Color>)>,
}

/// Properties shared by every node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Common {
    pub id: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "none")]
    pub descr: Option<String>,
    #[serde(default, skip_serializing_if = "no")]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "none")]
    pub ph: Option<Placeholder>,
    #[serde(default, skip_serializing_if = "none")]
    pub xfrm: Option<Xfrm>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Shape {
    #[serde(flatten)]
    pub common: Common,
    #[serde(default, skip_serializing_if = "no")]
    pub text_box: bool,
    #[serde(default, skip_serializing_if = "none")]
    pub geom: Option<Geometry>,
    #[serde(default, skip_serializing_if = "none")]
    pub fill: Option<Fill>,
    #[serde(default, skip_serializing_if = "none")]
    pub line: Option<Line>,
    #[serde(default, skip_serializing_if = "none")]
    pub effects: Option<Effects>,
    #[serde(default, skip_serializing_if = "none")]
    pub style: Option<ShapeStyle>,
    #[serde(default, skip_serializing_if = "none")]
    pub text: Option<TextBody>,
    /// Connector endpoints (shape id, connection site).
    #[serde(default, skip_serializing_if = "none")]
    pub connect: Option<[Option<(u32, u32)>; 2]>,
    /// Prism extension data (e.g. a Cameo camera frame), stored in `extLst`.
    #[serde(default, skip_serializing_if = "none")]
    pub prism: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Picture {
    #[serde(flatten)]
    pub common: Common,
    /// The picture (`p:blipFill`).
    pub image: Fill,
    #[serde(default, skip_serializing_if = "none")]
    pub geom: Option<Geometry>,
    #[serde(default, skip_serializing_if = "none")]
    pub line: Option<Line>,
    #[serde(default, skip_serializing_if = "none")]
    pub effects: Option<Effects>,
    /// Video/audio relationship (`a:videoFile` / `p14:media`).
    #[serde(default, skip_serializing_if = "none")]
    pub media: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    #[serde(flatten)]
    pub common: Common,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TableCell {
    pub text: TextBody,
    #[serde(default, skip_serializing_if = "none")]
    pub fill: Option<Fill>,
    #[serde(default, skip_serializing_if = "none")]
    pub grid_span: Option<u32>,
    #[serde(default, skip_serializing_if = "none")]
    pub row_span: Option<u32>,
    #[serde(default, skip_serializing_if = "no")]
    pub h_merge: bool,
    #[serde(default, skip_serializing_if = "no")]
    pub v_merge: bool,
    /// Borders: left, right, top, bottom.
    #[serde(default, skip_serializing_if = "none")]
    pub borders: Option<[Option<Line>; 4]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    /// Column widths (EMU).
    pub cols: Vec<i64>,
    /// Row heights (EMU) and cells.
    pub rows: Vec<(i64, Vec<TableCell>)>,
    #[serde(default, skip_serializing_if = "none")]
    pub style_id: Option<String>,
    #[serde(default, skip_serializing_if = "no")]
    pub first_row: bool,
    #[serde(default, skip_serializing_if = "no")]
    pub band_row: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    #[serde(flatten)]
    pub common: Common,
    /// table, chart, diagram, ole, other
    pub kind: String,
    #[serde(default, skip_serializing_if = "none")]
    pub table: Option<Table>,
    /// Chart / diagram relationship id.
    #[serde(default, skip_serializing_if = "none")]
    pub rel_id: Option<String>,
}

/// Content kept verbatim (AlternateContent, content parts, unknown).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Other {
    #[serde(flatten)]
    pub common: Common,
    pub tag: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Node {
    Shape(Shape),
    Connector(Shape),
    Picture(Picture),
    Group(Group),
    Frame(Frame),
    Other(Other),
}

impl Node {
    pub fn common(&self) -> &Common {
        match self {
            Node::Shape(s) | Node::Connector(s) => &s.common,
            Node::Picture(p) => &p.common,
            Node::Group(g) => &g.common,
            Node::Frame(f) => &f.common,
            Node::Other(o) => &o.common,
        }
    }

    pub fn common_mut(&mut self) -> &mut Common {
        match self {
            Node::Shape(s) | Node::Connector(s) => &mut s.common,
            Node::Picture(p) => &mut p.common,
            Node::Group(g) => &mut g.common,
            Node::Frame(f) => &mut f.common,
            Node::Other(o) => &mut o.common,
        }
    }

    pub fn text(&self) -> Option<&TextBody> {
        match self {
            Node::Shape(s) | Node::Connector(s) => s.text.as_ref(),
            _ => None,
        }
    }
}

/// URI of the Prism extension in `extLst`.
pub const PRISM_EXT_URI: &str = "{6F1C2A9E-7B4D-4E52-9C3A-5D8B0E1F2A47}";

// ---- reading -------------------------------------------------------------------------------

fn xfrm(e: &El) -> Xfrm {
    let off = e.find("off");
    let ext = e.find("ext");
    let child = match (e.find("chOff"), e.find("chExt")) {
        (Some(o), Some(x)) => Some([
            o.attr_i64("x").unwrap_or(0),
            o.attr_i64("y").unwrap_or(0),
            x.attr_i64("cx").unwrap_or(0),
            x.attr_i64("cy").unwrap_or(0),
        ]),
        _ => None,
    };
    Xfrm {
        x: off.and_then(|o| o.attr_i64("x")).unwrap_or(0),
        y: off.and_then(|o| o.attr_i64("y")).unwrap_or(0),
        cx: ext.and_then(|o| o.attr_i64("cx")).unwrap_or(0),
        cy: ext.and_then(|o| o.attr_i64("cy")).unwrap_or(0),
        rot: e.attr_i64("rot").unwrap_or(0),
        flip_h: e.attr_bool("flipH").unwrap_or(false),
        flip_v: e.attr_bool("flipV").unwrap_or(false),
        child,
    }
}

fn guide_list(e: Option<&El>) -> Vec<Guide> {
    e.map(|l| {
        l.find_all("gd")
            .map(|g| {
                let name = g.attr("name").unwrap_or_default().to_string();
                let f: Vec<String> = g
                    .attr("fmla")
                    .unwrap_or("val 0")
                    .split_whitespace()
                    .map(String::from)
                    .collect();
                Guide {
                    name,
                    op: f.first().cloned().unwrap_or_else(|| "val".into()),
                    args: f[1.min(f.len())..].to_vec(),
                }
            })
            .collect()
    })
    .unwrap_or_default()
}

fn geometry(sppr: &El) -> Option<Geometry> {
    if let Some(p) = sppr.find("prstGeom") {
        let adj = guide_list(p.find("avLst"))
            .into_iter()
            .map(|g| {
                (
                    g.name,
                    g.args.first().and_then(|a| a.parse().ok()).unwrap_or(0.0),
                )
            })
            .collect();
        return Some(Geometry::Preset {
            name: p.attr("prst").unwrap_or("rect").to_string(),
            adj,
        });
    }
    let c = sppr.find("custGeom")?;
    let pt = |e: &El| -> (String, String) {
        let p = e.find("pt");
        (
            p.and_then(|p| p.attr("x")).unwrap_or("0").to_string(),
            p.and_then(|p| p.attr("y")).unwrap_or("0").to_string(),
        )
    };
    let pts = |e: &El| -> Vec<String> {
        e.find_all("pt")
            .flat_map(|p| {
                [
                    p.attr("x").unwrap_or("0").to_string(),
                    p.attr("y").unwrap_or("0").to_string(),
                ]
            })
            .collect()
    };
    let paths = c
        .find("pathLst")
        .map(|l| {
            l.find_all("path")
                .map(|p| {
                    let cmds = p
                        .elements()
                        .filter_map(|e| {
                            Some(match e.local() {
                                "moveTo" => {
                                    let (x, y) = pt(e);
                                    PathCmd::M { x, y }
                                }
                                "lnTo" => {
                                    let (x, y) = pt(e);
                                    PathCmd::L { x, y }
                                }
                                "arcTo" => PathCmd::A {
                                    wr: e.attr("wR").unwrap_or("0").into(),
                                    hr: e.attr("hR").unwrap_or("0").into(),
                                    st: e.attr("stAng").unwrap_or("0").into(),
                                    sw: e.attr("swAng").unwrap_or("0").into(),
                                },
                                "cubicBezTo" => {
                                    let v = pts(e);
                                    if v.len() < 6 {
                                        return None;
                                    }
                                    PathCmd::C {
                                        pts: [
                                            v[0].clone(),
                                            v[1].clone(),
                                            v[2].clone(),
                                            v[3].clone(),
                                            v[4].clone(),
                                            v[5].clone(),
                                        ],
                                    }
                                }
                                "quadBezTo" => {
                                    let v = pts(e);
                                    if v.len() < 4 {
                                        return None;
                                    }
                                    PathCmd::Q {
                                        pts: [
                                            v[0].clone(),
                                            v[1].clone(),
                                            v[2].clone(),
                                            v[3].clone(),
                                        ],
                                    }
                                }
                                "close" => PathCmd::Z,
                                _ => return None,
                            })
                        })
                        .collect();
                    PathDef {
                        w: p.attr_f64("w").unwrap_or(0.0),
                        h: p.attr_f64("h").unwrap_or(0.0),
                        fill: match p.attr("fill") {
                            Some("none") => PathFill::None,
                            Some("lighten") => PathFill::Lighten,
                            Some("lightenLess") => PathFill::LightenLess,
                            Some("darken") => PathFill::Darken,
                            Some("darkenLess") => PathFill::DarkenLess,
                            _ => PathFill::Norm,
                        },
                        stroke: p.attr_bool("stroke").unwrap_or(true),
                        cmds,
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let text = c
        .find("rect")
        .map(|r| ["l", "t", "r", "b"].map(|k| r.attr(k).unwrap_or("0").to_string()));
    Some(Geometry::Custom {
        def: GeomDef {
            av: guide_list(c.find("avLst")),
            gd: guide_list(c.find("gdLst")),
            paths,
            text,
        },
    })
}

fn style_ref(e: Option<&El>) -> Option<StyleRef> {
    let e = e?;
    Some(StyleRef {
        idx: e.attr_i64("idx").unwrap_or(0) as u32,
        color: Color::from_parent(e),
    })
}

fn style(e: &El) -> ShapeStyle {
    ShapeStyle {
        line: style_ref(e.find("lnRef")),
        fill: style_ref(e.find("fillRef")),
        effect: style_ref(e.find("effectRef")),
        font: e.find("fontRef").map(|f| {
            (
                f.attr("idx").unwrap_or("minor").to_string(),
                Color::from_parent(f),
            )
        }),
    }
}

fn common(nv: Option<&El>) -> Common {
    let Some(nv) = nv else {
        return Common::default();
    };
    let c = nv.find("cNvPr");
    let ph = nv.path(&["nvPr", "ph"]).map(|p| Placeholder {
        kind: p.attr("type").map(String::from),
        idx: p.attr_i64("idx").map(|i| i as u32),
        size: p.attr("sz").map(String::from),
        orient: p.attr("orient").map(String::from),
    });
    Common {
        id: c.and_then(|c| c.attr_i64("id")).unwrap_or(0) as u32,
        name: c
            .and_then(|c| c.attr("name"))
            .unwrap_or_default()
            .to_string(),
        descr: c.and_then(|c| c.attr("descr")).map(String::from),
        hidden: c.and_then(|c| c.attr_bool("hidden")).unwrap_or(false),
        ph,
        xfrm: None,
    }
}

fn prism_ext(e: Option<&El>) -> Option<serde_json::Value> {
    let ext = e?
        .find_all("ext")
        .find(|x| x.attr("uri") == Some(PRISM_EXT_URI))?;
    serde_json::from_str(&ext.text()).ok()
}

fn sp_common<'a>(e: &'a El, nv_name: &str) -> (Common, Option<&'a El>) {
    let mut c = common(e.find(nv_name));
    let sppr = e.find("spPr");
    c.xfrm = sppr.and_then(|s| s.find("xfrm")).map(xfrm);
    (c, sppr)
}

fn read_shape(e: &El, nv: &str) -> Shape {
    let (common, sppr) = sp_common(e, nv);
    let nvel = e.find(nv);
    let conn = nvel.and_then(|n| n.find("cNvCxnSpPr")).map(|c| {
        let end = |n: &str| {
            c.find(n).map(|x| {
                (
                    x.attr_i64("id").unwrap_or(0) as u32,
                    x.attr_i64("idx").unwrap_or(0) as u32,
                )
            })
        };
        [end("stCxn"), end("endCxn")]
    });
    Shape {
        common,
        text_box: nvel
            .and_then(|n| n.find("cNvSpPr"))
            .and_then(|c| c.attr_bool("txBox"))
            .unwrap_or(false),
        geom: sppr.and_then(geometry),
        fill: sppr.and_then(Fill::from_parent),
        line: sppr.and_then(|s| s.find("ln")).map(Line::from_el),
        effects: sppr.and_then(|s| s.find("effectLst")).map(Effects::from_el),
        style: e.find("style").map(style),
        text: e.find("txBody").map(TextBody::from_el),
        connect: conn.filter(|c| c.iter().any(|x| x.is_some())),
        prism: prism_ext(
            nvel.and_then(|n| n.find("cNvPr"))
                .and_then(|c| c.find("extLst")),
        ),
    }
}

/// Read one node; `None` for non-node elements (nvGrpSpPr, grpSpPr …).
pub fn read_node(e: &El) -> Option<Node> {
    Some(match e.local() {
        "sp" => Node::Shape(read_shape(e, "nvSpPr")),
        "cxnSp" => Node::Connector(read_shape(e, "nvCxnSpPr")),
        "pic" => {
            let (common, sppr) = sp_common(e, "nvPicPr");
            let media = e
                .path(&["nvPicPr", "nvPr"])
                .and_then(|nv| {
                    nv.elements()
                        .find(|x| matches!(x.local(), "videoFile" | "audioFile"))
                })
                .and_then(|v| v.attr("r:link").or(v.attr("link")).map(String::from));
            Node::Picture(Picture {
                common,
                image: e
                    .find("blipFill")
                    .and_then(Fill::from_el)
                    .unwrap_or(Fill::None),
                geom: sppr.and_then(geometry),
                line: sppr.and_then(|s| s.find("ln")).map(Line::from_el),
                effects: sppr.and_then(|s| s.find("effectLst")).map(Effects::from_el),
                media,
            })
        }
        "grpSp" => {
            let mut common = common(e.find("nvGrpSpPr"));
            common.xfrm = e.path(&["grpSpPr", "xfrm"]).map(xfrm);
            Node::Group(Group {
                common,
                children: e.elements().filter_map(read_node).collect(),
            })
        }
        "graphicFrame" => {
            let mut common = common(e.find("nvGraphicFramePr"));
            common.xfrm = e.find("xfrm").map(xfrm);
            let data = e.path(&["graphic", "graphicData"]);
            let uri = data.and_then(|d| d.attr("uri")).unwrap_or_default();
            let kind = if uri.ends_with("/table") {
                "table"
            } else if uri.ends_with("/chart") {
                "chart"
            } else if uri.contains("diagram") {
                "diagram"
            } else if uri.contains("ole") {
                "ole"
            } else {
                "other"
            };
            let rel_id = data
                .and_then(|d| d.elements().next())
                .and_then(|c| c.attr("r:id").or(c.attr("r:dm")))
                .map(String::from);
            Node::Frame(Frame {
                common,
                kind: kind.into(),
                table: data.and_then(|d| d.find("tbl")).map(read_table),
                rel_id,
            })
        }
        "AlternateContent" | "contentPart" => {
            // show the fallback's geometry when there is one
            let inner = e
                .find("Fallback")
                .and_then(|f| f.elements().find_map(read_node));
            let mut c = inner.map(|n| n.common().clone()).unwrap_or_default();
            if c.name.is_empty() {
                c.name = e.local().into();
            }
            Node::Other(Other {
                common: c,
                tag: e.local().into(),
            })
        }
        _ => return None,
    })
}

fn read_table(t: &El) -> Table {
    let cols = t
        .find("tblGrid")
        .map(|g| {
            g.find_all("gridCol")
                .map(|c| c.attr_i64("w").unwrap_or(0))
                .collect()
        })
        .unwrap_or_default();
    let pr = t.find("tblPr");
    let rows = t
        .find_all("tr")
        .map(|r| {
            let cells = r
                .find_all("tc")
                .map(|c| {
                    let tcpr = c.find("tcPr");
                    let border = |n: &str| tcpr.and_then(|p| p.find(n)).map(Line::from_el);
                    let borders = [border("lnL"), border("lnR"), border("lnT"), border("lnB")];
                    TableCell {
                        text: c.find("txBody").map(TextBody::from_el).unwrap_or_default(),
                        fill: tcpr.and_then(Fill::from_parent),
                        grid_span: c.attr_i64("gridSpan").map(|v| v as u32),
                        row_span: c.attr_i64("rowSpan").map(|v| v as u32),
                        h_merge: c.attr_bool("hMerge").unwrap_or(false),
                        v_merge: c.attr_bool("vMerge").unwrap_or(false),
                        borders: borders.iter().any(|b| b.is_some()).then_some(borders),
                    }
                })
                .collect();
            (r.attr_i64("h").unwrap_or(0), cells)
        })
        .collect();
    Table {
        cols,
        rows,
        style_id: pr.and_then(|p| p.find("tableStyleId")).map(|s| s.text()),
        first_row: pr.and_then(|p| p.attr_bool("firstRow")).unwrap_or(false),
        band_row: pr.and_then(|p| p.attr_bool("bandRow")).unwrap_or(false),
    }
}

/// Originals of every node by shape id (for lossless writing).
pub type Originals = HashMap<u32, El>;

/// Read the children of a shape tree (`p:spTree` / `p:grpSp`).
pub fn read_tree(tree: &El) -> (Vec<Node>, Originals) {
    let mut originals = HashMap::new();
    fn collect(e: &El, out: &mut Originals) {
        for c in e.elements() {
            if let Some(n) = read_node(c) {
                out.insert(n.common().id, c.clone());
                if c.local() == "grpSp" {
                    collect(c, out);
                }
            }
        }
    }
    collect(tree, &mut originals);
    (tree.elements().filter_map(read_node).collect(), originals)
}

// ---- writing ---------------------------------------------------------------------------------

fn xfrm_el(name: &str, x: &Xfrm) -> El {
    let mut e = El::new(name);
    if x.rot != 0 {
        e.set("rot", x.rot);
    }
    if x.flip_h {
        e.set("flipH", "1");
    }
    if x.flip_v {
        e.set("flipV", "1");
    }
    e.push(El::new("a:off").with("x", x.x).with("y", x.y));
    e.push(El::new("a:ext").with("cx", x.cx).with("cy", x.cy));
    if let Some([ox, oy, cx, cy]) = x.child {
        e.push(El::new("a:chOff").with("x", ox).with("y", oy));
        e.push(El::new("a:chExt").with("cx", cx).with("cy", cy));
    }
    e
}

fn geometry_el(g: &Geometry) -> El {
    match g {
        Geometry::Preset { name, adj } => {
            let mut av = El::new("a:avLst");
            for (k, v) in adj {
                av.push(
                    El::new("a:gd")
                        .with("name", k)
                        .with("fmla", format!("val {}", *v as i64)),
                );
            }
            El::new("a:prstGeom").with("prst", name).child(av)
        }
        Geometry::Custom { def } => {
            let gl = |name: &str, gs: &[Guide]| {
                let mut l = El::new(name);
                for g in gs {
                    l.push(
                        El::new("a:gd")
                            .with("name", &g.name)
                            .with("fmla", format!("{} {}", g.op, g.args.join(" ")).trim()),
                    );
                }
                l
            };
            let mut c = El::new("a:custGeom")
                .child(gl("a:avLst", &def.av))
                .child(gl("a:gdLst", &def.gd))
                .child(El::new("a:ahLst"))
                .child(El::new("a:cxnLst"));
            let r = def
                .text
                .clone()
                .unwrap_or_else(|| ["l".into(), "t".into(), "r".into(), "b".into()]);
            c.push(
                El::new("a:rect")
                    .with("l", &r[0])
                    .with("t", &r[1])
                    .with("r", &r[2])
                    .with("b", &r[3]),
            );
            let mut pl = El::new("a:pathLst");
            for p in &def.paths {
                let mut pe = El::new("a:path");
                if p.w > 0.0 {
                    pe.set("w", p.w as i64);
                }
                if p.h > 0.0 {
                    pe.set("h", p.h as i64);
                }
                match p.fill {
                    PathFill::Norm => {}
                    f => {
                        pe.set(
                            "fill",
                            serde_json::to_value(f)
                                .ok()
                                .and_then(|v| v.as_str().map(String::from))
                                .unwrap_or_default(),
                        );
                    }
                }
                if !p.stroke {
                    pe.set("stroke", "0");
                }
                let pt = |x: &str, y: &str| El::new("a:pt").with("x", x).with("y", y);
                for cmd in &p.cmds {
                    pe.push(match cmd {
                        PathCmd::M { x, y } => El::new("a:moveTo").child(pt(x, y)),
                        PathCmd::L { x, y } => El::new("a:lnTo").child(pt(x, y)),
                        PathCmd::A { wr, hr, st, sw } => El::new("a:arcTo")
                            .with("wR", wr)
                            .with("hR", hr)
                            .with("stAng", st)
                            .with("swAng", sw),
                        PathCmd::C { pts } => El::new("a:cubicBezTo")
                            .child(pt(&pts[0], &pts[1]))
                            .child(pt(&pts[2], &pts[3]))
                            .child(pt(&pts[4], &pts[5])),
                        PathCmd::Q { pts } => El::new("a:quadBezTo")
                            .child(pt(&pts[0], &pts[1]))
                            .child(pt(&pts[2], &pts[3])),
                        PathCmd::Z => El::new("a:close"),
                    });
                }
                pl.push(pe);
            }
            c.push(pl);
            c
        }
    }
}

/// Children of an original element not produced by the writer (kept on edit).
fn extras(orig: Option<&El>, known: &[&str]) -> Vec<El> {
    orig.map(|o| {
        o.elements()
            .filter(|e| !known.contains(&e.local()))
            .cloned()
            .collect()
    })
    .unwrap_or_default()
}

fn sppr_el(
    name: &str,
    xfrm: Option<&Xfrm>,
    geom: Option<&Geometry>,
    fill: Option<&Fill>,
    line: Option<&Line>,
    effects: Option<&Effects>,
    orig: Option<&El>,
) -> El {
    let mut e = El::new(name);
    if let Some(o) = orig {
        e.attrs = o.attrs.clone();
    }
    if let Some(x) = xfrm {
        e.push(xfrm_el("a:xfrm", x));
    }
    if let Some(g) = geom {
        e.push(geometry_el(g));
    }
    if let Some(f) = fill {
        e.push(f.to_el());
    }
    if let Some(l) = line {
        e.push(l.to_el());
    }
    let known = [
        "xfrm",
        "prstGeom",
        "custGeom",
        "noFill",
        "solidFill",
        "gradFill",
        "blipFill",
        "pattFill",
        "grpFill",
        "ln",
        "effectLst",
        "effectDag",
    ];
    let ext = extras(orig, &known);
    if let Some(fx) = effects {
        e.push(fx.to_el());
    } else if let Some(dag) = orig.and_then(|o| o.find("effectDag")) {
        e.push(dag.clone());
    }
    for x in ext {
        e.push(x);
    }
    e
}

fn style_el(ns: &str, s: &ShapeStyle) -> El {
    let r = |name: &str, x: &Option<StyleRef>| {
        let x = x.clone().unwrap_or(StyleRef {
            idx: 0,
            color: None,
        });
        let mut e = El::new(name).with("idx", x.idx);
        if let Some(c) = &x.color {
            e.push(c.to_el());
        }
        e
    };
    let mut e = El::new(&format!("{ns}:style"))
        .child(r("a:lnRef", &s.line))
        .child(r("a:fillRef", &s.fill))
        .child(r("a:effectRef", &s.effect));
    let (idx, c) = s.font.clone().unwrap_or(("minor".into(), None));
    let mut f = El::new("a:fontRef").with("idx", idx);
    if let Some(c) = c {
        f.push(c.to_el());
    }
    e.push(f);
    e
}

fn cnvpr_el(ns: &str, c: &Common, orig: Option<&El>, prism: Option<&serde_json::Value>) -> El {
    let mut e = El::new(&format!("{ns}:cNvPr"));
    if let Some(o) = orig {
        e.attrs = o.attrs.clone();
        e.children = o.children.clone();
    }
    e.set("id", c.id).set("name", &c.name);
    e.attrs.retain(|(k, _)| k != "descr" && k != "hidden");
    if let Some(d) = &c.descr {
        e.set("descr", d);
    }
    if c.hidden {
        e.set("hidden", "1");
    }
    // Prism extension in extLst
    e.children.retain(|x| !matches!(x, XNode::El(el) if el.local() == "extLst" && el.find_all("ext").any(|x| x.attr("uri") == Some(PRISM_EXT_URI))));
    if let Some(v) = prism {
        e.push(
            El::new("a:extLst").child(
                El::new("a:ext")
                    .with("uri", PRISM_EXT_URI)
                    .text_child(&v.to_string()),
            ),
        );
    }
    e
}

fn nvpr_el(ns: &str, ph: Option<&Placeholder>, orig: Option<&El>) -> El {
    let mut e = El::new(&format!("{ns}:nvPr"));
    if let Some(o) = orig {
        e.attrs = o.attrs.clone();
        e.children = o
            .children
            .iter()
            .filter(|c| !matches!(c, XNode::El(el) if el.local() == "ph"))
            .cloned()
            .collect();
    }
    if let Some(p) = ph {
        let mut pe = El::new(&format!("{ns}:ph"));
        if let Some(k) = &p.kind {
            pe.set("type", k);
        }
        if let Some(o) = &p.orient {
            pe.set("orient", o);
        }
        if let Some(s) = &p.size {
            pe.set("sz", s);
        }
        if let Some(i) = p.idx {
            pe.set("idx", i);
        }
        e.children.insert(0, XNode::El(pe));
    }
    e
}

fn write_shape(ns: &str, s: &Shape, connector: bool, orig: Option<&El>) -> El {
    let (tag, nv, cnv) = if connector {
        ("cxnSp", "nvCxnSpPr", "cNvCxnSpPr")
    } else {
        ("sp", "nvSpPr", "cNvSpPr")
    };
    let o_nv = orig.and_then(|o| o.find(nv));
    let mut c = El::new(&format!("{ns}:{cnv}"));
    if let Some(oc) = o_nv.and_then(|n| n.find(cnv)) {
        c.attrs = oc.attrs.clone();
        c.children = oc
            .children
            .iter()
            .filter(|x| !matches!(x, XNode::El(el) if matches!(el.local(), "stCxn" | "endCxn")))
            .cloned()
            .collect();
    }
    if !connector {
        c.attrs.retain(|(k, _)| k != "txBox");
        if s.text_box {
            c.set("txBox", "1");
        }
    } else if let Some([st, en]) = &s.connect {
        for (n, v) in [("a:stCxn", st), ("a:endCxn", en)] {
            if let Some((id, idx)) = v {
                c.children
                    .insert(0, XNode::El(El::new(n).with("id", id).with("idx", idx)));
            }
        }
    }
    let nvel = El::new(&format!("{ns}:{nv}"))
        .child(cnvpr_el(
            ns,
            &s.common,
            o_nv.and_then(|n| n.find("cNvPr")),
            s.prism.as_ref(),
        ))
        .child(c)
        .child(nvpr_el(
            ns,
            s.common.ph.as_ref(),
            o_nv.and_then(|n| n.find("nvPr")),
        ));
    let mut e = El::new(&format!("{ns}:{tag}"));
    if let Some(o) = orig {
        e.attrs = o.attrs.clone();
    }
    e.push(nvel);
    e.push(sppr_el(
        &format!("{ns}:spPr"),
        s.common.xfrm.as_ref(),
        s.geom.as_ref(),
        s.fill.as_ref(),
        s.line.as_ref(),
        s.effects.as_ref(),
        orig.and_then(|o| o.find("spPr")),
    ));
    if let Some(st) = &s.style {
        e.push(style_el(ns, st));
    }
    if let Some(t) = &s.text {
        // keep the original body properties' unknown children by regenerating only when text changed
        let orig_tx = orig.and_then(|o| o.find("txBody"));
        match orig_tx {
            Some(ot) if TextBody::from_el(ot) == *t => e.push(ot.clone()),
            _ => e.push(t.to_el(&format!("{ns}:txBody"))),
        }
    }
    for x in extras(orig, &[nv, "spPr", "style", "txBody"]) {
        e.push(x);
    }
    e
}

fn write_table(t: &Table) -> El {
    let mut tbl = El::new("a:tbl");
    let mut pr = El::new("a:tblPr");
    if t.first_row {
        pr.set("firstRow", "1");
    }
    if t.band_row {
        pr.set("bandRow", "1");
    }
    if let Some(s) = &t.style_id {
        pr.push(El::new("a:tableStyleId").text_child(s));
    }
    tbl.push(pr);
    let mut grid = El::new("a:tblGrid");
    for w in &t.cols {
        grid.push(El::new("a:gridCol").with("w", w));
    }
    tbl.push(grid);
    for (h, cells) in &t.rows {
        let mut tr = El::new("a:tr").with("h", h);
        for c in cells {
            let mut tc = El::new("a:tc");
            if let Some(v) = c.grid_span {
                tc.set("gridSpan", v);
            }
            if let Some(v) = c.row_span {
                tc.set("rowSpan", v);
            }
            if c.h_merge {
                tc.set("hMerge", "1");
            }
            if c.v_merge {
                tc.set("vMerge", "1");
            }
            tc.push(c.text.to_el("a:txBody"));
            let mut pr = El::new("a:tcPr");
            if let Some(b) = &c.borders {
                for (n, l) in ["a:lnL", "a:lnR", "a:lnT", "a:lnB"].iter().zip(b) {
                    if let Some(l) = l {
                        let mut le = l.to_el();
                        le.name = n.to_string();
                        pr.push(le);
                    }
                }
            }
            if let Some(f) = &c.fill {
                pr.push(f.to_el());
            }
            tc.push(pr);
            tr.push(tc);
        }
        tbl.push(tr);
    }
    tbl
}

/// Write one node, reusing its original when unchanged.
pub fn write_node(ns: &str, n: &Node, originals: &Originals) -> El {
    let orig = originals.get(&n.common().id).filter(|o| {
        read_node(o).is_some_and(|r| std::mem::discriminant(&r) == std::mem::discriminant(n))
    });
    if let Some(o) = orig {
        if read_node(o).as_ref() == Some(n) {
            return o.clone();
        }
    }
    match n {
        Node::Shape(s) => write_shape(ns, s, false, orig),
        Node::Connector(s) => write_shape(ns, s, true, orig),
        Node::Picture(p) => {
            let o_nv = orig.and_then(|o| o.find("nvPicPr"));
            let mut cnv = El::new(&format!("{ns}:cNvPicPr"));
            match o_nv.and_then(|n| n.find("cNvPicPr")) {
                Some(oc) => cnv = oc.clone(),
                None => cnv.push(El::new("a:picLocks").with("noChangeAspect", "1")),
            }
            let nv = El::new(&format!("{ns}:nvPicPr"))
                .child(cnvpr_el(
                    ns,
                    &p.common,
                    o_nv.and_then(|n| n.find("cNvPr")),
                    None,
                ))
                .child(cnv)
                .child(nvpr_el(
                    ns,
                    p.common.ph.as_ref(),
                    o_nv.and_then(|n| n.find("nvPr")),
                ));
            let mut blip = p.image.to_el();
            blip.name = format!("{ns}:blipFill");
            // keep the original blip's effects/extensions when the picture itself is unchanged
            if let Some(ob) = orig.and_then(|o| o.find("blipFill")) {
                if Fill::from_el(ob).as_ref() == Some(&p.image) {
                    blip = ob.clone();
                }
            }
            let geom = p.geom.clone().or(Some(Geometry::Preset {
                name: "rect".into(),
                adj: vec![],
            }));
            let mut e = El::new(&format!("{ns}:pic"))
                .child(nv)
                .child(blip)
                .child(sppr_el(
                    &format!("{ns}:spPr"),
                    p.common.xfrm.as_ref(),
                    geom.as_ref(),
                    None,
                    p.line.as_ref(),
                    p.effects.as_ref(),
                    orig.and_then(|o| o.find("spPr")),
                ));
            for x in extras(orig, &["nvPicPr", "blipFill", "spPr"]) {
                e.push(x);
            }
            e
        }
        Node::Group(g) => {
            let o_nv = orig.and_then(|o| o.find("nvGrpSpPr"));
            let cnv = o_nv
                .and_then(|n| n.find("cNvGrpSpPr"))
                .cloned()
                .unwrap_or_else(|| El::new(&format!("{ns}:cNvGrpSpPr")));
            let nv = El::new(&format!("{ns}:nvGrpSpPr"))
                .child(cnvpr_el(
                    ns,
                    &g.common,
                    o_nv.and_then(|n| n.find("cNvPr")),
                    None,
                ))
                .child(cnv)
                .child(nvpr_el(
                    ns,
                    g.common.ph.as_ref(),
                    o_nv.and_then(|n| n.find("nvPr")),
                ));
            let mut gp = El::new(&format!("{ns}:grpSpPr"));
            if let Some(x) = &g.common.xfrm {
                gp.push(xfrm_el("a:xfrm", x));
            }
            for x in extras(orig.and_then(|o| o.find("grpSpPr")), &["xfrm"]) {
                gp.push(x);
            }
            let mut e = El::new(&format!("{ns}:grpSp")).child(nv).child(gp);
            for c in &g.children {
                e.push(write_node(ns, c, originals));
            }
            e
        }
        Node::Frame(f) => {
            let Some(o) = orig else {
                // new tables only (charts are created through their own parts)
                let nv = El::new(&format!("{ns}:nvGraphicFramePr"))
                    .child(cnvpr_el(ns, &f.common, None, None))
                    .child(
                        El::new(&format!("{ns}:cNvGraphicFramePr"))
                            .child(El::new("a:graphicFrameLocks").with("noGrp", "1")),
                    )
                    .child(El::new(&format!("{ns}:nvPr")));
                let x = f.common.xfrm.clone().unwrap_or_default();
                let data = El::new("a:graphicData")
                    .with(
                        "uri",
                        "http://schemas.openxmlformats.org/drawingml/2006/table",
                    )
                    .child(write_table(&f.table.clone().unwrap_or_default()));
                return El::new(&format!("{ns}:graphicFrame"))
                    .child(nv)
                    .child(xfrm_el(&format!("{ns}:xfrm"), &x))
                    .child(El::new("a:graphic").child(data));
            };
            let mut e = o.clone();
            for c in e.children.iter_mut() {
                if let XNode::El(el) = c {
                    match el.local() {
                        "xfrm" => {
                            if let Some(x) = &f.common.xfrm {
                                *el = xfrm_el(&el.name.clone(), x);
                            }
                        }
                        "nvGraphicFramePr" => {
                            for cc in el.children.iter_mut() {
                                if let XNode::El(c2) = cc {
                                    if c2.local() == "cNvPr" {
                                        *c2 = cnvpr_el(ns, &f.common, Some(&c2.clone()), None);
                                    }
                                }
                            }
                        }
                        "graphic" => {
                            if let (Some(t), Some(XNode::El(data))) = (
                                &f.table,
                                el.children.iter_mut().find(
                                    |x| matches!(x, XNode::El(d) if d.local() == "graphicData"),
                                ),
                            ) {
                                for dc in data.children.iter_mut() {
                                    if let XNode::El(tb) = dc {
                                        if tb.local() == "tbl" && read_table(tb) != *t {
                                            let mut nt = write_table(t);
                                            // keep the original table properties (style overrides, effects)
                                            if let Some(pr) = tb.find("tblPr") {
                                                if let Some(XNode::El(npr)) =
                                                    nt.children.first_mut()
                                                {
                                                    let style = npr.clone();
                                                    *npr = pr.clone();
                                                    npr.attrs = style.attrs;
                                                }
                                            }
                                            *tb = nt;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            e
        }
        Node::Other(_) => orig
            .cloned()
            .unwrap_or_else(|| El::new(&format!("{ns}:sp"))),
    }
}

/// Write a whole tree (children only; the caller adds nvGrpSpPr/grpSpPr).
pub fn write_tree(ns: &str, nodes: &[Node], originals: &Originals) -> Vec<El> {
    nodes.iter().map(|n| write_node(ns, n, originals)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TREE: &str = r#"<p:spTree xmlns:p="urn:p" xmlns:a="urn:a" xmlns:r="urn:r"><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>
<p:sp><p:nvSpPr><p:cNvPr id="2" name="Title 1"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>Hello</a:t></a:r></a:p></p:txBody></p:sp>
<p:sp><p:nvSpPr><p:cNvPr id="3" name="Star"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm rot="600000"><a:off x="100" y="200"/><a:ext cx="300" cy="400"/></a:xfrm><a:prstGeom prst="star5"><a:avLst><a:gd name="adj" fmla="val 25000"/></a:avLst></a:prstGeom><a:solidFill><a:schemeClr val="accent2"/></a:solidFill><a:scene3d><a:camera prst="orthographicFront"/><a:lightRig rig="threePt" dir="t"/></a:scene3d></p:spPr><p:style><a:lnRef idx="2"><a:schemeClr val="accent1"><a:shade val="50000"/></a:schemeClr></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></p:style></p:sp>
<p:pic><p:nvPicPr><p:cNvPr id="4" name="Picture" descr="A cat"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId2"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="10" cy="10"/></a:xfrm><a:prstGeom prst="ellipse"><a:avLst/></a:prstGeom></p:spPr></p:pic>
<p:grpSp><p:nvGrpSpPr><p:cNvPr id="5" name="Group"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="10" cy="10"/><a:chOff x="0" y="0"/><a:chExt cx="10" cy="10"/></a:xfrm></p:grpSpPr><p:sp><p:nvSpPr><p:cNvPr id="6" name="Inner"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:sp></p:grpSp>
<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="7" name="Table"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblPr firstRow="1"><a:tableStyleId>{5C22544A}</a:tableStyleId></a:tblPr><a:tblGrid><a:gridCol w="50"/><a:gridCol w="50"/></a:tblGrid><a:tr h="20"><a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>A</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc><a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>B</a:t></a:r></a:p></a:txBody><a:tcPr/></a:tc></a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>
</p:spTree>"#;

    #[test]
    fn reads_and_writes_trees_losslessly() {
        let tree = El::parse(TREE).unwrap();
        let (nodes, originals) = read_tree(&tree);
        assert_eq!(nodes.len(), 5);
        let Node::Shape(title) = &nodes[0] else {
            panic!()
        };
        assert_eq!(
            title.common.ph.as_ref().unwrap().kind.as_deref(),
            Some("title")
        );
        assert_eq!(title.text.as_ref().unwrap().plain_text(), "Hello");
        let Node::Shape(star) = &nodes[1] else {
            panic!()
        };
        assert!(
            matches!(&star.geom, Some(Geometry::Preset { name, adj }) if name == "star5" && adj[0].1 == 25000.0)
        );
        assert_eq!(star.common.xfrm.as_ref().unwrap().rot, 600000);
        assert_eq!(star.style.as_ref().unwrap().fill.as_ref().unwrap().idx, 1);
        let Node::Picture(pic) = &nodes[2] else {
            panic!()
        };
        assert!(matches!(&pic.image, Fill::Image { rel_id, .. } if rel_id == "rId2"));
        let Node::Group(g) = &nodes[3] else { panic!() };
        assert_eq!(g.children.len(), 1);
        let Node::Frame(f) = &nodes[4] else { panic!() };
        assert_eq!(
            f.table.as_ref().unwrap().rows[0].1[1].text.plain_text(),
            "B"
        );

        // unchanged → identical XML
        let out = write_tree("p", &nodes, &originals);
        let orig: Vec<&El> = tree.elements().skip(2).collect();
        for (a, b) in out.iter().zip(orig) {
            assert_eq!(a.to_xml(), b.to_xml());
        }

        // edits regenerate but keep unknown children (scene3d) and re-read equal
        let mut edited = nodes.clone();
        if let Node::Shape(s) = &mut edited[1] {
            s.common.xfrm.as_mut().unwrap().x = 999;
            s.fill = Some(Fill::Solid {
                color: Color::srgb("00FF00"),
            });
        }
        if let Node::Shape(s) = &mut edited[0] {
            s.text = Some(TextBody {
                paragraphs: vec![crate::text::Paragraph::plain("Changed")],
                ..Default::default()
            });
        }
        if let Node::Frame(f) = &mut edited[4] {
            f.table.as_mut().unwrap().rows[0].1[0].text = TextBody {
                paragraphs: vec![crate::text::Paragraph::plain("Z")],
                ..Default::default()
            };
        }
        let out = write_tree("p", &edited, &originals);
        assert!(out[1].to_xml().contains("scene3d"));
        assert!(out[1].to_xml().contains(r#"<a:off x="999""#));
        assert!(out[4].to_xml().contains("{5C22544A}"));
        let reread: Vec<Node> = out.iter().filter_map(read_node).collect();
        assert_eq!(reread, edited);
    }
}
