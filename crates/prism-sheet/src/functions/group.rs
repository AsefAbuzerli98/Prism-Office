//! GROUPBY, PIVOTBY, PERCENTOF, TRIMRANGE.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::logic::scalar_of;
use super::{apply, arg_num, arg_raw, arr, callable, err, numbers, present, tryv, Callable};
use crate::eval::{compare, truthy, Ctx};
use crate::parser::Expr;
use crate::value::{Array, ErrorKind, RangeRef, Value};

pub const NAMES: &[&str] = &[
    "GROUPBY",
    "PIVOTBY",
    "PERCENTOF",
    "TRIMRANGE",
    "_TRO_ALL",
    "_TRO_LEADING",
    "_TRO_TRAILING",
];

fn key_of(v: &[Value]) -> String {
    let mut s = String::new();
    for x in v {
        s.push(char::from(b'0' + x.type_rank()));
        s.push_str(&x.to_string().to_lowercase());
        s.push('\u{1f}');
    }
    s
}

fn cmp_keys(a: &[Value], b: &[Value]) -> Ordering {
    for (x, y) in a.iter().zip(b) {
        let c = match (x, y) {
            (Value::Empty, Value::Empty) => Ordering::Equal,
            (Value::Empty, _) => Ordering::Greater,
            (_, Value::Empty) => Ordering::Less,
            _ => compare(x, y),
        };
        if c != Ordering::Equal {
            return c;
        }
    }
    Ordering::Equal
}

fn arity(f: &Callable) -> usize {
    match f {
        Callable::Builtin(n) if n == "PERCENTOF" => 2,
        Callable::Value(Value::Lambda(l)) => l.params.len(),
        _ => 1,
    }
}

/// Apply the aggregation to one group's values (and all values for 2-ary functions).
fn agg(ctx: &mut Ctx, f: &Callable, group: Vec<Value>, all: &[Value]) -> Value {
    let g = Value::Array(std::sync::Arc::new(Array::column(group)));
    let args = if arity(f) >= 2 {
        vec![
            g,
            Value::Array(std::sync::Arc::new(Array::column(all.to_vec()))),
        ]
    } else {
        vec![g]
    };
    let v = apply(ctx, f, args);
    scalar_of(ctx, v)
}

struct Fields {
    keys: Vec<Vec<Value>>,
    headers: Option<Vec<Value>>,
}

fn rows_of(a: &Array) -> Vec<Vec<Value>> {
    (0..a.rows).map(|r| a.row_values(r).to_vec()).collect()
}

/// Field-header handling: 0 none, 1 has (hide), 2 none (generate), 3 has (show); auto when missing.
fn header_mode(ctx: &mut Ctx, args: &[Expr], i: usize, values: &Array) -> Result<i64, ErrorKind> {
    if present(args, i) {
        let m = arg_num(ctx, args, i, 0.0)? as i64;
        return if (0..=3).contains(&m) {
            Ok(m)
        } else {
            Err(ErrorKind::Value)
        };
    }
    let first_text = values.rows > 1
        && values
            .row_values(0)
            .iter()
            .all(|v| matches!(v, Value::Text(_)));
    let second_not = values.rows > 1
        && values
            .row_values(1)
            .iter()
            .any(|v| !matches!(v, Value::Text(_)));
    Ok(if first_text && second_not { 3 } else { 0 })
}

fn split_headers(a: &Array, mode: i64) -> Fields {
    let mut rows = rows_of(a);
    let headers = match mode {
        1 | 3 if !rows.is_empty() => Some(rows.remove(0)),
        2 => Some(
            (1..=a.cols)
                .map(|i| Value::text(format!("Field {i}")))
                .collect(),
        ),
        _ => None,
    };
    Fields {
        keys: rows,
        headers: if mode >= 2 { headers } else { None },
    }
}

fn filter_mask(
    ctx: &mut Ctx,
    args: &[Expr],
    i: usize,
    n: usize,
    skip_header: bool,
) -> Result<Vec<bool>, ErrorKind> {
    if !present(args, i) {
        return Ok(vec![true; n]);
    }
    let a = arr(ctx, args, i);
    let mut data: Vec<Value> = a.data.clone();
    if skip_header && data.len() == n + 1 {
        data.remove(0);
    }
    if data.len() != n {
        return Err(ErrorKind::Value);
    }
    data.iter().map(truthy).collect()
}

struct Grouped {
    keys: Vec<Vec<Value>>,
    members: Vec<Vec<usize>>,
}

