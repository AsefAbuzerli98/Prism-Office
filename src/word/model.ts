/** Mirrors `prism_word::model` (serde camelCase, tagged by `type`). */

export interface RawXml {
  name: string;
  xml: string;
}

export type Align = "left" | "center" | "right" | "justify";
export type VertAlign = "superscript" | "subscript";

export interface Marks {
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  strike?: boolean;
  font?: string;
  /** Half-points. */
  size?: number;
  /** RRGGBB */
  color?: string;
  highlight?: string;
  vertAlign?: VertAlign;
  style?: string;
  link?: string;
  rtl?: boolean;
  revision?: Revision;
  extra?: RawXml[];
}

export interface Revision {
  kind: "insert" | "delete";
  id: string;
  author: string;
  date?: string;
}

export interface TabStop {
  /** Twips from the page content edge. */
  pos: number;
  align: "left" | "center" | "right" | "decimal" | "bar" | "clear";
  leader?: string | null;
}

export interface ParaFormat {
  align?: Align;
  /** Twips. */
  spaceBefore?: number;
  spaceAfter?: number;
  /** 240ths of a line. */
  line?: number;
  indentLeft?: number;
  indentRight?: number;
  indentFirst?: number;
  keepNext?: boolean;
  pageBreakBefore?: boolean;
  outlineLevel?: number;
  bidi?: boolean;
  tabs?: TabStop[];
}

export interface ListRef {
  numId: number;
  level: number;
}

export interface Paragraph extends ParaFormat {
  type: "paragraph";
  style?: string;
  list?: ListRef;
  extra?: RawXml[];
  inlines: Inline[];
}

export type BreakKind = "line" | "page" | "column";

export type NoteKind = "footnote" | "endnote";

export type Inline =
  | { type: "text"; text: string; marks: Marks }
  | { type: "tab"; marks: Marks }
  | { type: "break"; kind: BreakKind }
  /** Picture; `cx`/`cy` in EMU (914400 per inch). */
  | { type: "image"; relId: string; cx: number; cy: number; descr: string; xml: string }
  | { type: "shape"; preset: string; cx: number; cy: number; stroke: string; fill?: string; xml: string }
  | { type: "note"; kind: NoteKind; id: number; xml: string }
  | { type: "comment"; id: string; edge: "start" | "end" | "reference" }
  | { type: "opaque"; label: string; xml: string };

export interface TableCell {
  blocks: Block[];
  span: number;
  vMerge?: "restart" | "continue";
  /** Twips. */
  width?: number;
  shading?: string;
  extra?: RawXml[];
}

export interface TableRow {
  cells: TableCell[];
  header?: boolean;
  props?: string;
}

export interface Table {
  type: "table";
  /** Grid column widths, twips. */
  grid: number[];
  rows: TableRow[];
  borders: boolean;
  props?: string;
}

export type OpaqueBlock = { type: "opaque"; label: string; xml: string };
export type Block = Paragraph | Table | OpaqueBlock;

export interface Note {
  kind: NoteKind;
  id: number;
  blocks: Block[];
}

export interface Comment {
  id: string;
  author: string;
  initials: string;
  date?: string;
  blocks: Block[];
}

export interface HeaderFooter {
  kind: "header" | "footer";
  which: string;
  relId: string;
  blocks: Block[];
}

export interface Source {
  tag: string;
  kind: string;
  title: string;
  authors: string[];
  year: string;
  publisher: string;
  url: string;
}

export interface DocSettings {
  trackRevisions: boolean;
  autoHyphenation: boolean;
}

export interface Columns {
  count: number;
  space: number;
  separator: boolean;
}

export interface Section {
  pageWidth: number;
  pageHeight: number;
  marginTop: number;
  marginRight: number;
  marginBottom: number;
  marginLeft: number;
  marginHeader: number;
  marginFooter: number;
  landscape: boolean;
  columns?: Columns | null;
  extra?: RawXml[];
}

export interface StyleInfo {
  id: string;
  name: string;
  kind: "paragraph" | "character" | "table" | "numbering";
  basedOn?: string;
  next?: string;
  isDefault: boolean;
  para: ParaFormat;
  run: Marks;
}

export interface LevelDef {
  format: string;
  text: string;
  start: number;
  indentLeft: number;
  hanging: number;
}

export interface NumberingDef {
  numId: number;
  levels: LevelDef[];
}

export interface WordDocument {
  blocks: Block[];
  section: Section;
  styles: StyleInfo[];
  defaultRun: Marks;
  defaultPara: ParaFormat;
  numbering: NumberingDef[];
  notes?: Note[];
  comments?: Comment[];
  headers?: HeaderFooter[];
  sources?: Source[];
  settings?: DocSettings;
}

