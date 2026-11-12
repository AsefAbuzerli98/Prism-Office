//! Dates and times (1900 serial system incl. the Lotus leap-year bug).

use super::{arg, arg_num, arr, err, lift, lifted, present, tryv};
use crate::eval::{num, text as to_text, Ctx};
use crate::parser::Expr;
use crate::value::{ErrorKind, Value};

pub const NAMES: &[&str] = &[
    "DATE",
    "TIME",
    "YEAR",
    "MONTH",
    "DAY",
    "HOUR",
    "MINUTE",
    "SECOND",
    "TODAY",
    "NOW",
    "WEEKDAY",
    "WEEKNUM",
    "ISOWEEKNUM",
    "EDATE",
    "EOMONTH",
    "DATEDIF",
    "DAYS",
    "NETWORKDAYS",
    "NETWORKDAYS.INTL",
    "WORKDAY",
    "WORKDAY.INTL",
    "DATEVALUE",
    "TIMEVALUE",
    "YEARFRAC",
    "DAYS360",
];

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(y) => 29,
        _ => 28,
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

const EPOCH_1970: i64 = 25569; // serial of 1970-01-01

/// Serial number for y/m/d with month/day overflow normalised like Excel.
pub fn serial(y: i64, m: i64, d: i64) -> Option<f64> {
    let y = if (0..1900).contains(&y) { y + 1900 } else { y };
    let total_m = y * 12 + (m - 1);
    let (y, m) = (total_m.div_euclid(12), total_m.rem_euclid(12) + 1);
    let days = days_from_civil(y, m, 1) + d - 1;
    let mut s = days + EPOCH_1970;
    // Excel counts the nonexistent 1900-02-29 (serial 60).
    if s < 61 {
        s -= 1;
    }
    (0..=2_958_465).contains(&s).then_some(s as f64)
}

/// (year, month, day) for a serial (serial 60 → 1900-02-29).
pub fn ymd(s: f64) -> Option<(i64, i64, i64)> {
    if !(0.0..2_958_466.0).contains(&s) {
        return None;
    }
    let s = s.floor() as i64;
    if s == 60 {
        return Some((1900, 2, 29));
    }
    if s == 0 {
        return Some((1900, 1, 0));
    }
    let adj = if s < 60 { s + 1 } else { s };
    Some(civil_from_days(adj - EPOCH_1970))
}

/// 0 = Sunday.
fn weekday0(s: f64) -> i64 {
    // serial 1 (1900-01-01) is reported as Sunday by Excel's convention (WEEKDAY(1)=1)
    ((s.floor() as i64) - 1).rem_euclid(7)
}

pub fn hms(s: f64) -> (i64, i64, i64) {
    let secs = ((s - s.floor()) * 86400.0).round() as i64;
    let secs = secs.min(86399);
    (secs / 3600, secs / 60 % 60, secs % 60)
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

fn month_name(t: &str) -> Option<i64> {
    let t = t.to_ascii_lowercase();
    if t.len() < 3 {
        return None;
    }
    MONTHS
        .iter()
        .position(|m| t.starts_with(m))
        .map(|i| i as i64 + 1)
}

fn parse_time(t: &str) -> Option<f64> {
    let t = t.trim();
    let lower = t.to_ascii_lowercase();
    let (body, pm) = if let Some(b) = lower.strip_suffix("pm") {
        (b.trim().to_string(), Some(true))
    } else if let Some(b) = lower.strip_suffix("am") {
        (b.trim().to_string(), Some(false))
    } else {
        (lower.clone(), None)
    };
    let parts: Vec<&str> = body.split(':').collect();
    if parts.len() < 2 && pm.is_none() || parts.len() > 3 {
        return None;
    }
    let mut h: f64 = parts[0].trim().parse().ok()?;
    let m: f64 = parts.get(1).map_or(Some(0.0), |p| p.trim().parse().ok())?;
    let s: f64 = parts.get(2).map_or(Some(0.0), |p| p.trim().parse().ok())?;
    if let Some(pm) = pm {
        if !(1.0..=12.0).contains(&h) {
            return None;
        }
        if h == 12.0 {
            h = 0.0;
        }
        if pm {
            h += 12.0;
        }
    }
    if m >= 60.0 || s >= 60.0 || h < 0.0 {
        return None;
    }
    Some((h * 3600.0 + m * 60.0 + s) / 86400.0)
}

fn parse_date_part(t: &str) -> Option<f64> {
    let t = t.trim().trim_end_matches(',');
    // ISO yyyy-mm-dd or yyyy/mm/dd
    let nums: Vec<&str> = t.split(['-', '/', '.']).collect();
    if nums.len() == 3
        && nums
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    {
        let v: Vec<i64> = nums.iter().map(|p| p.parse().unwrap()).collect();
        let (y, m, d) = if nums[0].len() == 4 {
            (v[0], v[1], v[2])
        } else {
            (
                if v[2] < 100 {
                    v[2] + if v[2] < 30 { 2000 } else { 1900 }
                } else {
                    v[2]
                },
                v[0],
                v[1],
            )
        };
        if !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
            return None;
        }
        return serial(y, m, d);
    }
    if nums.len() == 2
        && nums
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        && !t.contains('.')
    {
        // m/d in the current year is ambiguous without a clock; treat as m/yyyy when 4 digits
        let (a, b): (i64, i64) = (nums[0].parse().ok()?, nums[1].parse().ok()?);
        if nums[1].len() == 4 && (1..=12).contains(&a) {
            return serial(b, a, 1);
        }
        return None;
    }
    // 15-Jan-2024, 15 Jan 2024, Jan 15 2024, January 15, 2024
    let words: Vec<&str> = t
        .split([' ', '-', ',', '/'])
        .filter(|w| !w.is_empty())
        .collect();
    if words.len() == 3 {
        let (d, m, y) = if let Some(m) = month_name(words[1]) {
            (
                words[0].parse::<i64>().ok()?,
                m,
                words[2].parse::<i64>().ok()?,
            )
        } else {
            let m = month_name(words[0])?;
            (
                words[1].parse::<i64>().ok()?,
                m,
                words[2].parse::<i64>().ok()?,
            )
        };
        let y = if y < 100 {
            y + if y < 30 { 2000 } else { 1900 }
        } else {
            y
        };
        if d < 1 || d > days_in_month(y, m) {
            return None;
        }
        return serial(y, m, d);
    }
    None
}

