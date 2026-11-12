//! Math & statistics.

use super::{
    arg, arg_num, arg_raw, arr, criteria_mask, err, lift, lifted, numbers, present, tryv, Criterion,
};
use crate::eval::{num, Ctx};
use crate::parser::Expr;
use crate::value::{Array, ErrorKind, Value};

pub const NAMES: &[&str] = &[
    "SUM",
    "SUMPRODUCT",
    "SUMIF",
    "SUMIFS",
    "SUMSQ",
    "PRODUCT",
    "ABS",
    "ROUND",
    "ROUNDUP",
    "ROUNDDOWN",
    "INT",
    "TRUNC",
    "MOD",
    "POWER",
    "SQRT",
    "EXP",
    "LN",
    "LOG",
    "LOG10",
    "PI",
    "SIGN",
    "CEILING",
    "CEILING.MATH",
    "FLOOR",
    "FLOOR.MATH",
    "MROUND",
    "EVEN",
    "ODD",
    "FACT",
    "GCD",
    "LCM",
    "QUOTIENT",
    "RAND",
    "RANDBETWEEN",
    "RANDARRAY",
    "MIN",
    "MAX",
    "MINIFS",
    "MAXIFS",
    "MINA",
    "MAXA",
    "AVERAGE",
    "AVERAGEA",
    "AVERAGEIF",
    "AVERAGEIFS",
    "COUNT",
    "COUNTA",
    "COUNTBLANK",
    "COUNTIF",
    "COUNTIFS",
    "MEDIAN",
    "MODE",
    "MODE.SNGL",
    "STDEV",
    "STDEV.S",
    "STDEV.P",
    "STDEVP",
    "VAR",
    "VAR.S",
    "VAR.P",
    "VARP",
    "LARGE",
    "SMALL",
    "RANK",
    "RANK.EQ",
    "PERCENTILE",
    "PERCENTILE.INC",
    "QUARTILE",
    "QUARTILE.INC",
    "SUBTOTAL",
    "AGGREGATE",
    "SIN",
    "COS",
    "TAN",
    "ASIN",
    "ACOS",
    "ATAN",
    "ATAN2",
    "DEGREES",
    "RADIANS",
    "COMBIN",
    "PERMUT",
    "NORM.DIST",
    "NORM.INV",
    "CORREL",
    "SLOPE",
    "INTERCEPT",
];

fn round_to(x: f64, digits: f64, mode: i8) -> f64 {
    let f = 10f64.powi(digits as i32);
    let y = x * f;
    // nudge to defeat binary representation (2.675 → 2.68 like Excel)
    let y = y + y.signum() * 1e-9 * y.abs().max(1.0) * f64::EPSILON * 1e6;
    let r = match mode {
        0 => y.abs().round() * y.signum(),
        1 => y.abs().ceil() * y.signum(),
        _ => y.abs().floor() * y.signum(),
    };
    r / f
}

fn stats(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let ss = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>();
    (mean, ss)
}

fn percentile(mut xs: Vec<f64>, k: f64) -> Value {
    if xs.is_empty() || !(0.0..=1.0).contains(&k) {
        return err(ErrorKind::Num);
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pos = k * (xs.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    Value::Number(xs[lo] + (xs[hi] - xs[lo]) * (pos - lo as f64))
}

fn gcd(a: f64, b: f64) -> f64 {
    let (mut a, mut b) = (a.trunc().abs(), b.trunc().abs());
    while b > 0.0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

/// Normal CDF via erf (Abramowitz–Stegun 7.1.26 refined).
fn norm_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf(z / std::f64::consts::SQRT_2))
}

fn erf(x: f64) -> f64 {
    // W. J. Cody rational approximation via erfc for accuracy ~1e-12
    let t = 1.0 / (1.0 + 0.5 * x.abs());
    let y = 1.0
        - t * (-x * x - 1.26551223
            + t * (1.00002368
                + t * (0.37409196
                    + t * (0.09678418
                        + t * (-0.18628806
                            + t * (0.27886807
                                + t * (-1.13520398
                                    + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277)))))))))
            .exp();
    if x >= 0.0 {
        y
    } else {
        -y
    }
}