fn group(keys: &[Vec<Value>], mask: &[bool]) -> Grouped {
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut g = Grouped {
        keys: vec![],
        members: vec![],
    };
    for (i, k) in keys.iter().enumerate() {
        if !mask[i] {
            continue;
        }
        let id = *index.entry(key_of(k)).or_insert_with(|| {
            g.keys.push(k.clone());
            g.members.push(vec![]);
            g.keys.len() - 1
        });
        g.members[id].push(i);
    }
    g
}

fn total_label(width: usize, label: &str) -> Vec<Value> {
    let mut v = vec![Value::Empty; width];
    if width > 0 {
        v[0] = Value::text(label);
    }
    v
}

fn groupby(ctx: &mut Ctx, args: &[Expr]) -> Value {
    let keys_a = arr(ctx, args, 0);
    let vals_a = arr(ctx, args, 1);
    if keys_a.rows != vals_a.rows {
        return err(ErrorKind::Value);
    }
    let f = callable(ctx, args, 2);
    let mode = tryv!(header_mode(ctx, args, 3, &vals_a));
    let depth = if present(args, 4) {
        tryv!(arg_num(ctx, args, 4, 1.0)) as i64
    } else {
        1
    };
    let sort = if present(args, 5) {
        Some(tryv!(arg_num(ctx, args, 5, 1.0)) as i64)
    } else {
        None
    };
    let table_rel = tryv!(arg_num(ctx, args, 7, 0.0)) == 1.0;
    let kf = split_headers(&keys_a, mode);
    let vf = split_headers(&vals_a, mode);
    let n = kf.keys.len();
    let mask = tryv!(filter_mask(ctx, args, 6, n, mode == 1 || mode == 3));
    let kcols = keys_a.cols;
    let vcols = vals_a.cols;
    let col_vals = |c: usize, idx: &[usize]| -> Vec<Value> {
        idx.iter().map(|&i| vf.keys[i][c].clone()).collect()
    };
    let all_idx: Vec<usize> = (0..n).filter(|i| mask[*i]).collect();
    let all_cols: Vec<Vec<Value>> = (0..vcols).map(|c| col_vals(c, &all_idx)).collect();

    let g = group(&kf.keys, &mask);
    let mut rows: Vec<(Vec<Value>, Vec<usize>, Vec<Value>)> = Vec::new();
    for (k, m) in g.keys.into_iter().zip(g.members) {
        let mut res = Vec::with_capacity(vcols);
        for (c, all) in all_cols.iter().enumerate() {
            res.push(agg(ctx, &f, col_vals(c, &m), all));
        }
        rows.push((k, m, res));
    }
    // sort (default: ascending by row fields)
    let s = sort.unwrap_or(1);
    let col = s.unsigned_abs() as usize;
    if col == 0 || col > kcols + vcols {
        return err(ErrorKind::Value);
    }
    rows.sort_by(|a, b| {
        let c = if col <= kcols {
            cmp_keys(&a.0[col - 1..], &b.0[col - 1..]).then_with(|| cmp_keys(&a.0, &b.0))
        } else {
            cmp_keys(
                &a.2[col - kcols - 1..col - kcols],
                &b.2[col - kcols - 1..col - kcols],
            )
        };
        if s < 0 {
            c.reverse()
        } else {
            c
        }
    });
    if !table_rel && col > kcols && kcols > 1 {
        // hierarchy keeps parents together
        rows.sort_by(|a, b| cmp_keys(&a.0[..1], &b.0[..1]));
    }

    let mut out: Vec<Vec<Value>> = Vec::new();
    let line = |k: Vec<Value>, r: Vec<Value>| -> Vec<Value> { k.into_iter().chain(r).collect() };
    let total = |ctx: &mut Ctx, idx: &[usize]| -> Vec<Value> {
        (0..vcols)
            .map(|c| agg(ctx, &f, col_vals(c, idx), &all_cols[c]))
            .collect()
    };
    let mut body: Vec<Vec<Value>> = Vec::new();
    if depth.abs() >= 2 && kcols > 1 && !table_rel {
        let mut i = 0;
        while i < rows.len() {
            let mut j = i;
            let mut idx = Vec::new();
            while j < rows.len() && cmp_keys(&rows[j].0[..1], &rows[i].0[..1]) == Ordering::Equal {
                idx.extend_from_slice(&rows[j].1);
                j += 1;
            }
            let mut sub = vec![Value::Empty; kcols];
            sub[0] = rows[i].0[0].clone();
            let sub_line = line(sub, total(ctx, &idx));
            if depth < 0 {
                body.push(sub_line.clone());
            }
            for r in &rows[i..j] {
                body.push(line(r.0.clone(), r.2.clone()));
            }
            if depth > 0 {
                body.push(sub_line);
            }
            i = j;
        }
    } else {
        body = rows
            .iter()
            .map(|r| line(r.0.clone(), r.2.clone()))
            .collect();
    }
    if let (Some(kh), Some(vh)) = (&kf.headers, &vf.headers) {
        out.push(kh.iter().chain(vh).cloned().collect());
    }
    let grand = if depth != 0 {
        Some(line(total_label(kcols, "Total"), total(ctx, &all_idx)))
    } else {
        None
    };
    if depth < 0 {
        out.extend(grand.clone());
    }
    out.extend(body);
    if depth > 0 {
        out.extend(grand);
    }
    if out.is_empty() {
        return err(ErrorKind::Calc);
    }
    let w = kcols + vcols;
    Value::array(Array::new(
        out.len(),
        w,
        out.into_iter().flatten().collect(),
    ))
}

