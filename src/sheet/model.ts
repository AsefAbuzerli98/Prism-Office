/** Mirrors `prism_sheet::model` (Rust ↔ TS contract). */

export interface Rect {
  r0: number;
  c0: number;
  r1: number;
  c1: number;
}

export type CellValueModel = boolean | number | string | { e: string };

export interface CellModel {
  r: number;
  c: number;
  /** Formula text without the leading `=`. */
  f?: string;
  v?: CellValueModel;
  s?: number;
}

export interface CellStyle {
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  strike?: boolean;
  fontSize?: number;
  fontName?: string;
  color?: string;
  fill?: string;
  hAlign?: string;
  vAlign?: string;
  wrap?: boolean;
  /** 1 left, 2 right, 4 top, 8 bottom. */
  border?: number;
}

export interface TableDef {
  name: string;
  rect: Rect;
  columns: string[];
  headerRow: boolean;
  totalsRow: boolean;
}

export type ChartKind = "column" | "bar" | "line" | "area" | "pie" | "scatter";

export interface ChartDef {
  kind: ChartKind;
  title: string;
  data: Rect;
  anchor: Rect;
  fromFile?: boolean;
  part?: string;
}

export interface PivotDef {
  name: string;
  sourceSheet: string;
  source: Rect;
  rows: number;
  cols?: number;
  values: number;
  function: "sum" | "average" | "count" | "max" | "min";
  location: { row: number; col: number };
  extent?: Rect;
  part?: string;
  cachePart?: string;
}

export interface SheetModel {
  name: string;
  cells: CellModel[];
  colWidths: [number, number][];
  rowHeights: [number, number][];
  merges: Rect[];
  frozen: [number, number];
  hidden: boolean;
  tables: TableDef[];
  charts?: ChartDef[];
  pivots?: PivotDef[];
  source?: number;
}

export interface DefinedName {
  name: string;
  formula: string;
  sheet?: number;
}

export interface WorkbookModel {
  sheets: SheetModel[];
  names: DefinedName[];
  numFormats: string[];
  styles: CellStyle[];
  date1904: boolean;
  connections?: { kind: string; source: string; query?: string | null; sheet: string }[];
}

export function blankWorkbook(): WorkbookModel {
  return {
    sheets: [{ name: "Sheet1", cells: [], colWidths: [], rowHeights: [], merges: [], frozen: [0, 0], hidden: false, tables: [] }],
    names: [],
    numFormats: ["General"],
    styles: [{}],
    date1904: false,
  };
}
