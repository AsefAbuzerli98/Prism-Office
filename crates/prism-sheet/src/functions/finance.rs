//! Financial functions (Excel conventions: cash paid out is negative).

use super::{arg, arg_num, arr, err, tryv};
use crate::eval::{num, Ctx};
use crate::parser::Expr;
use crate::value::{ErrorKind, Value};

pub const NAMES: &[&str] = &[
    "PMT",
    "IPMT",
    "PPMT",
    "FV",
    "PV",
    "NPER",
    "RATE",
    "NPV",
    "XNPV",
    "IRR",
    "XIRR",
    "MIRR",
    "SLN",
    "SYD",
    "DB",
    "DDB",
    "EFFECT",
    "NOMINAL",
    "CUMIPMT",
    "CUMPRINC",
    "FVSCHEDULE",
    "RRI",
    "PDURATION",
];

fn fin(x: f64) -> Value {
    if x.is_finite() {
        Value::Number(x)
    } else {
        err(ErrorKind::Num)
    }
}

/// Payment per period.
pub fn pmt(rate: f64, nper: f64, pv: f64, fv: f64, due: bool) -> f64 {
    if rate == 0.0 {
        return -(pv + fv) / nper;
    }
    let t = (1.0 + rate).powf(nper);
    -(rate * (fv + pv * t)) / ((1.0 + rate * due as i32 as f64) * (t - 1.0))
}

pub fn fv(rate: f64, nper: f64, pmt: f64, pv: f64, due: bool) -> f64 {
    if rate == 0.0 {
        return -(pv + pmt * nper);
    }
    let t = (1.0 + rate).powf(nper);
    -(pv * t + pmt * (1.0 + rate * due as i32 as f64) * (t - 1.0) / rate)
}

pub fn pv(rate: f64, nper: f64, pmt: f64, fv: f64, due: bool) -> f64 {
    if rate == 0.0 {
        return -(fv + pmt * nper);
    }
    let t = (1.0 + rate).powf(nper);
    -(fv + pmt * (1.0 + rate * due as i32 as f64) * (t - 1.0) / rate) / t
}

/// Interest part of payment `per` (1-based).
pub fn ipmt(rate: f64, per: f64, nper: f64, pv: f64, fv: f64, due: bool) -> f64 {
    let p = pmt(rate, nper, pv, fv, due);
    if per == 1.0 && due {
        return 0.0;
    }
    let balance = if due {
        self::fv(rate, per - 2.0, p, pv, true) - p
    } else {
        self::fv(rate, per - 1.0, p, pv, false)
    };
    balance * rate
}

fn nper(rate: f64, pmt: f64, pv: f64, fv: f64, due: bool) -> f64 {
    if rate == 0.0 {
        return -(pv + fv) / pmt;
    }
    let k = pmt * (1.0 + rate * due as i32 as f64) / rate;
    ((k - fv) / (k + pv)).ln() / (1.0 + rate).ln()
}

/// Newton iteration with a bisection-free fallback grid of starting guesses.
fn solve(f: impl Fn(f64) -> f64, guess: f64) -> Option<f64> {
    for g in [guess, 0.1, 0.01, 0.5, -0.5, 1.0, 0.001] {
        let mut x = g;
        for _ in 0..100 {
            let y = f(x);
            if !y.is_finite() {
                break;
            }
            let h = 1e-7 * x.abs().max(1e-4);
            let d = (f(x + h) - y) / h;
            if d == 0.0 || !d.is_finite() {
                break;
            }
            let nx = x - y / d;
            if (nx - x).abs() < 1e-10 {
                return (nx > -1.0).then_some(nx);
            }
            x = nx;
        }
    }
    None
}

fn npv(rate: f64, flows: &[f64]) -> f64 {
    flows
        .iter()
        .enumerate()
        .map(|(i, c)| c / (1.0 + rate).powi(i as i32 + 1))
        .sum()
}

fn xnpv(rate: f64, flows: &[f64], dates: &[f64]) -> f64 {
    let d0 = dates[0];
    flows
        .iter()
        .zip(dates)
        .map(|(c, d)| c / (1.0 + rate).powf((d - d0) / 365.0))
        .sum()
}

fn numbers_of(ctx: &mut Ctx, args: &[Expr], i: usize) -> Result<Vec<f64>, ErrorKind> {
    let a = arr(ctx, args, i);
    let mut out = Vec::new();
    for v in &a.data {
        match v {
            Value::Number(n) => out.push(*n),
            Value::Error(e) => return Err(*e),
            _ => {}
        }
    }
    Ok(out)
}

