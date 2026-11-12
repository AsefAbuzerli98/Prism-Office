/** Table structure edits (P2-11). All functions are pure. */
import { paragraph, type Table, type TableCell } from "../model";

const emptyCell = (span = 1): TableCell => ({ blocks: [paragraph()], span });

function rescale(grid: number[], total: number): number[] {
  const sum = grid.reduce((a, b) => a + b, 0) || 1;
  return grid.map((w) => Math.max(200, Math.round((w * total) / sum)));
}

export function insertRow(t: Table, at: number, below: boolean): Table {
  const ref = t.rows[at] ?? t.rows[t.rows.length - 1]!;
  const row = { cells: ref.cells.map((c) => emptyCell(c.span)) };
  const rows = t.rows.slice();
  rows.splice(below ? at + 1 : at, 0, row);
  return { ...t, rows };
}

export function deleteRow(t: Table, at: number): Table | null {
  if (t.rows.length <= 1) return null;
  return { ...t, rows: t.rows.filter((_, i) => i !== at) };
}

export function insertColumn(t: Table, col: number, right: boolean): Table {
  const total = t.grid.reduce((a, b) => a + b, 0) || 9026;
  const at = right ? col + 1 : col;
  const grid = t.grid.slice();
  grid.splice(Math.min(at, grid.length), 0, Math.round(total / Math.max(1, grid.length)));
  return {
    ...t,
    grid: rescale(grid, total),
    rows: t.rows.map((r) => {
      const cells = r.cells.slice();
      cells.splice(Math.min(at, cells.length), 0, emptyCell());
      return { ...r, cells };
    }),
  };
}

export function deleteColumn(t: Table, col: number): Table | null {
  const maxCells = Math.max(...t.rows.map((r) => r.cells.length));
  if (maxCells <= 1) return null;
  const total = t.grid.reduce((a, b) => a + b, 0) || 9026;
  const grid = t.grid.filter((_, i) => i !== col);
  return { ...t, grid: grid.length ? rescale(grid, total) : grid, rows: t.rows.map((r) => ({ ...r, cells: r.cells.filter((_, i) => i !== col) })) };
}

/** Merge a cell with its right neighbour (spans add, content appends). */
export function mergeRight(t: Table, row: number, col: number): Table {
  const r = t.rows[row];
  const a = r?.cells[col];
  const b = r?.cells[col + 1];
  if (!r || !a || !b) return t;
  const blocks = [...a.blocks, ...b.blocks].filter((x, _i, arr) => !(x.type === "paragraph" && x.inlines.length === 0 && arr.length > 1));
  const merged: TableCell = { ...a, span: (a.span || 1) + (b.span || 1), blocks: blocks.length ? blocks : [paragraph()], width: undefined };
  const cells = r.cells.slice();
  cells.splice(col, 2, merged);
  return { ...t, rows: t.rows.map((x, i) => (i === row ? { ...x, cells } : x)) };
}

/** Split a spanning cell back into single-column cells. */
export function splitCell(t: Table, row: number, col: number): Table {
  const r = t.rows[row];
  const c = r?.cells[col];
  if (!r || !c || (c.span || 1) <= 1) return t;
  const cells = r.cells.slice();
  cells.splice(col, 1, { ...c, span: 1, width: undefined }, ...Array.from({ length: c.span - 1 }, () => emptyCell()));
  return { ...t, rows: t.rows.map((x, i) => (i === row ? { ...x, cells } : x)) };
}

export function setCell(t: Table, row: number, col: number, patch: Partial<TableCell>): Table {
  return { ...t, rows: t.rows.map((r, ri) => (ri !== row ? r : { ...r, cells: r.cells.map((c, ci) => (ci !== col ? c : { ...c, ...patch })) })) };
}

export function toggleHeaderRow(t: Table): Table {
  const on = !t.rows[0]?.header;
  return { ...t, rows: t.rows.map((r, i) => (i === 0 ? { ...r, header: on || undefined } : r)) };
}
