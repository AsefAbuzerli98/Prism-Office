//! Logical, information, LET/LAMBDA and lambda-helper functions, `=PY`.

use std::hash::{Hash, Hasher};

use super::{
    apply, arg, arg_num, arg_raw, arg_text, arr, callable, err, lift, lifted, present, tryv,
};
use crate::eval::{compare, num, truthy, Ctx};
use crate::parser::Expr;
use crate::value::{Array, ErrorKind, Value};
use crate::workbook::AsyncRequest;

pub const NAMES: &[&str] = &[
    "IF",
    "IFS",
    "IFERROR",
    "IFNA",
    "SWITCH",
    "CHOOSE",
    "AND",
    "OR",
    "XOR",
    "NOT",
    "TRUE",
    "FALSE",
    "ISNUMBER",
    "ISTEXT",
    "ISNONTEXT",
    "ISBLANK",
    "ISERROR",
    "ISERR",
    "ISNA",
    "ISLOGICAL",
    "ISREF",
    "ISEVEN",
    "ISODD",
    "ISFORMULA",
    "NA",
    "ERROR.TYPE",
    "TYPE",
    "N",
    "T",
    "LET",
    "LAMBDA",
    "MAP",
    "REDUCE",
    "SCAN",
    "BYROW",
    "BYCOL",
    "MAKEARRAY",
    "ISOMITTED",
    "PY",
];

/// A lambda result that must be a single value.
pub(crate) fn scalar_of(ctx: &Ctx, v: Value) -> Value {
    match ctx.deref(v) {
        Value::Array(a) if a.data.len() == 1 => a.data[0].clone(),
        Value::Array(_) => Value::Error(ErrorKind::Calc),
        s => s,
    }
}

fn helper_arrays(ctx: &mut Ctx, args: &[Expr], n: usize) -> Vec<std::sync::Arc<Array>> {
    (0..n).map(|i| arr(ctx, args, i)).collect()
}

fn type_code(v: &Value) -> f64 {
    match v {
        Value::Number(_) | Value::Empty => 1.0,
        Value::Text(_) => 2.0,
        Value::Bool(_) => 4.0,
        Value::Error(_) => 16.0,
        Value::Array(_) | Value::Ref(_) => 64.0,
        Value::Lambda(_) => 128.0,
    }
}

fn json_of(v: &Value) -> serde_json::Value {
    match v {
        Value::Number(n) => serde_json::json!(n),
        Value::Text(t) => serde_json::json!(t.as_ref()),
        Value::Bool(b) => serde_json::json!(b),
        Value::Error(e) => serde_json::json!({ "error": e.code() }),
        _ => serde_json::Value::Null,
    }
}