export const BULLET_NUM_ID = 9001;
export const DECIMAL_NUM_ID = 9002;

export const DEFAULT_SECTION: Section = {
  pageWidth: 11906,
  pageHeight: 16838,
  marginTop: 1440,
  marginRight: 1440,
  marginBottom: 1440,
  marginLeft: 1440,
  marginHeader: 708,
  marginFooter: 708,
  landscape: false,
};

const style = (id: string, name: string, para: ParaFormat, run: Marks, next?: string): StyleInfo => ({
  id,
  name,
  kind: "paragraph",
  basedOn: id === "Normal" ? undefined : "Normal",
  next,
  isDefault: id === "Normal",
  para,
  run,
});

/** Styles matching the Rust built-ins (used by the browser host and as a fallback). */
export const BUILTIN_STYLES: StyleInfo[] = [
  style("Normal", "Normal", {}, {}),
  style("Title", "Title", { spaceAfter: 80 }, { size: 56 }, "Normal"),
  style("Heading1", "heading 1", { keepNext: true, spaceBefore: 360, spaceAfter: 80, outlineLevel: 0 }, { bold: true, size: 40 }, "Normal"),
  style("Heading2", "heading 2", { keepNext: true, spaceBefore: 160, spaceAfter: 80, outlineLevel: 1 }, { bold: true, size: 32 }, "Normal"),
  style("Heading3", "heading 3", { keepNext: true, spaceBefore: 160, spaceAfter: 80, outlineLevel: 2 }, { bold: true, size: 28 }, "Normal"),
  style("Heading4", "heading 4", { keepNext: true, spaceBefore: 80, spaceAfter: 40, outlineLevel: 3 }, { bold: true, italic: true }, "Normal"),
  style("Quote", "Quote", { spaceBefore: 160, spaceAfter: 160, indentLeft: 864, indentRight: 864 }, { italic: true, color: "404040" }),
  style("Code", "Code", { spaceAfter: 0, line: 240 }, { font: "Consolas", size: 20 }),
  style("FootnoteText", "footnote text", { spaceAfter: 0, line: 240 }, { size: 20 }),
  style("TOC1", "toc 1", { spaceAfter: 100, tabs: [{ pos: 9016, align: "right", leader: "." }] }, {}),
  style("TOC2", "toc 2", { spaceAfter: 100, indentLeft: 220, tabs: [{ pos: 9016, align: "right", leader: "." }] }, {}),
  style("TOC3", "toc 3", { spaceAfter: 100, indentLeft: 440, tabs: [{ pos: 9016, align: "right", leader: "." }] }, {}),
  style("Bibliography", "Bibliography", { indentLeft: 720, indentFirst: -720 }, {}),
];

const levels = (bullet: boolean): LevelDef[] =>
  Array.from({ length: 9 }, (_, i) => ({
    format: bullet ? "bullet" : (["decimal", "lowerLetter", "lowerRoman"][i % 3] as string),
    text: bullet ? (["•", "◦", "▪"][i % 3] as string) : `%${i + 1}.`,
    start: 1,
    indentLeft: 720 * (i + 1),
    hanging: 360,
  }));

export const BUILTIN_NUMBERING: NumberingDef[] = [
  { numId: BULLET_NUM_ID, levels: levels(true) },
  { numId: DECIMAL_NUM_ID, levels: levels(false) },
];

export function paragraph(text = "", props: Partial<Omit<Paragraph, "type" | "inlines">> = {}): Paragraph {
  return { type: "paragraph", ...props, inlines: text ? [{ type: "text", text, marks: {} }] : [] };
}

export function blankDocument(): WordDocument {
  return {
    blocks: [paragraph()],
    section: { ...DEFAULT_SECTION },
    styles: BUILTIN_STYLES.map((s) => ({ ...s })),
    defaultRun: { font: "Calibri", size: 22 },
    defaultPara: { spaceAfter: 160, line: 259 },
    numbering: BUILTIN_NUMBERING,
    notes: [],
    comments: [],
    headers: [],
    sources: [],
    settings: { trackRevisions: false, autoHyphenation: false },
  };
}

export const isParagraph = (b: Block): b is Paragraph => b.type === "paragraph";
export const isTable = (b: Block): b is Table => b.type === "table";
export const blockLabel = (b: Block): string => (b.type === "paragraph" ? "" : b.type === "table" ? "Table" : b.label);

export const EMU_PER_PX = 9525;

export function newTable(rows: number, cols: number, contentTwips = 9026): Table {
  const w = Math.floor(contentTwips / cols);
  return {
    type: "table",
    grid: Array.from({ length: cols }, () => w),
    borders: true,
    rows: Array.from({ length: rows }, () => ({ cells: Array.from({ length: cols }, () => ({ blocks: [paragraph()], span: 1 })) })),
  };
}
