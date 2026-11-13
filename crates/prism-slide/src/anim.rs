//! Slide transitions (`p:transition`, incl. Morph via `mc:AlternateContent`)
//! and animations (`p:timing`) as a flat list of effects.

use prism_drawingml::dom::El;
use serde::{Deserialize, Serialize};

fn none<T>(o: &Option<T>) -> bool {
    o.is_none()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transition {
    /// fade, push, wipe, split, cover, pull, zoom, cut, dissolve, randomBar,
    /// blinds, checker, circle, diamond, plus, wedge, wheel, morph …
    pub kind: String,
    #[serde(default, skip_serializing_if = "none")]
    pub dir: Option<String>,
    /// Morph: byObject / byWord / byChar; fade: thruBlk.
    #[serde(default, skip_serializing_if = "none")]
    pub option: Option<String>,
    /// Duration in ms.
    #[serde(default, skip_serializing_if = "none")]
    pub duration: Option<u32>,
    #[serde(default = "yes")]
    pub advance_click: bool,
    /// Auto-advance after ms.
    #[serde(default, skip_serializing_if = "none")]
    pub advance_after: Option<u32>,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    pub shape: u32,
    /// entr, exit, emph, path
    pub class: String,
    /// Prism name: appear, fade, fly, wipe, zoom, split, float, spin, pulse,
    /// grow, teeter, transparency, path …
    pub preset: String,
    /// PowerPoint preset id / subtype (kept for faithful output).
    pub preset_id: u32,
    #[serde(default)]
    pub subtype: u32,
    /// onClick, withPrev, afterPrev
    pub trigger: String,
    #[serde(default)]
    pub delay: u32,
    pub duration: u32,
    /// Motion path (`M 0 0 L 0.25 0 E`, slide-relative).
    #[serde(default, skip_serializing_if = "none")]
    pub path: Option<String>,
    /// Repeat count (emphasis), 1 = once.
    #[serde(default, skip_serializing_if = "none")]
    pub repeat: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Timing {
    pub effects: Vec<Effect>,
    /// The original timing used constructs the flat model cannot express
    /// (triggers on shapes, media, interactive sequences); it is written back
    /// untouched unless the effect list is edited.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
}

// ---- transitions -----------------------------------------------------------------------------

const KNOWN: &[&str] = &[
    "fade",
    "push",
    "wipe",
    "split",
    "cover",
    "pull",
    "zoom",
    "cut",
    "dissolve",
    "randomBar",
    "blinds",
    "checker",
    "circle",
    "diamond",
    "plus",
    "wedge",
    "wheel",
    "strips",
    "comb",
    "newsflash",
    "random",
    "morph",
    "vortex",
    "ripple",
    "glitter",
    "honeycomb",
    "flash",
    "shred",
    "switch",
    "flip",
    "gallery",
    "cube",
    "doors",
    "box",
    "window",
    "ferris",
    "conveyor",
    "pan",
    "warp",
    "flythrough",
    "prism",
    "reveal",
    "wheelReverse",
];

fn transition_from(t: &El) -> Option<Transition> {
    let child = t
        .elements()
        .find(|e| KNOWN.contains(&e.local()) || e.local() == "sndAc")
        .filter(|e| e.local() != "sndAc");
    let kind = child
        .map(|c| c.local().to_string())
        .unwrap_or_else(|| "cut".into());
    let speed_ms = match t.attr("spd") {
        Some("slow") => Some(1000),
        Some("med") => Some(750),
        Some("fast") => Some(500),
        _ => None,
    };
    let dir = child.and_then(|c| c.attr("dir").or(c.attr("orient")).map(String::from));
    let option = child.and_then(|c| {
        c.attr("option").map(String::from).or_else(|| {
            c.attr_bool("thruBlk")
                .filter(|b| *b)
                .map(|_| "thruBlk".into())
        })
    });
    Some(Transition {
        kind,
        dir,
        option,
        duration: t.attr_i64("dur").map(|d| d as u32).or(speed_ms),
        advance_click: t.attr_bool("advClick").unwrap_or(true),
        advance_after: t.attr_i64("advTm").map(|d| d as u32),
    })
}

/// Read the slide's transition (prefers the `mc:Choice` branch for newer types).
pub fn read_transition(slide: &El) -> Option<Transition> {
    for e in slide.elements() {
        match e.local() {
            "transition" => return transition_from(e),
            "AlternateContent" => {
                if let Some(t) = e.find("Choice").and_then(|c| c.find("transition")) {
                    return transition_from(t);
                }
            }
            _ => {}
        }
    }
    None
}

fn speed_attr(ms: Option<u32>) -> &'static str {
    match ms.unwrap_or(750) {
        0..=600 => "fast",
        601..=900 => "med",
        _ => "slow",
    }
}

const P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";
const P159: &str = "http://schemas.microsoft.com/office/powerpoint/2015/09/main";
const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const BASIC: &[&str] = &[
    "fade",
    "push",
    "wipe",
    "split",
    "cover",
    "pull",
    "zoom",
    "cut",
    "dissolve",
    "randomBar",
    "blinds",
    "checker",
    "circle",
    "diamond",
    "plus",
    "wedge",
    "wheel",
    "strips",
    "comb",
    "newsflash",
    "random",
];

fn basic_transition(t: &Transition, kind: &str) -> El {
    let mut e = El::new("p:transition").with("spd", speed_attr(t.duration));
    if !t.advance_click {
        e.set("advClick", "0");
    }
    if let Some(a) = t.advance_after {
        e.set("advTm", a);
    }
    let mut c = El::new(&format!("p:{kind}"));
    if let Some(d) = &t.dir {
        let attr = if kind == "split"
            || kind == "blinds"
            || kind == "checker"
            || kind == "comb"
            || kind == "randomBar"
        {
            "orient"
        } else {
            "dir"
        };
        if kind == "split" && (d == "in" || d == "out") {
            c.set("dir", d);
        } else {
            c.set(attr, d);
        }
    }
    if kind == "fade" && t.option.as_deref() == Some("thruBlk") {
        c.set("thruBlk", "1");
    }
    e.push(c);
    e
}

/// Serialize; Morph and other PowerPoint 2010+ transitions use AlternateContent with a fade fallback.
pub fn transition_el(t: &Transition) -> El {
    if BASIC.contains(&t.kind.as_str()) {
        let mut e = basic_transition(t, &t.kind);
        if let Some(d) = t.duration {
            // exact duration needs the p14 attribute
            e.set("xmlns:p14", P14).set("p14:dur", d);
        }
        return e;
    }
    let mut choice_t = basic_transition(t, "fade");
    choice_t.children.clear();
    choice_t.set("xmlns:p14", P14);
    if let Some(d) = t.duration {
        choice_t.set("p14:dur", d);
    }
    let (req, inner) = if t.kind == "morph" {
        let mut m = El::new("p159:morph");
        m.set("option", t.option.as_deref().unwrap_or("byObject"));
        ("p159", m)
    } else {
        let mut x = El::new(&format!("p14:{}", t.kind));
        if let Some(d) = &t.dir {
            x.set("dir", d);
        }
        ("p14", x)
    };
    choice_t.push(inner);
    let mut choice = El::new("mc:Choice").with("Requires", req);
    if req == "p159" {
        choice.set("xmlns:p159", P159);
    } else {
        choice.set("xmlns:p14", P14);
    }
    choice.push(choice_t);
    El::new("mc:AlternateContent")
        .with("xmlns:mc", MC)
        .child(choice)
        .child(El::new("mc:Fallback").child(basic_transition(t, "fade")))
}

// ---- animations -------------------------------------------------------------------------------

/// Prism name for a PowerPoint preset (class, id).
pub fn preset_name(class: &str, id: u32) -> &'static str {
    match (class, id) {
        ("entr" | "exit", 1) => "appear",
        ("entr" | "exit", 2) => "fly",
        ("entr" | "exit", 3) => "blinds",
        ("entr" | "exit", 4) => "box",
        ("entr" | "exit", 5) => "checkerboard",
        ("entr" | "exit", 6) => "circle",
        ("entr" | "exit", 8) => "diamond",
        ("entr" | "exit", 9) => "dissolve",
        ("entr" | "exit", 10) => "fade",
        ("entr" | "exit", 12) => "peek",
        ("entr" | "exit", 16) => "split",
        ("entr" | "exit", 17) => "stretch",
        ("entr" | "exit", 18) => "strips",
        ("entr" | "exit", 21) => "wheel",
        ("entr" | "exit", 22) => "wipe",
        ("entr" | "exit", 23) => "zoom",
        ("entr" | "exit", 26) => "bounce",
        ("entr" | "exit", 42) => "float",
        ("entr" | "exit", 47) => "float",
        ("entr" | "exit", 53) => "zoom",
        ("entr" | "exit", 55) => "swivel",
        ("emph", 6) => "grow",
        ("emph", 8) => "spin",
        ("emph", 9) => "transparency",
        ("emph", 26) => "pulse",
        ("emph", 32) => "teeter",
        ("emph", 35) => "blink",
        ("path", _) => "path",
        _ => "fade",
    }
}

