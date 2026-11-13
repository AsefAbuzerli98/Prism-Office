//! DrawingML geometry: the shape-guide formula language (ECMA-376 §20.1.9),
//! preset shape definitions and custom geometry, evaluated into absolute
//! paths (move / line / cubic / quadratic / close).

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Fill treatment of one sub-path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PathFill {
    #[default]
    Norm,
    None,
    Lighten,
    LightenLess,
    Darken,
    DarkenLess,
}

/// A path command in shape coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "c")]
pub enum Cmd {
    M {
        x: f64,
        y: f64,
    },
    L {
        x: f64,
        y: f64,
    },
    C {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
    },
    Q {
        x1: f64,
        y1: f64,
        x: f64,
        y: f64,
    },
    Z,
}

/// One evaluated sub-path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathOut {
    pub fill: PathFill,
    pub stroke: bool,
    pub cmds: Vec<Cmd>,
}

/// Evaluated geometry: paths plus the text rectangle (l, t, r, b).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evaluated {
    pub paths: Vec<PathOut>,
    pub text_rect: [f64; 4],
}

// ---- definitions ---------------------------------------------------------------------

/// A guide formula: operator + operands (guide names or literals).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Guide {
    pub name: String,
    pub op: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "c")]
pub enum PathCmd {
    M {
        x: String,
        y: String,
    },
    L {
        x: String,
        y: String,
    },
    /// arcTo: radii, start angle and sweep (60000ths of a degree).
    A {
        wr: String,
        hr: String,
        st: String,
        sw: String,
    },
    C {
        pts: [String; 6],
    },
    Q {
        pts: [String; 4],
    },
    Z,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathDef {
    /// Path coordinate space (0 = shape size).
    pub w: f64,
    pub h: f64,
    pub fill: PathFill,
    pub stroke: bool,
    pub cmds: Vec<PathCmd>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GeomDef {
    /// Adjust values with defaults.
    pub av: Vec<Guide>,
    pub gd: Vec<Guide>,
    pub paths: Vec<PathDef>,
    /// Text rectangle guides (l, t, r, b).
    pub text: Option<[String; 4]>,
}

// ---- evaluation ----------------------------------------------------------------------

const DEG: f64 = 60000.0;

fn to_rad(a: f64) -> f64 {
    (a / DEG).to_radians()
}

struct Env {
    vals: HashMap<String, f64>,
}

impl Env {
    fn new(w: f64, h: f64) -> Self {
        let ss = w.min(h);
        let mut v = HashMap::new();
        for (k, x) in [
            ("l", 0.0),
            ("t", 0.0),
            ("r", w),
            ("b", h),
            ("w", w),
            ("h", h),
            ("hc", w / 2.0),
            ("vc", h / 2.0),
            ("ls", w.max(h)),
            ("ss", ss),
            ("cd2", 10800000.0),
            ("cd4", 5400000.0),
            ("cd8", 2700000.0),
            ("3cd4", 16200000.0),
            ("3cd8", 8100000.0),
            ("5cd8", 13500000.0),
            ("7cd8", 18900000.0),
        ] {
            v.insert(k.to_string(), x);
        }
        for d in [2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 32.0] {
            v.insert(format!("wd{d}"), w / d);
            v.insert(format!("hd{d}"), h / d);
        }
        for d in [2.0, 4.0, 6.0, 8.0, 16.0, 32.0] {
            v.insert(format!("ssd{d}"), ss / d);
        }
        Env { vals: v }
    }

    fn get(&self, s: &str) -> f64 {
        if let Some(v) = self.vals.get(s) {
            return *v;
        }
        s.parse().unwrap_or(0.0)
    }

    fn eval(&self, g: &Guide) -> f64 {
        let a = |i: usize| g.args.get(i).map(|x| self.get(x)).unwrap_or(0.0);
        match g.op.as_str() {
            "*/" => {
                let d = a(2);
                if d == 0.0 {
                    0.0
                } else {
                    a(0) * a(1) / d
                }
            }
            "+-" => a(0) + a(1) - a(2),
            "+/" => {
                let d = a(2);
                if d == 0.0 {
                    0.0
                } else {
                    (a(0) + a(1)) / d
                }
            }
            "?:" => {
                if a(0) > 0.0 {
                    a(1)
                } else {
                    a(2)
                }
            }
            "abs" => a(0).abs(),
            "at2" => a(1).atan2(a(0)).to_degrees() * DEG,
            "cat2" => a(0) * a(2).atan2(a(1)).cos(),
            "cos" => a(0) * to_rad(a(1)).cos(),
            "max" => a(0).max(a(1)),
            "min" => a(0).min(a(1)),
            "mod" => (a(0) * a(0) + a(1) * a(1) + a(2) * a(2)).sqrt(),
            "pin" => a(1).clamp(a(0).min(a(2)), a(2).max(a(0))),
            "sat2" => a(0) * a(2).atan2(a(1)).sin(),
            "sin" => a(0) * to_rad(a(1)).sin(),
            "sqrt" => a(0).max(0.0).sqrt(),
            "tan" => a(0) * to_rad(a(1)).tan(),
            _ => a(0), // "val"
        }
    }
}

/// Parametric angle of a point at visual angle `ang` on an ellipse.
fn param_angle(ang: f64, wr: f64, hr: f64) -> f64 {
    if wr == 0.0 || hr == 0.0 {
        return ang;
    }
    (ang.sin() / hr).atan2(ang.cos() / wr)
}

/// Append an elliptical arc (as cubic Béziers) from the current point.
fn arc(out: &mut Vec<Cmd>, cur: &mut (f64, f64), wr: f64, hr: f64, st: f64, sw: f64) {
    let st_v = to_rad(st);
    let sw_v = to_rad(sw);
    let t0 = param_angle(st_v, wr, hr);
    let t_end_raw = param_angle(st_v + sw_v, wr, hr);
    let mut dt = t_end_raw - t0;
    let full = std::f64::consts::TAU;
    if sw_v.abs() >= full - 1e-9 {
        dt = full * sw_v.signum();
    } else {
        // keep the sweep direction and size close to the visual sweep
        while sw_v > 0.0 && dt < 0.0 {
            dt += full;
        }
        while sw_v < 0.0 && dt > 0.0 {
            dt -= full;
        }
        if sw_v == 0.0 {
            dt = 0.0;
        }
    }
    let cx = cur.0 - wr * t0.cos();
    let cy = cur.1 - hr * t0.sin();
    let n = (dt.abs() / (std::f64::consts::FRAC_PI_2) - 1e-9)
        .ceil()
        .max(1.0) as usize;
    let step = dt / n as f64;
    let k = 4.0 / 3.0 * (step / 4.0).tan();
    let mut t = t0;
    for _ in 0..n {
        let t2 = t + step;
        let (c1, s1, c2, s2) = (t.cos(), t.sin(), t2.cos(), t2.sin());
        let p1 = (cx + wr * (c1 - k * s1), cy + hr * (s1 + k * c1));
        let p2 = (cx + wr * (c2 + k * s2), cy + hr * (s2 - k * c2));
        let p3 = (cx + wr * c2, cy + hr * s2);
        out.push(Cmd::C {
            x1: p1.0,
            y1: p1.1,
            x2: p2.0,
            y2: p2.1,
            x: p3.0,
            y: p3.1,
        });
        t = t2;
    }
    if n > 0 && dt != 0.0 {
        *cur = (cx + wr * (t0 + dt).cos(), cy + hr * (t0 + dt).sin());
    }
}

impl GeomDef {
    /// Evaluate at size (w, h) with adjust overrides.
    pub fn evaluate(&self, w: f64, h: f64, adj: &[(String, f64)]) -> Evaluated {
        let mut env = Env::new(w, h);
        for g in &self.av {
            let v = adj
                .iter()
                .find(|(k, _)| *k == g.name)
                .map(|(_, v)| *v)
                .unwrap_or_else(|| env.eval(g));
            env.vals.insert(g.name.clone(), v);
        }
        for g in &self.gd {
            let v = env.eval(g);
            env.vals.insert(g.name.clone(), v);
        }
        let mut paths = Vec::new();
        for p in &self.paths {
            let (sx, sy) = (
                if p.w > 0.0 { w / p.w } else { 1.0 },
                if p.h > 0.0 { h / p.h } else { 1.0 },
            );
            let mut cmds = Vec::new();
            let mut cur = (0.0, 0.0);
            let mut start = (0.0, 0.0);
            let pt = |x: &str, y: &str| (env.get(x) * sx, env.get(y) * sy);
            for c in &p.cmds {
                match c {
                    PathCmd::M { x, y } => {
                        cur = pt(x, y);
                        start = cur;
                        cmds.push(Cmd::M { x: cur.0, y: cur.1 });
                    }
                    PathCmd::L { x, y } => {
                        cur = pt(x, y);
                        cmds.push(Cmd::L { x: cur.0, y: cur.1 });
                    }
                    PathCmd::A { wr, hr, st, sw } => {
                        arc(
                            &mut cmds,
                            &mut cur,
                            env.get(wr) * sx,
                            env.get(hr) * sy,
                            env.get(st),
                            env.get(sw),
                        );
                    }
                    PathCmd::C { pts } => {
                        let a = pt(&pts[0], &pts[1]);
                        let b = pt(&pts[2], &pts[3]);
                        cur = pt(&pts[4], &pts[5]);
                        cmds.push(Cmd::C {
                            x1: a.0,
                            y1: a.1,
                            x2: b.0,
                            y2: b.1,
                            x: cur.0,
                            y: cur.1,
                        });
                    }
                    PathCmd::Q { pts } => {
                        let a = pt(&pts[0], &pts[1]);
                        cur = pt(&pts[2], &pts[3]);
                        cmds.push(Cmd::Q {
                            x1: a.0,
                            y1: a.1,
                            x: cur.0,
                            y: cur.1,
                        });
                    }
                    PathCmd::Z => {
                        cur = start;
                        cmds.push(Cmd::Z);
                    }
                }
            }
            paths.push(PathOut {
                fill: p.fill,
                stroke: p.stroke,
                cmds,
            });
        }
        let text_rect = match &self.text {
            Some([l, t, r, b]) => [env.get(l), env.get(t), env.get(r), env.get(b)],
            None => [0.0, 0.0, w, h],
        };
        Evaluated { paths, text_rect }
    }
}

// ---- preset DSL ------------------------------------------------------------------------
//
//   @name                 start a preset
//   av NAME FORMULA...    adjust value (default formula, usually `val N`-like literal)
//   gd NAME OP ARGS...    guide
//   tx L T R B            text rectangle
//   path [w=N] [h=N] [fill=none|darken|...] [stroke=false]
//   M x y | L x y | A wR hR st sw | C x1 y1 x2 y2 x y | Q x1 y1 x y | Z

fn parse_fill(s: &str) -> PathFill {
    match s {
        "none" => PathFill::None,
        "lighten" => PathFill::Lighten,
        "lightenLess" => PathFill::LightenLess,
        "darken" => PathFill::Darken,
        "darkenLess" => PathFill::DarkenLess,
        _ => PathFill::Norm,
    }
}

fn guide(name: &str, rest: &[&str]) -> Guide {
    match rest {
        [single] => Guide {
            name: name.into(),
            op: "val".into(),
            args: vec![single.to_string()],
        },
        [op, args @ ..] => Guide {
            name: name.into(),
            op: op.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        },
        [] => Guide {
            name: name.into(),
            op: "val".into(),
            args: vec!["0".into()],
        },
    }
}

/// Parse preset definitions written in the DSL above.
pub fn parse_dsl(src: &str) -> HashMap<String, GeomDef> {
    let mut out = HashMap::new();
    let mut name = String::new();
    let mut cur = GeomDef::default();
    let flush = |out: &mut HashMap<String, GeomDef>, name: &str, cur: &mut GeomDef| {
        if !name.is_empty() {
            out.insert(name.to_string(), std::mem::take(cur));
        }
    };
    for line in src.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        // several commands may share a line, separated by ';'
        if let Some(n) = line.strip_prefix('@') {
            flush(&mut out, &name, &mut cur);
            name = n.trim().to_string();
            continue;
        }
        match parts[0] {
            "av" => cur.av.push(guide(parts[1], &parts[2..])),
            "gd" => cur.gd.push(guide(parts[1], &parts[2..])),
            "tx" => {
                cur.text = Some([
                    parts[1].into(),
                    parts[2].into(),
                    parts[3].into(),
                    parts[4].into(),
                ])
            }
            "path" => {
                let mut p = PathDef {
                    w: 0.0,
                    h: 0.0,
                    fill: PathFill::Norm,
                    stroke: true,
                    cmds: vec![],
                };
                for kv in &parts[1..] {
                    if let Some((k, v)) = kv.split_once('=') {
                        match k {
                            "w" => p.w = v.parse().unwrap_or(0.0),
                            "h" => p.h = v.parse().unwrap_or(0.0),
                            "fill" => p.fill = parse_fill(v),
                            "stroke" => p.stroke = v != "false",
                            _ => {}
                        }
                    }
                }
                cur.paths.push(p);
            }
            _ => {
                for cmd in line.split(';') {
                    let t: Vec<&str> = cmd.split_whitespace().collect();
                    if t.is_empty() {
                        continue;
                    }
                    let c = match t[0] {
                        "M" if t.len() == 3 => PathCmd::M {
                            x: t[1].into(),
                            y: t[2].into(),
                        },
                        "L" if t.len() == 3 => PathCmd::L {
                            x: t[1].into(),
                            y: t[2].into(),
                        },
                        "A" if t.len() == 5 => PathCmd::A {
                            wr: t[1].into(),
                            hr: t[2].into(),
                            st: t[3].into(),
                            sw: t[4].into(),
                        },
                        "C" if t.len() == 7 => PathCmd::C {
                            pts: [
                                t[1].into(),
                                t[2].into(),
                                t[3].into(),
                                t[4].into(),
                                t[5].into(),
                                t[6].into(),
                            ],
                        },
                        "Q" if t.len() == 5 => PathCmd::Q {
                            pts: [t[1].into(), t[2].into(), t[3].into(), t[4].into()],
                        },
                        "Z" => PathCmd::Z,
                        _ => continue,
                    };
                    if cur.paths.is_empty() {
                        cur.paths.push(PathDef {
                            w: 0.0,
                            h: 0.0,
                            fill: PathFill::Norm,
                            stroke: true,
                            cmds: vec![],
                        });
                    }
                    cur.paths.last_mut().unwrap().cmds.push(c);
                }
            }
        }
    }
    flush(&mut out, &name, &mut cur);
    out
}

/// Regular polygon / star presets generated from their point count.
fn generated(name: &str) -> Option<GeomDef> {
    let poly = |n: usize| -> GeomDef {
        let mut cmds = Vec::new();
        let mut gd = Vec::new();
        for i in 0..n {
            let ang = -90.0 + 360.0 * i as f64 / n as f64;
            let a = (ang * DEG).round() as i64;
            gd.push(guide(&format!("dx{i}"), &["cos", "wd2", &a.to_string()]));
            gd.push(guide(&format!("dy{i}"), &["sin", "hd2", &a.to_string()]));
            gd.push(guide(
                &format!("x{i}"),
                &["+-", "hc", &format!("dx{i}"), "0"],
            ));
            gd.push(guide(
                &format!("y{i}"),
                &["+-", "vc", &format!("dy{i}"), "0"],
            ));
            let (x, y) = (format!("x{i}"), format!("y{i}"));
            cmds.push(if i == 0 {
                PathCmd::M { x, y }
            } else {
                PathCmd::L { x, y }
            });
        }
        cmds.push(PathCmd::Z);
        GeomDef {
            av: vec![],
            gd,
            paths: vec![PathDef {
                w: 0.0,
                h: 0.0,
                fill: PathFill::Norm,
                stroke: true,
                cmds,
            }],
            text: None,
        }
    };
    let star = |n: usize, default_adj: i64| -> GeomDef {
        let mut gd = vec![
            guide("a", &["pin", "0", "adj", "50000"]),
            guide("iwd2", &["*/", "wd2", "a", "50000"]),
            guide("ihd2", &["*/", "hd2", "a", "50000"]),
        ];
        let mut cmds = Vec::new();
        for i in 0..2 * n {
            let ang = -90.0 + 180.0 * i as f64 / n as f64;
            let a = (ang * DEG).round() as i64;
            let (rw, rh) = if i % 2 == 0 {
                ("wd2", "hd2")
            } else {
                ("iwd2", "ihd2")
            };
            gd.push(guide(&format!("dx{i}"), &["cos", rw, &a.to_string()]));
            gd.push(guide(&format!("dy{i}"), &["sin", rh, &a.to_string()]));
            gd.push(guide(
                &format!("x{i}"),
                &["+-", "hc", &format!("dx{i}"), "0"],
            ));
            gd.push(guide(
                &format!("y{i}"),
                &["+-", "vc", &format!("dy{i}"), "0"],
            ));
            let (x, y) = (format!("x{i}"), format!("y{i}"));
            cmds.push(if i == 0 {
                PathCmd::M { x, y }
            } else {
                PathCmd::L { x, y }
            });
        }
        cmds.push(PathCmd::Z);
        GeomDef {
            av: vec![guide("adj", &[&default_adj.to_string()])],
            gd,
            paths: vec![PathDef {
                w: 0.0,
                h: 0.0,
                fill: PathFill::Norm,
                stroke: true,
                cmds,
            }],
            text: None,
        }
    };
    Some(match name {
        "heptagon" => poly(7),
        "decagon" => poly(10),
        "dodecagon" => poly(12),
        "star6" => star(6, 28868),
        "star7" => star(7, 34601),
        "star8" => star(8, 37500),
        "star10" => star(10, 42533),
        "star12" => star(12, 37500),
        "star16" => star(16, 37500),
        "star24" => star(24, 37500),
        "star32" => star(32, 37500),
        _ => return None,
    })
}

/// Preset definitions (subset of ECMA-376 presetShapeDefinitions).
const PRESETS: &str = include_str!("presets.dsl");

fn presets() -> &'static HashMap<String, GeomDef> {
    static P: OnceLock<HashMap<String, GeomDef>> = OnceLock::new();
    P.get_or_init(|| {
        let mut m = parse_dsl(PRESETS);
        for n in [
            "heptagon",
            "decagon",
            "dodecagon",
            "star6",
            "star7",
            "star8",
            "star10",
            "star12",
            "star16",
            "star24",
            "star32",
        ] {
            if let Some(g) = generated(n) {
                m.insert(n.into(), g);
            }
        }
        // aliases with identical outlines
        for (alias, of) in [
            ("flowChartProcess", "rect"),
            ("flowChartDecision", "diamond"),
            ("flowChartConnector", "ellipse"),
            ("flowChartData", "flowChartInputOutput"),
            ("straightConnector1", "line"),
            ("flowChartExtract", "triangle"),
        ] {
            if let Some(g) = m.get(of).cloned() {
                m.insert(alias.into(), g);
            }
        }
        m
    })
}

