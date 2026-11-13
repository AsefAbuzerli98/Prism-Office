//! Excel number-format codes (`0.00`, `#,##0;[Red](#,##0)`, `yyyy-mm-dd`, `0%`, `0.0E+00`, `# ?/?` …).

use crate::functions::date::{hms, ymd};
use crate::value::{format_number, Value};

/// Formatted text plus an optional colour from `[Red]`-style tokens.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Formatted {
    pub text: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Lit(String),
    Digit(char),
    Point,
    Comma,
    Percent,
    Exp(bool),
    Slash,
    At,
    Date(String),
    AmPm(bool),
    Elapsed(char),
}

struct Section {
    toks: Vec<Tok>,
    color: Option<String>,
    cond: Option<(String, f64)>,
}

fn split_sections(code: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut q, mut br) = (0, false, false);
    let b = code.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => q = !q,
            b'\\' if !q => i += 1,
            b'[' if !q => br = true,
            b']' if !q => br = false,
            b';' if !q && !br => {
                out.push(&code[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&code[start..]);
    out
}

const COLORS: [&str; 8] = [
    "black", "blue", "cyan", "green", "magenta", "red", "white", "yellow",
];

fn tokenize(s: &str) -> Section {
    let chars: Vec<char> = s.chars().collect();
    let mut toks = Vec::new();
    let (mut color, mut cond) = (None, None);
    let mut i = 0;
    let lit = |toks: &mut Vec<Tok>, s: &str| {
        if let Some(Tok::Lit(prev)) = toks.last_mut() {
            prev.push_str(s);
        } else {
            toks.push(Tok::Lit(s.to_string()));
        }
    };
    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' => {
                let mut j = i + 1;
                let mut t = String::new();
                while j < chars.len() && chars[j] != '"' {
                    t.push(chars[j]);
                    j += 1;
                }
                lit(&mut toks, &t);
                i = j;
            }
            '\\' => {
                if let Some(n) = chars.get(i + 1) {
                    lit(&mut toks, &n.to_string());
                }
                i += 1;
            }
            '_' => {
                lit(&mut toks, " ");
                i += 1;
            }
            '*' => i += 1,
            '[' => {
                let end = chars[i..]
                    .iter()
                    .position(|c| *c == ']')
                    .map(|p| p + i)
                    .unwrap_or(chars.len());
                let inner: String = chars[i + 1..end].iter().collect();
                let low = inner.to_ascii_lowercase();
                if COLORS.contains(&low.as_str()) || low.starts_with("color") {
                    color = Some(low);
                } else if let Some(op) = ["<=", ">=", "<>", "<", ">", "="]
                    .iter()
                    .find(|o| inner.starts_with(**o))
                {
                    if let Ok(v) = inner[op.len()..].trim().parse() {
                        cond = Some((op.to_string(), v));
                    }
                } else if matches!(low.chars().next(), Some('h' | 'm' | 's'))
                    && low.chars().all(|c| c == low.chars().next().unwrap())
                {
                    toks.push(Tok::Elapsed(low.chars().next().unwrap()));
                } else if let Some(sym) = inner.strip_prefix('$') {
                    // [$€-407] currency/locale
                    lit(&mut toks, sym.split('-').next().unwrap_or(""));
                }
                i = end;
            }
            '0' | '#' | '?' => toks.push(Tok::Digit(c)),
            '.' => toks.push(Tok::Point),
            ',' => toks.push(Tok::Comma),
            '%' => toks.push(Tok::Percent),
            '/' => toks.push(Tok::Slash),
            '@' => toks.push(Tok::At),
            'E' | 'e' if matches!(chars.get(i + 1), Some('+' | '-')) => {
                toks.push(Tok::Exp(chars[i + 1] == '+'));
                i += 1;
            }
            'A' | 'a'
                if chars[i..]
                    .iter()
                    .take(5)
                    .collect::<String>()
                    .eq_ignore_ascii_case("AM/PM") =>
            {
                toks.push(Tok::AmPm(true));
                i += 4;
            }
            'A' | 'a'
                if chars[i..]
                    .iter()
                    .take(3)
                    .collect::<String>()
                    .eq_ignore_ascii_case("A/P") =>
            {
                toks.push(Tok::AmPm(false));
                i += 2;
            }
            'y' | 'Y' | 'm' | 'M' | 'd' | 'D' | 'h' | 'H' | 's' | 'S' => {
                let lc = c.to_ascii_lowercase();
                let mut j = i;
                while j < chars.len() && chars[j].to_ascii_lowercase() == lc {
                    j += 1;
                }
                toks.push(Tok::Date(std::iter::repeat(lc).take(j - i).collect()));
                i = j - 1;
            }
            c => lit(&mut toks, &c.to_string()),
        }
        i += 1;
    }
    // m/mm after h or before s means minutes
    let idx: Vec<usize> = toks
        .iter()
        .enumerate()
        .filter(|(_, t)| matches!(t, Tok::Date(_) | Tok::Elapsed(_)))
        .map(|(i, _)| i)
        .collect();
    let name = |t: &Tok| match t {
        Tok::Date(x) => Some(x.clone()),
        Tok::Elapsed(c) => Some(c.to_string()),
        _ => None,
    };
    for (k, &ti) in idx.iter().enumerate() {
        if let Tok::Date(d) = &toks[ti] {
            if d.starts_with('m') && d.len() <= 2 {
                let prev = k.checked_sub(1).and_then(|p| name(&toks[idx[p]]));
                let next = idx.get(k + 1).and_then(|n| name(&toks[*n]));
                if prev.is_some_and(|p| p.starts_with('h'))
                    || next.is_some_and(|n| n.starts_with('s'))
                {
                    toks[ti] = Tok::Date(if d.len() == 1 {
                        "n".into()
                    } else {
                        "nn".into()
                    });
                }
            }
        }
    }
    Section { toks, color, cond }
}