pub fn call(ctx: &mut Ctx, name: &str, args: &[Expr]) -> Option<Value> {
    let n = |ctx: &mut Ctx, i: usize, d: f64| arg_num(ctx, args, i, d);
    let due = |ctx: &mut Ctx, i: usize| -> Result<bool, ErrorKind> { Ok(n(ctx, i, 0.0)? != 0.0) };
    Some(match name {
        "PMT" => fin(pmt(
            tryv!(n(ctx, 0, 0.0)),
            tryv!(n(ctx, 1, 0.0)),
            tryv!(n(ctx, 2, 0.0)),
            tryv!(n(ctx, 3, 0.0)),
            tryv!(due(ctx, 4)),
        )),
        "FV" => fin(fv(
            tryv!(n(ctx, 0, 0.0)),
            tryv!(n(ctx, 1, 0.0)),
            tryv!(n(ctx, 2, 0.0)),
            tryv!(n(ctx, 3, 0.0)),
            tryv!(due(ctx, 4)),
        )),
        "PV" => fin(pv(
            tryv!(n(ctx, 0, 0.0)),
            tryv!(n(ctx, 1, 0.0)),
            tryv!(n(ctx, 2, 0.0)),
            tryv!(n(ctx, 3, 0.0)),
            tryv!(due(ctx, 4)),
        )),
        "NPER" => fin(nper(
            tryv!(n(ctx, 0, 0.0)),
            tryv!(n(ctx, 1, 0.0)),
            tryv!(n(ctx, 2, 0.0)),
            tryv!(n(ctx, 3, 0.0)),
            tryv!(due(ctx, 4)),
        )),
        "IPMT" | "PPMT" => {
            let (rate, per, np, p, f, d) = (
                tryv!(n(ctx, 0, 0.0)),
                tryv!(n(ctx, 1, 0.0)),
                tryv!(n(ctx, 2, 0.0)),
                tryv!(n(ctx, 3, 0.0)),
                tryv!(n(ctx, 4, 0.0)),
                tryv!(due(ctx, 5)),
            );
            if per < 1.0 || per > np {
                return Some(err(ErrorKind::Num));
            }
            let i = ipmt(rate, per, np, p, f, d);
            fin(if name == "IPMT" {
                i
            } else {
                pmt(rate, np, p, f, d) - i
            })
        }
        "CUMIPMT" | "CUMPRINC" => {
            let (rate, np, p, s, e, d) = (
                tryv!(n(ctx, 0, 0.0)),
                tryv!(n(ctx, 1, 0.0)),
                tryv!(n(ctx, 2, 0.0)),
                tryv!(n(ctx, 3, 0.0)),
                tryv!(n(ctx, 4, 0.0)),
                tryv!(due(ctx, 5)),
            );
            if rate <= 0.0 || np <= 0.0 || p <= 0.0 || s < 1.0 || e < s || e > np {
                return Some(err(ErrorKind::Num));
            }
            let payment = pmt(rate, np, p, 0.0, d);
            let mut total = 0.0;
            for per in s as i64..=e as i64 {
                let i = ipmt(rate, per as f64, np, p, 0.0, d);
                total += if name == "CUMIPMT" { i } else { payment - i };
            }
            fin(total)
        }
        "RATE" => {
            let (np, p, pval, f, d) = (
                tryv!(n(ctx, 0, 0.0)),
                tryv!(n(ctx, 1, 0.0)),
                tryv!(n(ctx, 2, 0.0)),
                tryv!(n(ctx, 3, 0.0)),
                tryv!(due(ctx, 4)),
            );
            let guess = tryv!(n(ctx, 5, 0.1));
            let f_of = |r: f64| {
                if r.abs() < 1e-12 {
                    pval + p * np + f
                } else {
                    pval * (1.0 + r).powf(np)
                        + p * (1.0 + r * d as i32 as f64) * ((1.0 + r).powf(np) - 1.0) / r
                        + f
                }
            };
            solve(f_of, guess)
                .map(Value::Number)
                .unwrap_or(err(ErrorKind::Num))
        }
        "NPV" => {
            let rate = tryv!(n(ctx, 0, 0.0));
            let mut flows = Vec::new();
            for i in 1..args.len() {
                flows.extend(tryv!(numbers_of(ctx, args, i)));
            }
            fin(npv(rate, &flows))
        }
        "IRR" => {
            let flows = tryv!(numbers_of(ctx, args, 0));
            let guess = tryv!(n(ctx, 1, 0.1));
            if !(flows.iter().any(|c| *c > 0.0) && flows.iter().any(|c| *c < 0.0)) {
                return Some(err(ErrorKind::Num));
            }
            solve(
                |r| {
                    flows
                        .iter()
                        .enumerate()
                        .map(|(i, c)| c / (1.0 + r).powi(i as i32))
                        .sum()
                },
                guess,
            )
            .map(Value::Number)
            .unwrap_or(err(ErrorKind::Num))
        }
        "MIRR" => {
            let flows = tryv!(numbers_of(ctx, args, 0));
            let fr = tryv!(n(ctx, 1, 0.0));
            let rr = tryv!(n(ctx, 2, 0.0));
            let k = flows.len() as f64;
            let pos: f64 = flows
                .iter()
                .enumerate()
                .filter(|(_, c)| **c > 0.0)
                .map(|(i, c)| c * (1.0 + rr).powf(k - 1.0 - i as f64))
                .sum();
            let neg: f64 = flows
                .iter()
                .enumerate()
                .filter(|(_, c)| **c < 0.0)
                .map(|(i, c)| c / (1.0 + fr).powi(i as i32))
                .sum();
            if pos == 0.0 || neg == 0.0 {
                return Some(err(ErrorKind::Div0));
            }
            fin((-pos / neg).powf(1.0 / (k - 1.0)) - 1.0)
        }
        "XNPV" | "XIRR" => {
            let (fi, di) = if name == "XNPV" { (1, 2) } else { (0, 1) };
            let flows = tryv!(numbers_of(ctx, args, fi));
            let dates = tryv!(numbers_of(ctx, args, di));
            if flows.len() != dates.len() || flows.is_empty() || dates.iter().any(|d| *d < dates[0])
            {
                return Some(err(ErrorKind::Num));
            }
            if name == "XNPV" {
                fin(xnpv(tryv!(n(ctx, 0, 0.0)), &flows, &dates))
            } else {
                let guess = tryv!(n(ctx, 2, 0.1));
                solve(|r| xnpv(r, &flows, &dates), guess)
                    .map(Value::Number)
                    .unwrap_or(err(ErrorKind::Num))
            }
        }
        "SLN" => {
            let (c, s, l) = (
                tryv!(n(ctx, 0, 0.0)),
                tryv!(n(ctx, 1, 0.0)),
                tryv!(n(ctx, 2, 0.0)),
            );
            if l == 0.0 {
                err(ErrorKind::Div0)
            } else {
                fin((c - s) / l)
            }
        }
        "SYD" => {
            let (c, s, l, p) = (
                tryv!(n(ctx, 0, 0.0)),
                tryv!(n(ctx, 1, 0.0)),
                tryv!(n(ctx, 2, 0.0)),
                tryv!(n(ctx, 3, 0.0)),
            );
            if l <= 0.0 || p < 1.0 || p > l {
                return Some(err(ErrorKind::Num));
            }
            fin((c - s) * (l - p + 1.0) * 2.0 / (l * (l + 1.0)))
        }
        "DDB" => {
            let (c, s, l, p) = (
                tryv!(n(ctx, 0, 0.0)),
                tryv!(n(ctx, 1, 0.0)),
                tryv!(n(ctx, 2, 0.0)),
                tryv!(n(ctx, 3, 0.0)),
            );
            let factor = tryv!(n(ctx, 4, 2.0));
            if l <= 0.0 || p < 1.0 || p > l || factor <= 0.0 {
                return Some(err(ErrorKind::Num));
            }
            let mut book = c;
            let mut dep = 0.0;
            for _ in 0..p as i64 {
                dep = (book * factor / l).min((book - s).max(0.0));
                book -= dep;
            }
            fin(dep)
        }
        "DB" => {
            let (c, s, l, p) = (
                tryv!(n(ctx, 0, 0.0)),
                tryv!(n(ctx, 1, 0.0)),
                tryv!(n(ctx, 2, 0.0)),
                tryv!(n(ctx, 3, 0.0)),
            );
            let month = tryv!(n(ctx, 4, 12.0));
            if c <= 0.0 || l <= 0.0 || p < 1.0 || p > l + 1.0 {
                return Some(err(ErrorKind::Num));
            }
            let rate = ((1.0 - (s / c).powf(1.0 / l)) * 1000.0).round() / 1000.0;
            let mut total = 0.0;
            let mut dep = 0.0;
            for k in 1..=p as i64 {
                dep = if k == 1 {
                    c * rate * month / 12.0
                } else if k as f64 == l + 1.0 {
                    (c - total) * rate * (12.0 - month) / 12.0
                } else {
                    (c - total) * rate
                };
                total += dep;
            }
            fin(dep)
        }
        "EFFECT" => {
            let (r, k) = (tryv!(n(ctx, 0, 0.0)), tryv!(n(ctx, 1, 0.0)).trunc());
            if r <= 0.0 || k < 1.0 {
                err(ErrorKind::Num)
            } else {
                fin((1.0 + r / k).powf(k) - 1.0)
            }
        }
        "NOMINAL" => {
            let (r, k) = (tryv!(n(ctx, 0, 0.0)), tryv!(n(ctx, 1, 0.0)).trunc());
            if r <= 0.0 || k < 1.0 {
                err(ErrorKind::Num)
            } else {
                fin(k * ((1.0 + r).powf(1.0 / k) - 1.0))
            }
        }
        "FVSCHEDULE" => {
            let p = tryv!(num(&arg(ctx, args, 0)));
            let rates = tryv!(numbers_of(ctx, args, 1));
            fin(rates.iter().fold(p, |acc, r| acc * (1.0 + r)))
        }
        "RRI" | "PDURATION" => {
            let (a, b, c) = (
                tryv!(n(ctx, 0, 0.0)),
                tryv!(n(ctx, 1, 0.0)),
                tryv!(n(ctx, 2, 0.0)),
            );
            if name == "RRI" {
                if a <= 0.0 || b == 0.0 {
                    err(ErrorKind::Num)
                } else {
                    fin((c / b).powf(1.0 / a) - 1.0)
                }
            } else if a <= 0.0 || b <= 0.0 || c <= 0.0 {
                err(ErrorKind::Num)
            } else {
                fin((c.ln() - b.ln()) / (1.0 + a).ln())
            }
        }
        _ => return None,
    })
}
