//! Expression evaluation.

use std::cmp::Ordering;
use std::sync::Arc;

use crate::addr::{CellRef, Rect, MAX_COLS, MAX_ROWS};
use crate::parser::{BinOp, Expr, RefExpr, RefKind, UnOp};
use crate::value::{Array, ErrorKind, Lambda, RangeRef, Value};
use crate::workbook::{parse_number, AsyncRequest, Workbook};

pub struct Ctx<'a> {
    pub wb: &'a Workbook,
    pub sheet: usize,
    pub cell: CellRef,
    /// LET / LAMBDA bindings, innermost last.
    pub env: Vec<(String, Value)>,
    /// Serial date-time of "now" for this recalculation.
    pub now: f64,
    pub rng: u64,
    pub requests: Vec<AsyncRequest>,
    pub depth: u32,
}

pub type Res<T> = Result<T, ErrorKind>;

impl<'a> Ctx<'a> {
    pub fn new(wb: &'a Workbook, sheet: usize, cell: CellRef, now: f64) -> Self {
        let seed = (sheet as u64) << 40 ^ (cell.row as u64) << 16 ^ cell.col as u64 ^ now.to_bits();
        Self {
            wb,
            sheet,
            cell,
            env: Vec::new(),
            now,
            rng: seed | 1,
            requests: Vec::new(),
            depth: 0,
        }
    }

