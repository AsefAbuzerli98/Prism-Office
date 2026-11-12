//! # prism-sheet
//!
//! Spreadsheet model and calculation engine: formula parser, dependency
//! graph with incremental recalculation, dynamic-array spilling, a function
//! library (incl. XLOOKUP regex mode, REGEX*, GROUPBY/PIVOTBY/PERCENTOF/
//! TRIMRANGE, LAMBDA/LET), number formatting and XLSX import/export.

pub mod addr;
pub mod api;
pub mod calc;
pub mod eval;
pub mod functions;
pub mod model;
pub mod numfmt;
pub mod parser;
pub mod value;
#[cfg(feature = "wasm")]
pub mod wasm;
pub mod workbook;
#[cfg(feature = "xlsx")]
pub mod xlsx;

pub use addr::{CellRef, Rect};
pub use parser::{parse, Expr};
pub use value::{Array, ErrorKind, Value};
pub use workbook::Workbook;
