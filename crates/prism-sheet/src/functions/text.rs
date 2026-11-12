//! Text functions.

use super::{arg, arg_num, arg_raw, arr, err, lift, lifted, present, tryv};
use crate::eval::{num, text as to_text, truthy, Ctx};
use crate::parser::Expr;
use crate::value::{Array, ErrorKind, Value};

pub const NAMES: &[&str] = &[
    "LEN",
    "LEFT",
    "RIGHT",
    "MID",
    "UPPER",
    "LOWER",
    "PROPER",
    "TRIM",
    "CLEAN",
    "CONCAT",
    "CONCATENATE",
    "TEXTJOIN",
    "SUBSTITUTE",
    "REPLACE",
    "REPT",
    "FIND",
    "SEARCH",
    "EXACT",
    "VALUE",
    "NUMBERVALUE",
    "TEXT",
    "FIXED",
    "DOLLAR",
    "CHAR",
    "CODE",
    "UNICHAR",
    "UNICODE",
    "TEXTBEFORE",
    "TEXTAFTER",
    "TEXTSPLIT",
    "VALUETOTEXT",
    "ARRAYTOTEXT",
];

fn chars_of(s: &str) -> Vec<char> {
    s.chars().collect()
}

/// Apply `f` element-wise to (text, number…) arguments.
fn text_fn(
    ctx: &mut Ctx,
    args: &[Expr],
    defaults: &[Option<f64>],
    f: impl Fn(&str, &[f64]) -> Value,
) -> Value {
    let mut vals = vec![arg(ctx, args, 0)];
    for (i, d) in defaults.iter().enumerate() {
        vals.push(if present(args, i + 1) {
            arg(ctx, args, i + 1)
        } else {
            d.map(Value::Number).unwrap_or(Value::Empty)
        });
    }
    lift(vals, |v| {
        let s = match to_text(&v[0]) {
            Ok(s) => s,
            Err(e) => return Value::Error(e),
        };
        let mut ns = Vec::with_capacity(v.len() - 1);
        for x in &v[1..] {
            match num(x) {
                Ok(n) => ns.push(n),
                Err(e) => return Value::Error(e),
            }
        }
        f(&s, &ns)
    })
}

fn before_after(ctx: &mut Ctx, args: &[Expr], after: bool) -> Value {
    let text = arg(ctx, args, 0);
    let delims: Vec<String> = match arg(ctx, args, 1) {
        Value::Array(a) => a.data.iter().map(|v| v.to_string()).collect(),
        Value::Error(e) => return err(e),
        v => vec![v.to_string()],
    };
    let n = tryv!(arg_num(ctx, args, 2, 1.0)) as i64;
    let ci = tryv!(arg_num(ctx, args, 3, 0.0)) != 0.0;
    let match_end = tryv!(arg_num(ctx, args, 4, 0.0)) != 0.0;
    let not_found = if present(args, 5) {
        Some(arg(ctx, args, 5))
    } else {
        None
    };
    if n == 0 {
        return err(ErrorKind::Value);
    }
    lift(vec![text], move |v| {
        let s = match to_text(&v[0]) {
            Ok(s) => s,
            Err(e) => return Value::Error(e),
        };
        let fold = |x: &str| if ci { x.to_lowercase() } else { x.to_string() };
        let hs = fold(&s);
        // all delimiter hits (byte offset, len)
        let mut hits: Vec<(usize, usize)> = Vec::new();
        for d in &delims {
            if d.is_empty() {
                continue;
            }
            hits.extend(hs.match_indices(&fold(d)).map(|(i, m)| (i, m.len())));
        }
        hits.sort();
        if delims.iter().any(|d| d.is_empty()) {
            return Value::text(if after {
                if n > 0 {
                    s.clone()
                } else {
                    String::new()
                }
            } else if n > 0 {
                String::new()
            } else {
                s.clone()
            });
        }
        let hit = if n > 0 {
            hits.get(n as usize - 1)
                .copied()
                .or_else(|| (match_end && hits.len() as i64 == n - 1).then_some((s.len(), 0)))
        } else {
            let k = n.unsigned_abs() as usize;
            hits.len()
                .checked_sub(k)
                .map(|i| hits[i])
                .or_else(|| (match_end && hits.len() == k - 1).then_some((0, 0)))
        };
        match hit {
            Some((i, l)) => Value::text(if after { &s[i + l..] } else { &s[..i] }),
            None => not_found.clone().unwrap_or(Value::Error(ErrorKind::NA)),
        }
    })
}