/// Parse text that Excel would coerce to a date/time serial.
pub fn parse_date_text(t: &str) -> Option<f64> {
    let t = t.trim();
    if t.is_empty() || !t.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    if let Some(d) = parse_date_part(t) {
        return Some(d);
    }
    if let Some(tm) = parse_time(t) {
        return Some(tm);
    }
    // date + time, split at the last space or 'T'
    for sep in ['T', ' '] {
        let mut idx: Vec<usize> = t.match_indices(sep).map(|(i, _)| i).collect();
        idx.reverse();
        for i in idx {
            let (d, tm) = (&t[..i], &t[i + 1..]);
            // "10:30 PM" contains a space: try two-word times too
            if let (Some(d), Some(tm)) = (parse_date_part(d), parse_time(tm)) {
                return Some(d + tm);
            }
        }
    }
    None
}

fn date_arg(v: &Value) -> Result<f64, ErrorKind> {
    let n = num(v)?;
    if n < 0.0 {
        Err(ErrorKind::Num)
    } else {
        Ok(n)
    }
}

fn eomonth(s: f64, months: f64) -> Option<f64> {
    let (y, m, _) = ymd(s)?;
    let t = y * 12 + (m - 1) + months.trunc() as i64;
    let (y, m) = (t.div_euclid(12), t.rem_euclid(12) + 1);
    serial(y, m, days_in_month(y, m))
}

fn edate(s: f64, months: f64) -> Option<f64> {
    let (y, m, d) = ymd(s)?;
    let t = y * 12 + (m - 1) + months.trunc() as i64;
    let (y, m) = (t.div_euclid(12), t.rem_euclid(12) + 1);
    serial(y, m, d.min(days_in_month(y, m)))
}

/// Weekend mask (Mon..Sun) from a WORKDAY.INTL/NETWORKDAYS.INTL code.
fn weekend_mask(v: &Value) -> Result<[bool; 7], ErrorKind> {
    if let Value::Text(t) = v {
        if t.len() == 7 && t.chars().all(|c| c == '0' || c == '1') {
            let mut m = [false; 7];
            for (i, c) in t.chars().enumerate() {
                m[i] = c == '1';
            }
            if m.iter().all(|x| *x) {
                return Err(ErrorKind::Value);
            }
            return Ok(m);
        }
        return Err(ErrorKind::Value);
    }
    let code = if matches!(v, Value::Empty) {
        1
    } else {
        num(v)? as i64
    };
    let mut m = [false; 7];
    match code {
        1..=7 => {
            // 1 = Sat,Sun; 2 = Sun,Mon; ... 7 = Fri,Sat
            let first = (code + 4) % 7; // index Mon=0: Sat=5
            m[first as usize] = true;
            m[((first + 1) % 7) as usize] = true;
        }
        11..=17 => m[((code - 11 + 6) % 7) as usize] = true,
        _ => return Err(ErrorKind::Num),
    }
    Ok(m)
}

