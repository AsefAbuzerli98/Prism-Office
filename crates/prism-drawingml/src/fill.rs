//! Fills, lines and effects (`a:solidFill`, `a:gradFill`, `a:blipFill`,
//! `a:pattFill`, `a:ln`, `a:effectLst`).

use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::dom::El;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    /// 0..=100000
    pub pos: i64,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum Fill {
    None,
    Solid {
        color: Color,
    },
    #[serde(rename_all = "camelCase")]
    Gradient {
        stops: Vec<GradientStop>,
        /// Linear angle in 60000ths of a degree; `None` with `path` set = radial/rect/shape.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        angle: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(default)]
        rot_with_shape: bool,
    },
    #[serde(rename_all = "camelCase")]
    Image {
        /// Relationship id of the picture part.
        rel_id: String,
        /// Crop in 1000ths of a percent (l, t, r, b).
        #[serde(default)]
        crop: [i64; 4],
        #[serde(default)]
        tile: bool,
    },
    Pattern {
        prst: String,
        fg: Color,
        bg: Color,
    },
    /// `a:grpFill`: use the group's fill.
    Group,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineEnd {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub len: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// Width in EMU.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Fill>,
    /// solid, dash, dot, lgDash, sysDash, dashDot …
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<String>,
    /// rnd, sq, flat
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap: Option<String>,
    /// round, bevel, miter
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<LineEnd>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail: Option<LineEnd>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shadow {
    pub inner: bool,
    /// EMU
    pub blur: i64,
    pub dist: i64,
    /// 60000ths of a degree
    pub dir: i64,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Effects {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<Shadow>,
    /// Glow radius (EMU) and colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glow: Option<(i64, Color)>,
    /// Soft edge radius (EMU).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soft_edge: Option<i64>,
    /// Reflection (blurred mirror) present.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reflection: bool,
}

impl Fill {
    /// Read a fill choice from `el` itself (when it is a fill element).
    pub fn from_el(e: &El) -> Option<Fill> {
        Some(match e.local() {
            "noFill" => Fill::None,
            "solidFill" => Fill::Solid {
                color: Color::from_parent(e)?,
            },
            "gradFill" => {
                let stops = e
                    .find("gsLst")
                    .map(|l| {
                        l.find_all("gs")
                            .filter_map(|g| {
                                Some(GradientStop {
                                    pos: g.attr_i64("pos").unwrap_or(0),
                                    color: Color::from_parent(g)?,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Fill::Gradient {
                    stops,
                    angle: e.find("lin").map(|l| l.attr_i64("ang").unwrap_or(0)),
                    path: e
                        .find("path")
                        .and_then(|p| p.attr("path"))
                        .map(String::from),
                    rot_with_shape: e.attr_bool("rotWithShape").unwrap_or(true),
                }
            }
            "blipFill" => {
                let blip = e.find("blip")?;
                let rel = blip
                    .attr("r:embed")
                    .or(blip.attr("embed"))
                    .unwrap_or_default()
                    .to_string();
                let crop = e
                    .find("srcRect")
                    .map(|s| {
                        [
                            s.attr_i64("l").unwrap_or(0),
                            s.attr_i64("t").unwrap_or(0),
                            s.attr_i64("r").unwrap_or(0),
                            s.attr_i64("b").unwrap_or(0),
                        ]
                    })
                    .unwrap_or_default();
                Fill::Image {
                    rel_id: rel,
                    crop,
                    tile: e.find("tile").is_some(),
                }
            }
            "pattFill" => Fill::Pattern {
                prst: e.attr("prst").unwrap_or("pct5").to_string(),
                fg: e
                    .find("fgClr")
                    .and_then(Color::from_parent)
                    .unwrap_or_else(|| Color::srgb("000000")),
                bg: e
                    .find("bgClr")
                    .and_then(Color::from_parent)
                    .unwrap_or_else(|| Color::srgb("FFFFFF")),
            },
            "grpFill" => Fill::Group,
            _ => return None,
        })
    }

    /// First fill child of `parent` (e.g. `p:spPr`).
    pub fn from_parent(parent: &El) -> Option<Fill> {
        parent.elements().find_map(Fill::from_el)
    }

    pub fn to_el(&self) -> El {
        match self {
            Fill::None => El::new("a:noFill"),
            Fill::Solid { color } => El::new("a:solidFill").child(color.to_el()),
            Fill::Gradient {
                stops,
                angle,
                path,
                rot_with_shape,
            } => {
                let mut g = El::new("a:gradFill")
                    .with("rotWithShape", if *rot_with_shape { "1" } else { "0" });
                let mut l = El::new("a:gsLst");
                for s in stops {
                    l.push(El::new("a:gs").with("pos", s.pos).child(s.color.to_el()));
                }
                g.push(l);
                if let Some(p) = path {
                    g.push(
                        El::new("a:path").with("path", p).child(
                            El::new("a:fillToRect")
                                .with("l", 50000)
                                .with("t", 50000)
                                .with("r", 50000)
                                .with("b", 50000),
                        ),
                    );
                } else {
                    g.push(
                        El::new("a:lin")
                            .with("ang", angle.unwrap_or(0))
                            .with("scaled", 0),
                    );
                }
                g
            }
            Fill::Image { rel_id, crop, tile } => {
                let mut b = El::new("a:blipFill").child(El::new("a:blip").with("r:embed", rel_id));
                if crop.iter().any(|c| *c != 0) {
                    b.push(
                        El::new("a:srcRect")
                            .with("l", crop[0])
                            .with("t", crop[1])
                            .with("r", crop[2])
                            .with("b", crop[3]),
                    );
                }
                b.push(if *tile {
                    El::new("a:tile")
                } else {
                    El::new("a:stretch").child(El::new("a:fillRect"))
                });
                b
            }
            Fill::Pattern { prst, fg, bg } => El::new("a:pattFill")
                .with("prst", prst)
                .child(El::new("a:fgClr").child(fg.to_el()))
                .child(El::new("a:bgClr").child(bg.to_el())),
            Fill::Group => El::new("a:grpFill"),
        }
    }
}

impl Line {
    pub fn from_el(ln: &El) -> Line {
        let end = |name: &str| {
            ln.find(name)
                .filter(|e| e.attr("type").is_some_and(|t| t != "none"))
                .map(|e| LineEnd {
                    kind: e.attr("type").unwrap_or("none").into(),
                    w: e.attr("w").map(String::from),
                    len: e.attr("len").map(String::from),
                })
        };
        Line {
            width: ln.attr_i64("w"),
            fill: Fill::from_parent(ln),
            dash: ln
                .find("prstDash")
                .and_then(|d| d.attr("val"))
                .map(String::from),
            cap: ln.attr("cap").map(String::from),
            join: ln
                .elements()
                .find(|e| matches!(e.local(), "round" | "bevel" | "miter"))
                .map(|e| e.local().to_string()),
            head: end("headEnd"),
            tail: end("tailEnd"),
        }
    }

    pub fn to_el(&self) -> El {
        let mut e = El::new("a:ln");
        if let Some(w) = self.width {
            e.set("w", w);
        }
        if let Some(c) = &self.cap {
            e.set("cap", c);
        }
        if let Some(f) = &self.fill {
            e.push(f.to_el());
        }
        if let Some(d) = &self.dash {
            e.push(El::new("a:prstDash").with("val", d));
        }
        if let Some(j) = &self.join {
            e.push(El::new(&format!("a:{j}")));
        }
        for (name, end) in [("a:headEnd", &self.head), ("a:tailEnd", &self.tail)] {
            if let Some(x) = end {
                let mut h = El::new(name).with("type", &x.kind);
                if let Some(w) = &x.w {
                    h.set("w", w);
                }
                if let Some(l) = &x.len {
                    h.set("len", l);
                }
                e.push(h);
            }
        }
        e
    }
}

impl Effects {
    pub fn from_el(lst: &El) -> Effects {
        let shadow = lst
            .elements()
            .find(|e| matches!(e.local(), "outerShdw" | "innerShdw"))
            .and_then(|s| {
                Some(Shadow {
                    inner: s.local() == "innerShdw",
                    blur: s.attr_i64("blurRad").unwrap_or(0),
                    dist: s.attr_i64("dist").unwrap_or(0),
                    dir: s.attr_i64("dir").unwrap_or(0),
                    color: Color::from_parent(s)?,
                })
            });
        Effects {
            shadow,
            glow: lst
                .find("glow")
                .and_then(|g| Some((g.attr_i64("rad").unwrap_or(0), Color::from_parent(g)?))),
            soft_edge: lst.find("softEdge").and_then(|s| s.attr_i64("rad")),
            reflection: lst.find("reflection").is_some(),
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Effects::default()
    }

    /// `a:effectLst` (children in schema order: glow, innerShdw, outerShdw, reflection, softEdge).
    pub fn to_el(&self) -> El {
        let mut e = El::new("a:effectLst");
        if let Some((rad, c)) = &self.glow {
            e.push(El::new("a:glow").with("rad", rad).child(c.to_el()));
        }
        if let Some(s) = &self.shadow {
            let mut sh = El::new(if s.inner {
                "a:innerShdw"
            } else {
                "a:outerShdw"
            })
            .with("blurRad", s.blur)
            .with("dist", s.dist)
            .with("dir", s.dir);
            if !s.inner {
                sh.set("algn", "tl").set("rotWithShape", "0");
            }
            e.push(sh.child(s.color.to_el()));
        }
        if self.reflection {
            e.push(
                El::new("a:reflection")
                    .with("blurRad", 6350)
                    .with("stA", 50000)
                    .with("endA", 300)
                    .with("endPos", 55000)
                    .with("dir", 5400000)
                    .with("sy", -100000)
                    .with("algn", "bl")
                    .with("rotWithShape", "0"),
            );
        }
        if let Some(r) = self.soft_edge {
            e.push(El::new("a:softEdge").with("rad", r));
        }
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_lines_effects_round_trip() {
        for xml in [
            r#"<a:solidFill><a:schemeClr val="accent1"><a:lumMod val="75000"/></a:schemeClr></a:solidFill>"#,
            r#"<a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill>"#,
            r#"<a:noFill/>"#,
        ] {
            let f = Fill::from_el(&El::parse(xml).unwrap()).unwrap();
            assert_eq!(
                Fill::from_el(&El::parse(&f.to_el().to_xml()).unwrap()),
                Some(f)
            );
        }
        let ln = Line::from_el(&El::parse(r#"<a:ln w="19050" cap="rnd"><a:solidFill><a:srgbClr val="000000"/></a:solidFill><a:prstDash val="dash"/><a:round/><a:tailEnd type="triangle"/></a:ln>"#).unwrap());
        assert_eq!(ln.width, Some(19050));
        assert_eq!(ln.tail.as_ref().unwrap().kind, "triangle");
        assert_eq!(Line::from_el(&El::parse(&ln.to_el().to_xml()).unwrap()), ln);
        let fx = Effects::from_el(&El::parse(r#"<a:effectLst><a:outerShdw blurRad="50800" dist="38100" dir="2700000"><a:prstClr val="black"><a:alpha val="40000"/></a:prstClr></a:outerShdw><a:softEdge rad="12700"/></a:effectLst>"#).unwrap());
        assert_eq!(fx.shadow.as_ref().unwrap().blur, 50800);
        assert_eq!(
            Effects::from_el(&El::parse(&fx.to_el().to_xml()).unwrap()),
            fx
        );
    }
}
