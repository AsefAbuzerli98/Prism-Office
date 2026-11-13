//! DrawingML colours: srgb/scheme/system/preset/HSL/scRGB bases with
//! transforms (lumMod, lumOff, tint, shade, alpha, satMod …), and themes.

use serde::{Deserialize, Serialize};

use crate::dom::El;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "camelCase")]
pub enum ColorBase {
    /// `RRGGBB`
    Srgb {
        v: String,
    },
    /// Theme slot: dk1, lt1, accent1 … or tx1/bg1 (mapped through clrMap), phClr.
    Scheme {
        v: String,
    },
    Sys {
        v: String,
        last: Option<String>,
    },
    Preset {
        v: String,
    },
    Hsl {
        h: f64,
        s: f64,
        l: f64,
    },
    ScRgb {
        r: f64,
        g: f64,
        b: f64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorMod {
    pub name: String,
    pub val: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub base: ColorBase,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mods: Vec<ColorMod>,
}

/// Resolved colour.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
}

impl Rgba {
    pub fn css(&self) -> String {
        if self.a >= 0.999 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("rgba({},{},{},{:.3})", self.r, self.g, self.b, self.a)
        }
    }
}

/// Theme colour scheme in slot order dk1, lt1, dk2, lt2, accent1-6, hlink, folHlink.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub name: String,
    pub colors: Vec<(String, String)>,
    pub major_font: String,
    pub minor_font: String,
    /// fillStyleLst / lnStyleLst / effectStyleLst / bgFillStyleLst entries (raw XML).
    #[serde(default)]
    pub fill_styles: Vec<String>,
    #[serde(default)]
    pub line_styles: Vec<String>,
    #[serde(default)]
    pub bg_fill_styles: Vec<String>,
}

impl Default for Theme {
    fn default() -> Self {
        let c = |k: &str, v: &str| (k.to_string(), v.to_string());
        Theme {
            name: "Office Theme".into(),
            colors: vec![
                c("dk1", "000000"),
                c("lt1", "FFFFFF"),
                c("dk2", "44546A"),
                c("lt2", "E7E6E6"),
                c("accent1", "4472C4"),
                c("accent2", "ED7D31"),
                c("accent3", "A5A5A5"),
                c("accent4", "FFC000"),
                c("accent5", "5B9BD5"),
                c("accent6", "70AD47"),
                c("hlink", "0563C1"),
                c("folHlink", "954F72"),
            ],
            major_font: "Calibri Light".into(),
            minor_font: "Calibri".into(),
            fill_styles: vec![],
            line_styles: vec![],
            bg_fill_styles: vec![],
        }
    }
}

impl Theme {
    /// Parse `a:theme` XML.
    pub fn parse(xml: &str) -> Theme {
        let mut t = Theme::default();
        let Some(root) = El::parse(xml) else { return t };
        t.name = root.attr("name").unwrap_or("Theme").to_string();
        if let Some(cs) = root.path(&["themeElements", "clrScheme"]) {
            let mut colors = Vec::new();
            for e in cs.elements() {
                let v = e.elements().next().and_then(|c| match c.local() {
                    "srgbClr" => c.attr("val").map(String::from),
                    "sysClr" => c.attr("lastClr").or(c.attr("val")).map(|v| {
                        if v == "windowText" {
                            "000000".into()
                        } else if v == "window" {
                            "FFFFFF".into()
                        } else {
                            v.to_string()
                        }
                    }),
                    _ => None,
                });
                if let Some(v) = v {
                    colors.push((e.local().to_string(), v));
                }
            }
            if colors.len() >= 10 {
                t.colors = colors;
            }
        }
        if let Some(fs) = root.path(&["themeElements", "fontScheme"]) {
            if let Some(v) = fs
                .path(&["majorFont", "latin"])
                .and_then(|l| l.attr("typeface"))
            {
                t.major_font = v.into();
            }
            if let Some(v) = fs
                .path(&["minorFont", "latin"])
                .and_then(|l| l.attr("typeface"))
            {
                t.minor_font = v.into();
            }
        }
        if let Some(fmt) = root.path(&["themeElements", "fmtScheme"]) {
            let list = |name: &str| {
                fmt.find(name)
                    .map(|l| l.elements().map(|e| e.to_xml()).collect())
                    .unwrap_or_default()
            };
            t.fill_styles = list("fillStyleLst");
            t.line_styles = list("lnStyleLst");
            t.bg_fill_styles = list("bgFillStyleLst");
        }
        t
    }

    pub fn color(&self, slot: &str) -> Option<&str> {
        self.colors
            .iter()
            .find(|(k, _)| k == slot)
            .map(|(_, v)| v.as_str())
    }

    /// Theme font for `+mj-lt` / `+mn-lt` references.
    pub fn font(&self, typeface: &str) -> String {
        match typeface {
            "+mj-lt" | "+mj-ea" | "+mj-cs" => self.major_font.clone(),
            "+mn-lt" | "+mn-ea" | "+mn-cs" => self.minor_font.clone(),
            other => other.to_string(),
        }
    }
}

