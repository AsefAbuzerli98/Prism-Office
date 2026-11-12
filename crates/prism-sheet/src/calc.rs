//! Dependency graph and incremental, level-parallel recalculation with
//! dynamic-array spilling.
//!
//! * Every formula cell registers the rectangles it reads (static analysis of
//!   its expression, defined names and structured references) in a spatial
//!   bucket index, so "who reads this cell?" is cheap.
//! * An edit marks positions pending; `recalc` computes the dirty closure,
//!   orders it into topological levels (Kahn) and evaluates each level —
//!   in parallel on native targets — before committing values and spills.
//! * Cells left in a cycle get the value 0 and are reported in
//!   `Workbook::circular`, as Excel does without iterative calculation.
//! * Spill rectangles that grow or shrink trigger another round so readers of
//!   newly covered/uncovered cells and blocked neighbours update.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::addr::{CellRef, Rect, MAX_COLS, MAX_ROWS};
use crate::eval::Ctx;
use crate::functions::VOLATILE;
use crate::parser::{Expr, RefKind};
use crate::value::{ErrorKind, Value};
use crate::workbook::{AsyncRequest, Input, Workbook};

pub type Node = (usize, CellRef);

const RB: u32 = 256;
const CB: u32 = 16;
const MAX_BUCKETS: u64 = 1024;

#[derive(Clone, Debug, Default)]
pub struct Graph {
    deps: HashMap<Node, Vec<(usize, Rect)>>,
    buckets: HashMap<(usize, u32, u32), Vec<(Node, Rect)>>,
    large: Vec<(Node, usize, Rect)>,
    volatile: BTreeSet<Node>,
}

fn bucket_span(r: &Rect) -> (u32, u32, u32, u32, u64) {
    let (rb0, rb1, cb0, cb1) = (r.r0 / RB, r.r1 / RB, r.c0 / CB, r.c1 / CB);
    (
        rb0,
        rb1,
        cb0,
        cb1,
        (rb1 - rb0 + 1) as u64 * (cb1 - cb0 + 1) as u64,
    )
}

impl Graph {
    fn remove(&mut self, n: Node) {
        self.volatile.remove(&n);
        let Some(deps) = self.deps.remove(&n) else {
            return;
        };
        for (s, r) in deps {
            let (rb0, rb1, cb0, cb1, count) = bucket_span(&r);
            if count > MAX_BUCKETS {
                self.large.retain(|(m, _, _)| *m != n);
                continue;
            }
            for rb in rb0..=rb1 {
                for cb in cb0..=cb1 {
                    if let Some(v) = self.buckets.get_mut(&(s, rb, cb)) {
                        v.retain(|(m, _)| *m != n);
                        if v.is_empty() {
                            self.buckets.remove(&(s, rb, cb));
                        }
                    }
                }
            }
        }
    }

    fn insert(&mut self, n: Node, deps: Vec<(usize, Rect)>, volatile: bool) {
        if volatile {
            self.volatile.insert(n);
        }
        for (s, r) in &deps {
            let (rb0, rb1, cb0, cb1, count) = bucket_span(r);
            if count > MAX_BUCKETS {
                self.large.push((n, *s, *r));
                continue;
            }
            for rb in rb0..=rb1 {
                for cb in cb0..=cb1 {
                    self.buckets.entry((*s, rb, cb)).or_default().push((n, *r));
                }
            }
        }
        self.deps.insert(n, deps);
    }

    /// Formula cells whose precedents intersect `r` on sheet `s`.
    pub fn dependents(&self, s: usize, r: &Rect) -> Vec<Node> {
        let mut out = Vec::new();
        let (rb0, rb1, cb0, cb1, count) = bucket_span(r);
        if count > 65_536 {
            for (n, deps) in &self.deps {
                if deps.iter().any(|(ds, dr)| *ds == s && dr.intersects(r)) {
                    out.push(*n);
                }
            }
        } else {
            for rb in rb0..=rb1 {
                for cb in cb0..=cb1 {
                    if let Some(v) = self.buckets.get(&(s, rb, cb)) {
                        out.extend(v.iter().filter(|(_, dr)| dr.intersects(r)).map(|(n, _)| *n));
                    }
                }
            }
        }
        out.extend(
            self.large
                .iter()
                .filter(|(_, ds, dr)| *ds == s && dr.intersects(r))
                .map(|(n, _, _)| *n),
        );
        out.sort_unstable();
        out.dedup();
        out
    }