/// Default PowerPoint preset id for a Prism name.
pub fn preset_id(class: &str, name: &str) -> u32 {
    match (class, name) {
        (_, "appear") => 1,
        (_, "fly") => 2,
        (_, "wipe") => 22,
        (_, "split") => 16,
        (_, "zoom") => 53,
        (_, "float") => 42,
        (_, "bounce") => 26,
        (_, "wheel") => 21,
        ("emph", "grow") => 6,
        ("emph", "spin") => 8,
        ("emph", "transparency") => 9,
        ("emph", "pulse") => 26,
        ("emph", "teeter") => 32,
        ("path", _) => 0,
        _ => 10,
    }
}

fn walk<'a>(e: &'a El, out: &mut Vec<&'a El>) {
    for c in e.elements() {
        if c.local() == "cTn" && c.attr("presetClass").is_some() {
            out.push(c);
        }
        walk(c, out);
    }
}

fn delay_of(ctn: &El) -> u32 {
    ctn.path(&["stCondLst", "cond"])
        .and_then(|c| c.attr("delay"))
        .and_then(|d| d.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0) as u32
}

pub fn read_timing(t: &El) -> Timing {
    let mut effects = Vec::new();
    let mut ctns = Vec::new();
    walk(t, &mut ctns);
    let mut partial = t
        .path(&["tnLst", "par", "cTn", "childTnLst"])
        .is_some_and(|l| {
            l.elements().any(|e| {
                e.local() == "seq"
                    && e.find("cTn")
                        .is_some_and(|c| c.attr("nodeType") == Some("interactiveSeq"))
            })
        });
    for c in ctns {
        let class = c.attr("presetClass").unwrap_or("entr").to_string();
        if !matches!(class.as_str(), "entr" | "exit" | "emph" | "path") {
            partial = true;
            continue;
        }
        let id = c.attr_i64("presetID").unwrap_or(10) as u32;
        let trigger = match c.attr("nodeType") {
            Some("withEffect") => "withPrev",
            Some("afterEffect") => "afterPrev",
            _ => "onClick",
        }
        .to_string();
        let mut spid = None;
        let mut dur = None;
        let mut path = None;
        let mut repeat = None;
        fn behaviors<'a>(e: &'a El, out: &mut Vec<&'a El>) {
            for c in e.elements() {
                if matches!(
                    c.local(),
                    "set"
                        | "anim"
                        | "animEffect"
                        | "animMotion"
                        | "animRot"
                        | "animScale"
                        | "animClr"
                ) {
                    out.push(c);
                }
                behaviors(c, out);
            }
        }
        let mut bs = Vec::new();
        behaviors(c, &mut bs);
        for b in bs {
            if let Some(id) = b
                .path(&["cBhvr", "tgtEl", "spTgt"])
                .and_then(|s| s.attr_i64("spid"))
            {
                spid.get_or_insert(id as u32);
            }
            let bd = b
                .path(&["cBhvr", "cTn"])
                .and_then(|t| t.attr_i64("dur"))
                .map(|d| d as u32);
            if b.local() != "set" {
                if let Some(d) = bd {
                    dur = Some(dur.unwrap_or(0).max(d));
                }
            }
            if b.local() == "animMotion" {
                path = b.attr("path").map(String::from);
            }
            if let Some(r) = b
                .path(&["cBhvr", "cTn"])
                .and_then(|t| t.attr_f64("repeatCount"))
            {
                repeat = Some(r / 1000.0);
            }
        }
        let Some(shape) = spid else {
            partial = true;
            continue;
        };
        // the group's delay sits on the parent par of the effect
        effects.push(Effect {
            shape,
            preset: preset_name(&class, id).to_string(),
            class,
            preset_id: id,
            subtype: c.attr_i64("presetSubtype").unwrap_or(0) as u32,
            trigger,
            delay: delay_of(c),
            duration: dur.unwrap_or(500),
            path,
            repeat,
        });
    }
    Timing { effects, partial }
}

