//! # prism-sheet
//!
//! Spreadsheet model and calculation engine: formula parser, dependency
//! graph with incremental recalculation, dynamic-array spilling, a function
//! library (incl. XLOOKUP regex mode, REGEX*, GROUPBY/PIVOTBY/PERCENTOF/
//! TRIMRANGE, LAMBDA/LET), number formatting and XLSX import/export.

pub mod addr;
pub mod api;
pub mod calc;
pub mod chart;
pub mod eval;
pub mod functions;
pub mod fxhash;
pub mod model;
pub mod numfmt;
pub mod parser;
pub mod pivot;
pub mod value;
#[cfg(feature = "wasm")]
pub mod wasm;
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
mod wasm_alloc;
pub mod workbook;
#[cfg(feature = "xlsx")]
pub mod xlsx;

pub use addr::{CellRef, Rect};
pub use parser::{parse, Expr};
pub use value::{Array, ErrorKind, Value};
pub use workbook::Workbook;
