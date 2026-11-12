//! Lookup & reference: XLOOKUP/XMATCH (incl. regex match mode 3), VLOOKUP,
//! HLOOKUP, MATCH, INDEX, LOOKUP, OFFSET, INDIRECT, ROW/COLUMN, ADDRESS …

use std::cmp::Ordering;
use std::sync::Arc;

use super::{arg, arg_num, arg_raw, arr, err, lift, present, tryv, wildcard_regex};
use crate::addr::{col_name, Rect, MAX_COLS, MAX_ROWS};
use crate::eval::{compare, text as to_text, truthy, Ctx};
use crate::parser::Expr;
use crate::value::{Array, ErrorKind, RangeRef, Value};

pub const NAMES: &[&str] = &[
    "XLOOKUP",
    "XMATCH",
    "VLOOKUP",
    "HLOOKUP",
    "MATCH",
    "INDEX",
    "LOOKUP",
    "OFFSET",
    "INDIRECT",
    "ROW",
    "COLUMN",
    "ROWS",
    "COLUMNS",
    "ADDRESS",
    "FORMULATEXT",
    "AREAS",
    "SHEET",
    "SHEETS",
];

/// Matcher for one lookup value under a match mode.
enum Needle {
    Value(Value),
    Wild(regex::Regex),
    Regex(fancy_regex::Regex),
}

impl Needle {
    fn new(v: &Value, mode: i64) -> Result<Self, ErrorKind> {
        Ok(match (mode, v) {
            (2, Value::Text(t)) => wildcard_regex(t)
                .map(Needle::Wild)
                .unwrap_or(Needle::Value(v.clone())),
            (3, v) => {
                let p = to_text(v)?;
                Needle::Regex(crate::functions::regex_fns::compile(&p, false)?)
            }
            _ => Needle::Value(v.clone()),
        })
    }

    fn exact(&self, x: &Value) -> bool {
        match self {
            Needle::Value(v) => same_kind(v, x) && compare(v, x) == Ordering::Equal,
            Needle::Wild(re) => matches!(x, Value::Text(_)) && re.is_match(&x.to_string()),
            Needle::Regex(re) => {
                !matches!(x, Value::Empty | Value::Error(_))
                    && re.is_match(&x.to_string()).unwrap_or(false)
            }
        }
    }
}

fn same_kind(a: &Value, b: &Value) -> bool {
    let k = |v: &Value| match v {
        Value::Number(_) => 0,
        Value::Text(_) => 1,
        Value::Bool(_) => 2,
        Value::Empty => 4,
        _ => 3,
    };
    k(a) == k(b)
}

/// Core of XMATCH/XLOOKUP: 0-based index into `hay`.
fn xfind(
    needle: &Value,
    hay: &[Value],
    match_mode: i64,
    search_mode: i64,
) -> Result<Option<usize>, ErrorKind> {
    if let Value::Error(e) = needle {
        return Err(*e);
    }
    if !matches!(match_mode, -1..=3) || !matches!(search_mode, -2 | -1 | 1 | 2) {
        return Err(ErrorKind::Value);
    }
    if search_mode.abs() == 2 && match_mode != 2 && match_mode != 3 {
        // binary search on sorted data
        let desc = search_mode == -2;
        let (mut lo, mut hi) = (0usize, hay.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            let c = compare(&hay[mid], needle);
            let c = if desc { c.reverse() } else { c };
            if c == Ordering::Less {
                lo = mid + 1
            } else {
                hi = mid
            }
        }
        if lo < hay.len()
            && same_kind(&hay[lo], needle)
            && compare(&hay[lo], needle) == Ordering::Equal
        {
            return Ok(Some(lo));
        }
        return Ok(match (match_mode, desc) {
            (0, _) => None,
            (-1, false) | (1, true) => lo.checked_sub(1),
            _ => (lo < hay.len()).then_some(lo),
        });
    }
    let n = Needle::new(needle, match_mode)?;
    let order: Box<dyn Iterator<Item = usize>> = if search_mode < 0 {
        Box::new((0..hay.len()).rev())
    } else {
        Box::new(0..hay.len())
    };
    let mut best: Option<usize> = None;
    for i in order {
        let x = &hay[i];
        if n.exact(x) {
            return Ok(Some(i));
        }
        if match_mode == 1 || match_mode == -1 {
            if !same_kind(x, needle) || matches!(x, Value::Empty) {
                continue;
            }
            let c = compare(x, needle);
            let better = |b: &Value| {
                if match_mode == 1 {
                    compare(x, b) == Ordering::Less
                } else {
                    compare(x, b) == Ordering::Greater
                }
            };
            if (match_mode == 1 && c == Ordering::Greater
                || match_mode == -1 && c == Ordering::Less)
                && best.map_or(true, |b| better(&hay[b]))
            {
                best = Some(i);
            }
        }
    }
    Ok(best)
}