fn is_date(s: &Section) -> bool {
    s.toks
        .iter()
        .any(|t| matches!(t, Tok::Date(_) | Tok::Elapsed(_) | Tok::AmPm(_)))
}

pub fn is_date_format(code: &str) -> bool {
    split_sections(code)
        .first()
        .is_some_and(|s| is_date(&tokenize(s)))
}

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DAY_NAMES: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

fn format_date(n: f64, sec: &Section) -> String {
    let Some((y, mo, d)) = ymd(n) else {
        return "#".repeat(8);
    };
    let has_ampm = sec.toks.iter().any(|t| matches!(t, Tok::AmPm(_)));
    let frac_digits = {
        // ss.000 → fractional seconds
        let mut digits = 0;
        let mut after_s = false;
        for t in &sec.toks {
            match t {
                Tok::Date(x) if x.starts_with('s') => after_s = true,
                Tok::Point if after_s => {}
                Tok::Digit('0') if after_s => digits += 1,
                _ if digits > 0 => break,
                _ => after_s = after_s && matches!(t, Tok::Point),
            }
        }
        digits
    };
    let total_secs = if frac_digits > 0 {
        let k = 10f64.powi(frac_digits as i32);
        (((n - n.floor()) * 86400.0) * k).round() / k
    } else {
        ((n - n.floor()) * 86400.0).round()
    };
    let (h, mi, s) = hms(n);
    let wd = ((n.floor() as i64) - 1).rem_euclid(7) as usize;
    let mut out = String::new();
    let mut skip_point_digits = false;
    for t in &sec.toks {
        match t {
            Tok::Date(x) => {
                skip_point_digits = false;
                out.push_str(&match x.as_str() {
                    "yy" | "y" => format!("{:02}", y % 100),
                    x if x.starts_with('y') => format!("{y:04}"),
                    "m" => mo.to_string(),
                    "mm" => format!("{mo:02}"),
                    "mmm" => MONTH_NAMES[(mo - 1) as usize][..3].to_string(),
                    "mmmmm" => MONTH_NAMES[(mo - 1) as usize][..1].to_string(),
                    x if x.starts_with('m') => MONTH_NAMES[(mo - 1) as usize].to_string(),
                    "d" => d.to_string(),
                    "dd" => format!("{d:02}"),
                    "ddd" => DAY_NAMES[wd][..3].to_string(),
                    x if x.starts_with('d') => DAY_NAMES[wd].to_string(),
                    x if x.starts_with('h') => {
                        let hh = if has_ampm { (h + 11) % 12 + 1 } else { h };
                        if x.len() >= 2 {
                            format!("{hh:02}")
                        } else {
                            hh.to_string()
                        }
                    }
                    "n" => mi.to_string(),
                    "nn" => format!("{mi:02}"),
                    x if x.starts_with('s') => {
                        skip_point_digits = frac_digits > 0;
                        let whole = if frac_digits > 0 {
                            (total_secs.floor() as i64) % 60
                        } else {
                            s
                        };
                        if x.len() >= 2 {
                            format!("{whole:02}")
                        } else {
                            whole.to_string()
                        }
                    }
                    _ => String::new(),
                });
            }
            Tok::Point if skip_point_digits => {
                let frac = total_secs - total_secs.floor();
                let f = format!("{:.*}", frac_digits, frac);
                out.push_str(&f[1..]);
            }
            Tok::Digit(_) if skip_point_digits => {}
            Tok::Elapsed(c) => {
                let total = (n * 86400.0).round() as i64;
                out.push_str(&match c {
                    'h' => (total / 3600).to_string(),
                    'm' => (total / 60).to_string(),
                    _ => total.to_string(),
                });
            }
            Tok::AmPm(full) => {
                let pm = h >= 12;
                out.push_str(match (full, pm) {
                    (true, true) => "PM",
                    (true, false) => "AM",
                    (false, true) => "P",
                    (false, false) => "A",
                });
            }
            Tok::Lit(s) => out.push_str(s),
            Tok::Comma => out.push(','),
            Tok::Slash => out.push('/'),
            Tok::Percent => out.push('%'),
            Tok::Point => out.push('.'),
            Tok::Digit(c) => out.push(*c),
            _ => {}
        }
    }
    out
}

