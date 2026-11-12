//! Dynamic-array functions: FILTER, SORT, SORTBY, UNIQUE, SEQUENCE, stacking & shaping.

use std::cmp::Ordering;
use std::sync::Arc;

use super::{arg, arg_num, arr, err, present, tryv};
use crate::eval::{compare, num, truthy, Ctx};
use crate::parser::Expr;
use crate::value::{Array, ErrorKind, Value};

pub const NAMES: &[&str] = &[
    "FILTER",
    "SORT",
    "SORTBY",
    "UNIQUE",
    "SEQUENCE",
    "TRANSPOSE",
    "VSTACK",
    "HSTACK",
    "TAKE",
    "DROP",
    "CHOOSEROWS",
    "CHOOSECOLS",
    "TOROW",
    "TOCOL",
    "WRAPROWS",
    "WRAPCOLS",
    "EXPAND",
    "MMULT",
    "MUNIT",
];

const MAX_CELLS: usize = 16_777_216; // guard against runaway arrays (16M cells)

fn rows_of(a: &Array) -> Vec<Vec<Value>> {
    (0..a.rows).map(|r| a.row_values(r).to_vec()).collect()
}

fn from_rows(rows: Vec<Vec<Value>>) -> Value {
    if rows.is_empty() {
        return err(ErrorKind::Calc);
    }
    let cols = rows[0].len();
    let n = rows.len();
    Value::array(Array::new(n, cols, rows.into_iter().flatten().collect()))
}

fn sort_cmp(a: &Value, b: &Value) -> Ordering {
    // blanks sort last regardless of direction is handled by caller; errors last
    match (a, b) {
        (Value::Empty, Value::Empty) => Ordering::Equal,
        (Value::Empty, _) => Ordering::Greater,
        (_, Value::Empty) => Ordering::Less,
        _ => compare(a, b),
    }
}

fn select(n: usize, idx: &[f64]) -> Result<Vec<usize>, ErrorKind> {
    idx.iter()
        .map(|&i| {
            let i = i.trunc() as i64;
            let k = if i > 0 { i - 1 } else { n as i64 + i };
            if i == 0 || k < 0 || k as usize >= n {
                Err(ErrorKind::Value)
            } else {
                Ok(k as usize)
            }
        })
        .collect()
}

fn nums(ctx: &mut Ctx, args: &[Expr], from: usize) -> Result<Vec<f64>, ErrorKind> {
    let mut out = Vec::new();
    for i in from..args.len() {
        for v in &arr(ctx, args, i).data {
            out.push(num(v)?);
        }
    }
    Ok(out)
}

fn stack(arrays: &[Arc<Array>], vertical: bool) -> Value {
    let pad = Value::Error(ErrorKind::NA);
    if vertical {
        let cols = arrays.iter().map(|a| a.cols).max().unwrap_or(0);
        let mut data = Vec::new();
        let mut rows = 0;
        for a in arrays {
            for r in 0..a.rows {
                for c in 0..cols {
                    data.push(if c < a.cols {
                        a.get(r, c).clone()
                    } else {
                        pad.clone()
                    });
                }
            }
            rows += a.rows;
        }
        Value::array(Array::new(rows, cols, data))
    } else {
        let rows = arrays.iter().map(|a| a.rows).max().unwrap_or(0);
        let cols: usize = arrays.iter().map(|a| a.cols).sum();
        let mut data = Vec::with_capacity(rows * cols);
        for r in 0..rows {
            for a in arrays {
                for c in 0..a.cols {
                    data.push(if r < a.rows {
                        a.get(r, c).clone()
                    } else {
                        pad.clone()
                    });
                }
            }
        }
        Value::array(Array::new(rows, cols, data))
    }
}