/// Excel's approximate MATCH/VLOOKUP: binary search for the last value ≤ needle.
fn approx_asc(needle: &Value, hay: &[Value]) -> Option<usize> {
    let (mut lo, mut hi) = (0usize, hay.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        // skip values of other kinds the way Excel effectively does
        let c = if same_kind(&hay[mid], needle) && !matches!(hay[mid], Value::Empty) {
            compare(&hay[mid], needle)
        } else {
            Ordering::Greater
        };
        if c != Ordering::Greater {
            lo = mid + 1
        } else {
            hi = mid
        }
    }
    let mut i = lo.checked_sub(1)?;
    loop {
        if same_kind(&hay[i], needle) && !matches!(hay[i], Value::Empty) {
            return Some(i);
        }
        i = i.checked_sub(1)?;
    }
}

fn vector(a: &Array) -> Option<Vec<Value>> {
    (a.rows == 1 || a.cols == 1).then(|| a.data.clone())
}

fn xlookup(ctx: &mut Ctx, args: &[Expr]) -> Value {
    let needle = arg(ctx, args, 0);
    let hay_raw = arg_raw(ctx, args, 1);
    let hay = ctx.array(hay_raw);
    let ret = arr(ctx, args, 2);
    let not_found = if present(args, 3) {
        Some(arg(ctx, args, 3))
    } else {
        None
    };
    let mm = tryv!(arg_num(ctx, args, 4, 0.0)) as i64;
    let sm = tryv!(arg_num(ctx, args, 5, 1.0)) as i64;
    let Some(hv) = vector(&hay) else {
        return err(ErrorKind::Value);
    };
    let vertical = hay.cols == 1 && hay.rows > 1
        || (hay.rows == 1 && hay.cols == 1 && ret.rows >= 1 && ret.cols == 1);
    if vertical && ret.rows != hay.rows || !vertical && ret.cols != hay.cols {
        return err(ErrorKind::Value);
    }
    let pick = move |i: usize| -> Value {
        if vertical {
            Value::array(Array::new(1, ret.cols, ret.row_values(i).to_vec()))
        } else {
            Value::array(Array::new(
                ret.rows,
                1,
                (0..ret.rows).map(|r| ret.get(r, i).clone()).collect(),
            ))
        }
    };
    let one = |n: &Value| -> Value {
        match xfind(n, &hv, mm, sm) {
            Ok(Some(i)) => pick(i),
            Ok(None) => not_found.clone().unwrap_or(Value::Error(ErrorKind::NA)),
            Err(e) => Value::Error(e),
        }
    };
    match &needle {
        Value::Array(a) => {
            // vector of needles → one scalar per needle (first column/row of result)
            let data = a.data.iter().map(|n| match one(n) {
                Value::Array(x) => x.data.first().cloned().unwrap_or_default(),
                v => v,
            });
            Value::array(Array::new(a.rows, a.cols, data.collect()))
        }
        n => one(n),
    }
}