    pub fn precedents(&self, n: Node) -> &[(usize, Rect)] {
        self.deps.get(&n).map_or(&[], |v| v.as_slice())
    }
}

/// Static precedents of an expression.
fn collect(
    wb: &Workbook,
    sheet: usize,
    cell: CellRef,
    e: &Expr,
    out: &mut Vec<(usize, Rect)>,
    volatile: &mut bool,
    depth: u32,
) {
    if depth > 32 {
        return;
    }
    let rec = |x: &Expr, out: &mut Vec<(usize, Rect)>, v: &mut bool| {
        collect(wb, sheet, cell, x, out, v, depth + 1)
    };
    match e {
        Expr::Ref(r) => {
            let s = match &r.sheet {
                Some(n) => match wb.sheet_index(n) {
                    Some(i) => i,
                    None => return,
                },
                None => sheet,
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
            out.push((s, rect));
        }
        Expr::Name(n) => {
            if let Some(x) = wb.name_expr(n, sheet) {
                rec(&x, out, volatile);
            }
        }
        Expr::Structured { table, .. } => {
            let t = match table {
                Some(name) => wb.find_table(name),
                None => wb.sheets[sheet].table_at(cell).map(|t| (sheet, t)),
            };
            if let Some((s, t)) = t {
                out.push((s, t.rect));
            }
        }
        Expr::Spill(x) => {
            rec(x, out, volatile);
            if let Expr::Ref(r) = x.as_ref() {
                if let RefKind::Cell(a) = &r.kind {
                    let s = r
                        .sheet
                        .as_ref()
                        .and_then(|n| wb.sheet_index(n))
                        .unwrap_or(sheet);
                    if let Some(sp) = wb.sheets.get(s).and_then(|sh| sh.spills.get(&a.cell)) {
                        out.push((s, *sp));
                    }
                }
            }
        }
        Expr::Unary(_, x) => rec(x, out, volatile),
        Expr::Binary(_, a, b) => {
            rec(a, out, volatile);
            rec(b, out, volatile);
        }
        Expr::Array(rows) => rows.iter().flatten().for_each(|x| rec(x, out, volatile)),
        Expr::Call(name, args) => {
            if VOLATILE.contains(&name.as_str()) {
                *volatile = true;
            }
            if name == "PY" {
                if let Some(Expr::Text(code)) = args.first() {
                    for r in crate::functions::logic::xl_refs(code) {
                        if let Ok(x) = crate::parser::parse(&r, cell) {
                            rec(&x, out, volatile);
                        }
                    }
                }
            }
            args.iter().for_each(|x| rec(x, out, volatile));
        }
        Expr::Invoke(f, args) => {
            rec(f, out, volatile);
            args.iter().for_each(|x| rec(x, out, volatile));
        }
        _ => {}
    }
}

/// Refresh the graph entry of one cell after its input changed.
pub(crate) fn reindex(wb: &mut Workbook, sheet: usize, c: CellRef) {
    let n = (sheet, c);
    wb.graph.remove(n);
    let Some(Input::Formula { expr, .. }) = wb.sheets[sheet].cells.get(&c).map(|x| &x.input) else {
        return;
    };
    let expr = expr.clone();
    let mut deps = Vec::new();
    let mut volatile = false;
    collect(wb, sheet, c, &expr, &mut deps, &mut volatile, 0);
    deps.sort_by_key(|(s, r)| (*s, r.r0, r.c0, r.r1, r.c1));
    deps.dedup();
    wb.graph.insert(n, deps, volatile);
}

/// Result of a recalculation.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Recalc {
    /// Regions whose displayed values may have changed.
    pub changed: Vec<(usize, Rect)>,
    /// Number of formula evaluations performed.
    pub evaluated: usize,
}