fn textsplit(ctx: &mut Ctx, args: &[Expr]) -> Value {
    let s = tryv!(to_text(&arg(ctx, args, 0)));
    let list = |v: Value| -> Vec<String> {
        match v {
            Value::Array(a) => a
                .data
                .iter()
                .map(|x| x.to_string())
                .filter(|x| !x.is_empty())
                .collect(),
            Value::Empty => vec![],
            v => vec![v.to_string()]
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect(),
        }
    };
    let col_d = list(arg(ctx, args, 1));
    let row_d = list(arg(ctx, args, 2));
    let ignore_empty = present(args, 3) && tryv!(truthy(&arg(ctx, args, 3)));
    let ci = tryv!(arg_num(ctx, args, 4, 0.0)) != 0.0;
    let pad = if present(args, 5) {
        arg(ctx, args, 5)
    } else {
        Value::Error(ErrorKind::NA)
    };
    let split = |t: &str, ds: &[String]| -> Vec<String> {
        if ds.is_empty() {
            return vec![t.to_string()];
        }
        let hay = if ci { t.to_lowercase() } else { t.to_string() };
        let mut out = Vec::new();
        let mut start = 0;
        let mut i = 0;
        while i < hay.len() {
            if let Some(d) = ds
                .iter()
                .find(|d| hay[i..].starts_with(&*if ci { d.to_lowercase() } else { d.to_string() }))
            {
                out.push(t[start..i].to_string());
                i += d.len();
                start = i;
            } else {
                i += hay[i..].chars().next().map_or(1, |c| c.len_utf8());
            }
        }
        out.push(t[start..].to_string());
        if ignore_empty {
            out.retain(|x| !x.is_empty());
        }
        out
    };
    let rows: Vec<Vec<String>> = split(&s, &row_d).iter().map(|r| split(r, &col_d)).collect();
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
    let mut data = Vec::new();
    for r in &rows {
        for c in 0..cols {
            data.push(
                r.get(c)
                    .map(|x| Value::text(x.clone()))
                    .unwrap_or_else(|| pad.clone()),
            );
        }
    }
    Value::array(Array::new(rows.len().max(1), cols, data))
}

fn value_to_text(v: &Value, strict: bool) -> String {
    match v {
        Value::Text(t) if strict => format!("\"{}\"", t.replace('"', "\"\"")),
        other => other.to_string(),
    }
}