// writing: PowerPoint's canonical structure

struct Ids(u32);
impl Ids {
    fn next(&mut self) -> u32 {
        self.0 += 1;
        self.0
    }
}

fn cond(delay: &str) -> El {
    El::new("p:stCondLst").child(El::new("p:cond").with("delay", delay))
}

fn tgt(shape: u32) -> El {
    El::new("p:tgtEl").child(El::new("p:spTgt").with("spid", shape))
}

fn cbhvr(ids: &mut Ids, dur: &str, shape: u32, attr: Option<&str>, extra: &[(&str, &str)]) -> El {
    let mut ctn = El::new("p:cTn").with("id", ids.next()).with("dur", dur);
    for (k, v) in extra {
        ctn.set(k, v);
    }
    if dur == "1" {
        ctn.push(cond("0"));
    }
    let mut b = El::new("p:cBhvr").child(ctn).child(tgt(shape));
    if let Some(a) = attr {
        b.push(El::new("p:attrNameLst").child(El::new("p:attrName").text_child(a)));
    }
    b
}

fn set_vis(ids: &mut Ids, shape: u32, visible: bool, at: &str) -> El {
    let mut c = cbhvr(ids, "1", shape, Some("style.visibility"), &[]);
    if at != "0" {
        if let Some(prism_drawingml::dom::XNode::El(ctn)) = c.children.first_mut() {
            *ctn = El::new("p:cTn")
                .with("id", ctn.attr("id").unwrap_or("0").to_string())
                .with("dur", "1")
                .with("fill", "hold")
                .child(cond(at));
        }
    }
    El::new("p:set").child(c).child(
        El::new("p:to")
            .child(El::new("p:strVal").with("val", if visible { "visible" } else { "hidden" })),
    )
}