    /// xorshift64* in [0, 1).
    pub fn random(&mut self) -> f64 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        (x.wrapping_mul(0x2545F4914F6CDD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn eval(&mut self, e: &Expr) -> Value {
        match e {
            Expr::Number(n) => Value::Number(*n),
            Expr::Text(s) => Value::text(s.clone()),
            Expr::Bool(b) => Value::Bool(*b),
            Expr::Error(k) => Value::Error(*k),
            Expr::Missing => Value::Empty,
            Expr::Array(rows) => {
                let cols = rows.first().map_or(0, |r| r.len());
                let data = rows.iter().flatten().map(|x| self.eval(x)).collect();
                Value::array(Array::new(rows.len(), cols, data))
            }
            Expr::Ref(r) => self
                .resolve_ref(r)
                .map(Value::Ref)
                .unwrap_or(Value::Error(ErrorKind::Ref)),
            Expr::Name(n) => self.name(n),
            Expr::Unary(op, x) => {
                let v = self.eval(x);
                match op {
                    UnOp::Implicit => self.implicit(v),
                    UnOp::Neg => self.lift1(v, |v| {
                        num(&v)
                            .map(|n| Value::Number(-n))
                            .unwrap_or_else(Value::Error)
                    }),
                    UnOp::Plus => self.deref(v),
                    UnOp::Percent => self.lift1(v, |v| {
                        num(&v)
                            .map(|n| Value::Number(n / 100.0))
                            .unwrap_or_else(Value::Error)
                    }),
                }
            }
            Expr::Binary(BinOp::Range, l, r) => {
                let (a, b) = (self.eval(l), self.eval(r));
                match (a, b) {
                    (Value::Ref(a), Value::Ref(b)) if a.sheet == b.sheet => Value::Ref(RangeRef {
                        sheet: a.sheet,
                        rect: Rect {
                            r0: a.rect.r0.min(b.rect.r0),
                            c0: a.rect.c0.min(b.rect.c0),
                            r1: a.rect.r1.max(b.rect.r1),
                            c1: a.rect.c1.max(b.rect.c1),
                        },
                    }),
                    (Value::Error(e), _) | (_, Value::Error(e)) => Value::Error(e),
                    _ => Value::Error(ErrorKind::Value),
                }
            }
            Expr::Binary(op, l, r) => {
                let a = self.eval(l);
                let b = self.eval(r);
                let (a, b) = (self.deref(a), self.deref(b));
                broadcast2(&a, &b, |x, y| binop(*op, x, y))
            }
            Expr::Call(name, args) => crate::functions::call(self, name, args),
            Expr::Invoke(f, args) => {
                let fv = self.eval(f);
                let vals: Vec<Value> = args.iter().map(|a| self.eval(a)).collect();
                self.invoke(&fv, vals)
            }
            Expr::Spill(x) => match self.eval(x) {
                Value::Ref(r) if r.rect.rows() == 1 && r.rect.cols() == 1 => {
                    let anchor = r.rect.start();
                    match self.wb.sheets[r.sheet].spills.get(&anchor) {
                        Some(rect) => Value::Ref(RangeRef {
                            sheet: r.sheet,
                            rect: *rect,
                        }),
                        None if self.wb.sheets[r.sheet].cells.contains_key(&anchor) => {
                            Value::Ref(r)
                        }
                        None => Value::Error(ErrorKind::Ref),
                    }
                }
                Value::Error(e) => Value::Error(e),
                _ => Value::Error(ErrorKind::Ref),
            },
            Expr::Structured { table, spec } => self
                .structured(table.as_deref(), spec)
                .map(Value::Ref)
                .unwrap_or(Value::Error(ErrorKind::Ref)),
        }
    }

    pub fn resolve_ref(&self, r: &RefExpr) -> Option<RangeRef> {
        let sheet = match &r.sheet {
            Some(s) => self.wb.sheet_index(s)?,
            None => self.sheet,
        };
        let rect = match &r.kind {
            RefKind::Cell(a) => Rect::cell(a.cell),
            RefKind::Range(a, b) => Rect::new(a.cell, b.cell),
            RefKind::Cols(a, b) => Rect {
                r0: 0,
                r1: MAX_ROWS - 1,
                c0: a.cell.col.min(b.cell.col),
                c1: a.cell.col.max(b.cell.col),
            },
            RefKind::Rows(a, b) => Rect {
                r0: a.cell.row.min(b.cell.row),
                r1: a.cell.row.max(b.cell.row),
                c0: 0,
                c1: MAX_COLS - 1,
            },
        };
        Some(RangeRef { sheet, rect })
    }

    fn structured(&self, table: Option<&str>, spec: &str) -> Option<RangeRef> {
        let (sheet, t) = match table {
            Some(name) => self.wb.find_table(name)?,
            None => (self.sheet, self.wb.sheets[self.sheet].table_at(self.cell)?),
        };
        let data = Rect {
            r0: t.rect.r0 + t.header_row as u32,
            r1: t.rect.r1 - t.totals_row as u32,
            ..t.rect
        };
        let col_rect = |name: &str, base: Rect| -> Option<Rect> {
            let name = name
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .replace("'", "");
            let i = t
                .columns
                .iter()
                .position(|c| c.eq_ignore_ascii_case(&name))? as u32;
            Some(Rect {
                c0: t.rect.c0 + i,
                c1: t.rect.c0 + i,
                ..base
            })
        };
        let spec = spec.trim();
        let rect = if spec.is_empty() || spec.eq_ignore_ascii_case("#Data") {
            data
        } else if spec.eq_ignore_ascii_case("#All") {
            t.rect
        } else if spec.eq_ignore_ascii_case("#Headers") {
            Rect {
                r1: t.rect.r0,
                ..t.rect
            }
        } else if spec.eq_ignore_ascii_case("#Totals") {
            Rect {
                r0: t.rect.r1,
                ..t.rect
            }
        } else if let Some(col) = spec.strip_prefix('@') {
            let row = self.cell.row;
            let r = col_rect(col, data)?;
            Rect {
                r0: row,
                r1: row,
                ..r
            }
        } else if spec.contains("#This Row") {
            let col = spec.rsplit(',').next()?;
            let r = col_rect(col, data)?;
            Rect {
                r0: self.cell.row,
                r1: self.cell.row,
                ..r
            }
        } else if let Some((a, b)) = spec.split_once("]:[") {
            let ra = col_rect(a, data)?;
            let rb = col_rect(b, data)?;
            Rect {
                c0: ra.c0,
                c1: rb.c1,
                ..data
            }
        } else {
            col_rect(spec, data)?
        };
        Some(RangeRef { sheet, rect })
    }

    fn name(&mut self, n: &str) -> Value {
        if let Some((_, v)) = self
            .env
            .iter()
            .rev()
            .find(|(k, _)| k.eq_ignore_ascii_case(n))
        {
            return v.clone();
        }
        if let Some(e) = self.wb.name_expr(n, self.sheet) {
            if self.depth > 64 {
                return Value::Error(ErrorKind::Calc);
            }
            self.depth += 1;
            let v = self.eval(&e);
            self.depth -= 1;
            return v;
        }
        Value::Error(ErrorKind::Name)
    }

    /// Call a lambda value with arguments.
    pub fn invoke(&mut self, f: &Value, args: Vec<Value>) -> Value {
        let Value::Lambda(l) = f else {
            return match f {
                Value::Error(e) => Value::Error(*e),
                _ => Value::Error(ErrorKind::Value),
            };
        };
        if args.len() > l.params.len() {
            return Value::Error(ErrorKind::Value);
        }
        if self.depth > 256 {
            return Value::Error(ErrorKind::Num);
        }
        let saved = std::mem::replace(&mut self.env, l.env.as_ref().clone());
        for (i, p) in l.params.iter().enumerate() {
            let v = args.get(i).cloned().unwrap_or(Value::Empty);
            self.env.push((p.clone(), v));
        }
        self.depth += 1;
        let v = self.eval(&l.body);
        self.depth -= 1;
        self.env = saved;
        v
    }

    pub fn make_lambda(&self, params: Vec<String>, body: &Expr) -> Value {
        Value::Lambda(Arc::new(Lambda {
            params,
            body: Arc::new(body.clone()),
            env: Arc::new(self.env.clone()),
        }))
    }

    fn implicit(&mut self, v: Value) -> Value {
        match v {
            Value::Ref(r) if r.rect.rows() == 1 && r.rect.cols() == 1 => {
                self.wb.value_at(r.sheet, r.rect.start())
            }
            Value::Ref(r) => {
                // intersect with the formula's row or column
                if r.rect.cols() == 1 && r.rect.r0 <= self.cell.row && self.cell.row <= r.rect.r1 {
                    self.wb
                        .value_at(r.sheet, CellRef::new(self.cell.row, r.rect.c0))
                } else if r.rect.rows() == 1
                    && r.rect.c0 <= self.cell.col
                    && self.cell.col <= r.rect.c1
                {
                    self.wb
                        .value_at(r.sheet, CellRef::new(r.rect.r0, self.cell.col))
                } else {
                    Value::Error(ErrorKind::Value)
                }
            }
            Value::Array(a) => a.data.first().cloned().unwrap_or_default(),
            other => other,
        }
    }

    /// Clip whole-column/row references to the sheet's used area.
    pub fn trim_rect(&self, r: &RangeRef) -> Rect {
        let mut rect = r.rect;
        if rect.size() > 1_000_000 {
            if let Some(used) = self.wb.sheets[r.sheet].used_rect() {
                rect.r1 = rect.r1.min(used.r1.max(rect.r0));
                rect.c1 = rect.c1.min(used.c1.max(rect.c0));
            } else {
                rect.r1 = rect.r0;
                rect.c1 = rect.c0;
            }
        }
        rect
    }

    /// Materialise a reference into a value (scalar for a single cell).
    pub fn deref(&self, v: Value) -> Value {
        match v {
            Value::Ref(r) => {
                let rect = self.trim_rect(&r);
                if rect.rows() == 1 && rect.cols() == 1 {
                    return self.wb.value_at(r.sheet, rect.start());
                }
                Value::Array(Arc::new(self.range_array(r.sheet, &rect)))
            }
            other => other,
        }
    }

    pub fn range_array(&self, sheet: usize, rect: &Rect) -> Array {
        let mut data = Vec::with_capacity(rect.size() as usize);
        for row in rect.r0..=rect.r1 {
            for col in rect.c0..=rect.c1 {
                data.push(self.wb.value_at(sheet, CellRef::new(row, col)));
            }
        }
        Array::new(rect.rows() as usize, rect.cols() as usize, data)
    }

    /// Any value as an array (scalars become 1×1).
    pub fn array(&self, v: Value) -> Arc<Array> {
        match self.deref(v) {
            Value::Array(a) => a,
            s => Arc::new(Array::new(1, 1, vec![s])),
        }
    }

    /// Apply a scalar function element-wise if `v` is an array.
    pub fn lift1(&self, v: Value, f: impl Fn(Value) -> Value) -> Value {
        match self.deref(v) {
            Value::Array(a) => Value::array(Array::new(
                a.rows,
                a.cols,
                a.data.iter().cloned().map(f).collect(),
            )),
            s => f(s),
        }
    }
}

// ---- coercions --------------------------------------------------------------------

pub fn num(v: &Value) -> Res<f64> {
    match v {
        Value::Number(n) => Ok(*n),
        Value::Bool(b) => Ok(*b as i32 as f64),
        Value::Empty => Ok(0.0),
        Value::Text(t) => parse_number(t.trim())
            .or_else(|| crate::functions::date::parse_date_text(t))
            .ok_or(ErrorKind::Value),
        Value::Error(e) => Err(*e),
        Value::Array(a) => a.data.first().map_or(Ok(0.0), num),
        _ => Err(ErrorKind::Value),
    }
}

pub fn text(v: &Value) -> Res<String> {
    match v {
        Value::Error(e) => Err(*e),
        Value::Array(a) => a.data.first().map_or(Ok(String::new()), text),
        Value::Lambda(_) => Err(ErrorKind::Calc),
        other => Ok(other.to_string()),
    }
}

pub fn truthy(v: &Value) -> Res<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::Number(n) => Ok(*n != 0.0),
        Value::Empty => Ok(false),
        Value::Text(t) if t.eq_ignore_ascii_case("TRUE") => Ok(true),
        Value::Text(t) if t.eq_ignore_ascii_case("FALSE") => Ok(false),
        Value::Error(e) => Err(*e),
        Value::Array(a) => a.data.first().map_or(Ok(false), truthy),
        _ => Err(ErrorKind::Value),
    }
}