/// Slide colour map (bg1 → lt1 …).
pub fn map_slot<'a>(slot: &'a str, clr_map: &'a [(String, String)]) -> &'a str {
    if let Some((_, v)) = clr_map.iter().find(|(k, _)| k == slot) {
        return v;
    }
    match slot {
        "bg1" => "lt1",
        "tx1" => "dk1",
        "bg2" => "lt2",
        "tx2" => "dk2",
        s => s,
    }
}

fn preset_rgb(name: &str) -> Option<(u8, u8, u8)> {
    Some(match name {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "lime" => (0, 255, 0),
        "blue" => (0, 0, 255),
        "yellow" => (255, 255, 0),
        "cyan" | "aqua" => (0, 255, 255),
        "magenta" | "fuchsia" => (255, 0, 255),
        "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192),
        "orange" => (255, 165, 0),
        "purple" => (128, 0, 128),
        "navy" => (0, 0, 128),
        "maroon" => (128, 0, 0),
        "olive" => (128, 128, 0),
        "teal" => (0, 128, 128),
        "darkGray" => (169, 169, 169),
        "lightGray" => (211, 211, 211),
        _ => return None,
    })
}

fn hex(s: &str) -> Option<(u8, u8, u8)> {
    let v = u32::from_str_radix(s.get(..6)?, 16).ok()?;
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

fn rgb_to_hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    (h, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

fn to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(c: f64) -> f64 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

impl Color {
    pub fn srgb(hex: &str) -> Self {
        Color {
            base: ColorBase::Srgb {
                v: hex.trim_start_matches('#').to_uppercase(),
            },
            mods: vec![],
        }
    }

    pub fn scheme(slot: &str) -> Self {
        Color {
            base: ColorBase::Scheme { v: slot.into() },
            mods: vec![],
        }
    }

    /// Read from a colour choice element's parent (e.g. `a:solidFill`).
    pub fn from_parent(parent: &El) -> Option<Color> {
        parent.elements().find_map(Color::from_el)
    }

    pub fn from_el(e: &El) -> Option<Color> {
        let base = match e.local() {
            "srgbClr" => ColorBase::Srgb {
                v: e.attr("val")?.to_uppercase(),
            },
            "schemeClr" => ColorBase::Scheme {
                v: e.attr("val")?.into(),
            },
            "sysClr" => ColorBase::Sys {
                v: e.attr("val")?.into(),
                last: e.attr("lastClr").map(String::from),
            },
            "prstClr" => ColorBase::Preset {
                v: e.attr("val")?.into(),
            },
            "hslClr" => ColorBase::Hsl {
                h: e.attr_f64("hue").unwrap_or(0.0),
                s: e.attr_f64("sat").unwrap_or(0.0),
                l: e.attr_f64("lum").unwrap_or(0.0),
            },
            "scrgbClr" => ColorBase::ScRgb {
                r: e.attr_f64("r").unwrap_or(0.0),
                g: e.attr_f64("g").unwrap_or(0.0),
                b: e.attr_f64("b").unwrap_or(0.0),
            },
            _ => return None,
        };
        let mods = e
            .elements()
            .map(|m| ColorMod {
                name: m.local().to_string(),
                val: m.attr_i64("val").unwrap_or(0),
            })
            .collect();
        Some(Color { base, mods })
    }

    /// Serialize as `a:*Clr`.
    pub fn to_el(&self) -> El {
        let mut e = match &self.base {
            ColorBase::Srgb { v } => El::new("a:srgbClr").with("val", v),
            ColorBase::Scheme { v } => El::new("a:schemeClr").with("val", v),
            ColorBase::Sys { v, last } => {
                let mut e = El::new("a:sysClr").with("val", v);
                if let Some(l) = last {
                    e.set("lastClr", l);
                }
                e
            }
            ColorBase::Preset { v } => El::new("a:prstClr").with("val", v),
            ColorBase::Hsl { h, s, l } => El::new("a:hslClr")
                .with("hue", h)
                .with("sat", s)
                .with("lum", l),
            ColorBase::ScRgb { r, g, b } => {
                El::new("a:scrgbClr").with("r", r).with("g", g).with("b", b)
            }
        };
        for m in &self.mods {
            e.push(El::new(&format!("a:{}", m.name)).with("val", m.val));
        }
        e
    }

    /// Resolve against a theme, colour map and the placeholder colour (`phClr`).
    pub fn resolve(&self, theme: &Theme, clr_map: &[(String, String)], ph: Option<Rgba>) -> Rgba {
        let (mut r, mut g, mut b) = match &self.base {
            ColorBase::Srgb { v } => hex(v).unwrap_or((0, 0, 0)),
            ColorBase::Scheme { v } if v == "phClr" => {
                ph.map(|c| (c.r, c.g, c.b)).unwrap_or((0, 0, 0))
            }
            ColorBase::Scheme { v } => theme
                .color(map_slot(v, clr_map))
                .and_then(hex)
                .unwrap_or((0, 0, 0)),
            ColorBase::Sys { v, last } => {
                last.as_deref().and_then(hex).unwrap_or(if v == "window" {
                    (255, 255, 255)
                } else {
                    (0, 0, 0)
                })
            }
            ColorBase::Preset { v } => preset_rgb(v).unwrap_or((0, 0, 0)),
            ColorBase::Hsl { h, s, l } => {
                let (r, g, b) = hsl_to_rgb(h / 21600000.0, s / 100000.0, l / 100000.0);
                ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
            }
            ColorBase::ScRgb { r, g, b } => {
                let c = |x: f64| (to_srgb(x / 100000.0).clamp(0.0, 1.0) * 255.0).round() as u8;
                (c(*r), c(*g), c(*b))
            }
        }
        .into_f();
        let mut a = 1.0;
        for m in &self.mods {
            let v = m.val as f64 / 100000.0;
            match m.name.as_str() {
                "alpha" => a = v,
                "alphaMod" => a *= v,
                "alphaOff" => a += v,
                "lumMod" | "lumOff" | "satMod" | "satOff" | "hueMod" | "hueOff" | "lum" | "sat"
                | "hue" => {
                    let (mut h, mut s, mut l) = rgb_to_hsl(r, g, b);
                    match m.name.as_str() {
                        "lumMod" => l *= v,
                        "lumOff" => l += v,
                        "lum" => l = v,
                        "satMod" => s *= v,
                        "satOff" => s += v,
                        "sat" => s = v,
                        "hueMod" => h *= v,
                        "hueOff" => h += m.val as f64 / 21600000.0,
                        _ => h = m.val as f64 / 21600000.0,
                    }
                    let (nr, ng, nb) =
                        hsl_to_rgb(h.rem_euclid(1.0), s.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
                    (r, g, b) = (nr, ng, nb);
                }
                "tint" => {
                    // towards white in linear light
                    let f = |c: f64| to_srgb(to_linear(c) * v + (1.0 - v));
                    (r, g, b) = (f(r), f(g), f(b));
                }
                "shade" => {
                    let f = |c: f64| to_srgb(to_linear(c) * v);
                    (r, g, b) = (f(r), f(g), f(b));
                }
                "inv" => (r, g, b) = (1.0 - r, 1.0 - g, 1.0 - b),
                "gray" => {
                    let y = 0.3 * r + 0.59 * g + 0.11 * b;
                    (r, g, b) = (y, y, y);
                }
                "comp" => {
                    let (h, s, l) = rgb_to_hsl(r, g, b);
                    (r, g, b) = hsl_to_rgb((h + 0.5).rem_euclid(1.0), s, l);
                }
                "redMod" => r *= v,
                "greenMod" => g *= v,
                "blueMod" => b *= v,
                _ => {}
            }
        }
        let c = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
        Rgba {
            r: c(r),
            g: c(g),
            b: c(b),
            a: a.clamp(0.0, 1.0),
        }
    }
}

trait IntoF {
    fn into_f(self) -> (f64, f64, f64);
}
impl IntoF for (u8, u8, u8) {
    fn into_f(self) -> (f64, f64, f64) {
        (
            self.0 as f64 / 255.0,
            self.1 as f64 / 255.0,
            self.2 as f64 / 255.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn css(xml: &str) -> String {
        let e = El::parse(xml).unwrap();
        Color::from_el(&e)
            .unwrap()
            .resolve(&Theme::default(), &[], None)
            .css()
    }

    #[test]
    fn resolves_theme_colours_and_transforms() {
        assert_eq!(css(r#"<a:srgbClr val="FF0000"/>"#), "#ff0000");
        assert_eq!(css(r#"<a:schemeClr val="accent1"/>"#), "#4472c4");
        assert_eq!(css(r#"<a:schemeClr val="tx1"/>"#), "#000000");
        // "Blue, Accent 1, Lighter 40%" → 8FAADC in PowerPoint's palette
        assert_eq!(
            css(
                r#"<a:schemeClr val="accent1"><a:lumMod val="60000"/><a:lumOff val="40000"/></a:schemeClr>"#
            ),
            "#8faadc"
        );
        // "Darker 25%" → 2F5597
        assert_eq!(
            css(r#"<a:schemeClr val="accent1"><a:lumMod val="75000"/></a:schemeClr>"#),
            "#2f5597"
        );
        assert_eq!(
            css(r#"<a:srgbClr val="000000"><a:alpha val="50000"/></a:srgbClr>"#),
            "rgba(0,0,0,0.500)"
        );
        let c = Color::from_el(
            &El::parse(r#"<a:schemeClr val="accent2"><a:lumMod val="50000"/></a:schemeClr>"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            El::parse(&c.to_el().to_xml()).and_then(|e| Color::from_el(&e)),
            Some(c)
        );
    }
}