fn is_workday(s: i64, mask: &[bool; 7], holidays: &[i64]) -> bool {
    let mon0 = (weekday0(s as f64) + 6) % 7;
    !mask[mon0 as usize] && !holidays.contains(&s)
}

fn holidays(ctx: &mut Ctx, args: &[Expr], i: usize) -> Result<Vec<i64>, ErrorKind> {
    if !present(args, i) {
        return Ok(vec![]);
    }
    let a = arr(ctx, args, i);
    a.data
        .iter()
        .filter(|v| !matches!(v, Value::Empty))
        .map(|v| num(v).map(|n| n.floor() as i64))
        .collect()
}

fn days360(a: f64, b: f64, european: bool) -> Option<f64> {
    let (y1, m1, mut d1) = ymd(a)?;
    let (y2, m2, mut d2) = ymd(b)?;
    if european {
        d1 = d1.min(30);
        d2 = d2.min(30);
    } else {
        let last_feb = |y: i64, m: i64, d: i64| m == 2 && d == days_in_month(y, 2);
        if last_feb(y1, m1, d1) {
            d1 = 30;
        }
        if d1 == 31 {
            d1 = 30;
        }
        if d2 == 31 && d1 >= 30 {
            d2 = 30;
        }
    }
    Some(((y2 - y1) * 360 + (m2 - m1) * 30 + (d2 - d1)) as f64)
}