fn fill_int(digits: &str, pattern: &[char], grouping: bool) -> String {
    // pattern: digit placeholders for the integer part (left→right)
    let min_zeros = pattern.iter().filter(|c| **c == '0').count();
    let min_q = pattern.iter().filter(|c| **c == '?').count();
    let mut d = digits.trim_start_matches('0').to_string();
    if d.len() < min_zeros {
        d = format!("{}{}", "0".repeat(min_zeros - d.len()), d);
    }
    let mut s = if grouping {
        let mut out = String::new();
        for (i, ch) in d.chars().enumerate() {
            if i > 0 && (d.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(ch);
        }
        out
    } else {
        d
    };
    let width = min_zeros.max(min_q);
    while s.chars().count() < width {
        s.insert(0, ' ');
    }
    s
}

/// Closest fraction to `x` (0 ≤ x) with denominator ≤ `max_den`: continued
/// fractions with the best semiconvergent (O(log max_den), any digit count).
fn best_rational(x: f64, max_den: i64) -> (i64, i64) {
    if !x.is_finite() || x <= 0.0 {
        return (0, 1);
    }
    let (mut p0, mut q0, mut p1, mut q1) = (0i64, 1i64, 1i64, 0i64);
    let mut v = x;
    for _ in 0..64 {
        let a = v.floor();
        if a > i64::MAX as f64 / 2.0 {
            break;
        }
        let a = a as i64;
        let q2 = a.saturating_mul(q1).saturating_add(q0);
        if q2 > max_den {
            // largest semiconvergent that still fits
            let k = (max_den - q0) / q1.max(1);
            let (ps, qs) = (
                k.saturating_mul(p1).saturating_add(p0),
                k.saturating_mul(q1).saturating_add(q0),
            );
            let err = |p: i64, q: i64| (x - p as f64 / q as f64).abs();
            return if q1 > 0 && (qs == 0 || err(p1, q1) <= err(ps, qs)) {
                (p1, q1)
            } else {
                (ps, qs.max(1))
            };
        }
        let p2 = a.saturating_mul(p1).saturating_add(p0);
        (p0, q0, p1, q1) = (p1, q1, p2, q2);
        let f = v - a as f64;
        if f < 1e-12 {
            break;
        }
        v = 1.0 / f;
    }
    if q1 == 0 {
        (x.round() as i64, 1)
    } else {
        (p1, q1)
    }
}

#[cfg(test)]
#[test]
fn best_rational_matches_brute_force() {
    for i in 1..2000 {
        let x = (i as f64 * 0.7548776662).fract();
        for max in [9i64, 99, 999] {
            let (p, q) = best_rational(x, max);
            let e = (x - p as f64 / q as f64).abs();
            let brute = (1..=max)
                .map(|d| (x - (x * d as f64).round() / d as f64).abs())
                .fold(f64::INFINITY, f64::min);
            assert!(
                e <= brute + 1e-12,
                "x={x} max={max}: {p}/{q} err {e} vs {brute}"
            );
            assert!(q <= max);
        }
    }
}

fn format_fraction(n: f64, sec: &Section) -> String {
    let slash = sec.toks.iter().position(|t| *t == Tok::Slash).unwrap();
    let den_toks: Vec<&Tok> = sec.toks[slash + 1..]
        .iter()
        .take_while(|t| matches!(t, Tok::Digit(_)))
        .collect();
    let fixed_den: Option<i64> = sec.toks[slash + 1..].iter().find_map(|t| {
        if let Tok::Lit(l) = t {
            l.trim().parse().ok()
        } else {
            None
        }
    });
    let max_den = 10i64.pow(den_toks.len().clamp(1, 15) as u32) - 1;
    let has_int = sec.toks[..slash]
        .iter()
        .filter(|t| matches!(t, Tok::Digit(_)))
        .count()
        > 1
        || sec.toks[..slash]
            .iter()
            .any(|t| matches!(t, Tok::Lit(l) if l.contains(' ')));
    let neg = n < 0.0;
    let a = n.abs();
    let (int, frac) = if has_int {
        (a.floor(), a - a.floor())
    } else {
        (0.0, a)
    };
    let (num, den) = if let Some(d) = fixed_den {
        ((frac * d as f64).round() as i64, d)
    } else {
        best_rational(frac, max_den)
    };
    let sign = if neg { "-" } else { "" };
    if num == 0 {
        return format!("{sign}{}", int);
    }
    if has_int && int > 0.0 {
        format!("{sign}{} {num}/{den}", int)
    } else {
        format!("{sign}{num}/{den}")
    }
}

fn format_num(n: f64, sec: &Section, text: Option<&str>) -> String {
    if sec.toks.contains(&Tok::Slash) && !is_date(sec) {
        return format_fraction(n, sec);
    }
    let toks = &sec.toks;
    let pct = toks.iter().filter(|t| **t == Tok::Percent).count() as i32;
    let exp = toks.iter().position(|t| matches!(t, Tok::Exp(_)));
    let mantissa_end = exp.unwrap_or(toks.len());
    // a decimal point after the exponent (e.g. `00E-0.00`) is literal text, not the mantissa's point
    let point = toks
        .iter()
        .position(|t| *t == Tok::Point)
        .filter(|p| *p < mantissa_end);
    let int_end = point.unwrap_or(mantissa_end);
    let int_pat: Vec<char> = toks[..int_end]
        .iter()
        .filter_map(|t| {
            if let Tok::Digit(c) = t {
                Some(*c)
            } else {
                None
            }
        })
        .collect();
    let frac_pat: Vec<char> = point.map_or(vec![], |p| {
        toks[p + 1..mantissa_end]
            .iter()
            .filter_map(|t| {
                if let Tok::Digit(c) = t {
                    Some(*c)
                } else {
                    None
                }
            })
            .collect()
    });
    // thousands: comma between digit placeholders; trailing commas scale by 1000
    let last_digit = toks[..int_end]
        .iter()
        .rposition(|t| matches!(t, Tok::Digit(_)));
    let first_digit = toks.iter().position(|t| matches!(t, Tok::Digit(_)));
    let grouping = toks[..int_end].iter().enumerate().any(|(i, t)| {
        *t == Tok::Comma && first_digit.is_some_and(|f| i > f) && last_digit.is_some_and(|l| i < l)
    });
    let scale_commas = last_digit.map_or(0, |l| {
        toks[l + 1..int_end]
            .iter()
            .take_while(|t| **t == Tok::Comma)
            .count()
    });
    let mut v = n.abs() * 100f64.powi(pct) / 1000f64.powi(scale_commas as i32);
    let mut exp_val = 0i32;
    if exp.is_some() && v != 0.0 {
        let int_digits = int_pat.len().max(1) as i32;
        exp_val = v.log10().floor() as i32 - (int_digits - 1);
        if int_pat.len() > 1 {
            // engineering style: exponent multiple of integer digit count
            exp_val = (v.log10().floor() as i32).div_euclid(int_digits) * int_digits;
        }
        v /= 10f64.powi(exp_val);
    }
    let frac_n = frac_pat.len();
    let rounded = format!("{:.*}", frac_n, v + 1e-12 * v.max(1.0));
    let (int_digits, frac_digits) = rounded
        .split_once('.')
        .map_or((rounded.as_str(), ""), |(a, b)| (a, b));
    let int_str = if int_pat.is_empty() {
        if int_digits == "0" {
            String::new()
        } else {
            int_digits.to_string()
        }
    } else {
        fill_int(int_digits, &int_pat, grouping)
    };
    let mut frac_str: String = frac_digits.to_string();
    // trim optional trailing digits (#, ?)
    let mut chars: Vec<char> = frac_str.chars().collect();
    for (i, p) in frac_pat.iter().enumerate().rev() {
        if *p == '0' || chars.get(i) != Some(&'0') {
            break;
        }
        chars[i] = if *p == '?' { ' ' } else { '\0' };
    }
    frac_str = chars.into_iter().filter(|c| *c != '\0').collect();
    let mut out = String::new();
    let mut int_emitted = false;
    let mut frac_emitted = false;
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::Digit(_) if i < int_end => {
                if !int_emitted {
                    out.push_str(&int_str);
                    int_emitted = true;
                }
            }
            Tok::Digit(_) if i < mantissa_end => {
                if !frac_emitted {
                    out.push_str(&frac_str);
                    frac_emitted = true;
                }
            }
            Tok::Digit(_) => {}
            Tok::Point => {
                if !int_emitted {
                    out.push_str(&int_str);
                    int_emitted = true;
                }
                if !frac_str.is_empty() || frac_pat.is_empty() {
                    out.push('.');
                }
            }
            Tok::Exp(plus) => {
                let ndig = toks[i + 1..]
                    .iter()
                    .take_while(|t| matches!(t, Tok::Digit(_)))
                    .count()
                    .max(1);
                let sign = if exp_val < 0 {
                    "-"
                } else if *plus {
                    "+"
                } else {
                    ""
                };
                out.push_str(&format!("E{sign}{:0width$}", exp_val.abs(), width = ndig));
            }
            Tok::Comma => {}
            Tok::Percent => out.push('%'),
            Tok::Lit(s) => out.push_str(s),
            Tok::At => out.push_str(text.unwrap_or("")),
            Tok::Slash => out.push('/'),
            _ => {}
        }
    }
    if !int_emitted && first_digit.is_none() {
        // literal-only section like "Zero"
    }
    out
}

