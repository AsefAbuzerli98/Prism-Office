//! `word/numbering.xml`: list definitions.

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::model::*;
use crate::raw::{attr, local};
use crate::{Error, Result};

pub(crate) fn parse_numbering(data: &[u8]) -> Result<Vec<NumberingDef>> {
    let text = std::str::from_utf8(data).map_err(Error::xml)?;
    let mut r = Reader::from_str(text);
    let mut abstracts: HashMap<u32, Vec<LevelDef>> = HashMap::new();
    let mut nums: Vec<(u32, u32)> = Vec::new();
    let mut cur_abs: Option<u32> = None;
    let mut cur_num: Option<u32> = None;
    let mut lvl: Option<(usize, LevelDef)> = None;
    loop {
        match r.read_event().map_err(Error::xml)? {
            Event::Start(e) | Event::Empty(e) => {
                let v = |n: &str| attr(&e, n);
                match local(&e).as_str() {
                    "abstractNum" => cur_abs = v("abstractNumId").and_then(|x| x.parse().ok()),
                    "lvl" if cur_abs.is_some() => {
                        let idx = v("ilvl").and_then(|x| x.parse().ok()).unwrap_or(0);
                        lvl = Some((
                            idx,
                            LevelDef {
                                format: "decimal".into(),
                                text: format!("%{}.", idx + 1),
                                start: 1,
                                indent_left: 720 * (idx as i32 + 1),
                                hanging: 360,
                            },
                        ));
                    }
                    "start" => {
                        if let (Some((_, l)), Some(s)) =
                            (lvl.as_mut(), v("val").and_then(|x| x.parse().ok()))
                        {
                            l.start = s;
                        }
                    }
                    "numFmt" => {
                        if let (Some((_, l)), Some(f)) = (lvl.as_mut(), v("val")) {
                            l.format = f;
                        }
                    }
                    "lvlText" => {
                        if let (Some((_, l)), Some(t)) = (lvl.as_mut(), v("val")) {
                            l.text = t;
                        }
                    }
                    "ind" => {
                        if let Some((_, l)) = lvl.as_mut() {
                            if let Some(x) = v("left")
                                .or_else(|| v("start"))
                                .and_then(|x| x.parse().ok())
                            {
                                l.indent_left = x;
                            }
                            if let Some(x) = v("hanging").and_then(|x| x.parse().ok()) {
                                l.hanging = x;
                            }
                        }
                    }
                    "num" => cur_num = v("numId").and_then(|x| x.parse().ok()),
                    "abstractNumId" => {
                        if let (Some(n), Some(a)) = (cur_num, v("val").and_then(|x| x.parse().ok()))
                        {
                            nums.push((n, a));
                        }
                    }
                    _ => {}
                }
            }
            Event::End(e) => match e.local_name().into_inner() {
                "lvl" => {
                    if let (Some(a), Some((idx, l))) = (cur_abs, lvl.take()) {
                        let levels = abstracts.entry(a).or_default();
                        if levels.len() <= idx {
                            levels.resize(idx + 1, l.clone());
                        }
                        levels[idx] = l;
                    }
                }
                "abstractNum" => cur_abs = None,
                "num" => cur_num = None,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(nums
        .into_iter()
        .map(|(num_id, a)| NumberingDef {
            num_id,
            levels: abstracts.get(&a).cloned().unwrap_or_default(),
        })
        .collect())
}

const BULLETS: [&str; 3] = ["\u{2022}", "\u{25E6}", "\u{25AA}"];
const DECIMAL_FORMATS: [&str; 3] = ["decimal", "lowerLetter", "lowerRoman"];

fn builtin_levels(bullet: bool) -> Vec<LevelDef> {
    (0..9)
        .map(|i| LevelDef {
            format: if bullet {
                "bullet".into()
            } else {
                DECIMAL_FORMATS[i % 3].into()
            },
            text: if bullet {
                BULLETS[i % 3].into()
            } else {
                format!("%{}.", i + 1)
            },
            start: 1,
            indent_left: 720 * (i as i32 + 1),
            hanging: 360,
        })
        .collect()
}

pub(crate) fn add_builtin_defs(defs: &mut Vec<NumberingDef>) {
    for (id, bullet) in [(BULLET_NUM_ID, true), (DECIMAL_NUM_ID, false)] {
        if !defs.iter().any(|d| d.num_id == id) {
            defs.push(NumberingDef {
                num_id: id,
                levels: builtin_levels(bullet),
            });
        }
    }
}

/// XML for the built-in abstractNum + num pair with the given id.
pub(crate) fn builtin_xml(num_id: u32) -> Option<(String, String)> {
    let bullet = match num_id {
        BULLET_NUM_ID => true,
        DECIMAL_NUM_ID => false,
        _ => return None,
    };
    let mut abs = format!(
        r#"<w:abstractNum w:abstractNumId="{num_id}"><w:multiLevelType w:val="hybridMultilevel"/>"#
    );
    for (i, l) in builtin_levels(bullet).iter().enumerate() {
        let font = if bullet {
            r#"<w:rPr><w:rFonts w:ascii="Symbol" w:hAnsi="Symbol" w:hint="default"/></w:rPr>"#
        } else {
            ""
        };
        let font = if bullet && i % 3 != 0 {
            r#"<w:rPr><w:rFonts w:ascii="Courier New" w:hAnsi="Courier New" w:hint="default"/></w:rPr>"#
        } else {
            font
        };
        // Symbol-font bullets use the private-use code point Word expects.
        let text = if bullet && i % 3 == 0 {
            "\u{F0B7}".to_string()
        } else if bullet {
            "o".to_string()
        } else {
            l.text.clone()
        };
        abs.push_str(&format!(
            r#"<w:lvl w:ilvl="{i}"><w:start w:val="1"/><w:numFmt w:val="{}"/><w:lvlText w:val="{}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{}" w:hanging="{}"/></w:pPr>{font}</w:lvl>"#,
            l.format,
            crate::raw::escape_attr(&text),
            l.indent_left,
            l.hanging
        ));
    }
    abs.push_str("</w:abstractNum>");
    let num = format!(r#"<w:num w:numId="{num_id}"><w:abstractNumId w:val="{num_id}"/></w:num>"#);
    Some((abs, num))
}
