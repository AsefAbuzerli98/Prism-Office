//! REGEXTEST, REGEXEXTRACT, REGEXREPLACE (PCRE-style syntax via the `regex` crate).

use super::{arg, arg_num, err, lift, present, tryv};
use crate::eval::{num, text as to_text, Ctx};
use crate::parser::Expr;
use crate::value::{Array, ErrorKind, Value};

pub const NAMES: &[&str] = &["REGEXTEST", "REGEXEXTRACT", "REGEXREPLACE"];

/// Compile with Excel's case_sensitivity argument (0 = sensitive, 1 = insensitive).
/// `fancy-regex` adds PCRE features Excel supports (look-around, back-references,
/// atomic groups) on top of the linear-time engine; backtracking is bounded.
pub fn compile(pattern: &str, insensitive: bool) -> Result<fancy_regex::Regex, ErrorKind> {
    fancy_regex::RegexBuilder::new(pattern)
        .case_insensitive(insensitive)
        .backtrack_limit(1_000_000)
        .build()
        .map_err(|_| ErrorKind::Value)
}

/// Excel replacement strings use `$1`/`${name}`; `$0` is the whole match.
fn replacement(s: &str) -> String {
    // regex crate treats `$1a` as group "1a"; wrap numeric groups in braces
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '$' {
            if it.peek() == Some(&'$') {
                out.push_str("$$");
                it.next();
                continue;
            }
            let mut digits = String::new();
            while let Some(d) = it.peek().filter(|d| d.is_ascii_digit()) {
                digits.push(*d);
                it.next();
            }
            if digits.is_empty() {
                out.push('$');
            } else {
                out.push_str(&format!("${{{digits}}}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn flag(ctx: &mut Ctx, args: &[Expr], i: usize) -> Result<bool, ErrorKind> {
    if !present(args, i) {
        return Ok(false);
    }
    match num(&arg(ctx, args, i))? as i64 {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ErrorKind::Value),
    }
}

pub fn call(ctx: &mut Ctx, name: &str, args: &[Expr]) -> Option<Value> {
    Some(match name {
        "REGEXTEST" => {
            let text = arg(ctx, args, 0);
            let pat = arg(ctx, args, 1);
            let ci = tryv!(flag(ctx, args, 2));
            lift(vec![text, pat], move |v| {
                match (to_text(&v[0]), to_text(&v[1])) {
                    (Ok(t), Ok(p)) => match compile(&p, ci) {
                        Ok(re) => re
                            .is_match(&t)
                            .map(Value::Bool)
                            .unwrap_or(Value::Error(ErrorKind::Value)),
                        Err(e) => Value::Error(e),
                    },
                    (Err(e), _) | (_, Err(e)) => Value::Error(e),
                }
            })
        }
        "REGEXEXTRACT" => {
            let text = arg(ctx, args, 0);
            let p = tryv!(to_text(&arg(ctx, args, 1)));
            let mode = tryv!(arg_num(ctx, args, 2, 0.0)) as i64;
            let ci = tryv!(flag(ctx, args, 3));
            let re = tryv!(compile(&p, ci));
            if !(0..=2).contains(&mode) {
                return Some(err(ErrorKind::Value));
            }
            if mode == 0 {
                return Some(lift(vec![text], |v| match to_text(&v[0]) {
                    Ok(t) => match re.find(&t) {
                        Ok(Some(m)) => Value::text(m.as_str()),
                        Ok(None) => Value::Error(ErrorKind::NA),
                        Err(_) => Value::Error(ErrorKind::Value),
                    },
                    Err(e) => Value::Error(e),
                }));
            }
            // modes 1/2 return arrays, so only a single text input is meaningful
            let t = tryv!(to_text(&text));
            if mode == 1 {
                let all: Vec<Value> = re
                    .find_iter(&t)
                    .filter_map(|m| m.ok())
                    .map(|m| Value::text(m.as_str()))
                    .collect();
                if all.is_empty() {
                    err(ErrorKind::NA)
                } else {
                    Value::array(Array::column(all))
                }
            } else {
                match re.captures(&t).ok().flatten() {
                    Some(c) if c.len() > 1 => Value::array(Array::row(
                        (1..c.len())
                            .map(|i| {
                                c.get(i)
                                    .map(|m| Value::text(m.as_str()))
                                    .unwrap_or(Value::text(""))
                            })
                            .collect(),
                    )),
                    Some(c) => Value::text(c.get(0).map_or("", |m| m.as_str())),
                    None => err(ErrorKind::NA),
                }
            }
        }
        "REGEXREPLACE" => {
            let text = arg(ctx, args, 0);
            let p = tryv!(to_text(&arg(ctx, args, 1)));
            let rep = replacement(&tryv!(to_text(&arg(ctx, args, 2))));
            let occ = tryv!(arg_num(ctx, args, 3, 0.0)) as i64;
            let ci = tryv!(flag(ctx, args, 4));
            let re = tryv!(compile(&p, ci));
            lift(vec![text], move |v| {
                let t = match to_text(&v[0]) {
                    Ok(t) => t,
                    Err(e) => return Value::Error(e),
                };
                if occ == 0 {
                    return Value::text(re.replace_all(&t, rep.as_str()).into_owned());
                }
                let caps: Vec<fancy_regex::Captures> =
                    re.captures_iter(&t).filter_map(|c| c.ok()).collect();
                let k = if occ > 0 {
                    occ as usize - 1
                } else {
                    match caps.len().checked_sub(occ.unsigned_abs() as usize) {
                        Some(k) => k,
                        None => return Value::text(t),
                    }
                };
                let Some(c) = caps.get(k) else {
                    return Value::text(t);
                };
                let m = c.get(0).unwrap();
                let mut dst = String::new();
                c.expand(&rep, &mut dst);
                Value::text(format!("{}{}{}", &t[..m.start()], dst, &t[m.end()..]))
            })
        }
        _ => return None,
    })
}