fn anim_prop(ids: &mut Ids, shape: u32, dur: u32, attr: &str, from: &str, to: &str) -> El {
    let tav = |tm: &str, v: &str| {
        El::new("p:tav")
            .with("tm", tm)
            .child(El::new("p:val").child(El::new("p:strVal").with("val", v)))
    };
    El::new("p:anim")
        .with("calcmode", "lin")
        .with("valueType", "num")
        .child(cbhvr(
            ids,
            &dur.to_string(),
            shape,
            Some(attr),
            &[("fill", "hold")],
        ))
        .child(
            El::new("p:tavLst")
                .child(tav("0", from))
                .child(tav("100000", to)),
        )
}

fn behaviours(e: &Effect, ids: &mut Ids) -> Vec<El> {
    let d = e.duration;
    let sh = e.shape;
    let entr = e.class == "entr";
    let mut v = Vec::new();
    match e.class.as_str() {
        "entr" | "exit" => {
            if entr {
                v.push(set_vis(ids, sh, true, "0"));
            }
            let filter = |f: &str, ids: &mut Ids| {
                El::new("p:animEffect")
                    .with("transition", if entr { "in" } else { "out" })
                    .with("filter", f)
                    .child(cbhvr(ids, &d.to_string(), sh, None, &[]))
            };
            match e.preset.as_str() {
                "appear" => {}
                "fly" => {
                    let (from, to) = if entr {
                        ("1+#ppt_h/2", "#ppt_y")
                    } else {
                        ("#ppt_y", "1+#ppt_h/2")
                    };
                    v.push(anim_prop(ids, sh, d, "ppt_x", "#ppt_x", "#ppt_x"));
                    v.push(anim_prop(ids, sh, d, "ppt_y", from, to));
                }
                "wipe" => v.push(filter("wipe(down)", ids)),
                "split" => v.push(filter("barn(inVertical)", ids)),
                "wheel" => v.push(filter("wheel(1)", ids)),
                "dissolve" => v.push(filter("dissolve", ids)),
                "zoom" => {
                    let (a, b) = if entr {
                        ("0", "#ppt_w")
                    } else {
                        ("#ppt_w", "0")
                    };
                    let (c, dd) = if entr {
                        ("0", "#ppt_h")
                    } else {
                        ("#ppt_h", "0")
                    };
                    v.push(filter("fade", ids));
                    v.push(anim_prop(ids, sh, d, "ppt_w", a, b));
                    v.push(anim_prop(ids, sh, d, "ppt_h", c, dd));
                }
                "float" => {
                    let (from, to) = if entr {
                        ("#ppt_y+0.1", "#ppt_y")
                    } else {
                        ("#ppt_y", "#ppt_y-0.1")
                    };
                    v.push(filter("fade", ids));
                    v.push(anim_prop(ids, sh, d, "ppt_y", from, to));
                }
                _ => v.push(filter("fade", ids)),
            }
            if !entr {
                v.push(set_vis(ids, sh, false, &d.saturating_sub(1).to_string()));
            }
        }
        "emph" => match e.preset.as_str() {
            "spin" => v.push(El::new("p:animRot").with("by", 21600000).child(cbhvr(
                ids,
                &d.to_string(),
                sh,
                Some("r"),
                &[("fill", "hold")],
            ))),
            "grow" => v.push(
                El::new("p:animScale")
                    .child(cbhvr(ids, &d.to_string(), sh, None, &[("fill", "hold")]))
                    .child(El::new("p:by").with("x", 150000).with("y", 150000)),
            ),
            "pulse" => v.push(
                El::new("p:animScale")
                    .child(cbhvr(
                        ids,
                        &(d / 2).to_string(),
                        sh,
                        None,
                        &[("autoRev", "1"), ("fill", "hold")],
                    ))
                    .child(El::new("p:by").with("x", 105000).with("y", 105000)),
            ),
            "teeter" => v.push(El::new("p:animRot").with("by", 120000).child(cbhvr(
                ids,
                &(d / 8).max(1).to_string(),
                sh,
                Some("r"),
                &[
                    ("accel", "20000"),
                    ("decel", "20000"),
                    ("autoRev", "1"),
                    ("repeatCount", "4000"),
                    ("fill", "hold"),
                ],
            ))),
            _ => {
                let tav = El::new("p:to").child(El::new("p:strVal").with("val", "0.5"));
                v.push(
                    El::new("p:set")
                        .child(cbhvr(
                            ids,
                            &d.to_string(),
                            sh,
                            Some("style.opacity"),
                            &[("fill", "hold")],
                        ))
                        .child(tav),
                );
            }
        },
        _ => {
            let p = e.path.clone().unwrap_or_else(|| "M 0 0 L 0.25 0 E".into());
            v.push(
                El::new("p:animMotion")
                    .with("origin", "layout")
                    .with("path", p)
                    .with("pathEditMode", "relative")
                    .with("ptsTypes", "")
                    .child(cbhvr(
                        ids,
                        &d.to_string(),
                        sh,
                        Some("ppt_x"),
                        &[("fill", "hold")],
                    )),
            );
        }
    }
    v
}