fn finalize(ctx: &Ctx, v: Value) -> Value {
    match ctx.deref(v) {
        Value::Lambda(_) => Value::Error(ErrorKind::Calc),
        Value::Array(a) if a.data.is_empty() => Value::Error(ErrorKind::Calc),
        Value::Array(a) if a.data.len() == 1 => a.data[0].clone(),
        Value::Ref(_) => Value::Error(ErrorKind::Value),
        v => v,
    }
}

fn eval_node(wb: &Workbook, n: Node, now: f64) -> (Node, Value, Vec<AsyncRequest>) {
    let Some(Input::Formula { expr, .. }) = wb.sheets[n.0].cells.get(&n.1).map(|c| &c.input) else {
        return (n, Value::Empty, vec![]);
    };
    let mut ctx = Ctx::new(wb, n.0, n.1, now);
    let v = ctx.eval(expr);
    let v = finalize(&ctx, v);
    (n, v, ctx.requests)
}

fn outputs(wb: &Workbook, n: Node) -> Vec<Rect> {
    let s = &wb.sheets[n.0];
    let mut v = vec![Rect::cell(n.1)];
    v.extend(s.spills.get(&n.1).copied());
    v
}

impl Workbook {
    /// Mark every formula dirty (after load or name changes).
    pub fn mark_all_dirty(&mut self) {
        self.reparse_names();
        let all: Vec<Node> = self
            .sheets
            .iter()
            .enumerate()
            .flat_map(|(i, s)| {
                s.cells
                    .iter()
                    .filter(|(_, c)| matches!(c.input, Input::Formula { .. }))
                    .map(move |(k, _)| (i, *k))
            })
            .collect();
        for n in &all {
            reindex(self, n.0, n.1);
        }
        self.pending.extend(all);
    }

    /// Feed the answer to an asynchronous request (e.g. `=PY`) and mark the cell dirty.
    pub fn supply_async(&mut self, key: &str, sheet: usize, c: CellRef, v: Value) {
        self.async_results.insert((sheet, c), (key.to_string(), v));
        self.async_requests
            .retain(|r| !(r.sheet == sheet && r.row == c.row && r.col == c.col));
        self.pending.insert((sheet, c));
    }

    /// Recalculate everything affected by pending edits (plus volatile cells).
    pub fn recalc(&mut self, now: f64) -> Recalc {
        let mut seeds: Vec<(usize, Rect)> = std::mem::take(&mut self.pending)
            .into_iter()
            .map(|(s, c)| (s, Rect::cell(c)))
            .collect();
        let mut extra_nodes: Vec<Node> = self.graph.volatile.iter().copied().collect();
        let mut report = Recalc::default();
        self.circular.clear();
        for _round in 0..8 {
            if seeds.is_empty() && extra_nodes.is_empty() {
                break;
            }
            let (next, next_nodes) = self.round(&seeds, &extra_nodes, now, &mut report);
            seeds = next;
            extra_nodes = next_nodes;
        }
        report
            .changed
            .sort_by_key(|(s, r)| (*s, r.r0, r.c0, r.r1, r.c1));
        report.changed.dedup();
        report
    }