/// Definition of a preset; unknown names fall back to a rectangle.
pub fn preset(name: &str) -> &'static GeomDef {
    let p = presets();
    p.get(name)
        .unwrap_or_else(|| p.get("rect").expect("rect preset"))
}

pub fn is_known_preset(name: &str) -> bool {
    presets().contains_key(name)
}

/// Names of all available presets (for the shape gallery).
pub fn preset_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = presets().keys().map(|s| s.as_str()).collect();
    v.sort_unstable();
    v
}

/// Evaluate a preset at a size.
pub fn evaluate_preset(name: &str, w: f64, h: f64, adj: &[(String, f64)]) -> Evaluated {
    preset(name).evaluate(w, h, adj)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox(e: &Evaluated) -> (f64, f64, f64, f64) {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in &e.paths {
            for c in &p.cmds {
                let pts: Vec<(f64, f64)> = match c {
                    Cmd::M { x, y } | Cmd::L { x, y } => vec![(*x, *y)],
                    Cmd::C { x, y, .. } | Cmd::Q { x, y, .. } => vec![(*x, *y)],
                    Cmd::Z => vec![],
                };
                for (x, y) in pts {
                    b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
                }
            }
        }
        b
    }

    #[test]
    fn every_preset_evaluates_inside_its_box() {
        for name in preset_names() {
            let e = evaluate_preset(name, 200.0, 100.0, &[]);
            assert!(!e.paths.is_empty(), "{name}");
            let (x0, y0, x1, y1) = bbox(&e);
            assert!(x0.is_finite() && y1.is_finite(), "{name}");
            // most shapes stay within the box; callouts reach outside via their tail
            if !name.contains("Callout") {
                assert!(
                    x0 >= -0.5 && y0 >= -0.5 && x1 <= 200.5 && y1 <= 100.5,
                    "{name}: {:?}",
                    (x0, y0, x1, y1)
                );
            }
        }
        assert!(preset_names().len() >= 70, "{}", preset_names().len());
    }

    #[test]
    fn rounded_rect_and_ellipse_geometry() {
        let e = evaluate_preset("roundRect", 100.0, 50.0, &[]);
        // radius = 16667/100000 of the short side
        if let Cmd::M { x, y } = e.paths[0].cmds[0] {
            assert!((x - 0.0).abs() < 1e-9 && (y - 50.0 * 0.16667).abs() < 1e-3);
        }
        let e = evaluate_preset("ellipse", 100.0, 50.0, &[]);
        let (x0, y0, x1, y1) = bbox(&e);
        assert!(
            (x0 - 0.0).abs() < 1e-6
                && (x1 - 100.0).abs() < 1e-6
                && (y0 - 0.0).abs() < 1e-6
                && (y1 - 50.0).abs() < 1e-6
        );
        // adjust values are applied
        let a = evaluate_preset("rightArrow", 100.0, 50.0, &[("adj2".into(), 0.0)]);
        assert!(matches!(a.paths[0].cmds[1], Cmd::L { x, .. } if (x - 100.0).abs() < 1e-9));
    }
}