/// Serialize the effect list as PowerPoint's main sequence.
pub fn timing_el(t: &Timing) -> El {
    let mut ids = Ids(1);
    let mut groups: Vec<Vec<&Effect>> = Vec::new();
    for e in &t.effects {
        if e.trigger == "onClick" || groups.is_empty() {
            groups.push(vec![e]);
        } else {
            groups.last_mut().unwrap().push(e);
        }
    }
    let mut seq_children = El::new("p:childTnLst");
    let mut bld = El::new("p:bldLst");
    let mut built: Vec<u32> = Vec::new();
    for (gi, g) in groups.iter().enumerate() {
        // sub-groups: effects chained by afterPrev start when the previous one ends
        let mut subs: Vec<(u32, Vec<&Effect>)> = Vec::new();
        let mut t0 = 0u32;
        let mut last_end = 0u32;
        for e in g {
            if e.trigger == "afterPrev" || subs.is_empty() {
                t0 = if subs.is_empty() { 0 } else { last_end };
                subs.push((t0, vec![*e]));
            } else {
                subs.last_mut().unwrap().1.push(*e);
            }
            last_end = last_end.max(t0 + e.delay + e.duration);
        }
        let mut click_children = El::new("p:childTnLst");
        for (start, list) in subs {
            let mut sub_children = El::new("p:childTnLst");
            for e in list {
                let node = match e.trigger.as_str() {
                    "withPrev" => "withEffect",
                    "afterPrev" => "afterEffect",
                    _ => "clickEffect",
                };
                let mut ctn = El::new("p:cTn")
                    .with("id", ids.next())
                    .with("presetID", e.preset_id)
                    .with("presetClass", &e.class)
                    .with("presetSubtype", e.subtype)
                    .with("fill", "hold")
                    .with("grpId", 0)
                    .with("nodeType", node);
                if let Some(r) = e.repeat {
                    ctn.set("repeatCount", (r * 1000.0) as i64);
                }
                ctn.push(cond(&e.delay.to_string()));
                let mut bl = El::new("p:childTnLst");
                for b in behaviours(e, &mut ids) {
                    bl.push(b);
                }
                ctn.push(bl);
                sub_children.push(El::new("p:par").child(ctn));
                if !built.contains(&e.shape) && e.class != "path" {
                    built.push(e.shape);
                    bld.push(
                        El::new("p:bldP")
                            .with("spid", e.shape)
                            .with("grpId", 0)
                            .with("animBg", 1),
                    );
                }
            }
            click_children.push(
                El::new("p:par").child(
                    El::new("p:cTn")
                        .with("id", ids.next())
                        .with("fill", "hold")
                        .child(cond(&start.to_string()))
                        .child(sub_children),
                ),
            );
        }
        let first_click = g.first().is_some_and(|e| e.trigger == "onClick");
        let delay = if first_click || gi > 0 {
            "indefinite"
        } else {
            "0"
        };
        seq_children.push(
            El::new("p:par").child(
                El::new("p:cTn")
                    .with("id", ids.next())
                    .with("fill", "hold")
                    .child(cond(delay))
                    .child(click_children),
            ),
        );
    }
    let seq = El::new("p:seq")
        .with("concurrent", 1)
        .with("nextAc", "seek")
        .child(
            El::new("p:cTn")
                .with("id", 2)
                .with("dur", "indefinite")
                .with("nodeType", "mainSeq")
                .child(seq_children),
        )
        .child(
            El::new("p:prevCondLst").child(
                El::new("p:cond")
                    .with("evt", "onPrev")
                    .with("delay", 0)
                    .child(El::new("p:tgtEl").child(El::new("p:sldTgt"))),
            ),
        )
        .child(
            El::new("p:nextCondLst").child(
                El::new("p:cond")
                    .with("evt", "onNext")
                    .with("delay", 0)
                    .child(El::new("p:tgtEl").child(El::new("p:sldTgt"))),
            ),
        );
    let root = El::new("p:par").child(
        El::new("p:cTn")
            .with("id", 1)
            .with("dur", "indefinite")
            .with("restart", "never")
            .with("nodeType", "tmRoot")
            .child(El::new("p:childTnLst").child(seq)),
    );
    let mut timing = El::new("p:timing").child(El::new("p:tnLst").child(root));
    if !built.is_empty() {
        timing.push(bld);
    }
    timing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transitions_round_trip() {
        for t in [
            Transition {
                kind: "push".into(),
                dir: Some("l".into()),
                option: None,
                duration: Some(800),
                advance_click: true,
                advance_after: Some(3000),
            },
            Transition {
                kind: "morph".into(),
                dir: None,
                option: Some("byWord".into()),
                duration: Some(2000),
                advance_click: true,
                advance_after: None,
            },
            Transition {
                kind: "vortex".into(),
                dir: Some("r".into()),
                option: None,
                duration: Some(3000),
                advance_click: false,
                advance_after: None,
            },
        ] {
            let slide = El::new("p:sld").child(transition_el(&t));
            let xml = slide.to_xml();
            let back = read_transition(&El::parse(&xml).unwrap()).unwrap();
            assert_eq!(back, t, "{xml}");
        }
    }

    #[test]
    fn animations_round_trip() {
        let e = |shape, class: &str, preset: &str, trigger: &str, delay, duration| Effect {
            shape,
            class: class.into(),
            preset: preset.into(),
            preset_id: preset_id(class, preset),
            subtype: 0,
            trigger: trigger.into(),
            delay,
            duration,
            path: None,
            repeat: None,
        };
        let mut motion = e(5, "path", "path", "onClick", 0, 2000);
        motion.path = Some("M 0 0 L 0.5 0.1 E".into());
        let t = Timing {
            effects: vec![
                e(2, "entr", "fade", "onClick", 0, 500),
                e(3, "entr", "fly", "withPrev", 250, 500),
                e(4, "emph", "spin", "afterPrev", 0, 2000),
                e(2, "exit", "zoom", "onClick", 0, 500),
                motion,
            ],
            partial: false,
        };
        let xml = timing_el(&t).to_xml();
        let back = read_timing(&El::parse(&xml).unwrap());
        assert_eq!(back, t, "{xml}");
    }
}