    /// One closure → order → evaluate → commit pass. Returns follow-up seeds.
    fn round(
        &mut self,
        seeds: &[(usize, Rect)],
        extra: &[Node],
        now: f64,
        report: &mut Recalc,
    ) -> (Vec<(usize, Rect)>, Vec<Node>) {
        let mut nodes: HashSet<Node> = HashSet::new();
        let mut edges: HashSet<(Node, Node)> = HashSet::new();
        let mut queue: Vec<(usize, Rect, Option<Node>)> = Vec::new();
        let is_formula = |wb: &Workbook, n: Node| {
            matches!(
                wb.sheets[n.0].cells.get(&n.1).map(|c| &c.input),
                Some(Input::Formula { .. })
            )
        };
        for &(s, r) in seeds {
            report.changed.push((s, r));
            // edits inside a spill (or a blocked spill area) re-evaluate its anchor
            let sh = &self.sheets[s];
            for (a, sr) in sh.spills.iter().chain(sh.blocked.iter()) {
                if sr.intersects(&r) && nodes.insert((s, *a)) {
                    queue.push((s, Rect::cell(*a), Some((s, *a))));
                    for o in outputs(self, (s, *a)) {
                        queue.push((s, o, Some((s, *a))));
                    }
                }
            }
            if r.rows() == 1 && r.cols() == 1 && is_formula(self, (s, r.start())) {
                let n = (s, r.start());
                if nodes.insert(n) {
                    for o in outputs(self, n) {
                        queue.push((s, o, Some(n)));
                    }
                }
            } else {
                queue.push((s, r, None));
            }
        }
        for &n in extra {
            if is_formula(self, n) && nodes.insert(n) {
                for o in outputs(self, n) {
                    queue.push((n.0, o, Some(n)));
                }
            }
        }
        while let Some((s, r, from)) = queue.pop() {
            for d in self.graph.dependents(s, &r) {
                if let Some(f) = from {
                    edges.insert((f, d));
                }
                if nodes.insert(d) {
                    for o in outputs(self, d) {
                        queue.push((d.0, o, Some(d)));
                    }
                }
            }
        }
        // Kahn levels
        let mut indeg: HashMap<Node, usize> = nodes.iter().map(|n| (*n, 0)).collect();
        let mut adj: HashMap<Node, Vec<Node>> = HashMap::new();
        for (a, b) in &edges {
            if nodes.contains(a) && nodes.contains(b) {
                *indeg.get_mut(b).unwrap() += 1;
                adj.entry(*a).or_default().push(*b);
            }
        }
        let mut level: Vec<Node> = indeg
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(n, _)| *n)
            .collect();
        level.sort_unstable();
        let mut done = 0usize;
        let mut follow: Vec<(usize, Rect)> = Vec::new();
        while !level.is_empty() {
            let results = self.eval_level(&level, now);
            report.evaluated += results.len();
            for (n, v, reqs) in results {
                self.async_requests
                    .retain(|r| !(r.sheet == n.0 && r.row == n.1.row && r.col == n.1.col));
                self.async_requests.extend(reqs);
                let (old, new) = self.commit(n, v);
                report.changed.push((n.0, Rect::cell(n.1)));
                report
                    .changed
                    .extend(old.iter().chain(new.iter()).map(|r| (n.0, *r)));
                if old != new {
                    // `A1#` readers registered the previous spill rectangle
                    for d in self.graph.dependents(n.0, &Rect::cell(n.1)) {
                        reindex(self, d.0, d.1);
                    }
                    // readers of uncovered/newly covered cells and neighbours blocked there
                    for r in old.iter().chain(new.iter()) {
                        follow.push((n.0, *r));
                    }
                }
            }
            done += level.len();
            let mut next = Vec::new();
            for n in &level {
                if let Some(v) = adj.get(n) {
                    for m in v {
                        let d = indeg.get_mut(m).unwrap();
                        *d -= 1;
                        if *d == 0 {
                            next.push(*m);
                        }
                    }
                }
            }
            next.sort_unstable();
            level = next;
        }
        if done < nodes.len() {
            // cycle: Excel shows 0 for unresolved circular references
            let mut stuck: Vec<Node> = indeg
                .into_iter()
                .filter(|(_, d)| *d > 0)
                .map(|(n, _)| n)
                .collect();
            stuck.sort_unstable();
            for n in stuck {
                self.commit(n, Value::Number(0.0));
                report.changed.push((n.0, Rect::cell(n.1)));
                self.circular.push(n);
            }
        }
        // follow-up seeds: cells whose spill coverage changed, minus the anchors themselves
        let mut next_nodes = Vec::new();
        for (s, r) in follow {
            let sh = &self.sheets[s];
            for (a, br) in sh.blocked.iter() {
                if br.intersects(&r) {
                    next_nodes.push((s, *a));
                }
            }
            for d in self.graph.dependents(s, &r) {
                if !next_nodes.contains(&d) {
                    next_nodes.push(d);
                }
            }
        }
        // avoid ping-pong on the same anchors: only re-run nodes whose inputs moved
        next_nodes.sort_unstable();
        next_nodes.dedup();
        (Vec::new(), next_nodes)
    }

    fn eval_level(&self, level: &[Node], now: f64) -> Vec<(Node, Value, Vec<AsyncRequest>)> {
        #[cfg(not(target_arch = "wasm32"))]
        if level.len() >= 64 {
            use rayon::prelude::*;
            return level.par_iter().map(|n| eval_node(self, *n, now)).collect();
        }
        level.iter().map(|n| eval_node(self, *n, now)).collect()
    }

    /// Store a computed value, spilling arrays. Returns (old, new) spill rects.
    fn commit(&mut self, n: Node, v: Value) -> (Option<Rect>, Option<Rect>) {
        let (s, c) = n;
        let sh = &mut self.sheets[s];
        let old = sh.spills.get(&c).copied();
        let mut value = v;
        let mut new = None;
        Workbook::drop_spill(sh, c);
        if let Value::Array(a) = &value {
            let (r1, c1) = (
                c.row as u64 + a.rows as u64 - 1,
                c.col as u64 + a.cols as u64 - 1,
            );
            let rect = Rect {
                r0: c.row,
                c0: c.col,
                r1: r1.min(MAX_ROWS as u64 - 1) as u32,
                c1: c1.min(MAX_COLS as u64 - 1) as u32,
            };
            let off_sheet = r1 >= MAX_ROWS as u64 || c1 >= MAX_COLS as u64;
            let blocked = off_sheet
                || sh
                    .spills
                    .iter()
                    .any(|(k, r)| *k != c && r.intersects(&rect))
                || {
                    let mut hit = false;
                    if (sh.cells.len() as u64) < rect.rows() as u64 * 4 {
                        hit = sh.cells.iter().any(|(k, cell)| {
                            *k != c
                                && rect.contains(*k)
                                && !(matches!(cell.input, Input::Value(Value::Empty)))
                        });
                    } else {
                        'rows: for row in rect.r0..=rect.r1 {
                            for (k, cell) in sh
                                .cells
                                .range(CellRef::new(row, rect.c0)..=CellRef::new(row, rect.c1))
                            {
                                if *k != c && !matches!(cell.input, Input::Value(Value::Empty)) {
                                    hit = true;
                                    break 'rows;
                                }
                            }
                        }
                    }
                    hit
                };
            if blocked {
                sh.blocked.insert(c, rect);
                value = Value::Error(ErrorKind::Spill);
            } else {
                for row in rect.r0..=rect.r1 {
                    for col in rect.c0..=rect.c1 {
                        let k = CellRef::new(row, col);
                        if k != c {
                            sh.spill_owner.insert(k, c);
                        }
                    }
                }
                sh.spills.insert(c, rect);
                new = Some(rect);
            }
        }
        if let Some(cell) = sh.cells.get_mut(&c) {
            cell.value = value;
        }
        (old, new)
    }

    /// Convenience: set input by A1 address and recalculate.
    pub fn set(
        &mut self,
        sheet: usize,
        a1: &str,
        text: &str,
        now: f64,
    ) -> Result<Recalc, crate::parser::ParseError> {
        let c =
            crate::addr::parse_a1(a1)
                .map(|x| x.0)
                .ok_or_else(|| crate::parser::ParseError {
                    pos: 0,
                    msg: format!("bad address {a1}"),
                })?;
        self.put_input(sheet, c, text)?;
        Ok(self.recalc(now))
    }

    /// Displayed value by A1 address.
    pub fn get(&self, sheet: usize, a1: &str) -> Value {
        crate::addr::parse_a1(a1)
            .map(|x| self.value_at(sheet, x.0))
            .unwrap_or(Value::Error(ErrorKind::Ref))
    }
}