fn cond_ok(cond: &Option<(String, f64)>, n: f64) -> Option<bool> {
    let (op, v) = cond.as_ref()?;
    Some(match op.as_str() {
        "<" => n < *v,
        "<=" => n <= *v,
        ">" => n > *v,
        ">=" => n >= *v,
        "=" => n == *v,
        _ => n != *v,
    })
}

/// Format a value with a number-format code.
pub fn format_value(v: &Value, code: &str) -> Formatted {
    let code = if code.is_empty() { "General" } else { code };
    match v {
        Value::Number(n) => format_number_code(*n, code),
        Value::Text(t) => {
            let parts = split_sections(code);
            if parts.len() >= 4 {
                let s = tokenize(parts[3]);
                return Formatted {
                    text: format_num(
                        0.0,
                        &Section {
                            toks: s
                                .toks
                                .into_iter()
                                .filter(|t| !matches!(t, Tok::Digit(_)))
                                .collect(),
                            ..s
                        },
                        Some(t),
                    ),
                    color: None,
                };
            }
            if parts.len() == 1 && parts[0].contains('@') {
                let s = tokenize(parts[0]);
                let mut out = String::new();
                for tk in &s.toks {
                    match tk {
                        Tok::At => out.push_str(t),
                        Tok::Lit(l) => out.push_str(l),
                        _ => {}
                    }
                }
                return Formatted {
                    text: out,
                    color: s.color,
                };
            }
            Formatted {
                text: t.to_string(),
                color: None,
            }
        }
        other => Formatted {
            text: other.to_string(),
            color: None,
        },
    }
}

