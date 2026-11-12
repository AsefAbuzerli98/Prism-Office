//! Cell values, errors and arrays.

use std::fmt;
use std::sync::Arc as Rc;

use serde::{Deserialize, Serialize};

use crate::addr::Rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorKind {
    Null,
    Div0,
    Value,
    Ref,
    Name,
    Num,
    NA,
    Spill,
    Calc,
    /// Waiting for an asynchronous result (e.g. `=PY`).
    Busy,
}

impl ErrorKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::Null => "#NULL!",
            Self::Div0 => "#DIV/0!",
            Self::Value => "#VALUE!",
            Self::Ref => "#REF!",
            Self::Name => "#NAME?",
            Self::Num => "#NUM!",
            Self::NA => "#N/A",
            Self::Spill => "#SPILL!",
            Self::Calc => "#CALC!",
            Self::Busy => "#BUSY!",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_uppercase().as_str() {
            "#NULL!" => Self::Null,
            "#DIV/0!" => Self::Div0,
            "#VALUE!" => Self::Value,
            "#REF!" => Self::Ref,
            "#NAME?" => Self::Name,
            "#NUM!" => Self::Num,
            "#N/A" => Self::NA,
            "#SPILL!" => Self::Spill,
            "#CALC!" => Self::Calc,
            "#BUSY!" => Self::Busy,
            _ => return None,
        })
    }
}

/// A 2-D array of scalar values (row-major).
#[derive(Clone, Debug, PartialEq)]
pub struct Array {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<Value>,
}

impl Array {
    pub fn new(rows: usize, cols: usize, data: Vec<Value>) -> Self {
        debug_assert_eq!(rows * cols, data.len());
        Self { rows, cols, data }
    }
    pub fn filled(rows: usize, cols: usize, v: Value) -> Self {
        Self {
            rows,
            cols,
            data: vec![v; rows * cols],
        }
    }
    pub fn get(&self, r: usize, c: usize) -> &Value {
        &self.data[r * self.cols + c]
    }
    pub fn column(values: Vec<Value>) -> Self {
        Self {
            rows: values.len(),
            cols: 1,
            data: values,
        }
    }
    pub fn row(values: Vec<Value>) -> Self {
        Self {
            rows: 1,
            cols: values.len(),
            data: values,
        }
    }
    pub fn row_values(&self, r: usize) -> &[Value] {
        &self.data[r * self.cols..(r + 1) * self.cols]
    }
    pub fn transpose(&self) -> Array {
        let mut d = Vec::with_capacity(self.data.len());
        for c in 0..self.cols {
            for r in 0..self.rows {
                d.push(self.get(r, c).clone());
            }
        }
        Array::new(self.cols, self.rows, d)
    }
}

/// A sheet range referenced by a formula (resolved lazily).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RangeRef {
    pub sheet: usize,
    pub rect: Rect,
}

/// A user-defined function value (`LAMBDA`).
#[derive(Clone, Debug)]
pub struct Lambda {
    pub params: Vec<String>,
    pub body: Rc<crate::parser::Expr>,
    /// Captured LET/LAMBDA bindings.
    pub env: Rc<Vec<(String, Value)>>,
}

impl PartialEq for Lambda {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.body, &other.body)
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
    #[default]
    Empty,
    Number(f64),
    Text(Rc<str>),
    Bool(bool),
    Error(ErrorKind),
    Array(Rc<Array>),
    /// Unresolved reference (only during evaluation).
    Ref(RangeRef),
    Lambda(Rc<Lambda>),
}

impl Value {
    pub fn text(s: impl Into<String>) -> Self {
        Value::Text(Rc::from(s.into()))
    }
    pub fn array(a: Array) -> Self {
        if a.rows == 1 && a.cols == 1 {
            return a.data.into_iter().next().unwrap();
        }
        Value::Array(Rc::new(a))
    }
    pub fn is_error(&self) -> bool {
        matches!(self, Value::Error(_))
    }
    pub fn type_rank(&self) -> u8 {
        match self {
            Value::Number(_) => 0,
            Value::Text(_) => 1,
            Value::Bool(_) => 2,
            Value::Error(_) => 3,
            _ => 4,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Empty => Ok(()),
            Value::Number(n) => f.write_str(&format_number(*n)),
            Value::Text(s) => f.write_str(s),
            Value::Bool(b) => f.write_str(if *b { "TRUE" } else { "FALSE" }),
            Value::Error(e) => f.write_str(e.code()),
            Value::Array(_) => f.write_str("{array}"),
            Value::Ref(_) => f.write_str("{ref}"),
            Value::Lambda(_) => f.write_str("#CALC!"),
        }
    }
}

/// General number format: up to 15 significant digits, no trailing zeros.
pub fn format_number(n: f64) -> String {
    if n == 0.0 {
        return "0".into();
    }
    if !n.is_finite() {
        return "#NUM!".into();
    }
    let abs = n.abs();
    if !(1e-9..1e15).contains(&abs) {
        let s = format!("{:.10E}", n);
        let (m, e) = s.split_once('E').unwrap();
        let m = m.trim_end_matches('0').trim_end_matches('.');
        let e: i32 = e.parse().unwrap();
        return format!("{m}E{}{:02}", if e < 0 { "-" } else { "+" }, e.abs());
    }
    let digits = 15 - (abs.log10().floor() as i32 + 1);
    let s = format!("{:.*}", digits.clamp(0, 15) as usize, n);
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_format() {
        assert_eq!(format_number(0.1 + 0.2), "0.3");
        assert_eq!(format_number(1234.5), "1234.5");
        assert_eq!(format_number(-2.0), "-2");
        assert_eq!(format_number(1e20), "1E+20");
        assert_eq!(format_number(1.5e-12), "1.5E-12");
        assert_eq!(format_number(123456789012345.0), "123456789012345");
        assert_eq!(format_number(2.0 / 3.0), "0.666666666666667");
    }

    #[test]
    fn errors() {
        for e in [
            ErrorKind::Div0,
            ErrorKind::NA,
            ErrorKind::Spill,
            ErrorKind::Name,
        ] {
            assert_eq!(ErrorKind::parse(e.code()), Some(e));
        }
    }
}
