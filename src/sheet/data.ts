/** Data connections: CSV / JSON parsing and pivot formula building. */
import { a1 } from "./geometry";
import type { Rect } from "./model";

/** Guess the delimiter from the first lines (comma, semicolon, tab, pipe). */
export function sniffDelimiter(text: string): string {
  const sample = text.split(/\r?\n/).slice(0, 10).join("\n");
  let best = ",";
  let bestScore = -1;
  for (const d of [",", ";", "\t", "|"]) {
    const counts = sample.split("\n").filter(Boolean).map((l) => splitLine(l, d).length);
    if (!counts.length) continue;
    const consistent = counts.every((c) => c === counts[0]);
    const score = (consistent ? 1000 : 0) + counts[0]!;
    if (counts[0]! > 1 && score > bestScore) {
      best = d;
      bestScore = score;
    }
  }
  return best;
}

function splitLine(line: string, d: string): string[] {
  return parseCsv(line, d)[0] ?? [];
}

/** RFC 4180 CSV (quoted fields, doubled quotes, CRLF, embedded newlines). */
export function parseCsv(text: string, delimiter?: string): string[][] {
  const d = delimiter ?? sniffDelimiter(text);
  const rows: string[][] = [];
  let row: string[] = [];
  let cur = "";
  let q = false;
  const t = text.replace(/^﻿/, "");
  for (let i = 0; i < t.length; i++) {
    const ch = t[i]!;
    if (q) {
      if (ch === '"') {
        if (t[i + 1] === '"') {
          cur += '"';
          i++;
        } else q = false;
      } else cur += ch;
      continue;
    }
    if (ch === '"' && cur === "") q = true;
    else if (ch === d) {
      row.push(cur);
      cur = "";
    } else if (ch === "\n" || ch === "\r") {
      if (ch === "\r" && t[i + 1] === "\n") i++;
      row.push(cur);
      rows.push(row);
      row = [];
      cur = "";
    } else cur += ch;
  }
  if (cur !== "" || row.length) {
    row.push(cur);
    rows.push(row);
  }
  return rows;
}

/** JSON → rows: array of objects (header = union of keys), array of arrays, or `{data:[…]}`. */
export function jsonToRows(value: unknown): unknown[][] {
  let v = value;
  if (v && typeof v === "object" && !Array.isArray(v)) {
    const arr = Object.values(v as Record<string, unknown>).find(Array.isArray);
    v = arr ?? [v];
  }
  if (!Array.isArray(v)) return [[v]];
  if (v.every(Array.isArray)) return v as unknown[][];
  const keys: string[] = [];
  for (const item of v) {
    if (item && typeof item === "object") for (const k of Object.keys(item)) if (!keys.includes(k)) keys.push(k);
  }
  if (!keys.length) return v.map((x) => [x]);
  return [keys, ...v.map((item) => keys.map((k) => (item && typeof item === "object" ? (item as Record<string, unknown>)[k] ?? null : null)))];
}

export type Aggregate = "SUM" | "AVERAGE" | "COUNT" | "COUNTA" | "MAX" | "MIN";

export interface PivotSpec {
  sheet: string;
  /** Data range including the header row. */
  data: Rect;
  rows: number;
  cols: number | null;
  values: number;
  fn: Aggregate;
  /** Reference the source from another sheet. */
  qualify: boolean;
}

/** A dynamic-array pivot: PIVOTBY (rows × columns) or GROUPBY (rows only). */
export function pivotFormula(p: PivotSpec): string {
  const q = p.qualify ? (/^[A-Za-z_][A-Za-z0-9_.]*$/.test(p.sheet) ? `${p.sheet}!` : `'${p.sheet.replace(/'/g, "''")}'!`) : "";
  const col = (c: number) => `${q}${a1(p.data.r0 + 1, p.data.c0 + c)}:${a1(p.data.r1, p.data.c0 + c)}`;
  const rows = col(p.rows);
  const vals = col(p.values);
  if (p.cols === null) return `=GROUPBY(${rows},${vals},${p.fn})`;
  return `=PIVOTBY(${rows},${col(p.cols)},${vals},${p.fn})`;
}