pub fn format_number_code(n: f64, code: &str) -> Formatted {
    if code.eq_ignore_ascii_case("General") || code.is_empty() {
        return Formatted {
            text: format_number(n),
            color: None,
        };
    }
    let parts = split_sections(code);
    let secs: Vec<Section> = parts.iter().map(|p| tokenize(p)).collect();
    // choose a section
    let (sec, signed) = if secs.iter().any(|s| s.cond.is_some()) {
        let idx = secs
            .iter()
            .position(|s| cond_ok(&s.cond, n) == Some(true))
            .or_else(|| secs.iter().position(|s| s.cond.is_none()))
            .unwrap_or(0);
        (
            &secs[idx],
            idx == 0 && n < 0.0 && secs[idx].cond.as_ref().is_some_and(|c| c.1 >= 0.0),
        )
    } else if n > 0.0 || secs.len() == 1 || (n == 0.0 && secs.len() == 2) {
        (&secs[0], n < 0.0)
    } else if n < 0.0 {
        (&secs[1], false)
    } else {
        (&secs[2.min(secs.len() - 1)], false)
    };
    let general = parts
        .get(secs.iter().position(|s| std::ptr::eq(s, sec)).unwrap_or(0))
        .is_some_and(|p| p.to_ascii_lowercase().contains("general"));
    let body = if is_date(sec) {
        format_date(n, sec)
    } else if general {
        let lits: String = sec
            .toks
            .iter()
            .filter_map(|t| {
                if let Tok::Lit(l) = t {
                    Some(l.replace("General", "").replace("general", ""))
                } else {
                    None
                }
            })
            .collect();
        format!("{}{}", format_number(n.abs()), lits)
    } else {
        format_num(n, sec, None)
    };
    let neg = signed && !is_date(sec) && body.chars().any(|c| c.is_ascii_digit() && c != '0');
    Formatted {
        text: if neg { format!("-{body}") } else { body },
        color: sec.color.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(n: f64, c: &str) -> String {
        format_number_code(n, c).text
    }

    #[test]
    fn numbers() {
        assert_eq!(f(1234.567, "0.00"), "1234.57");
        assert_eq!(f(1234.567, "#,##0.00"), "1,234.57");
        assert_eq!(f(-1234.5, "#,##0"), "-1,235");
        assert_eq!(f(-1234.5, "#,##0;(#,##0)"), "(1,235)");
        assert_eq!(f(0.256, "0.0%"), "25.6%");
        assert_eq!(f(12345.0, "0.00E+00"), "1.23E+04");
        assert_eq!(f(0.5, "# ?/?"), "1/2");
        assert_eq!(f(std::f64::consts::PI, "# ???/???"), "3 16/113");
        assert_eq!(f(0.142857, "?/?"), "1/7");
        // 11+ denominator digits must not loop or overflow
        let _ = f(0.123456789, "? ./?##00#0#00");
        let _ = f(1e300, "?/????????????????????????");
        assert_eq!(f(1.25, "# ?/?"), "1 1/4");
        assert_eq!(f(5.0, "000"), "005");
        assert_eq!(f(1.5, "#.##"), "1.5");
        assert_eq!(f(1234567.0, "#,##0,"), "1,235");
        assert_eq!(f(3.0, "\"$\"#,##0.00"), "$3.00");
        assert_eq!(f(0.0, "0;-0;\"zero\""), "zero");
        assert_eq!(
            format_number_code(-5.0, "0;[Red]-0").color.as_deref(),
            Some("red")
        );
    }

    #[test]
    fn dates() {
        assert_eq!(f(45306.75, "yyyy-mm-dd"), "2024-01-15");
        assert_eq!(f(45306.75, "d mmm yyyy"), "15 Jan 2024");
        assert_eq!(f(45306.75, "dddd, mmmm d"), "Monday, January 15");
        assert_eq!(f(45306.75, "hh:mm"), "18:00");
        assert_eq!(f(45306.75, "h:mm AM/PM"), "6:00 PM");
        assert_eq!(f(1.5, "[h]:mm"), "36:00");
        assert_eq!(f(0.000011574, "ss.00"), "01.00");
        assert!(is_date_format("m/d/yyyy"));
        assert!(!is_date_format("0.00"));
    }

    #[test]
    fn text_sections() {
        assert_eq!(
            format_value(&Value::text("x"), "0;-0;0;\"<\"@\">\"").text,
            "<x>"
        );
        assert_eq!(format_value(&Value::text("x"), "@").text, "x");
    }
}