pub fn call(ctx: &mut Ctx, name: &str, args: &[Expr]) -> Option<Value> {
    Some(match name {
        "TODAY" => Value::Number(ctx.now.floor()),
        "NOW" => Value::Number(ctx.now),
        "DATE" => lifted(ctx, args, |v| match (num(&v[0]), num(&v[1]), num(&v[2])) {
            (Ok(y), Ok(m), Ok(d)) => serial(y.trunc() as i64, m.trunc() as i64, d.trunc() as i64)
                .map(Value::Number)
                .unwrap_or(Value::Error(ErrorKind::Num)),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Value::Error(e),
        }),
        "TIME" => lifted(ctx, args, |v| match (num(&v[0]), num(&v[1]), num(&v[2])) {
            (Ok(h), Ok(m), Ok(s)) => {
                let t = h.trunc() * 3600.0 + m.trunc() * 60.0 + s.trunc();
                if t < 0.0 {
                    Value::Error(ErrorKind::Num)
                } else {
                    Value::Number((t % 86400.0) / 86400.0)
                }
            }
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Value::Error(e),
        }),
        "YEAR" | "MONTH" | "DAY" => {
            let part = name.to_string();
            lifted(ctx, args, move |v| match date_arg(&v[0]) {
                Ok(s) => match ymd(s) {
                    Some((y, m, d)) => Value::Number(match part.as_str() {
                        "YEAR" => y,
                        "MONTH" => m,
                        _ => d,
                    } as f64),
                    None => Value::Error(ErrorKind::Num),
                },
                Err(e) => Value::Error(e),
            })
        }
        "HOUR" | "MINUTE" | "SECOND" => {
            let part = name.to_string();
            lifted(ctx, args, move |v| match date_arg(&v[0]) {
                Ok(s) => {
                    let (h, m, sec) = hms(s);
                    Value::Number(match part.as_str() {
                        "HOUR" => h,
                        "MINUTE" => m,
                        _ => sec,
                    } as f64)
                }
                Err(e) => Value::Error(e),
            })
        }
        "WEEKDAY" => {
            let s = arg(ctx, args, 0);
            let t = if present(args, 1) {
                arg(ctx, args, 1)
            } else {
                Value::Number(1.0)
            };
            lift(vec![s, t], |v| {
                let (s, t) = match (date_arg(&v[0]), num(&v[1])) {
                    (Ok(s), Ok(t)) => (s, t as i64),
                    (Err(e), _) | (_, Err(e)) => return Value::Error(e),
                };
                let w = weekday0(s); // 0 = Sunday
                Value::Number(match t {
                    1 | 17 => w + 1,
                    2 | 11 => (w + 6) % 7 + 1,
                    3 => (w + 6) % 7,
                    12..=16 => (w - (t - 10) + 7) % 7 + 1,
                    _ => return Value::Error(ErrorKind::Num),
                } as f64)
            })
        }
        "ISOWEEKNUM" => lifted(ctx, args, |v| match date_arg(&v[0]) {
            Ok(s) => {
                let mon0 = (weekday0(s) + 6) % 7;
                let thursday = s.floor() - mon0 as f64 + 3.0;
                let Some((y, _, _)) = ymd(thursday) else {
                    return Value::Error(ErrorKind::Num);
                };
                let jan1 = serial(y, 1, 1).unwrap_or(0.0);
                Value::Number(((thursday - jan1) / 7.0).floor() + 1.0)
            }
            Err(e) => Value::Error(e),
        }),
        "WEEKNUM" => {
            let s = tryv!(date_arg(&arg(ctx, args, 0)));
            let t = tryv!(arg_num(ctx, args, 1, 1.0)) as i64;
            if t == 21 {
                return call(ctx, "ISOWEEKNUM", &args[..1]);
            }
            let start = match t {
                1 | 17 => 0,
                2 | 11 => 1,
                12..=16 => t - 10,
                _ => return Some(err(ErrorKind::Num)),
            };
            let Some((y, _, _)) = ymd(s) else {
                return Some(err(ErrorKind::Num));
            };
            let jan1 = serial(y, 1, 1).unwrap_or(1.0);
            let offset = (weekday0(jan1) - start).rem_euclid(7);
            Value::Number(((s.floor() - jan1 + offset as f64) / 7.0).floor() + 1.0)
        }
        "EDATE" | "EOMONTH" => {
            let f = if name == "EDATE" { edate } else { eomonth };
            lifted(ctx, args, move |v| match (date_arg(&v[0]), num(&v[1])) {
                (Ok(s), Ok(m)) => f(s, m)
                    .map(Value::Number)
                    .unwrap_or(Value::Error(ErrorKind::Num)),
                (Err(e), _) | (_, Err(e)) => Value::Error(e),
            })
        }
        "DAYS" => lifted(ctx, args, |v| match (date_arg(&v[0]), date_arg(&v[1])) {
            (Ok(a), Ok(b)) => Value::Number(a.floor() - b.floor()),
            (Err(e), _) | (_, Err(e)) => Value::Error(e),
        }),
        "DAYS360" => {
            let a = tryv!(date_arg(&arg(ctx, args, 0)));
            let b = tryv!(date_arg(&arg(ctx, args, 1)));
            let eu = present(args, 2) && tryv!(crate::eval::truthy(&arg(ctx, args, 2)));
            days360(a, b, eu)
                .map(Value::Number)
                .unwrap_or(err(ErrorKind::Num))
        }
        "DATEDIF" => {
            let a = tryv!(date_arg(&arg(ctx, args, 0))).floor();
            let b = tryv!(date_arg(&arg(ctx, args, 1))).floor();
            let unit = tryv!(to_text(&arg(ctx, args, 2))).to_ascii_uppercase();
            if a > b {
                return Some(err(ErrorKind::Num));
            }
            let (Some((y1, m1, d1)), Some((y2, m2, d2))) = (ymd(a), ymd(b)) else {
                return Some(err(ErrorKind::Num));
            };
            let months = (y2 - y1) * 12 + (m2 - m1) - (d2 < d1) as i64;
            Value::Number(match unit.as_str() {
                "D" => b - a,
                "M" => months as f64,
                "Y" => (months / 12) as f64,
                "YM" => (months % 12) as f64,
                "MD" => {
                    if d2 >= d1 {
                        (d2 - d1) as f64
                    } else {
                        let (py, pm) = if m2 == 1 { (y2 - 1, 12) } else { (y2, m2 - 1) };
                        (days_in_month(py, pm) - d1 + d2).max(0) as f64
                    }
                }
                "YD" => {
                    let anniv =
                        serial(if (m2, d2) >= (m1, d1) { y2 } else { y2 - 1 }, m1, d1).unwrap_or(a);
                    b - anniv
                }
                _ => return Some(err(ErrorKind::Num)),
            })
        }
        "NETWORKDAYS" | "NETWORKDAYS.INTL" => {
            let a = tryv!(date_arg(&arg(ctx, args, 0))).floor() as i64;
            let b = tryv!(date_arg(&arg(ctx, args, 1))).floor() as i64;
            let intl = name.ends_with("INTL");
            let mask = if intl {
                tryv!(weekend_mask(&arg(ctx, args, 2)))
            } else {
                tryv!(weekend_mask(&Value::Number(1.0)))
            };
            let hol = tryv!(holidays(ctx, args, if intl { 3 } else { 2 }));
            let (lo, hi, sign) = if a <= b { (a, b, 1.0) } else { (b, a, -1.0) };
            if hi - lo > 3_000_000 {
                return Some(err(ErrorKind::Num));
            }
            Value::Number(sign * (lo..=hi).filter(|d| is_workday(*d, &mask, &hol)).count() as f64)
        }
        "WORKDAY" | "WORKDAY.INTL" => {
            let mut d = tryv!(date_arg(&arg(ctx, args, 0))).floor() as i64;
            let mut n = tryv!(arg_num(ctx, args, 1, 0.0)).trunc() as i64;
            let intl = name.ends_with("INTL");
            let mask = if intl {
                tryv!(weekend_mask(&arg(ctx, args, 2)))
            } else {
                tryv!(weekend_mask(&Value::Number(1.0)))
            };
            let hol = tryv!(holidays(ctx, args, if intl { 3 } else { 2 }));
            let step = n.signum();
            let mut guard = 0;
            while n != 0 {
                d += step;
                if is_workday(d, &mask, &hol) {
                    n -= step;
                }
                guard += 1;
                if guard > 10_000_000 || d < 0 {
                    return Some(err(ErrorKind::Num));
                }
            }
            Value::Number(d as f64)
        }
        "DATEVALUE" => lifted(ctx, args, |v| match &v[0] {
            Value::Text(t) => parse_date_part(t)
                .or_else(|| parse_date_text(t).map(f64::floor))
                .map(Value::Number)
                .unwrap_or(Value::Error(ErrorKind::Value)),
            Value::Error(e) => Value::Error(*e),
            _ => Value::Error(ErrorKind::Value),
        }),
        "TIMEVALUE" => lifted(ctx, args, |v| match &v[0] {
            Value::Text(t) => parse_date_text(t)
                .map(|x| Value::Number(x - x.floor()))
                .unwrap_or(Value::Error(ErrorKind::Value)),
            Value::Error(e) => Value::Error(*e),
            _ => Value::Error(ErrorKind::Value),
        }),
        "YEARFRAC" => {
            let mut a = tryv!(date_arg(&arg(ctx, args, 0))).floor();
            let mut b = tryv!(date_arg(&arg(ctx, args, 1))).floor();
            let basis = tryv!(arg_num(ctx, args, 2, 0.0)) as i64;
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            Value::Number(match basis {
                0 => days360(a, b, false).unwrap_or(0.0) / 360.0,
                1 => {
                    let (Some((y1, _, _)), Some((y2, _, _))) = (ymd(a), ymd(b)) else {
                        return Some(err(ErrorKind::Num));
                    };
                    let years = y2 - y1 + 1;
                    let days: i64 = (y1..=y2).map(|y| if is_leap(y) { 366 } else { 365 }).sum();
                    let avg = if y1 == y2 {
                        days as f64
                    } else {
                        days as f64 / years as f64
                    };
                    (b - a) / avg
                }
                2 => (b - a) / 360.0,
                3 => (b - a) / 365.0,
                4 => days360(a, b, true).unwrap_or(0.0) / 360.0,
                _ => return Some(err(ErrorKind::Num)),
            })
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serials_match_excel() {
        assert_eq!(serial(1900, 1, 1), Some(1.0));
        assert_eq!(serial(1900, 2, 28), Some(59.0));
        assert_eq!(serial(1900, 3, 1), Some(61.0));
        assert_eq!(serial(2024, 1, 15), Some(45306.0));
        assert_eq!(serial(2024, 14, 1), serial(2025, 2, 1));
        assert_eq!(ymd(45306.0), Some((2024, 1, 15)));
        assert_eq!(ymd(60.0), Some((1900, 2, 29)));
        assert_eq!(ymd(61.0), Some((1900, 3, 1)));
        assert_eq!(weekday0(45306.0), 1); // Monday
    }

    #[test]
    fn date_text() {
        assert_eq!(parse_date_text("2024-01-15"), Some(45306.0));
        assert_eq!(parse_date_text("1/15/2024"), Some(45306.0));
        assert_eq!(parse_date_text("15-Jan-2024"), Some(45306.0));
        assert_eq!(parse_date_text("January 15, 2024"), Some(45306.0));
        assert_eq!(parse_date_text("12:00"), Some(0.5));
        assert_eq!(parse_date_text("6:00 PM"), Some(0.75));
        assert_eq!(parse_date_text("2024-01-15 06:00"), Some(45306.25));
        assert_eq!(parse_date_text("hello"), None);
    }
}