pub fn call(ctx: &mut Ctx, name: &str, args: &[Expr]) -> Option<Value> {
    Some(match name {
        "LEN" => text_fn(ctx, args, &[], |s, _| {
            Value::Number(s.chars().count() as f64)
        }),
        "LEFT" | "RIGHT" => {
            let right = name == "RIGHT";
            text_fn(ctx, args, &[Some(1.0)], move |s, n| {
                if n[0] < 0.0 {
                    return Value::Error(ErrorKind::Value);
                }
                let c = chars_of(s);
                let k = (n[0] as usize).min(c.len());
                Value::text(if right {
                    c[c.len() - k..].iter().collect::<String>()
                } else {
                    c[..k].iter().collect()
                })
            })
        }
        "MID" => text_fn(ctx, args, &[None, None], |s, n| {
            if n[0] < 1.0 || n[1] < 0.0 {
                return Value::Error(ErrorKind::Value);
            }
            Value::text(
                s.chars()
                    .skip(n[0] as usize - 1)
                    .take(n[1] as usize)
                    .collect::<String>(),
            )
        }),
        "UPPER" => text_fn(ctx, args, &[], |s, _| Value::text(s.to_uppercase())),
        "LOWER" => text_fn(ctx, args, &[], |s, _| Value::text(s.to_lowercase())),
        "PROPER" => text_fn(ctx, args, &[], |s, _| {
            let mut out = String::with_capacity(s.len());
            let mut prev_alpha = false;
            for c in s.chars() {
                if prev_alpha {
                    out.extend(c.to_lowercase())
                } else {
                    out.extend(c.to_uppercase())
                }
                prev_alpha = c.is_alphabetic();
            }
            Value::text(out)
        }),
        "TRIM" => text_fn(ctx, args, &[], |s, _| {
            Value::text(
                s.split(' ')
                    .filter(|w| !w.is_empty())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        }),
        "CLEAN" => text_fn(ctx, args, &[], |s, _| {
            Value::text(s.chars().filter(|c| *c as u32 >= 32).collect::<String>())
        }),
        "CONCAT" | "CONCATENATE" => {
            let mut out = String::new();
            for i in 0..args.len() {
                let v = arg_raw(ctx, args, i);
                if name == "CONCATENATE" {
                    if let Value::Array(_) = ctx.deref(v.clone()) {
                        // CONCATENATE lifts over arrays
                        let vals = (0..args.len()).map(|k| arg(ctx, args, k)).collect();
                        return Some(lift(vals, |vs| {
                            let mut s = String::new();
                            for x in vs {
                                match to_text(x) {
                                    Ok(t) => s.push_str(&t),
                                    Err(e) => return Value::Error(e),
                                }
                            }
                            Value::text(s)
                        }));
                    }
                }
                for x in &ctx.array(v).data {
                    match to_text(x) {
                        Ok(t) => out.push_str(&t),
                        Err(e) => return Some(err(e)),
                    }
                }
            }
            if out.chars().count() > 32767 {
                err(ErrorKind::Value)
            } else {
                Value::text(out)
            }
        }
        "TEXTJOIN" => {
            let delims: Vec<String> = arr(ctx, args, 0)
                .data
                .iter()
                .map(|v| v.to_string())
                .collect();
            let skip = tryv!(truthy(&arg(ctx, args, 1)));
            let mut parts = Vec::new();
            for i in 2..args.len() {
                for x in &arr(ctx, args, i).data {
                    if let Value::Error(e) = x {
                        return Some(err(*e));
                    }
                    let t = x.to_string();
                    if !(skip && t.is_empty()) {
                        parts.push(t);
                    }
                }
            }
            let mut out = String::new();
            for (i, p) in parts.iter().enumerate() {
                if i > 0 && !delims.is_empty() {
                    out.push_str(&delims[(i - 1) % delims.len()]);
                }
                out.push_str(p);
            }
            Value::text(out)
        }
        "SUBSTITUTE" => {
            let vals = vec![
                arg(ctx, args, 0),
                arg(ctx, args, 1),
                arg(ctx, args, 2),
                if present(args, 3) {
                    arg(ctx, args, 3)
                } else {
                    Value::Empty
                },
            ];
            lift(vals, |v| {
                let (s, old, new) = match (to_text(&v[0]), to_text(&v[1]), to_text(&v[2])) {
                    (Ok(a), Ok(b), Ok(c)) => (a, b, c),
                    (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return Value::Error(e),
                };
                if old.is_empty() {
                    return Value::text(s);
                }
                match &v[3] {
                    Value::Empty => Value::text(s.replace(&old, &new)),
                    n => match num(n) {
                        Ok(k) if k >= 1.0 => match s.match_indices(&old).nth(k as usize - 1) {
                            Some((i, _)) => {
                                Value::text(format!("{}{}{}", &s[..i], new, &s[i + old.len()..]))
                            }
                            None => Value::text(s),
                        },
                        Ok(_) => Value::Error(ErrorKind::Value),
                        Err(e) => Value::Error(e),
                    },
                }
            })
        }
        "REPLACE" => {
            let vals = vec![
                arg(ctx, args, 0),
                arg(ctx, args, 1),
                arg(ctx, args, 2),
                arg(ctx, args, 3),
            ];
            lift(vals, |v| {
                let (s, st, n, new) = match (to_text(&v[0]), num(&v[1]), num(&v[2]), to_text(&v[3]))
                {
                    (Ok(a), Ok(b), Ok(c), Ok(d)) => (a, b, c, d),
                    (Err(e), ..) | (_, Err(e), ..) | (_, _, Err(e), _) | (.., Err(e)) => {
                        return Value::Error(e)
                    }
                };
                if st < 1.0 || n < 0.0 {
                    return Value::Error(ErrorKind::Value);
                }
                let c = chars_of(&s);
                let a = (st as usize - 1).min(c.len());
                let b = (a + n as usize).min(c.len());
                Value::text(format!(
                    "{}{}{}",
                    c[..a].iter().collect::<String>(),
                    new,
                    c[b..].iter().collect::<String>()
                ))
            })
        }
        "REPT" => text_fn(ctx, args, &[None], |s, n| {
            if n[0] < 0.0 || s.len() as f64 * n[0] > 32767.0 {
                Value::Error(ErrorKind::Value)
            } else {
                Value::text(s.repeat(n[0] as usize))
            }
        }),
        "FIND" | "SEARCH" => {
            let search = name == "SEARCH";
            let vals = vec![
                arg(ctx, args, 0),
                arg(ctx, args, 1),
                if present(args, 2) {
                    arg(ctx, args, 2)
                } else {
                    Value::Number(1.0)
                },
            ];
            lift(vals, move |v| {
                let (needle, hay, start) = match (to_text(&v[0]), to_text(&v[1]), num(&v[2])) {
                    (Ok(a), Ok(b), Ok(c)) => (a, b, c),
                    (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return Value::Error(e),
                };
                let hc = chars_of(&hay);
                if start < 1.0 || start as usize > hc.len() + 1 {
                    return Value::Error(ErrorKind::Value);
                }
                let tail: String = hc[start as usize - 1..].iter().collect();
                let pos = if search {
                    super::wildcard_regex(&needle)
                        .map(|re| {
                            // unanchored search
                            let src = re
                                .as_str()
                                .trim_start_matches("(?is)^")
                                .trim_end_matches('$')
                                .to_string();
                            regex::Regex::new(&format!("(?is){src}"))
                                .ok()
                                .and_then(|r| {
                                    r.find(&tail).map(|m| tail[..m.start()].chars().count())
                                })
                        })
                        .unwrap_or_else(|| {
                            tail.to_lowercase()
                                .find(&needle.to_lowercase())
                                .map(|i| tail.to_lowercase()[..i].chars().count())
                        })
                } else {
                    tail.find(&needle).map(|i| tail[..i].chars().count())
                };
                pos.map(|p| Value::Number((p + start as usize) as f64))
                    .unwrap_or(Value::Error(ErrorKind::Value))
            })
        }
        "EXACT" => lifted(ctx, args, |v| match (to_text(&v[0]), to_text(&v[1])) {
            (Ok(a), Ok(b)) => Value::Bool(a == b),
            (Err(e), _) | (_, Err(e)) => Value::Error(e),
        }),
        "VALUE" => lifted(ctx, args, |v| match &v[0] {
            Value::Number(n) => Value::Number(*n),
            Value::Empty => Value::Number(0.0),
            Value::Text(t) => num(&Value::Text(t.clone()))
                .map(Value::Number)
                .unwrap_or(Value::Error(ErrorKind::Value)),
            Value::Error(e) => Value::Error(*e),
            _ => Value::Error(ErrorKind::Value),
        }),
        "NUMBERVALUE" => {
            let s = tryv!(to_text(&arg(ctx, args, 0)));
            let dec = if present(args, 1) {
                tryv!(to_text(&arg(ctx, args, 1)))
            } else {
                ".".into()
            };
            let grp = if present(args, 2) {
                tryv!(to_text(&arg(ctx, args, 2)))
            } else {
                ",".into()
            };
            let cleaned: String = s
                .replace(char::is_whitespace, "")
                .replace(&grp, "")
                .replace(&dec, ".");
            let (body, pcts) = (
                cleaned.trim_end_matches('%'),
                cleaned.len() - cleaned.trim_end_matches('%').len(),
            );
            if body.is_empty() {
                return Some(Value::Number(0.0));
            }
            body.parse::<f64>()
                .map(|n| Value::Number(n / 100f64.powi(pcts as i32)))
                .unwrap_or(err(ErrorKind::Value))
        }
        "TEXT" => {
            let v = arg(ctx, args, 0);
            let f = arg(ctx, args, 1);
            lift(vec![v, f], |x| {
                let code = match to_text(&x[1]) {
                    Ok(c) => c,
                    Err(e) => return Value::Error(e),
                };
                let val = match &x[0] {
                    Value::Text(t) => num(&Value::Text(t.clone()))
                        .map(Value::Number)
                        .unwrap_or(x[0].clone()),
                    Value::Error(e) => return Value::Error(*e),
                    v => v.clone(),
                };
                Value::text(crate::numfmt::format_value(&val, &code).text)
            })
        }
        "FIXED" | "DOLLAR" => {
            let dollar = name == "DOLLAR";
            let n = tryv!(arg_num(ctx, args, 0, 0.0));
            let d = tryv!(arg_num(ctx, args, 1, 2.0)) as i32;
            let no_commas = !dollar && present(args, 2) && tryv!(truthy(&arg(ctx, args, 2)));
            let r = (n * 10f64.powi(d)).round() / 10f64.powi(d);
            let frac = "0".repeat(d.max(0) as usize);
            let int = if no_commas { "0" } else { "#,##0" };
            let code = if d > 0 {
                format!("{int}.{frac}")
            } else {
                int.to_string()
            };
            let code = if dollar {
                format!("\"$\"{code};(\"$\"{code})")
            } else {
                code
            };
            Value::text(crate::numfmt::format_number_code(r, &code).text)
        }
        "CHAR" | "UNICHAR" => {
            let uni = name == "UNICHAR";
            lifted(ctx, args, move |v| match num(&v[0]) {
                Ok(n) if n >= 1.0 && (uni || n <= 255.0) => {
                    let c = if uni {
                        char::from_u32(n as u32)
                    } else {
                        Some(windows_1252(n as u8))
                    };
                    c.map(|c| Value::text(c.to_string()))
                        .unwrap_or(Value::Error(ErrorKind::Value))
                }
                Ok(_) => Value::Error(ErrorKind::Value),
                Err(e) => Value::Error(e),
            })
        }
        "CODE" | "UNICODE" => {
            let uni = name == "UNICODE";
            lifted(ctx, args, move |v| match to_text(&v[0]) {
                Ok(s) => match s.chars().next() {
                    Some(c) if uni => Value::Number(c as u32 as f64),
                    Some(c) => Value::Number(
                        (0u8..=255)
                            .find(|b| windows_1252(*b) == c)
                            .map_or(63.0, |b| b as f64),
                    ),
                    None => Value::Error(ErrorKind::Value),
                },
                Err(e) => Value::Error(e),
            })
        }
        "TEXTBEFORE" => before_after(ctx, args, false),
        "TEXTAFTER" => before_after(ctx, args, true),
        "TEXTSPLIT" => textsplit(ctx, args),
        "VALUETOTEXT" => {
            let strict = tryv!(arg_num(ctx, args, 1, 0.0)) == 1.0;
            lift(vec![arg(ctx, args, 0)], move |v| {
                Value::text(value_to_text(&v[0], strict))
            })
        }
        "ARRAYTOTEXT" => {
            let a = arr(ctx, args, 0);
            let strict = tryv!(arg_num(ctx, args, 1, 0.0)) == 1.0;
            if strict {
                let rows: Vec<String> = (0..a.rows)
                    .map(|r| {
                        a.row_values(r)
                            .iter()
                            .map(|v| value_to_text(v, true))
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .collect();
                Value::text(format!("{{{}}}", rows.join(";")))
            } else {
                Value::text(
                    a.data
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
            }
        }
        _ => return None,
    })
}

/// Windows-1252 code page (CHAR/CODE on Windows Excel).
fn windows_1252(b: u8) -> char {
    const HI: [u16; 32] = [
        0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160,
        0x2039, 0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013,
        0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
    ];
    if (0x80..0xA0).contains(&b) {
        char::from_u32(HI[(b - 0x80) as usize] as u32).unwrap_or('?')
    } else {
        b as char
    }
}