fn index(ctx: &mut Ctx, args: &[Expr]) -> Value {
    let raw = arg_raw(ctx, args, 0);
    let r = tryv!(arg_num(ctx, args, 1, 0.0));
    let c = if present(args, 2) {
        tryv!(arg_num(ctx, args, 2, 0.0))
    } else {
        -1.0
    };
    if r < 0.0 || c < -1.0 {
        return err(ErrorKind::Value);
    }
    let (rows, cols) = match &raw {
        Value::Ref(rr) => (rr.rect.rows() as usize, rr.rect.cols() as usize),
        Value::Error(e) => return err(*e),
        other => {
            let a = ctx.array(other.clone());
            (a.rows, a.cols)
        }
    };
    // a single row/column vector takes one index as position
    let (ri, ci) = if c < 0.0 {
        if rows == 1 {
            (1.0, r)
        } else if cols == 1 {
            (r, 1.0)
        } else {
            (r, 0.0)
        }
    } else {
        (r, c)
    };
    if ri as usize > rows || ci as usize > cols {
        return err(ErrorKind::Ref);
    }
    match raw {
        Value::Ref(rr) => {
            let mut rect = rr.rect;
            if ri >= 1.0 {
                rect.r0 += ri as u32 - 1;
                rect.r1 = rect.r0;
            }
            if ci >= 1.0 {
                rect.c0 += ci as u32 - 1;
                rect.c1 = rect.c0;
            }
            Value::Ref(RangeRef {
                sheet: rr.sheet,
                rect,
            })
        }
        other => {
            let a = ctx.array(other);
            match (ri as usize, ci as usize) {
                (0, 0) => Value::Array(a),
                (0, c) => Value::array(Array::new(
                    a.rows,
                    1,
                    (0..a.rows).map(|r| a.get(r, c - 1).clone()).collect(),
                )),
                (r, 0) => Value::array(Array::new(1, a.cols, a.row_values(r - 1).to_vec())),
                (r, c) => a.get(r - 1, c - 1).clone(),
            }
        }
    }
}

fn offset(ctx: &mut Ctx, args: &[Expr]) -> Value {
    let Value::Ref(base) = arg_raw(ctx, args, 0) else {
        return err(ErrorKind::Value);
    };
    let dr = tryv!(arg_num(ctx, args, 1, 0.0)).trunc() as i64;
    let dc = tryv!(arg_num(ctx, args, 2, 0.0)).trunc() as i64;
    let h = if present(args, 3) {
        tryv!(arg_num(ctx, args, 3, 0.0)).trunc() as i64
    } else {
        base.rect.rows() as i64
    };
    let w = if present(args, 4) {
        tryv!(arg_num(ctx, args, 4, 0.0)).trunc() as i64
    } else {
        base.rect.cols() as i64
    };
    if h == 0 || w == 0 {
        return err(ErrorKind::Ref);
    }
    let r0 = base.rect.r0 as i64 + dr;
    let c0 = base.rect.c0 as i64 + dc;
    let (r0, r1) = if h > 0 {
        (r0, r0 + h - 1)
    } else {
        (r0 + h + 1, r0)
    };
    let (c0, c1) = if w > 0 {
        (c0, c0 + w - 1)
    } else {
        (c0 + w + 1, c0)
    };
    if r0 < 0 || c0 < 0 || r1 >= MAX_ROWS as i64 || c1 >= MAX_COLS as i64 {
        return err(ErrorKind::Ref);
    }
    Value::Ref(RangeRef {
        sheet: base.sheet,
        rect: Rect {
            r0: r0 as u32,
            c0: c0 as u32,
            r1: r1 as u32,
            c1: c1 as u32,
        },
    })
}

fn indirect(ctx: &mut Ctx, args: &[Expr]) -> Value {
    let t = tryv!(to_text(&arg(ctx, args, 0)));
    let a1 = !present(args, 1) || tryv!(truthy(&arg(ctx, args, 1)));
    match crate::parser::parse_mode(t.trim(), ctx.cell, !a1) {
        Ok(
            e @ (Expr::Ref(_)
            | Expr::Name(_)
            | Expr::Structured { .. }
            | Expr::Binary(crate::parser::BinOp::Range, ..)),
        ) => {
            let saved = std::mem::take(&mut ctx.offset);
            let v = ctx.eval(&e);
            ctx.offset = saved;
            match v {
                v @ Value::Ref(_) => v,
                _ => err(ErrorKind::Ref),
            }
        }
        _ => err(ErrorKind::Ref),
    }
}