fn pivotby(ctx: &mut Ctx, args: &[Expr]) -> Value {
    let rk = arr(ctx, args, 0);
    let ck = arr(ctx, args, 1);
    let va = arr(ctx, args, 2);
    if rk.rows != va.rows || ck.rows != va.rows {
        return err(ErrorKind::Value);
    }
    let f = callable(ctx, args, 3);
    let mode = tryv!(header_mode(ctx, args, 4, &va));
    let rdepth = if present(args, 5) {
        tryv!(arg_num(ctx, args, 5, 1.0)) as i64
    } else {
        1
    };
    let rsort = tryv!(arg_num(ctx, args, 6, 1.0)) as i64;
    let cdepth = if present(args, 7) {
        tryv!(arg_num(ctx, args, 7, 1.0)) as i64
    } else {
        1
    };
    let csort = tryv!(arg_num(ctx, args, 8, 1.0)) as i64;
    let rel = tryv!(arg_num(ctx, args, 10, 0.0)) as i64;
    let rf = split_headers(&rk, mode);
    let cf = split_headers(&ck, mode);
    let vf = split_headers(&va, mode);
    let n = rf.keys.len();
    let mask = tryv!(filter_mask(ctx, args, 9, n, mode == 1 || mode == 3));
    if va.cols != 1 {
        // one value column per pivot keeps the layout unambiguous
        return err(ErrorKind::Value);
    }
    let vals =
        |idx: &[usize]| -> Vec<Value> { idx.iter().map(|&i| vf.keys[i][0].clone()).collect() };
    let mut rg = group(&rf.keys, &mask);
    let mut cg = group(&cf.keys, &mask);
    let order = |g: &mut Grouped, s: i64| {
        let mut ix: Vec<usize> = (0..g.keys.len()).collect();
        ix.sort_by(|a, b| {
            let c = cmp_keys(&g.keys[*a], &g.keys[*b]);
            if s < 0 {
                c.reverse()
            } else {
                c
            }
        });
        g.keys = ix.iter().map(|i| g.keys[*i].clone()).collect();
        g.members = ix.iter().map(|i| g.members[*i].clone()).collect();
    };
    order(&mut rg, rsort);
    order(&mut cg, csort);
    let all: Vec<usize> = (0..n).filter(|i| mask[*i]).collect();
    let all_vals = vals(&all);
    let (rw, cw) = (rk.cols, ck.cols);
    let ncols = rw + cg.keys.len() + (cdepth != 0) as usize;
    let mut out: Vec<Vec<Value>> = Vec::new();
    // column header rows
    for level in 0..cw {
        let mut row = vec![Value::Empty; rw];
        if level == cw - 1 {
            if let Some(h) = &rf.headers {
                row = h.clone();
            }
        }
        for k in &cg.keys {
            row.push(k[level].clone());
        }
        if cdepth != 0 {
            row.push(if level == 0 {
                Value::text("Total")
            } else {
                Value::Empty
            });
        }
        out.push(row);
    }
    let member_set =
        |m: &[usize]| -> std::collections::HashSet<usize> { m.iter().copied().collect() };
    let col_sets: Vec<_> = cg.members.iter().map(|m| member_set(m)).collect();
    let cell = |ctx: &mut Ctx, rows: &[usize], col: Option<usize>, row_total: &[usize]| -> Value {
        let idx: Vec<usize> = match col {
            Some(c) => rows
                .iter()
                .copied()
                .filter(|i| col_sets[c].contains(i))
                .collect(),
            None => rows.to_vec(),
        };
        if idx.is_empty() {
            return Value::Empty;
        }
        // relative_to: 0 column totals, 1 row totals, 2 grand total, 3 parent column, 4 parent row
        let base: Vec<Value> = match (rel, col) {
            (1 | 4, _) => vals(row_total),
            (0 | 3, Some(c)) => vals(&cg.members[c]),
            _ => all_vals.clone(),
        };
        agg(ctx, &f, vals(&idx), &base)
    };
    let mut body = Vec::new();
    for (k, m) in rg.keys.iter().zip(&rg.members) {
        let mut row = k.clone();
        for c in 0..cg.keys.len() {
            row.push(cell(ctx, m, Some(c), m));
        }
        if cdepth != 0 {
            row.push(cell(ctx, m, None, m));
        }
        body.push(row);
    }
    let total_row = if rdepth != 0 {
        let mut row = total_label(rw, "Total");
        for c in 0..cg.keys.len() {
            row.push(cell(ctx, &all, Some(c), &all));
        }
        if cdepth != 0 {
            row.push(cell(ctx, &all, None, &all));
        }
        Some(row)
    } else {
        None
    };
    if rdepth < 0 {
        out.extend(total_row.clone());
    }
    out.extend(body);
    if rdepth > 0 {
        out.extend(total_row);
    }
    Value::array(Array::new(
        out.len(),
        ncols,
        out.into_iter().flatten().collect(),
    ))
}

