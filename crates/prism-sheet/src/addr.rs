//! Cell addresses: A1 and R1C1 notation.

use serde::{Deserialize, Serialize};

/// Excel limits.
pub const MAX_ROWS: u32 = 1_048_576;
pub const MAX_COLS: u32 = 16_384;

/// Zero-based cell coordinate.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct CellRef {
    pub row: u32,
    pub col: u32,
}

impl CellRef {
    pub const fn new(row: u32, col: u32) -> Self {
        Self { row, col }
    }
}

/// Inclusive rectangle of cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    pub r0: u32,
    pub c0: u32,
    pub r1: u32,
    pub c1: u32,
}

impl Rect {
    pub fn new(a: CellRef, b: CellRef) -> Self {
        Self {
            r0: a.row.min(b.row),
            c0: a.col.min(b.col),
            r1: a.row.max(b.row),
            c1: a.col.max(b.col),
        }
    }
    pub fn cell(c: CellRef) -> Self {
        Self::new(c, c)
    }
    pub fn rows(&self) -> u32 {
        self.r1 - self.r0 + 1
    }
    pub fn cols(&self) -> u32 {
        self.c1 - self.c0 + 1
    }
    pub fn contains(&self, c: CellRef) -> bool {
        c.row >= self.r0 && c.row <= self.r1 && c.col >= self.c0 && c.col <= self.c1
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        self.r0 <= o.r1 && o.r0 <= self.r1 && self.c0 <= o.c1 && o.c0 <= self.c1
    }
    pub fn start(&self) -> CellRef {
        CellRef::new(self.r0, self.c0)
    }
    pub fn size(&self) -> u64 {
        self.rows() as u64 * self.cols() as u64
    }
}

/// 0 → "A", 25 → "Z", 26 → "AA".
pub fn col_name(mut col: u32) -> String {
    let mut s = Vec::new();
    col += 1;
    while col > 0 {
        let r = (col - 1) % 26;
        s.push(b'A' + r as u8);
        col = (col - 1) / 26;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

/// "A" → 0, "XFD" → 16383.
pub fn col_index(s: &str) -> Option<u32> {
    if s.is_empty() || s.len() > 3 {
        return None;
    }
    let mut n: u32 = 0;
    for c in s.chars() {
        if !c.is_ascii_alphabetic() {
            return None;
        }
        n = n * 26 + (c.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
    }
    (1..=MAX_COLS).contains(&n).then(|| n - 1)
}

pub fn a1(c: CellRef) -> String {
    format!("{}{}", col_name(c.col), c.row + 1)
}

pub fn rect_a1(r: &Rect) -> String {
    if r.r0 == r.r1 && r.c0 == r.c1 {
        a1(r.start())
    } else {
        format!("{}:{}", a1(r.start()), a1(CellRef::new(r.r1, r.c1)))
    }
}

/// Parse "B3" / "$B$3" → (cell, col_abs, row_abs).
pub fn parse_a1(s: &str) -> Option<(CellRef, bool, bool)> {
    let b = s.as_bytes();
    let mut i = 0;
    let col_abs = b.first() == Some(&b'$');
    if col_abs {
        i += 1;
    }
    let cs = i;
    while i < b.len() && b[i].is_ascii_alphabetic() {
        i += 1;
    }
    let col = col_index(&s[cs..i])?;
    let row_abs = b.get(i) == Some(&b'$');
    if row_abs {
        i += 1;
    }
    let rs = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i != b.len() || rs == i {
        return None;
    }
    let row: u32 = s[rs..i].parse().ok()?;
    (1..=MAX_ROWS)
        .contains(&row)
        .then(|| (CellRef::new(row - 1, col), col_abs, row_abs))
}

/// R1C1 text of a reference relative to `base` (`R[1]C[-2]`, `R3C4`).
pub fn r1c1(target: CellRef, base: CellRef, row_abs: bool, col_abs: bool) -> String {
    let part = |p: &str, abs: bool, t: u32, b: u32| {
        if abs {
            format!("{p}{}", t + 1)
        } else if t == b {
            p.to_string()
        } else {
            format!("{p}[{}]", t as i64 - b as i64)
        }
    };
    format!(
        "{}{}",
        part("R", row_abs, target.row, base.row),
        part("C", col_abs, target.col, base.col)
    )
}

/// Parse an R1C1 reference relative to `base` → (cell, row_abs, col_abs).
pub fn parse_r1c1(s: &str, base: CellRef) -> Option<(CellRef, bool, bool)> {
    let up = s.to_ascii_uppercase();
    let rest = up.strip_prefix('R')?;
    let cpos = rest.find('C')?;
    let (rp, cp) = (&rest[..cpos], &rest[cpos + 1..]);
    let parse = |p: &str, b: u32, max: u32| -> Option<(u32, bool)> {
        if p.is_empty() {
            return Some((b, false));
        }
        if let Some(inner) = p.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
            let d: i64 = inner.parse().ok()?;
            let v = b as i64 + d;
            return (v >= 0 && v < max as i64).then_some((v as u32, false));
        }
        let v: u32 = p.parse().ok()?;
        (v >= 1 && v <= max).then_some((v - 1, true))
    };
    let (row, ra) = parse(rp, base.row, MAX_ROWS)?;
    let (col, ca) = parse(cp, base.col, MAX_COLS)?;
    Some((CellRef::new(row, col), ra, ca))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a1_roundtrip() {
        for (i, n) in [
            (0, "A"),
            (25, "Z"),
            (26, "AA"),
            (701, "ZZ"),
            (702, "AAA"),
            (16383, "XFD"),
        ] {
            assert_eq!(col_name(i), n);
            assert_eq!(col_index(n), Some(i));
        }
        assert_eq!(col_index("XFE"), None);
        assert_eq!(parse_a1("$B$3"), Some((CellRef::new(2, 1), true, true)));
        assert_eq!(parse_a1("c10"), Some((CellRef::new(9, 2), false, false)));
        assert!(parse_a1("A0").is_none() && parse_a1("A").is_none() && parse_a1("1A").is_none());
        assert_eq!(
            rect_a1(&Rect::new(CellRef::new(4, 3), CellRef::new(0, 0))),
            "A1:D5"
        );
    }

    #[test]
    fn r1c1_roundtrip() {
        let base = CellRef::new(4, 4);
        assert_eq!(r1c1(CellRef::new(5, 2), base, false, false), "R[1]C[-2]");
        assert_eq!(r1c1(CellRef::new(0, 0), base, true, true), "R1C1");
        assert_eq!(r1c1(base, base, false, false), "RC");
        assert_eq!(
            parse_r1c1("R[1]C[-2]", base),
            Some((CellRef::new(5, 2), false, false))
        );
        assert_eq!(
            parse_r1c1("r1c1", base),
            Some((CellRef::new(0, 0), true, true))
        );
        assert_eq!(parse_r1c1("RC", base), Some((base, false, false)));
        assert!(parse_r1c1("R[-9]C", base).is_none());
    }
}