fn lookup_vec(ctx: &mut Ctx, args: &[Expr], horizontal: bool) -> Value {
    let needle = arg(ctx, args, 0);
    let table = arr(ctx, args, 1);
    let idx = tryv!(arg_num(ctx, args, 2, 1.0)).trunc() as usize;
    let approx = !present(args, 3) || tryv!(truthy(&arg(ctx, args, 3)));
    let (n_keys, n_out) = if horizontal {
        (table.cols, table.rows)
    } else {
        (table.rows, table.cols)
    };
    if idx < 1 {
        return err(ErrorKind::Value);
    }
    if idx > n_out {
        return err(ErrorKind::Ref);
    }
    let keys: Vec<Value> = (0..n_keys)
        .map(|i| {
            if horizontal {
                table.get(0, i).clone()
            } else {
                table.get(i, 0).clone()
            }
        })
        .collect();
    let table: Arc<Array> = table;
    lift(vec![needle], move |v| {
        let found = if approx {
            approx_asc(&v[0], &keys)
        } else {
            xfind(
                &v[0],
                &keys,
                if matches!(v[0], Value::Text(_)) { 2 } else { 0 },
                1,
            )
            .ok()
            .flatten()
        };
        match found {
            Some(i) => {
                if horizontal {
                    table.get(idx - 1, i).clone()
                } else {
                    table.get(i, idx - 1).clone()
                }
            }
            None => Value::Error(ErrorKind::NA),
        }
    })
}