/// Excel ordering: numbers < text < booleans; text is case-insensitive.
pub fn compare(a: &Value, b: &Value) -> Ordering {
    let norm = |v: &Value| match v {
        Value::Empty => Value::Number(0.0),
        other => other.clone(),
    };
    let (a, b) = match (a, b) {
        (Value::Empty, Value::Text(_)) => (Value::text(""), b.clone()),
        (Value::Text(_), Value::Empty) => (a.clone(), Value::text("")),
        _ => (norm(a), norm(b)),
    };
    match (&a, &b) {
        (Value::Number(x), Value::Number(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (Value::Text(x), Value::Text(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        _ => a.type_rank().cmp(&b.type_rank()),
    }
}

fn binop(op: BinOp, a: &Value, b: &Value) -> Value {
    if let Value::Error(e) = a {
        return Value::Error(*e);
    }
    if let Value::Error(e) = b {
        return Value::Error(*e);
    }
    let arith = |f: fn(f64, f64) -> Option<f64>, err: ErrorKind| match (num(a), num(b)) {
        (Ok(x), Ok(y)) => f(x, y)
            .filter(|r| r.is_finite())
            .map(Value::Number)
            .unwrap_or(Value::Error(err)),
        (Err(e), _) | (_, Err(e)) => Value::Error(e),
    };
    match op {
        BinOp::Add => arith(|x, y| Some(x + y), ErrorKind::Num),
        BinOp::Sub => arith(|x, y| Some(x - y), ErrorKind::Num),
        BinOp::Mul => arith(|x, y| Some(x * y), ErrorKind::Num),
        BinOp::Div => arith(|x, y| (y != 0.0).then(|| x / y), ErrorKind::Div0),
        BinOp::Pow => match (num(a), num(b)) {
            (Ok(x), Ok(y)) if x == 0.0 && y == 0.0 => Value::Error(ErrorKind::Num),
            (Ok(x), Ok(y)) if x == 0.0 && y < 0.0 => Value::Error(ErrorKind::Div0),
            _ => arith(|x, y| Some(x.powf(y)), ErrorKind::Num),
        },
        BinOp::Concat => match (text(a), text(b)) {
            (Ok(x), Ok(y)) => Value::text(x + &y),
            (Err(e), _) | (_, Err(e)) => Value::Error(e),
        },
        BinOp::Eq => Value::Bool(compare(a, b) == Ordering::Equal),
        BinOp::Ne => Value::Bool(compare(a, b) != Ordering::Equal),
        BinOp::Lt => Value::Bool(compare(a, b) == Ordering::Less),
        BinOp::Le => Value::Bool(compare(a, b) != Ordering::Greater),
        BinOp::Gt => Value::Bool(compare(a, b) == Ordering::Greater),
        BinOp::Ge => Value::Bool(compare(a, b) != Ordering::Less),
        BinOp::Range => Value::Error(ErrorKind::Value),
    }
}

/// Element-wise binary operation with Excel's broadcasting rules: scalars
/// expand, single rows/columns stretch, mismatched cells become #N/A.
pub fn broadcast2(a: &Value, b: &Value, f: impl Fn(&Value, &Value) -> Value) -> Value {
    match (a, b) {
        (Value::Array(x), Value::Array(y)) => {
            let rows = if x.rows == 1 {
                y.rows
            } else if y.rows == 1 {
                x.rows
            } else {
                x.rows.max(y.rows)
            };
            let cols = if x.cols == 1 {
                y.cols
            } else if y.cols == 1 {
                x.cols
            } else {
                x.cols.max(y.cols)
            };
            let pick = |m: &Array, r: usize, c: usize| -> Option<Value> {
                let rr = if m.rows == 1 { 0 } else { r };
                let cc = if m.cols == 1 { 0 } else { c };
                (rr < m.rows && cc < m.cols).then(|| m.get(rr, cc).clone())
            };
            let mut data = Vec::with_capacity(rows * cols);
            for r in 0..rows {
                for c in 0..cols {
                    data.push(match (pick(x, r, c), pick(y, r, c)) {
                        (Some(p), Some(q)) => f(&p, &q),
                        _ => Value::Error(ErrorKind::NA),
                    });
                }
            }
            Value::array(Array::new(rows, cols, data))
        }
        (Value::Array(x), s) => Value::array(Array::new(
            x.rows,
            x.cols,
            x.data.iter().map(|v| f(v, s)).collect(),
        )),
        (s, Value::Array(y)) => Value::array(Array::new(
            y.rows,
            y.cols,
            y.data.iter().map(|v| f(s, v)).collect(),
        )),
        _ => f(a, b),
    }
}