/// `xl("A1:B5")` / `xl("Table1[#All]", headers=True)` references inside Python code.
pub fn xl_refs(code: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"xl\(\s*"((?:[^"\\]|\\.)*)""#).expect("static regex");
    let mut out: Vec<String> = re.captures_iter(code).map(|c| c[1].to_string()).collect();
    out.dedup();
    out
}

fn py(ctx: &mut Ctx, args: &[Expr]) -> Value {
    let code = tryv!(arg_text(ctx, args, 0));
    let return_type = tryv!(arg_num(ctx, args, 1, 0.0)) as i32;
    let mut inputs = Vec::new();
    for r in xl_refs(&code) {
        let Ok(e) = crate::parser::parse(&r, ctx.cell) else {
            return err(ErrorKind::Ref);
        };
        let saved = std::mem::take(&mut ctx.offset);
        let v = ctx.eval(&e);
        ctx.offset = saved;
        let a = ctx.array(v);
        let rows = (0..a.rows)
            .map(|i| a.row_values(i).iter().map(json_of).collect())
            .collect();
        inputs.push((r, rows));
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut h);
    return_type.hash(&mut h);
    serde_json::to_string(&inputs)
        .unwrap_or_default()
        .hash(&mut h);
    let key = format!("{:016x}", h.finish());
    if let Some((k, v)) = ctx.wb.async_results.get(&(ctx.sheet, ctx.cell)) {
        if *k == key {
            return v.clone();
        }
    }
    ctx.requests.push(AsyncRequest {
        key,
        sheet: ctx.sheet,
        row: ctx.cell.row,
        col: ctx.cell.col,
        function: "PY".into(),
        code,
        return_type,
        inputs,
    });
    err(ErrorKind::Busy)
}

pub fn call(ctx: &mut Ctx, name: &str, args: &[Expr]) -> Option<Value> {
    Some(match name {
        "IF" => {
            let c = arg(ctx, args, 0);
            if let Value::Array(_) = c {
                let t = if args.len() > 1 {
                    arg(ctx, args, 1)
                } else {
                    Value::Bool(true)
                };
                let f = if args.len() > 2 {
                    arg(ctx, args, 2)
                } else {
                    Value::Bool(false)
                };
                return Some(lift(vec![c, t, f], |v| match truthy(&v[0]) {
                    Ok(true) => v[1].clone(),
                    Ok(false) => v[2].clone(),
                    Err(e) => Value::Error(e),
                }));
            }
            match truthy(&c) {
                Ok(true) if args.len() > 1 => arg_raw(ctx, args, 1),
                Ok(true) => Value::Bool(true),
                Ok(false) if args.len() > 2 => arg_raw(ctx, args, 2),
                Ok(false) => Value::Bool(false),
                Err(e) => err(e),
            }
        }
        "IFS" => {
            for p in args.chunks(2) {
                if p.len() < 2 {
                    return Some(err(ErrorKind::NA));
                }
                match truthy(&arg(ctx, p, 0)) {
                    Ok(true) => return Some(arg_raw(ctx, p, 1)),
                    Ok(false) => {}
                    Err(e) => return Some(err(e)),
                }
            }
            err(ErrorKind::NA)
        }
        "IFERROR" | "IFNA" => {
            let raw = arg_raw(ctx, args, 0);
            let hit = |v: &Value| match v {
                Value::Error(ErrorKind::NA) => true,
                Value::Error(_) => name == "IFERROR",
                _ => false,
            };
            match ctx.deref(raw.clone()) {
                v if hit(&v) => arg_raw(ctx, args, 1),
                Value::Array(a) if a.data.iter().any(hit) => {
                    let alt = arg(ctx, args, 1);
                    let alt = scalar_of(ctx, alt);
                    Value::array(Array::new(
                        a.rows,
                        a.cols,
                        a.data
                            .iter()
                            .map(|v| if hit(v) { alt.clone() } else { v.clone() })
                            .collect(),
                    ))
                }
                _ => raw,
            }
        }
        "SWITCH" => {
            let v = arg(ctx, args, 0);
            if let Value::Error(e) = v {
                return Some(err(e));
            }
            let rest = &args[1.min(args.len())..];
            let mut i = 0;
            while i + 1 < rest.len() {
                let k = arg(ctx, rest, i);
                if compare(&v, &k).is_eq() && v.type_rank() == k.type_rank() {
                    return Some(arg_raw(ctx, rest, i + 1));
                }
                i += 2;
            }
            if i < rest.len() {
                arg_raw(ctx, rest, i)
            } else {
                err(ErrorKind::NA)
            }
        }
        "CHOOSE" => {
            let idx = arg(ctx, args, 0);
            if let Value::Array(_) = idx {
                let opts: Vec<Value> = (1..args.len()).map(|i| arg(ctx, args, i)).collect();
                return Some(lift(vec![idx], move |v| match num(&v[0]) {
                    Ok(n) if n >= 1.0 && (n as usize) <= opts.len() => {
                        scalar_or(&opts[n as usize - 1])
                    }
                    Ok(_) => Value::Error(ErrorKind::Value),
                    Err(e) => Value::Error(e),
                }));
            }
            match num(&idx) {
                Ok(n) if n >= 1.0 && (n as usize) < args.len() => arg_raw(ctx, args, n as usize),
                Ok(_) => err(ErrorKind::Value),
                Err(e) => err(e),
            }
        }
        "AND" | "OR" | "XOR" => {
            let mut seen = false;
            let (mut all, mut any, mut odd) = (true, false, false);
            for i in 0..args.len() {
                let raw = arg_raw(ctx, args, i);
                let direct = !matches!(raw, Value::Ref(_) | Value::Array(_));
                for v in &ctx.array(raw).data {
                    let b = match v {
                        Value::Error(e) => return Some(err(*e)),
                        Value::Number(n) => *n != 0.0,
                        Value::Bool(b) => *b,
                        Value::Text(_) if direct => match truthy(v) {
                            Ok(b) => b,
                            Err(e) => return Some(err(e)),
                        },
                        _ => continue,
                    };
                    seen = true;
                    all &= b;
                    any |= b;
                    odd ^= b;
                }
            }
            if !seen {
                return Some(err(ErrorKind::Value));
            }
            Value::Bool(match name {
                "AND" => all,
                "OR" => any,
                _ => odd,
            })
        }
        "NOT" => lifted(ctx, args, |v| {
            truthy(&v[0])
                .map(|b| Value::Bool(!b))
                .unwrap_or_else(Value::Error)
        }),
        "TRUE" => Value::Bool(true),
        "FALSE" => Value::Bool(false),
        "ISNUMBER" => lifted(ctx, args, |v| Value::Bool(matches!(v[0], Value::Number(_)))),
        "ISTEXT" => lifted(ctx, args, |v| Value::Bool(matches!(v[0], Value::Text(_)))),
        "ISNONTEXT" => lifted(ctx, args, |v| Value::Bool(!matches!(v[0], Value::Text(_)))),
        "ISBLANK" => lifted(ctx, args, |v| Value::Bool(matches!(v[0], Value::Empty))),
        "ISLOGICAL" => lifted(ctx, args, |v| Value::Bool(matches!(v[0], Value::Bool(_)))),
        "ISERROR" => lifted(ctx, args, |v| Value::Bool(matches!(v[0], Value::Error(_)))),
        "ISERR" => lifted(ctx, args, |v| {
            Value::Bool(matches!(v[0], Value::Error(e) if e != ErrorKind::NA))
        }),
        "ISNA" => lifted(ctx, args, |v| {
            Value::Bool(matches!(v[0], Value::Error(ErrorKind::NA)))
        }),
        "ISEVEN" | "ISODD" => {
            let odd = name == "ISODD";
            lifted(ctx, args, move |v| {
                num(&v[0])
                    .map(|n| Value::Bool(((n.trunc() as i64).rem_euclid(2) == 1) == odd))
                    .unwrap_or_else(Value::Error)
            })
        }
        "ISREF" => Value::Bool(matches!(arg_raw(ctx, args, 0), Value::Ref(_))),
        "ISFORMULA" => match arg_raw(ctx, args, 0) {
            Value::Ref(r) => Value::Bool(matches!(
                ctx.wb.sheets[r.sheet]
                    .cells
                    .get(&r.rect.start())
                    .map(|c| &c.input),
                Some(crate::workbook::Input::Formula { .. })
            )),
            _ => err(ErrorKind::Value),
        },
        "NA" => err(ErrorKind::NA),
        "ERROR.TYPE" => lifted(ctx, args, |v| match v[0] {
            Value::Error(e) => Value::Number(match e {
                ErrorKind::Null => 1.0,
                ErrorKind::Div0 => 2.0,
                ErrorKind::Value => 3.0,
                ErrorKind::Ref => 4.0,
                ErrorKind::Name => 5.0,
                ErrorKind::Num => 6.0,
                ErrorKind::NA => 7.0,
                ErrorKind::Spill => 9.0,
                ErrorKind::Calc => 14.0,
                ErrorKind::Busy => 8.0,
                ErrorKind::Python => 19.0,
            }),
            _ => Value::Error(ErrorKind::NA),
        }),
        "TYPE" => {
            let v = arg(ctx, args, 0);
            Value::Number(type_code(&v))
        }
        "N" => match scalar_or(&arg(ctx, args, 0)) {
            Value::Number(n) => Value::Number(n),
            Value::Bool(b) => Value::Number(b as i32 as f64),
            Value::Error(e) => err(e),
            _ => Value::Number(0.0),
        },
        "T" => match scalar_or(&arg(ctx, args, 0)) {
            Value::Text(t) => Value::Text(t),
            Value::Error(e) => err(e),
            _ => Value::text(""),
        },
        "LET" => {
            if args.len() < 3 || args.len() % 2 == 0 {
                return Some(err(ErrorKind::Value));
            }
            let base = ctx.env.len();
            for p in args[..args.len() - 1].chunks(2) {
                let Expr::Name(n) = &p[0] else {
                    ctx.env.truncate(base);
                    return Some(err(ErrorKind::Value));
                };
                let v = ctx.eval(&p[1]);
                ctx.env.push((n.clone(), v));
            }
            let v = ctx.eval(&args[args.len() - 1]);
            ctx.env.truncate(base);
            v
        }
        "LAMBDA" => {
            let Some((body, params)) = args.split_last() else {
                return Some(err(ErrorKind::Value));
            };
            let mut names = Vec::new();
            for p in params {
                match p {
                    Expr::Name(n) => names.push(n.clone()),
                    _ => return Some(err(ErrorKind::Value)),
                }
            }
            ctx.make_lambda(names, body)
        }
        "ISOMITTED" => match args.first() {
            Some(Expr::Name(n)) => Value::Bool(
                ctx.env
                    .iter()
                    .rev()
                    .find(|(k, _)| k.eq_ignore_ascii_case(n))
                    .is_some_and(|(_, v)| matches!(v, Value::Empty)),
            ),
            _ => Value::Bool(false),
        },
        "MAP" => {
            if args.len() < 2 {
                return Some(err(ErrorKind::Value));
            }
            let f = callable(ctx, args, args.len() - 1);
            let arrays = helper_arrays(ctx, args, args.len() - 1);
            let rows = arrays.iter().map(|a| a.rows).max().unwrap_or(1);
            let cols = arrays.iter().map(|a| a.cols).max().unwrap_or(1);
            let mut data = Vec::with_capacity(rows * cols);
            for r in 0..rows {
                for c in 0..cols {
                    let vals = arrays
                        .iter()
                        .map(|a| {
                            let (rr, cc) = (
                                if a.rows == 1 { 0 } else { r },
                                if a.cols == 1 { 0 } else { c },
                            );
                            if rr < a.rows && cc < a.cols {
                                a.get(rr, cc).clone()
                            } else {
                                Value::Error(ErrorKind::NA)
                            }
                        })
                        .collect();
                    let v = apply(ctx, &f, vals);
                    data.push(scalar_of(ctx, v));
                }
            }
            Value::array(Array::new(rows, cols, data))
        }
        "REDUCE" | "SCAN" => {
            let mut acc = arg(ctx, args, 0);
            let arr = arr(ctx, args, 1);
            let f = callable(ctx, args, 2);
            let mut out = Vec::new();
            for v in &arr.data {
                acc = apply(ctx, &f, vec![acc, v.clone()]);
                acc = ctx.deref(acc);
                if name == "SCAN" {
                    acc = scalar_of(ctx, acc);
                    out.push(acc.clone());
                }
            }
            if name == "SCAN" {
                Value::array(Array::new(arr.rows, arr.cols, out))
            } else {
                acc
            }
        }
        "BYROW" | "BYCOL" => {
            let arr = arr(ctx, args, 0);
            let f = callable(ctx, args, 1);
            let n = if name == "BYROW" { arr.rows } else { arr.cols };
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let slice = if name == "BYROW" {
                    Array::new(1, arr.cols, arr.row_values(i).to_vec())
                } else {
                    Array::new(
                        arr.rows,
                        1,
                        (0..arr.rows).map(|r| arr.get(r, i).clone()).collect(),
                    )
                };
                let v = apply(ctx, &f, vec![Value::array(slice)]);
                out.push(scalar_of(ctx, v));
            }
            Value::array(if name == "BYROW" {
                Array::column(out)
            } else {
                Array::row(out)
            })
        }
        "MAKEARRAY" => {
            let rows = tryv!(arg_num(ctx, args, 0, 0.0)) as usize;
            let cols = tryv!(arg_num(ctx, args, 1, 0.0)) as usize;
            if rows == 0 || cols == 0 || rows * cols > 1_000_000 || !present(args, 2) {
                return Some(err(ErrorKind::Value));
            }
            let f = callable(ctx, args, 2);
            let mut data = Vec::with_capacity(rows * cols);
            for r in 0..rows {
                for c in 0..cols {
                    let v = apply(
                        ctx,
                        &f,
                        vec![Value::Number(r as f64 + 1.0), Value::Number(c as f64 + 1.0)],
                    );
                    data.push(scalar_of(ctx, v));
                }
            }
            Value::array(Array::new(rows, cols, data))
        }
        "PY" => py(ctx, args),
        _ => return None,
    })
}

fn scalar_or(v: &Value) -> Value {
    match v {
        Value::Array(a) => a.data.first().cloned().unwrap_or_default(),
        v => v.clone(),
    }
}