/// Inverse normal CDF (Acklam's algorithm).
fn norm_inv(p: f64) -> f64 {
    let a = [
        -3.969683028665376e1,
        2.209460984245205e2,
        -2.759285104469687e2,
        1.383_577_518_672_69e2,
        -3.066479806614716e1,
        2.506628277459239,
    ];
    let b = [
        -5.447609879822406e1,
        1.615858368580409e2,
        -1.556989798598866e2,
        6.680131188771972e1,
        -1.328068155288572e1,
    ];
    let c = [
        -7.784894002430293e-3,
        -3.223964580411365e-1,
        -2.400758277161838,
        -2.549732539343734,
        4.374664141464968,
        2.938163982698783,
    ];
    let d = [
        7.784695709041462e-3,
        3.224671290700398e-1,
        2.445134137142996,
        3.754408661907416,
    ];
    let pl = 0.02425;
    if p < pl {
        let q = (-2.0 * p.ln()).sqrt();
        (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    } else if p <= 1.0 - pl {
        let q = p - 0.5;
        let r = q * q;
        (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q
            / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0)
    } else {
        -norm_inv(1.0 - p)
    }
}

fn ifs_agg(
    ctx: &mut Ctx,
    args: &[Expr],
    value_idx: Option<usize>,
    pairs_from: usize,
) -> Result<Vec<f64>, ErrorKind> {
    let mut ranges = Vec::new();
    let mut crits = Vec::new();
    let mut i = pairs_from;
    while i + 1 < args.len() + 1 && i < args.len() {
        let r = arr(ctx, args, i);
        let c = arg(ctx, args, i + 1);
        ranges.push(r);
        crits.push(Criterion::parse(&c));
        i += 2;
    }
    let mask = criteria_mask(&ranges, &crits)?;
    let values = match value_idx {
        Some(k) => arr(ctx, args, k),
        None => return Ok(mask.iter().filter(|m| **m).map(|_| 1.0).collect()),
    };
    if values.data.len() != mask.len() {
        return Err(ErrorKind::Value);
    }
    Ok(values
        .data
        .iter()
        .zip(&mask)
        .filter(|(_, m)| **m)
        .filter_map(|(v, _)| {
            if let Value::Number(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .collect())
}

fn subtotal(code: i64, xs: Vec<f64>, counta: usize) -> Value {
    match code % 100 {
        1 => {
            if xs.is_empty() {
                err(ErrorKind::Div0)
            } else {
                Value::Number(xs.iter().sum::<f64>() / xs.len() as f64)
            }
        }
        2 => Value::Number(xs.len() as f64),
        3 => Value::Number(counta as f64),
        4 => Value::Number(xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max).max(
            if xs.is_empty() {
                0.0
            } else {
                f64::NEG_INFINITY
            },
        )),
        5 => Value::Number(if xs.is_empty() {
            0.0
        } else {
            xs.iter().cloned().fold(f64::INFINITY, f64::min)
        }),
        6 => Value::Number(xs.iter().product()),
        7 | 8 | 10 | 11 => {
            let n = xs.len();
            let sample = code % 100 == 7 || code % 100 == 10;
            if n < 1 + sample as usize {
                return err(ErrorKind::Div0);
            }
            let (_, ss) = stats(&xs);
            let var = ss / (n - sample as usize) as f64;
            Value::Number(if code % 100 <= 8 { var.sqrt() } else { var })
        }
        9 => Value::Number(xs.iter().sum()),
        _ => err(ErrorKind::Value),
    }
}

pub fn call(ctx: &mut Ctx, name: &str, args: &[Expr]) -> Option<Value> {
    let scalar1 = |ctx: &mut Ctx, f: fn(f64) -> Option<f64>, e: ErrorKind| {
        lifted(ctx, args, move |v| match num(&v[0]) {
            Ok(x) => f(x)
                .filter(|r| r.is_finite())
                .map(Value::Number)
                .unwrap_or(Value::Error(e)),
            Err(er) => Value::Error(er),
        })
    };
    let scalar2 = |ctx: &mut Ctx, f: fn(f64, f64) -> Option<f64>, e: ErrorKind, default: f64| {
        let a = arg(ctx, args, 0);
        let b = if present(args, 1) {
            arg(ctx, args, 1)
        } else {
            Value::Number(default)
        };
        lift(vec![a, b], move |v| match (num(&v[0]), num(&v[1])) {
            (Ok(x), Ok(y)) => f(x, y)
                .filter(|r| r.is_finite())
                .map(Value::Number)
                .unwrap_or(Value::Error(e)),
            (Err(er), _) | (_, Err(er)) => Value::Error(er),
        })
    };
    Some(match name {
        "SUM" => Value::Number(tryv!(numbers(ctx, args)).iter().sum()),
        "SUMSQ" => Value::Number(tryv!(numbers(ctx, args)).iter().map(|x| x * x).sum()),
        "PRODUCT" => Value::Number(tryv!(numbers(ctx, args)).iter().product()),
        "SUMPRODUCT" => {
            let arrays: Vec<_> = (0..args.len()).map(|i| arr(ctx, args, i)).collect();
            let Some(first) = arrays.first() else {
                return Some(err(ErrorKind::Value));
            };
            if arrays
                .iter()
                .any(|a| a.rows != first.rows || a.cols != first.cols)
            {
                return Some(err(ErrorKind::Value));
            }
            let mut total = 0.0;
            for i in 0..first.data.len() {
                let mut p = 1.0;
                for a in &arrays {
                    p *= match &a.data[i] {
                        Value::Number(n) => *n,
                        Value::Bool(b) => *b as i32 as f64,
                        Value::Error(e) => return Some(err(*e)),
                        _ => 0.0,
                    };
                }
                total += p;
            }
            Value::Number(total)
        }
        "SUMIF" => {
            let range = arr(ctx, args, 0);
            let crit = Criterion::parse(&arg(ctx, args, 1));
            let values = if present(args, 2) {
                arr(ctx, args, 2)
            } else {
                range.clone()
            };
            let mut s = 0.0;
            for (i, v) in range.data.iter().enumerate() {
                if crit.matches(v) {
                    if let Some(Value::Number(n)) = values.data.get(i) {
                        s += n;
                    }
                }
            }
            Value::Number(s)
        }
        "SUMIFS" => Value::Number(tryv!(ifs_agg(ctx, args, Some(0), 1)).iter().sum()),
        "AVERAGEIFS" => {
            let xs = tryv!(ifs_agg(ctx, args, Some(0), 1));
            if xs.is_empty() {
                err(ErrorKind::Div0)
            } else {
                Value::Number(xs.iter().sum::<f64>() / xs.len() as f64)
            }
        }
        "AVERAGEIF" => {
            let range = arr(ctx, args, 0);
            let crit = Criterion::parse(&arg(ctx, args, 1));
            let values = if present(args, 2) {
                arr(ctx, args, 2)
            } else {
                range.clone()
            };
            let xs: Vec<f64> = range
                .data
                .iter()
                .enumerate()
                .filter(|(_, v)| crit.matches(v))
                .filter_map(|(i, _)| {
                    if let Some(Value::Number(n)) = values.data.get(i) {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .collect();
            if xs.is_empty() {
                err(ErrorKind::Div0)
            } else {
                Value::Number(xs.iter().sum::<f64>() / xs.len() as f64)
            }
        }
        "COUNTIFS" => Value::Number(tryv!(ifs_agg(ctx, args, None, 0)).len() as f64),
        "COUNTIF" => {
            let range = arr(ctx, args, 0);
            let crit = Criterion::parse(&arg(ctx, args, 1));
            Value::Number(range.data.iter().filter(|v| crit.matches(v)).count() as f64)
        }
        "MAXIFS" | "MINIFS" => {
            let xs = tryv!(ifs_agg(ctx, args, Some(0), 1));
            Value::Number(if xs.is_empty() {
                0.0
            } else if name == "MAXIFS" {
                xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            } else {
                xs.iter().cloned().fold(f64::INFINITY, f64::min)
            })
        }
        "ABS" => scalar1(ctx, |x| Some(x.abs()), ErrorKind::Num),
        "INT" => scalar1(ctx, |x| Some(x.floor()), ErrorKind::Num),
        "SQRT" => scalar1(ctx, |x| (x >= 0.0).then(|| x.sqrt()), ErrorKind::Num),
        "EXP" => scalar1(ctx, |x| Some(x.exp()), ErrorKind::Num),
        "LN" => scalar1(ctx, |x| (x > 0.0).then(|| x.ln()), ErrorKind::Num),
        "LOG10" => scalar1(ctx, |x| (x > 0.0).then(|| x.log10()), ErrorKind::Num),
        "SIGN" => scalar1(
            ctx,
            |x| {
                Some(if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                })
            },
            ErrorKind::Num,
        ),
        "SIN" => scalar1(ctx, |x| Some(x.sin()), ErrorKind::Num),
        "COS" => scalar1(ctx, |x| Some(x.cos()), ErrorKind::Num),
        "TAN" => scalar1(ctx, |x| Some(x.tan()), ErrorKind::Num),
        "ASIN" => scalar1(
            ctx,
            |x| (-1.0..=1.0).contains(&x).then(|| x.asin()),
            ErrorKind::Num,
        ),
        "ACOS" => scalar1(
            ctx,
            |x| (-1.0..=1.0).contains(&x).then(|| x.acos()),
            ErrorKind::Num,
        ),
        "ATAN" => scalar1(ctx, |x| Some(x.atan()), ErrorKind::Num),
        "DEGREES" => scalar1(ctx, |x| Some(x.to_degrees()), ErrorKind::Num),
        "RADIANS" => scalar1(ctx, |x| Some(x.to_radians()), ErrorKind::Num),
        "EVEN" => scalar1(
            ctx,
            |x| {
                let c = (x.abs() / 2.0).ceil() * 2.0;
                Some(c * x.signum())
            },
            ErrorKind::Num,
        ),
        "ODD" => scalar1(
            ctx,
            |x| {
                let c = ((x.abs() + 1.0) / 2.0).ceil() * 2.0 - 1.0;
                Some(c.max(1.0) * if x < 0.0 { -1.0 } else { 1.0 })
            },
            ErrorKind::Num,
        ),
        "FACT" => scalar1(
            ctx,
            |x| {
                (0.0..171.0)
                    .contains(&x)
                    .then(|| (1..=x.trunc() as u64).map(|i| i as f64).product())
            },
            ErrorKind::Num,
        ),
        "PI" => Value::Number(std::f64::consts::PI),
        "ATAN2" => scalar2(
            ctx,
            |x, y| (x != 0.0 || y != 0.0).then(|| y.atan2(x)),
            ErrorKind::Div0,
            0.0,
        ),
        "ROUND" => scalar2(ctx, |x, d| Some(round_to(x, d, 0)), ErrorKind::Num, 0.0),
        "ROUNDUP" => scalar2(ctx, |x, d| Some(round_to(x, d, 1)), ErrorKind::Num, 0.0),
        "ROUNDDOWN" => scalar2(ctx, |x, d| Some(round_to(x, d, 2)), ErrorKind::Num, 0.0),
        "TRUNC" => scalar2(ctx, |x, d| Some(round_to(x, d, 2)), ErrorKind::Num, 0.0),
        "MOD" => scalar2(
            ctx,
            |x, y| (y != 0.0).then(|| x - y * (x / y).floor()),
            ErrorKind::Div0,
            1.0,
        ),
        "QUOTIENT" => scalar2(
            ctx,
            |x, y| (y != 0.0).then(|| (x / y).trunc()),
            ErrorKind::Div0,
            1.0,
        ),
        "POWER" => scalar2(ctx, |x, y| Some(x.powf(y)), ErrorKind::Num, 1.0),
        "LOG" => scalar2(
            ctx,
            |x, b| (x > 0.0 && b > 0.0 && b != 1.0).then(|| x.ln() / b.ln()),
            ErrorKind::Num,
            10.0,
        ),
        "MROUND" => scalar2(
            ctx,
            |x, m| {
                if m == 0.0 {
                    Some(0.0)
                } else if x * m < 0.0 {
                    None
                } else {
                    Some((x / m).round() * m)
                }
            },
            ErrorKind::Num,
            1.0,
        ),
        "CEILING" | "CEILING.MATH" => scalar2(
            ctx,
            |x, s| {
                if s == 0.0 {
                    Some(0.0)
                } else {
                    Some((x / s.abs()).ceil() * s.abs())
                }
            },
            ErrorKind::Num,
            1.0,
        ),
        "FLOOR" | "FLOOR.MATH" => scalar2(
            ctx,
            |x, s| {
                if s == 0.0 {
                    Some(0.0)
                } else {
                    Some((x / s.abs()).floor() * s.abs())
                }
            },
            ErrorKind::Num,
            1.0,
        ),
        "COMBIN" => scalar2(
            ctx,
            |n, k| {
                let (n, k) = (n.trunc(), k.trunc());
                (n >= k && k >= 0.0).then(|| {
                    (0..k as u64)
                        .fold(1.0, |acc, i| acc * (n - i as f64) / (i as f64 + 1.0))
                        .round()
                })
            },
            ErrorKind::Num,
            0.0,
        ),
        "PERMUT" => scalar2(
            ctx,
            |n, k| {
                let (n, k) = (n.trunc(), k.trunc());
                (n >= k && k >= 0.0).then(|| (0..k as u64).fold(1.0, |acc, i| acc * (n - i as f64)))
            },
            ErrorKind::Num,
            0.0,
        ),
        "GCD" => Value::Number(tryv!(numbers(ctx, args)).into_iter().fold(0.0, gcd)),
        "LCM" => Value::Number(tryv!(numbers(ctx, args)).into_iter().fold(1.0, |a, b| {
            if a == 0.0 || b == 0.0 {
                0.0
            } else {
                (a * b.trunc() / gcd(a, b)).abs()
            }
        })),
        "RAND" => Value::Number(ctx.random()),
        "RANDBETWEEN" => {
            let lo = tryv!(arg_num(ctx, args, 0, 0.0)).ceil();
            let hi = tryv!(arg_num(ctx, args, 1, 0.0)).floor();
            if lo > hi {
                err(ErrorKind::Num)
            } else {
                Value::Number(lo + (ctx.random() * (hi - lo + 1.0)).floor())
            }
        }
        "RANDARRAY" => {
            let rows = tryv!(arg_num(ctx, args, 0, 1.0)) as usize;
            let cols = tryv!(arg_num(ctx, args, 1, 1.0)) as usize;
            let lo = tryv!(arg_num(ctx, args, 2, 0.0));
            let hi = tryv!(arg_num(ctx, args, 3, 1.0));
            let whole = present(args, 4) && tryv!(crate::eval::truthy(&arg(ctx, args, 4)));
            if rows == 0 || cols == 0 || lo > hi || rows * cols > 10_000_000 {
                return Some(err(ErrorKind::Value));
            }
            let data = (0..rows * cols)
                .map(|_| {
                    let r = ctx.random();
                    Value::Number(if whole {
                        lo.ceil() + (r * (hi.floor() - lo.ceil() + 1.0)).floor()
                    } else {
                        lo + r * (hi - lo)
                    })
                })
                .collect();
            Value::array(Array::new(rows, cols, data))
        }
        "MIN" | "MAX" => {
            let xs = tryv!(numbers(ctx, args));
            Value::Number(if xs.is_empty() {
                0.0
            } else if name == "MAX" {
                xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            } else {
                xs.iter().cloned().fold(f64::INFINITY, f64::min)
            })
        }
        "MINA" | "MAXA" | "AVERAGEA" => {
            let mut xs = Vec::new();
            for i in 0..args.len() {
                for v in &arr(ctx, args, i).data {
                    match v {
                        Value::Number(n) => xs.push(*n),
                        Value::Bool(b) => xs.push(*b as i32 as f64),
                        Value::Text(_) => xs.push(0.0),
                        Value::Error(e) => return Some(err(*e)),
                        _ => {}
                    }
                }
            }
            match name {
                "AVERAGEA" => {
                    if xs.is_empty() {
                        err(ErrorKind::Div0)
                    } else {
                        Value::Number(xs.iter().sum::<f64>() / xs.len() as f64)
                    }
                }
                "MAXA" => Value::Number(if xs.is_empty() {
                    0.0
                } else {
                    xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                }),
                _ => Value::Number(if xs.is_empty() {
                    0.0
                } else {
                    xs.iter().cloned().fold(f64::INFINITY, f64::min)
                }),
            }
        }
        "AVERAGE" => {
            let xs = tryv!(numbers(ctx, args));
            if xs.is_empty() {
                err(ErrorKind::Div0)
            } else {
                Value::Number(xs.iter().sum::<f64>() / xs.len() as f64)
            }
        }
        "COUNT" => {
            let mut n = 0;
            for i in 0..args.len() {
                let raw = arg_raw(ctx, args, i);
                let direct = !matches!(raw, Value::Ref(_) | Value::Array(_));
                for v in &ctx.array(raw).data {
                    if matches!(v, Value::Number(_))
                        || (direct && num(v).is_ok() && !matches!(v, Value::Empty))
                    {
                        n += 1;
                    }
                }
            }
            Value::Number(n as f64)
        }
        "COUNTA" => Value::Number(
            (0..args.len())
                .map(|i| {
                    arr(ctx, args, i)
                        .data
                        .iter()
                        .filter(|v| !matches!(v, Value::Empty))
                        .count()
                })
                .sum::<usize>() as f64,
        ),
        "COUNTBLANK" => Value::Number(
            arr(ctx, args, 0)
                .data
                .iter()
                .filter(|v| {
                    matches!(v, Value::Empty) || matches!(v, Value::Text(t) if t.is_empty())
                })
                .count() as f64,
        ),
        "MEDIAN" => percentile(tryv!(numbers(ctx, args)), 0.5),
        "MODE" | "MODE.SNGL" => {
            let xs = tryv!(numbers(ctx, args));
            let mut best: Option<(f64, usize)> = None;
            for x in &xs {
                let c = xs.iter().filter(|y| *y == x).count();
                if c > 1 && best.map_or(true, |(_, bc)| c > bc) {
                    best = Some((*x, c));
                }
            }
            best.map(|b| Value::Number(b.0))
                .unwrap_or(err(ErrorKind::NA))
        }
        "STDEV" | "STDEV.S" | "STDEV.P" | "STDEVP" | "VAR" | "VAR.S" | "VAR.P" | "VARP" => {
            let xs = tryv!(numbers(ctx, args));
            let pop = name.ends_with('P');
            let n = xs.len();
            if n < 1 + (!pop) as usize {
                return Some(err(ErrorKind::Div0));
            }
            let (_, ss) = stats(&xs);
            let var = ss / (n - (!pop) as usize) as f64;
            Value::Number(if name.starts_with("STDEV") {
                var.sqrt()
            } else {
                var
            })
        }
        "LARGE" | "SMALL" => {
            let mut xs = tryv!(numbers(ctx, &args[..1.min(args.len())]));
            let k = arg(ctx, args, 1);
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if name == "LARGE" {
                xs.reverse();
            }
            lift(vec![k], move |v| match num(&v[0]) {
                Ok(k) if k >= 1.0 && (k as usize) <= xs.len() => Value::Number(xs[k as usize - 1]),
                Ok(_) => err(ErrorKind::Num),
                Err(e) => err(e),
            })
        }
        "RANK" | "RANK.EQ" => {
            let x = tryv!(arg_num(ctx, args, 0, 0.0));
            let xs = tryv!(numbers(ctx, &args[1..2.min(args.len())]));
            let asc = tryv!(arg_num(ctx, args, 2, 0.0)) != 0.0;
            if !xs.contains(&x) {
                return Some(err(ErrorKind::NA));
            }
            Value::Number(
                1.0 + xs
                    .iter()
                    .filter(|y| if asc { **y < x } else { **y > x })
                    .count() as f64,
            )
        }
        "PERCENTILE" | "PERCENTILE.INC" => {
            let xs = tryv!(numbers(ctx, &args[..1.min(args.len())]));
            percentile(xs, tryv!(arg_num(ctx, args, 1, 0.0)))
        }
        "QUARTILE" | "QUARTILE.INC" => {
            let xs = tryv!(numbers(ctx, &args[..1.min(args.len())]));
            let q = tryv!(arg_num(ctx, args, 1, 0.0)).trunc();
            if !(0.0..=4.0).contains(&q) {
                err(ErrorKind::Num)
            } else {
                percentile(xs, q / 4.0)
            }
        }
        "SUBTOTAL" | "AGGREGATE" => {
            let code = tryv!(arg_num(ctx, args, 0, 9.0)) as i64;
            let skip = if name == "AGGREGATE" { 2 } else { 1 };
            let rest = &args[skip.min(args.len())..];
            let xs = tryv!(numbers(ctx, rest));
            let counta = (0..rest.len())
                .map(|i| {
                    arr(ctx, rest, i)
                        .data
                        .iter()
                        .filter(|v| !matches!(v, Value::Empty))
                        .count()
                })
                .sum();
            subtotal(code, xs, counta)
        }
        "NORM.DIST" => {
            let x = tryv!(arg_num(ctx, args, 0, 0.0));
            let m = tryv!(arg_num(ctx, args, 1, 0.0));
            let sd = tryv!(arg_num(ctx, args, 2, 1.0));
            let cum = tryv!(crate::eval::truthy(&arg(ctx, args, 3)));
            if sd <= 0.0 {
                return Some(err(ErrorKind::Num));
            }
            let z = (x - m) / sd;
            Value::Number(if cum {
                norm_cdf(z)
            } else {
                (-z * z / 2.0).exp() / (sd * (2.0 * std::f64::consts::PI).sqrt())
            })
        }
        "NORM.INV" => {
            let p = tryv!(arg_num(ctx, args, 0, 0.5));
            let m = tryv!(arg_num(ctx, args, 1, 0.0));
            let sd = tryv!(arg_num(ctx, args, 2, 1.0));
            if p <= 0.0 || p >= 1.0 || sd <= 0.0 {
                err(ErrorKind::Num)
            } else {
                Value::Number(m + sd * norm_inv(p))
            }
        }
        "CORREL" | "SLOPE" | "INTERCEPT" => {
            let (ya, xa) = (arr(ctx, args, 0), arr(ctx, args, 1));
            if ya.data.len() != xa.data.len() {
                return Some(err(ErrorKind::NA));
            }
            let pairs: Vec<(f64, f64)> = ya
                .data
                .iter()
                .zip(&xa.data)
                .filter_map(|(y, x)| match (y, x) {
                    (Value::Number(y), Value::Number(x)) => Some((*x, *y)),
                    _ => None,
                })
                .collect();
            if pairs.len() < 2 {
                return Some(err(ErrorKind::Div0));
            }
            let n = pairs.len() as f64;
            let mx = pairs.iter().map(|p| p.0).sum::<f64>() / n;
            let my = pairs.iter().map(|p| p.1).sum::<f64>() / n;
            let sxy: f64 = pairs.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
            let sxx: f64 = pairs.iter().map(|p| (p.0 - mx).powi(2)).sum();
            let syy: f64 = pairs.iter().map(|p| (p.1 - my).powi(2)).sum();
            if sxx == 0.0 || (name == "CORREL" && syy == 0.0) {
                return Some(err(ErrorKind::Div0));
            }
            let slope = sxy / sxx;
            Value::Number(match name {
                "CORREL" => sxy / (sxx * syy).sqrt(),
                "SLOPE" => slope,
                _ => my - slope * mx,
            })
        }
        _ => return None,
    })
}