fn trimrange(ctx: &mut Ctx, args: &[Expr]) -> Value {
    let Value::Ref(r) = arg_raw(ctx, args, 0) else {
        return err(ErrorKind::Value);
    };
    let tr = tryv!(arg_num(ctx, args, 1, 3.0)) as i64;
    let tc = tryv!(arg_num(ctx, args, 2, 3.0)) as i64;
    if !(0..=3).contains(&tr) || !(0..=3).contains(&tc) {
        return err(ErrorKind::Value);
    }
    let sheet = &ctx.wb.sheets[r.sheet];
    // bound the scan by the used area so whole-column refs stay cheap
    let Some(used) = sheet.used_rect() else {
        return err(ErrorKind::Ref);
    };
    let mut rect = r.rect;
    let occupied: Vec<crate::addr::CellRef> =
        sheet
            .cells
            .iter()
            .filter(|(c, cell)| rect.contains(**c) && !matches!(cell.value, Value::Empty))
            .map(|(c, _)| *c)
            .chain(sheet.spill_owner.keys().copied().filter(|c| {
                rect.contains(*c) && !matches!(ctx.wb.value_at(r.sheet, *c), Value::Empty)
            }))
            .collect();
    let _ = used;
    if occupied.is_empty() {
        return err(ErrorKind::Ref);
    }
    let (mut r0, mut r1, mut c0, mut c1) = (u32::MAX, 0, u32::MAX, 0);
    for c in &occupied {
        r0 = r0.min(c.row);
        r1 = r1.max(c.row);
        c0 = c0.min(c.col);
        c1 = c1.max(c.col);
    }
    if tr & 1 != 0 {
        rect.r0 = r0;
    }
    if tr & 2 != 0 {
        rect.r1 = r1;
    }
    if tc & 1 != 0 {
        rect.c0 = c0;
    }
    if tc & 2 != 0 {
        rect.c1 = c1;
    }
    Value::Ref(RangeRef {
        sheet: r.sheet,
        rect,
    })
}

pub fn call(ctx: &mut Ctx, name: &str, args: &[Expr]) -> Option<Value> {
    Some(match name {
        "GROUPBY" => groupby(ctx, args),
        "PIVOTBY" => pivotby(ctx, args),
        "PERCENTOF" => {
            let part: f64 = tryv!(numbers(ctx, &args[..1.min(args.len())])).iter().sum();
            let whole: f64 = tryv!(numbers(ctx, &args[1.min(args.len())..2.min(args.len())]))
                .iter()
                .sum();
            if whole == 0.0 {
                err(ErrorKind::Div0)
            } else {
                Value::Number(part / whole)
            }
        }
        "TRIMRANGE" => trimrange(ctx, args),
        "_TRO_ALL" | "_TRO_LEADING" | "_TRO_TRAILING" => {
            let mode = match name {
                "_TRO_ALL" => 3.0,
                "_TRO_LEADING" => 1.0,
                _ => 2.0,
            };
            let full = [
                args.first().cloned().unwrap_or(Expr::Missing),
                Expr::Number(mode),
                Expr::Number(mode),
            ];
            trimrange(ctx, &full)
        }
        _ => return None,
    })
}
