//! Function library and dispatch.

pub mod array;
pub mod date;
pub mod group;
pub mod logic;
pub mod lookup;
pub mod math;
pub mod regex_fns;
pub mod text;

use std::sync::Arc;

use crate::eval::{compare, num, text as to_text, Ctx, Res};
use crate::parser::Expr;
use crate::value::{Array, ErrorKind, Value};

/// Evaluate and materialise argument `i` (missing → Empty).
pub(crate) fn arg(ctx: &mut Ctx, args: &[Expr], i: usize) -> Value {
    match args.get(i) {
        None | Some(Expr::Missing) => Value::Empty,
        Some(e) => {
            let v = ctx.eval(e);
            ctx.deref(v)
        }
    }
}

/// Evaluate argument `i` keeping references unresolved.
pub(crate) fn arg_raw(ctx: &mut Ctx, args: &[Expr], i: usize) -> Value {
    match args.get(i) {
        None | Some(Expr::Missing) => Value::Empty,
        Some(e) => ctx.eval(e),
    }
}

/// Evaluate argument `i` as an array (scalars become 1×1).
pub(crate) fn arr(ctx: &mut Ctx, args: &[Expr], i: usize) -> Arc<Array> {
    let v = arg_raw(ctx, args, i);
    ctx.array(v)
}

pub(crate) fn present(args: &[Expr], i: usize) -> bool {
    !matches!(args.get(i), None | Some(Expr::Missing))
}

pub(crate) fn arg_num(ctx: &mut Ctx, args: &[Expr], i: usize, default: f64) -> Res<f64> {
    if !present(args, i) {
        return Ok(default);
    }
    num(&arg(ctx, args, i))
}

pub(crate) fn arg_text(ctx: &mut Ctx, args: &[Expr], i: usize) -> Res<String> {
    to_text(&arg(ctx, args, i))
}

pub(crate) fn err(e: ErrorKind) -> Value {
    Value::Error(e)
}

macro_rules! tryv {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => return ::core::convert::From::from(Value::Error(e)),
        }
    };
}
pub(crate) use tryv;

/// Numbers for aggregates: direct scalar arguments are coerced (text
/// numbers, booleans); numbers inside ranges/arrays only, text/bools ignored.
pub(crate) fn numbers(ctx: &mut Ctx, args: &[Expr]) -> Res<Vec<f64>> {
    let mut out = Vec::new();
    for a in args {
        if matches!(a, Expr::Missing) {
            continue;
        }
        let raw = ctx.eval(a);
        let from_range = matches!(raw, Value::Ref(_) | Value::Array(_));
        match ctx.deref(raw) {
            Value::Array(arr) => {
                for v in &arr.data {
                    match v {
                        Value::Number(n) => out.push(*n),
                        Value::Error(e) => return Err(*e),
                        _ => {}
                    }
                }
            }
            Value::Error(e) => return Err(e),
            Value::Empty if from_range => {}
            Value::Text(_) | Value::Bool(_) if from_range => {}
            v => out.push(num(&v)?),
        }
    }
    Ok(out)
}