pub fn call(ctx: &mut Ctx, name: &str, args: &[Expr]) -> Option<Value> {
    Some(match name {
        "XLOOKUP" => xlookup(ctx, args),
        "XMATCH" => {
            let needle = arg(ctx, args, 0);
            let hay = arr(ctx, args, 1);
            let mm = tryv!(arg_num(ctx, args, 2, 0.0)) as i64;
            let sm = tryv!(arg_num(ctx, args, 3, 1.0)) as i64;
            let Some(hv) = vector(&hay) else {
                return Some(err(ErrorKind::Value));
            };
            lift(vec![needle], move |v| match xfind(&v[0], &hv, mm, sm) {
                Ok(Some(i)) => Value::Number(i as f64 + 1.0),
                Ok(None) => Value::Error(ErrorKind::NA),
                Err(e) => Value::Error(e),
            })
        }
        "MATCH" => {
            let needle = arg(ctx, args, 0);
            let hay = arr(ctx, args, 1);
            let t = tryv!(arg_num(ctx, args, 2, 1.0)) as i64;
            let Some(hv) = vector(&hay) else {
                return Some(err(ErrorKind::NA));
            };
            lift(vec![needle], move |v| {
                let r = match t {
                    0 => xfind(
                        &v[0],
                        &hv,
                        if matches!(v[0], Value::Text(_)) { 2 } else { 0 },
                        1,
                    )
                    .ok()
                    .flatten(),
                    t if t > 0 => approx_asc(&v[0], &hv),
                    _ => {
                        // descending: first position where value >= needle, last such before smaller
                        let mut best = None;
                        for (i, x) in hv.iter().enumerate() {
                            if same_kind(x, &v[0]) && compare(x, &v[0]) != Ordering::Less {
                                best = Some(i);
                            } else {
                                break;
                            }
                        }
                        best
                    }
                };
                r.map(|i| Value::Number(i as f64 + 1.0))
                    .unwrap_or(Value::Error(ErrorKind::NA))
            })
        }
        "VLOOKUP" => lookup_vec(ctx, args, false),
        "HLOOKUP" => lookup_vec(ctx, args, true),
        "LOOKUP" => {
            let needle = arg(ctx, args, 0);
            let lv = arr(ctx, args, 1);
            let (keys, results): (Vec<Value>, Vec<Value>) = if present(args, 2) {
                let rv = arr(ctx, args, 2);
                (vector(&lv).unwrap_or_default(), rv.data.clone())
            } else if lv.rows >= lv.cols {
                (
                    (0..lv.rows).map(|r| lv.get(r, 0).clone()).collect(),
                    (0..lv.rows)
                        .map(|r| lv.get(r, lv.cols - 1).clone())
                        .collect(),
                )
            } else {
                (
                    lv.row_values(0).to_vec(),
                    lv.row_values(lv.rows - 1).to_vec(),
                )
            };
            lift(vec![needle], move |v| {
                approx_asc(&v[0], &keys)
                    .and_then(|i| results.get(i).cloned())
                    .unwrap_or(Value::Error(ErrorKind::NA))
            })
        }
        "INDEX" => index(ctx, args),
        "OFFSET" => offset(ctx, args),
        "INDIRECT" => indirect(ctx, args),
        "ROW" | "COLUMN" => {
            let row = name == "ROW";
            if !present(args, 0) {
                return Some(Value::Number(
                    if row { ctx.cell.row } else { ctx.cell.col } as f64 + 1.0,
                ));
            }
            match arg_raw(ctx, args, 0) {
                Value::Ref(r) => {
                    let rect = ctx.trim_rect(&r);
                    if row {
                        Value::array(Array::column(
                            (rect.r0..=rect.r1)
                                .map(|x| Value::Number(x as f64 + 1.0))
                                .collect(),
                        ))
                    } else {
                        Value::array(Array::row(
                            (rect.c0..=rect.c1)
                                .map(|x| Value::Number(x as f64 + 1.0))
                                .collect(),
                        ))
                    }
                }
                Value::Error(e) => err(e),
                _ => err(ErrorKind::Value),
            }
        }
        "ROWS" | "COLUMNS" => {
            let row = name == "ROWS";
            match arg_raw(ctx, args, 0) {
                Value::Ref(r) => {
                    Value::Number(if row { r.rect.rows() } else { r.rect.cols() } as f64)
                }
                Value::Error(e) => err(e),
                v => {
                    let a = ctx.array(v);
                    Value::Number(if row { a.rows } else { a.cols } as f64)
                }
            }
        }
        "AREAS" => match arg_raw(ctx, args, 0) {
            Value::Ref(_) => Value::Number(1.0),
            _ => err(ErrorKind::Value),
        },
        "SHEET" => match arg_raw(ctx, args, 0) {
            Value::Ref(r) => Value::Number(r.sheet as f64 + 1.0),
            Value::Empty => Value::Number(ctx.sheet as f64 + 1.0),
            Value::Text(t) => ctx
                .wb
                .sheet_index(&t)
                .map(|i| Value::Number(i as f64 + 1.0))
                .unwrap_or(err(ErrorKind::NA)),
            _ => err(ErrorKind::Value),
        },
        "SHEETS" => Value::Number(ctx.wb.sheets.len() as f64),
        "ADDRESS" => {
            let r = tryv!(arg_num(ctx, args, 0, 1.0)) as i64;
            let c = tryv!(arg_num(ctx, args, 1, 1.0)) as i64;
            let abs = tryv!(arg_num(ctx, args, 2, 1.0)) as i64;
            let a1 = !present(args, 3) || tryv!(truthy(&arg(ctx, args, 3)));
            if r < 1
                || c < 1
                || r > MAX_ROWS as i64
                || c > MAX_COLS as i64
                || !(1..=4).contains(&abs)
            {
                return Some(err(ErrorKind::Value));
            }
            let (ra, ca) = (abs == 1 || abs == 2, abs == 1 || abs == 3);
            let body = if a1 {
                format!(
                    "{}{}{}{}",
                    if ca { "$" } else { "" },
                    col_name(c as u32 - 1),
                    if ra { "$" } else { "" },
                    r
                )
            } else {
                format!(
                    "R{}C{}",
                    if ra { r.to_string() } else { format!("[{r}]") },
                    if ca { c.to_string() } else { format!("[{c}]") }
                )
            };
            if present(args, 4) {
                let s = tryv!(to_text(&arg(ctx, args, 4)));
                let q = if s.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    s
                } else {
                    format!("'{}'", s.replace('\'', "''"))
                };
                Value::text(format!("{q}!{body}"))
            } else {
                Value::text(body)
            }
        }
        "FORMULATEXT" => match arg_raw(ctx, args, 0) {
            Value::Ref(r) => match ctx.wb.sheets[r.sheet]
                .cells
                .get(&r.rect.start())
                .map(|c| &c.input)
            {
                Some(f @ crate::workbook::Input::Formula { .. }) => Value::text(format!(
                    "={}",
                    f.formula_text(r.rect.start()).unwrap_or_default()
                )),
                _ => err(ErrorKind::NA),
            },
            _ => err(ErrorKind::NA),
        },
        _ => return None,
    })
}