pub fn call(ctx: &mut Ctx, name: &str, args: &[Expr]) -> Option<Value> {
    Some(match name {
        "FILTER" => {
            let a = arr(ctx, args, 0);
            let inc = arr(ctx, args, 1);
            let by_row = inc.cols == 1 && inc.rows == a.rows;
            let by_col = inc.rows == 1 && inc.cols == a.cols;
            if !by_row && !by_col {
                return Some(err(ErrorKind::Value));
            }
            let mut keep = Vec::new();
            for v in &inc.data {
                keep.push(tryv!(truthy(v)));
            }
            let out = if by_row && (!by_col || a.cols == 1 || a.rows != 1) {
                let rows: Vec<Vec<Value>> = rows_of(&a)
                    .into_iter()
                    .zip(&keep)
                    .filter(|(_, k)| **k)
                    .map(|(r, _)| r)
                    .collect();
                if rows.is_empty() {
                    None
                } else {
                    Some(from_rows(rows))
                }
            } else {
                let t = a.transpose();
                let cols: Vec<Vec<Value>> = rows_of(&t)
                    .into_iter()
                    .zip(&keep)
                    .filter(|(_, k)| **k)
                    .map(|(r, _)| r)
                    .collect();
                if cols.is_empty() {
                    None
                } else {
                    match from_rows(cols) {
                        Value::Array(x) => Some(Value::array(x.transpose())),
                        v => Some(v),
                    }
                }
            };
            out.unwrap_or_else(|| {
                if present(args, 2) {
                    arg(ctx, args, 2)
                } else {
                    err(ErrorKind::Calc)
                }
            })
        }
        "SORT" => {
            let a = arr(ctx, args, 0);
            let idx = nums(ctx, &args[..2.min(args.len())], 1);
            let ord = nums(ctx, &args[..3.min(args.len())], 2);
            let (idx, ord) = (tryv!(idx), tryv!(ord));
            let by_col = present(args, 3) && tryv!(truthy(&arg(ctx, args, 3)));
            let m = if by_col { a.transpose() } else { (*a).clone() };
            let keys: Vec<usize> = if idx.is_empty() {
                vec![0]
            } else {
                idx.iter().map(|i| *i as usize - 1).collect()
            };
            if keys.iter().any(|k| *k >= m.cols) || idx.iter().any(|i| *i < 1.0) {
                return Some(err(ErrorKind::Value));
            }
            let dirs: Vec<f64> = if ord.is_empty() { vec![1.0] } else { ord };
            if dirs.iter().any(|d| *d != 1.0 && *d != -1.0) {
                return Some(err(ErrorKind::Value));
            }
            let mut rows = rows_of(&m);
            rows.sort_by(|x, y| {
                for (i, k) in keys.iter().enumerate() {
                    let d = dirs.get(i).copied().unwrap_or(dirs[0]);
                    let c = sort_cmp(&x[*k], &y[*k]);
                    let c = if d < 0.0
                        && !matches!((&x[*k], &y[*k]), (Value::Empty, _) | (_, Value::Empty))
                    {
                        c.reverse()
                    } else {
                        c
                    };
                    if c != Ordering::Equal {
                        return c;
                    }
                }
                Ordering::Equal
            });
            match from_rows(rows) {
                Value::Array(x) if by_col => Value::array(x.transpose()),
                v => v,
            }
        }
        "SORTBY" => {
            let a = arr(ctx, args, 0);
            let mut keys = Vec::new();
            let mut i = 1;
            while i < args.len() {
                let k = arr(ctx, args, i);
                let d = tryv!(arg_num(ctx, args, i + 1, 1.0));
                if d != 1.0 && d != -1.0 {
                    return Some(err(ErrorKind::Value));
                }
                keys.push((k, d));
                i += 2;
            }
            let by_row = keys
                .first()
                .map_or(true, |(k, _)| k.cols == 1 && k.rows == a.rows);
            let n = if by_row { a.rows } else { a.cols };
            if keys
                .iter()
                .any(|(k, _)| k.data.len() != n || !(k.rows == 1 || k.cols == 1))
            {
                return Some(err(ErrorKind::Value));
            }
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|&x, &y| {
                for (k, d) in &keys {
                    let c = sort_cmp(&k.data[x], &k.data[y]);
                    let c = if *d < 0.0 { c.reverse() } else { c };
                    if c != Ordering::Equal {
                        return c;
                    }
                }
                Ordering::Equal
            });
            let m = if by_row { (*a).clone() } else { a.transpose() };
            let rows = rows_of(&m);
            match from_rows(order.into_iter().map(|i| rows[i].clone()).collect()) {
                Value::Array(x) if !by_row => Value::array(x.transpose()),
                v => v,
            }
        }
        "UNIQUE" => {
            let a = arr(ctx, args, 0);
            let by_col = present(args, 1) && tryv!(truthy(&arg(ctx, args, 1)));
            let once = present(args, 2) && tryv!(truthy(&arg(ctx, args, 2)));
            let m = if by_col { a.transpose() } else { (*a).clone() };
            let rows = rows_of(&m);
            let eq = |x: &Vec<Value>, y: &Vec<Value>| {
                x.iter().zip(y).all(|(p, q)| {
                    p.type_rank() == q.type_rank() && compare(p, q) == Ordering::Equal
                })
            };
            let mut groups: Vec<(Vec<Value>, usize)> = Vec::new();
            for r in rows {
                match groups.iter_mut().find(|(g, _)| eq(g, &r)) {
                    Some(g) => g.1 += 1,
                    None => groups.push((r, 1)),
                }
            }
            let out: Vec<Vec<Value>> = groups
                .into_iter()
                .filter(|(_, n)| !once || *n == 1)
                .map(|(r, _)| r)
                .collect();
            if out.is_empty() {
                return Some(err(ErrorKind::Calc));
            }
            match from_rows(out) {
                Value::Array(x) if by_col => Value::array(x.transpose()),
                v => v,
            }
        }
        "SEQUENCE" => {
            let rows = tryv!(arg_num(ctx, args, 0, 1.0)).trunc();
            let cols = tryv!(arg_num(ctx, args, 1, 1.0)).trunc();
            let start = tryv!(arg_num(ctx, args, 2, 1.0));
            let step = tryv!(arg_num(ctx, args, 3, 1.0));
            if rows < 1.0 || cols < 1.0 {
                return Some(err(ErrorKind::Calc));
            }
            if rows * cols > MAX_CELLS as f64 {
                return Some(err(ErrorKind::Num));
            }
            let (r, c) = (rows as usize, cols as usize);
            Value::array(Array::new(
                r,
                c,
                (0..r * c)
                    .map(|i| Value::Number(start + step * i as f64))
                    .collect(),
            ))
        }
        "TRANSPOSE" => Value::array(arr(ctx, args, 0).transpose()),
        "VSTACK" | "HSTACK" => {
            let arrays: Vec<_> = (0..args.len()).map(|i| arr(ctx, args, i)).collect();
            stack(&arrays, name == "VSTACK")
        }
        "TAKE" | "DROP" => {
            let a = arr(ctx, args, 0);
            let take = name == "TAKE";
            let span = |n: Option<f64>, len: usize| -> (usize, usize) {
                match n {
                    None => (0, len),
                    Some(n) => {
                        let k = (n.abs().trunc() as usize).min(len);
                        match (take, n >= 0.0) {
                            (true, true) => (0, k),
                            (true, false) => (len - k, len),
                            (false, true) => (k, len),
                            (false, false) => (0, len - k),
                        }
                    }
                }
            };
            let rn = if present(args, 1) {
                Some(tryv!(arg_num(ctx, args, 1, 0.0)))
            } else {
                None
            };
            let cn = if present(args, 2) {
                Some(tryv!(arg_num(ctx, args, 2, 0.0)))
            } else {
                None
            };
            let (r0, r1) = span(rn, a.rows);
            let (c0, c1) = span(cn, a.cols);
            if r0 >= r1 || c0 >= c1 {
                return Some(err(ErrorKind::Calc));
            }
            let mut data = Vec::new();
            for r in r0..r1 {
                data.extend_from_slice(&a.row_values(r)[c0..c1]);
            }
            Value::array(Array::new(r1 - r0, c1 - c0, data))
        }
        "CHOOSEROWS" | "CHOOSECOLS" => {
            let a = arr(ctx, args, 0);
            let cols = name == "CHOOSECOLS";
            let m = if cols { a.transpose() } else { (*a).clone() };
            let idx = tryv!(select(m.rows, &tryv!(nums(ctx, args, 1))));
            if idx.is_empty() {
                return Some(err(ErrorKind::Value));
            }
            let rows = rows_of(&m);
            match from_rows(idx.into_iter().map(|i| rows[i].clone()).collect()) {
                Value::Array(x) if cols => Value::array(x.transpose()),
                v => v,
            }
        }
        "TOROW" | "TOCOL" => {
            let a = arr(ctx, args, 0);
            let ignore = tryv!(arg_num(ctx, args, 1, 0.0)) as i64;
            let by_col = present(args, 2) && tryv!(truthy(&arg(ctx, args, 2)));
            let m = if by_col { a.transpose() } else { (*a).clone() };
            let vals: Vec<Value> = m
                .data
                .into_iter()
                .filter(|v| match ignore {
                    1 => !matches!(v, Value::Empty),
                    2 => !matches!(v, Value::Error(_)),
                    3 => !matches!(v, Value::Empty | Value::Error(_)),
                    _ => true,
                })
                .collect();
            if vals.is_empty() {
                return Some(err(ErrorKind::Calc));
            }
            Value::array(if name == "TOROW" {
                Array::row(vals)
            } else {
                Array::column(vals)
            })
        }
        "WRAPROWS" | "WRAPCOLS" => {
            let a = arr(ctx, args, 0);
            if a.rows != 1 && a.cols != 1 {
                return Some(err(ErrorKind::Value));
            }
            let n = tryv!(arg_num(ctx, args, 1, 0.0)).trunc();
            if n < 1.0 {
                return Some(err(ErrorKind::Num));
            }
            let n = n as usize;
            let pad = if present(args, 2) {
                arg(ctx, args, 2)
            } else {
                err(ErrorKind::NA)
            };
            let k = a.data.len().div_ceil(n);
            let mut data = a.data.clone();
            data.resize(k * n, pad);
            let m = Array::new(k, n, data);
            Value::array(if name == "WRAPROWS" { m } else { m.transpose() })
        }
        "EXPAND" => {
            let a = arr(ctx, args, 0);
            let r = if present(args, 1) {
                tryv!(arg_num(ctx, args, 1, 0.0)) as usize
            } else {
                a.rows
            };
            let c = if present(args, 2) {
                tryv!(arg_num(ctx, args, 2, 0.0)) as usize
            } else {
                a.cols
            };
            if r < a.rows || c < a.cols {
                return Some(err(ErrorKind::Value));
            }
            let pad = if present(args, 3) {
                arg(ctx, args, 3)
            } else {
                err(ErrorKind::NA)
            };
            let mut data = Vec::with_capacity(r * c);
            for i in 0..r {
                for j in 0..c {
                    data.push(if i < a.rows && j < a.cols {
                        a.get(i, j).clone()
                    } else {
                        pad.clone()
                    });
                }
            }
            Value::array(Array::new(r, c, data))
        }
        "MMULT" => {
            let a = arr(ctx, args, 0);
            let b = arr(ctx, args, 1);
            if a.cols != b.rows {
                return Some(err(ErrorKind::Value));
            }
            let mut data = Vec::with_capacity(a.rows * b.cols);
            for i in 0..a.rows {
                for j in 0..b.cols {
                    let mut s = 0.0;
                    for k in 0..a.cols {
                        match (a.get(i, k), b.get(k, j)) {
                            (Value::Number(x), Value::Number(y)) => s += x * y,
                            _ => return Some(err(ErrorKind::Value)),
                        }
                    }
                    data.push(Value::Number(s));
                }
            }
            Value::array(Array::new(a.rows, b.cols, data))
        }
        "MUNIT" => {
            let n = tryv!(arg_num(ctx, args, 0, 1.0)) as usize;
            if n == 0 {
                return Some(err(ErrorKind::Value));
            }
            Value::array(Array::new(
                n,
                n,
                (0..n * n)
                    .map(|i| Value::Number((i / n == i % n) as i32 as f64))
                    .collect(),
            ))
        }
        _ => return None,
    })
}