/// Broadcast a scalar function over any array arguments.
pub(crate) fn lift(vals: Vec<Value>, f: impl Fn(&[Value]) -> Value) -> Value {
    let dims = vals
        .iter()
        .filter_map(|v| {
            if let Value::Array(a) = v {
                Some((a.rows, a.cols))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if dims.is_empty() {
        return f(&vals);
    }
    let rows = dims.iter().map(|d| d.0).max().unwrap();
    let cols = dims.iter().map(|d| d.1).max().unwrap();
    let mut data = Vec::with_capacity(rows * cols);
    let mut buf = vec![Value::Empty; vals.len()];
    for r in 0..rows {
        for c in 0..cols {
            let mut na = false;
            for (i, v) in vals.iter().enumerate() {
                buf[i] = match v {
                    Value::Array(a) => {
                        let rr = if a.rows == 1 { 0 } else { r };
                        let cc = if a.cols == 1 { 0 } else { c };
                        if rr < a.rows && cc < a.cols {
                            a.get(rr, cc).clone()
                        } else {
                            na = true;
                            Value::Empty
                        }
                    }
                    s => s.clone(),
                };
            }
            data.push(if na {
                Value::Error(ErrorKind::NA)
            } else {
                f(&buf)
            });
        }
    }
    Value::array(Array::new(rows, cols, data))
}

/// Evaluate every argument (materialised) and lift `f` over arrays.
pub(crate) fn lifted(ctx: &mut Ctx, args: &[Expr], f: impl Fn(&[Value]) -> Value) -> Value {
    let vals = (0..args.len()).map(|i| arg(ctx, args, i)).collect();
    lift(vals, f)
}

// ---- criteria (COUNTIF, SUMIFS …) -------------------------------------------------------

pub(crate) struct Criterion {
    op: &'static str,
    value: Value,
    pattern: Option<regex::Regex>,
}

/// Convert an Excel wildcard pattern (`*`, `?`, `~` escape) to a regex.
pub(crate) fn wildcard_regex(p: &str) -> Option<regex::Regex> {
    if !p.contains(['*', '?']) {
        return None;
    }
    let mut s = String::from("(?is)^");
    let mut chars = p.chars();
    while let Some(c) = chars.next() {
        match c {
            '~' => {
                if let Some(n) = chars.next() {
                    s.push_str(&regex::escape(&n.to_string()));
                }
            }
            '*' => s.push_str(".*"),
            '?' => s.push('.'),
            c => s.push_str(&regex::escape(&c.to_string())),
        }
    }
    s.push('$');
    regex::Regex::new(&s).ok()
}

impl Criterion {
    pub(crate) fn parse(v: &Value) -> Self {
        let Value::Text(t) = v else {
            return Self {
                op: "=",
                value: v.clone(),
                pattern: None,
            };
        };
        let t: &str = t;
        let (op, rest) = ["<=", ">=", "<>", "=", "<", ">"]
            .iter()
            .find_map(|o| t.strip_prefix(o).map(|r| (*o, r)))
            .unwrap_or(("=", t));
        let value = if rest.is_empty() {
            Value::Empty
        } else {
            match crate::workbook::parse_literal(rest) {
                Value::Text(_) => Value::text(rest),
                n => n,
            }
        };
        let pattern = if matches!(op, "=" | "<>") {
            wildcard_regex(rest)
        } else {
            None
        };
        Self { op, value, pattern }
    }

    pub(crate) fn matches(&self, v: &Value) -> bool {
        if let Some(re) = &self.pattern {
            let hit = matches!(v, Value::Text(_)) && re.is_match(&v.to_string());
            return if self.op == "=" { hit } else { !hit };
        }
        if matches!(self.value, Value::Empty) {
            let blank = matches!(v, Value::Empty) || matches!(v, Value::Text(t) if t.is_empty());
            return if self.op == "=" { blank } else { !blank };
        }
        // type must match for ordering comparisons (numbers vs numbers, text vs text)
        let same_kind = matches!(
            (&self.value, v),
            (Value::Number(_), Value::Number(_))
                | (Value::Text(_), Value::Text(_))
                | (Value::Bool(_), Value::Bool(_))
        );
        let ord = compare(v, &self.value);
        match self.op {
            "=" => same_kind && ord.is_eq(),
            "<>" => !(same_kind && ord.is_eq()),
            "<" => same_kind && ord.is_lt(),
            "<=" => same_kind && ord.is_le(),
            ">" => same_kind && ord.is_gt(),
            _ => same_kind && ord.is_ge(),
        }
    }
}

/// Positions in `ranges[k]` that satisfy every `criteria[k]` (shapes must match).
pub(crate) fn criteria_mask(ranges: &[Arc<Array>], criteria: &[Criterion]) -> Res<Vec<bool>> {
    let Some(first) = ranges.first() else {
        return Ok(vec![]);
    };
    if ranges
        .iter()
        .any(|r| r.rows != first.rows || r.cols != first.cols)
    {
        return Err(ErrorKind::Value);
    }
    Ok((0..first.data.len())
        .map(|i| {
            ranges
                .iter()
                .zip(criteria)
                .all(|(r, c)| c.matches(&r.data[i]))
        })
        .collect())
}

/// A function argument: a LAMBDA value or an eta-reduced built-in (`SUM`).
pub(crate) enum Callable {
    Builtin(String),
    Value(Value),
}

pub(crate) fn is_builtin(name: &str) -> bool {
    static SET: std::sync::OnceLock<std::collections::HashSet<&'static str>> =
        std::sync::OnceLock::new();
    SET.get_or_init(|| function_names().into_iter().collect())
        .contains(name)
}

pub(crate) fn callable(ctx: &mut Ctx, args: &[Expr], i: usize) -> Callable {
    if let Some(Expr::Name(n)) = args.get(i) {
        let up = crate::parser::strip_prefixes(&n.to_ascii_uppercase()).to_string();
        let bound = ctx.env.iter().any(|(k, _)| k.eq_ignore_ascii_case(n))
            || ctx.wb.name_expr(n, ctx.sheet).is_some();
        if !bound && is_builtin(&up) {
            return Callable::Builtin(up);
        }
    }
    Callable::Value(arg_raw(ctx, args, i))
}

pub(crate) fn apply(ctx: &mut Ctx, f: &Callable, vals: Vec<Value>) -> Value {
    match f {
        Callable::Value(v) => ctx.invoke(v, vals),
        Callable::Builtin(name) => {
            let base = ctx.env.len();
            let mut exprs = Vec::with_capacity(vals.len());
            for (k, v) in vals.into_iter().enumerate() {
                let n = format!("\u{1}{k}");
                ctx.env.push((n.clone(), v));
                exprs.push(Expr::Name(n));
            }
            let r = call(ctx, name, &exprs);
            ctx.env.truncate(base);
            r
        }
    }
}

/// Dispatch a function call.
pub fn call(ctx: &mut Ctx, name: &str, args: &[Expr]) -> Value {
    if let Some(v) = logic::call(ctx, name, args)
        .or_else(|| math::call(ctx, name, args))
        .or_else(|| text::call(ctx, name, args))
        .or_else(|| date::call(ctx, name, args))
        .or_else(|| lookup::call(ctx, name, args))
        .or_else(|| array::call(ctx, name, args))
        .or_else(|| group::call(ctx, name, args))
        .or_else(|| regex_fns::call(ctx, name, args))
    {
        return v;
    }
    // A name bound to a LAMBDA (LET variable or defined name) can be called.
    let f = ctx.eval(&Expr::Name(name.to_string()));
    if let Value::Lambda(_) = f {
        let vals: Vec<Value> = args.iter().map(|a| ctx.eval(a)).collect();
        return ctx.invoke(&f, vals);
    }
    Value::Error(ErrorKind::Name)
}

/// Names of all built-in functions (formula autocomplete).
pub fn function_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = [
        logic::NAMES,
        math::NAMES,
        text::NAMES,
        date::NAMES,
        lookup::NAMES,
        array::NAMES,
        group::NAMES,
        regex_fns::NAMES,
    ]
    .concat();
    v.sort_unstable();
    v
}

/// Functions whose result can change without any input changing.
pub const VOLATILE: &[&str] = &[
    "NOW",
    "TODAY",
    "RAND",
    "RANDBETWEEN",
    "RANDARRAY",
    "OFFSET",
    "INDIRECT",
    "CELL",
    "INFO",
];

#[cfg(test)]
mod tests {
    #[test]
    fn registry_has_no_duplicates() {
        let v = super::function_names();
        let mut d = v.clone();
        d.dedup();
        assert_eq!(v.len(), d.len());
        println!("functions: {}", v.len());
    }
}
