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

use std::collections::BTreeSet;

use crate::fxhash::FxHashMap as HashMap;

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
        self.dependents_into(s, r, &mut out);
        out
    }

    /// [`Graph::dependents`] into a reusable buffer.
    pub fn dependents_into(&self, s: usize, r: &Rect, out: &mut Vec<Node>) {
        let start = out.len();
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
        out[start..].sort_unstable();
        let mut w = start;
        for i in start..out.len() {
            if i == start || out[i] != out[w - 1] {
                out[w] = out[i];
                w += 1;
            }
        }
        out.truncate(w);
    }

    pub fn precedents(&self, n: Node) -> &[(usize, Rect)] {
        self.deps.get(&n).map_or(&[], |v| v.as_slice())
    }
}

/// Static precedents of an expression (`off`: shared-formula shift).
#[allow(clippy::too_many_arguments)]
fn collect(
    wb: &Workbook,
    sheet: usize,
    cell: CellRef,
    e: &Expr,
    out: &mut Vec<(usize, Rect)>,
    volatile: &mut bool,
    depth: u32,
    off: (i64, i64),
) {
    if depth > 32 {
        return;
    }
    let rec = |x: &Expr, out: &mut Vec<(usize, Rect)>, v: &mut bool| {
        collect(wb, sheet, cell, x, out, v, depth + 1, off)
    };
    let rec0 = |x: &Expr, out: &mut Vec<(usize, Rect)>, v: &mut bool| {
        collect(wb, sheet, cell, x, out, v, depth + 1, (0, 0))
    };
    match e {
        Expr::Ref(r) => {
            let mut ctx = Ctx::new(wb, sheet, cell, 0.0);
            ctx.offset = off;
            if let Some(rr) = ctx.resolve_ref(r) {
                out.push((rr.sheet, rr.rect));
            }
        }
        Expr::Name(n) => {
            if let Some(x) = wb.name_expr(n, sheet) {
                rec0(&x, out, volatile);
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
                if let Some(a) = match &r.kind {
                    RefKind::Cell(a) => crate::parser::shift_anchor(a, off.0, off.1),
                    _ => None,
                } {
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
                            rec0(&x, out, volatile);
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
    let Some(Input::Formula { expr, base }) = wb.sheets[sheet].cells.get(&c).map(|x| &x.input)
    else {
        return;
    };
    let (expr, off) = (
        expr.clone(),
        (
            c.row as i64 - base.row as i64,
            c.col as i64 - base.col as i64,
        ),
    );
    let mut deps = Vec::new();
    let mut volatile = false;
    collect(wb, sheet, c, &expr, &mut deps, &mut volatile, 0, off);
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
    let Some(Input::Formula { expr, base }) = wb.sheets[n.0].cells.get(&n.1).map(|c| &c.input)
    else {
        return (n, Value::Empty, vec![]);
    };
    let mut ctx = Ctx::new(wb, n.0, n.1, now);
    ctx.offset = (
        n.1.row as i64 - base.row as i64,
        n.1.col as i64 - base.col as i64,
    );
    let v = ctx.eval(expr);
    let v = finalize(&ctx, v);
    (n, v, ctx.requests)
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
        // dirty nodes get dense ids; edges are a flat list turned into CSR
        let mut ids: HashMap<Node, u32> = HashMap::default();
        let mut list: Vec<Node> = Vec::new();
        let mut edges: Vec<(u32, u32)> = Vec::new();
        let mut queue: Vec<(usize, Rect, Option<u32>)> = Vec::new();
        let mut deps_buf: Vec<Node> = Vec::new();
        let is_formula = |wb: &Workbook, n: Node| {
            matches!(
                wb.sheets[n.0].cells.get(&n.1).map(|c| &c.input),
                Some(Input::Formula { .. })
            )
        };
        // returns (id, newly added)
        fn add(ids: &mut HashMap<Node, u32>, list: &mut Vec<Node>, n: Node) -> (u32, bool) {
            if let Some(&i) = ids.get(&n) {
                return (i, false);
            }
            let i = list.len() as u32;
            ids.insert(n, i);
            list.push(n);
            (i, true)
        }
        let push_outputs =
            |wb: &Workbook, queue: &mut Vec<(usize, Rect, Option<u32>)>, n: Node, id: u32| {
                queue.push((n.0, Rect::cell(n.1), Some(id)));
                if let Some(sp) = wb.sheets[n.0].spills.get(&n.1) {
                    queue.push((n.0, *sp, Some(id)));
                }
            };
        for &(s, r) in seeds {
            report.changed.push((s, r));
            // edits inside a spill (or a blocked spill area) re-evaluate its anchor
            let sh = &self.sheets[s];
            let anchors: Vec<CellRef> = sh
                .spills
                .iter()
                .chain(sh.blocked.iter())
                .filter(|(_, sr)| sr.intersects(&r))
                .map(|(a, _)| *a)
                .collect();
            for a in anchors {
                let (id, fresh) = add(&mut ids, &mut list, (s, a));
                if fresh {
                    push_outputs(self, &mut queue, (s, a), id);
                }
            }
            if r.rows() == 1 && r.cols() == 1 && is_formula(self, (s, r.start())) {
                let n = (s, r.start());
                let (id, fresh) = add(&mut ids, &mut list, n);
                if fresh {
                    push_outputs(self, &mut queue, n, id);
                }
            } else {
                queue.push((s, r, None));
            }
        }
        for &n in extra {
            if is_formula(self, n) {
                let (id, fresh) = add(&mut ids, &mut list, n);
                if fresh {
                    push_outputs(self, &mut queue, n, id);
                }
            }
        }
        while let Some((s, r, from)) = queue.pop() {
            deps_buf.clear();
            self.graph.dependents_into(s, &r, &mut deps_buf);
            for &d in &deps_buf {
                let (id, fresh) = add(&mut ids, &mut list, d);
                if let Some(f) = from {
                    edges.push((f, id));
                }
                if fresh {
                    push_outputs(self, &mut queue, d, id);
                }
            }
        }
        drop(ids);
        edges.sort_unstable();
        edges.dedup();
        let n_nodes = list.len();
        let mut indeg = vec![0u32; n_nodes];
        let mut start = vec![0u32; n_nodes + 1];
        for &(a, b) in &edges {
            indeg[b as usize] += 1;
            start[a as usize + 1] += 1;
        }
        for i in 0..n_nodes {
            start[i + 1] += start[i];
        }
        let targets: Vec<u32> = edges.iter().map(|e| e.1).collect();
        drop(edges);
        let mut level: Vec<u32> = (0..n_nodes as u32)
            .filter(|&i| indeg[i as usize] == 0)
            .collect();
        level.sort_unstable_by_key(|&i| list[i as usize]);
        let mut done = 0usize;
        let mut follow: Vec<(usize, Rect)> = Vec::new();
        while !level.is_empty() {
            let level_nodes: Vec<Node> = level.iter().map(|&i| list[i as usize]).collect();
            let results = self.eval_level(&level_nodes, now);
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
            for &i in &level {
                for &m in &targets[start[i as usize] as usize..start[i as usize + 1] as usize] {
                    let d = &mut indeg[m as usize];
                    *d -= 1;
                    if *d == 0 {
                        next.push(m);
                    }
                }
            }
            next.sort_unstable_by_key(|&i| list[i as usize]);
            level = next;
        }
        if done < n_nodes {
            // cycle: Excel shows 0 for unresolved circular references
            let mut stuck: Vec<Node> = (0..n_nodes)
                .filter(|&i| indeg[i] > 0)
                .map(|i| list[i])
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
                sh.touch();
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
