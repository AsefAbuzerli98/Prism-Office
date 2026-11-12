/**
 * Markdown ⇄ document model (P2-09).
 *
 * Unsupported content (opaque blocks/inlines) and empty paragraphs are
 * exported as HTML-comment placeholders (`<!--prism:b3-->`) that map back to
 * the original objects on import, so editing in the Markdown pane never
 * drops tables, images or fields.
 */
import { lexer, type Token, type Tokens } from "marked";
import { BULLET_NUM_ID, DECIMAL_NUM_ID, isParagraph, isTable, type Block, type Inline, type Marks, type Paragraph, type Table } from "./model";
import { normalize } from "./ops";

export interface MarkdownExport {
  markdown: string;
  /** Placeholder id → preserved object. */
  objects: Map<string, Block | Inline>;
}

const CODE_FONTS = /^(consolas|courier new|courier|menlo|monaco|ui-monospace|monospace|source code pro|fira code|jetbrains mono)$/i;
const HEADINGS: Record<string, number> = { Title: 1, Heading1: 1, Heading2: 2, Heading3: 3, Heading4: 4, Heading5: 5, Heading6: 6 };

function escapeMd(s: string): string {
  return s.replace(/([\\`*_[\]<>~|])/g, "\\$1");
}

function wrap(text: string, open: string, close = open): string {
  const m = /^(\s*)([\s\S]*?)(\s*)$/.exec(text)!;
  return m[2] ? `${m[1]}${open}${m[2]}${close}${m[3]}` : text;
}

function inlineMd(inl: Inline, objects: Map<string, Block | Inline>): string {
  switch (inl.type) {
    case "tab":
      return "\t";
    case "break":
      return "\\\n";
    case "image":
    case "shape":
    case "note":
    case "comment":
    case "opaque": {
      const id = `i${objects.size}`;
      objects.set(id, inl);
      return `<!--prism:${id}-->`;
    }
    case "text": {
      const m = inl.marks;
      const code = !!m.font && CODE_FONTS.test(m.font);
      let s = code ? `\`${inl.text.replace(/`/g, "ˋ")}\`` : escapeMd(inl.text);
      if (m.underline) s = wrap(s, "<u>", "</u>");
      if (m.strike) s = wrap(s, "~~");
      if (m.italic) s = wrap(s, "*");
      if (m.bold) s = wrap(s, "**");
      if (m.link) s = `[${s}](${m.link.replace(/[()\s]/g, encodeURIComponent)})`;
      return s;
    }
  }
}

function paragraphInlines(p: Paragraph, objects: Map<string, Block | Inline>): string {
  // Merge adjacent links so `[a **b**](url)` stays one link.
  const out: string[] = [];
  let i = 0;
  while (i < p.inlines.length) {
    const cur = p.inlines[i]!;
    const link = cur.type === "text" ? cur.marks.link : undefined;
    if (!link) {
      out.push(inlineMd(cur, objects));
      i++;
      continue;
    }
    let j = i;
    const parts: string[] = [];
    while (j < p.inlines.length) {
      const n = p.inlines[j]!;
      if (n.type !== "text" || n.marks.link !== link) break;
      parts.push(inlineMd({ ...n, marks: { ...n.marks, link: undefined } }, objects));
      j++;
    }
    out.push(`[${parts.join("")}](${link.replace(/[()\s]/g, encodeURIComponent)})`);
    i = j;
  }
  return out.join("");
}

export function toMarkdown(blocks: Block[]): MarkdownExport {
  const objects = new Map<string, Block | Inline>();
  const chunks: string[] = [];
  let i = 0;
  let prevList = false;
  while (i < blocks.length) {
    const b = blocks[i]!;
    if (isTable(b) && simpleTable(b)) {
      chunks.push(tableMd(b, objects));
      prevList = false;
      i++;
      continue;
    }
    if (!isParagraph(b)) {
      const id = `b${objects.size}`;
      objects.set(id, b);
      chunks.push(`<!--prism:${id}-->`);
      prevList = false;
      i++;
      continue;
    }
    if (b.style === "Code") {
      const lines: string[] = [];
      while (i < blocks.length) {
        const c = blocks[i]!;
        if (!isParagraph(c) || c.style !== "Code") break;
        lines.push(c.inlines.map((x) => (x.type === "text" ? x.text : x.type === "tab" ? "\t" : "")).join(""));
        i++;
      }
      chunks.push("```\n" + lines.join("\n") + "\n```");
      prevList = false;
      continue;
    }
    const body = paragraphInlines(b, objects);
    let line: string;
    if (b.list) {
      const marker = b.list.numId === BULLET_NUM_ID || isBulletList(b.list.numId) ? "-" : "1.";
      line = `${"   ".repeat(b.list.level)}${marker} ${body}`;
      // consecutive list items share a block (no blank line)
      if (prevList) {
        chunks[chunks.length - 1] += `\n${line}`;
        i++;
        continue;
      }
      prevList = true;
    } else {
      prevList = false;
      if (b.style && HEADINGS[b.style]) line = `${"#".repeat(HEADINGS[b.style]!)} ${body}`;
      else if (b.style === "Quote") line = `> ${body}`;
      else if (!body.trim()) {
        const id = `b${objects.size}`;
        objects.set(id, b);
        line = `<!--prism:${id}-->`;
      } else line = body.replace(/^(\s*)(#|>|-|\+|\d+\.)/, "$1\\$2");
    }
    chunks.push(line);
    i++;
  }
  return { markdown: chunks.join("\n\n") + "\n", objects };
}

/** GFM pipe tables can express tables without spans/merges and single-paragraph cells. */
function simpleTable(t: Table): boolean {
  return t.rows.length > 0 && t.rows.every((r) => r.cells.length === t.rows[0]!.cells.length && r.cells.every((c) => c.span <= 1 && !c.vMerge && !c.shading && c.blocks.length === 1 && isParagraph(c.blocks[0]!) && !c.blocks[0]!.list && !c.blocks[0]!.style));
}

function tableMd(t: Table, objects: Map<string, Block | Inline>): string {
  const row = (r: Table["rows"][number]) => `| ${r.cells.map((c) => paragraphInlines(c.blocks[0] as Paragraph, objects).replace(/\|/g, "\\|").replace(/\n/g, " ")).join(" | ")} |`;
  const [head, ...rest] = t.rows;
  return [row(head!), `|${head!.cells.map(() => " --- ").join("|")}|`, ...rest.map(row)].join("\n");
}

let bulletIds = new Set<number>([BULLET_NUM_ID]);
/** Tell the exporter which numbering ids are bullets (from the document). */
export function setBulletIds(ids: Iterable<number>) {
  bulletIds = new Set([BULLET_NUM_ID, ...ids]);
}
function isBulletList(numId: number) {
  return bulletIds.has(numId);
}

// ---- import ------------------------------------------------------------------

const PLACEHOLDER = /^<!--prism:([bi]\d+)-->$/;

function inlinesFrom(tokens: Token[] | undefined, marks: Marks, objects: Map<string, Block | Inline>, out: Inline[]) {
  for (const t of tokens ?? []) {
    switch (t.type) {
      case "text": {
        const tt = t as Tokens.Text;
        if (tt.tokens?.length) inlinesFrom(tt.tokens, marks, objects, out);
        else pushText(out, decode(tt.text), marks);
        break;
      }
      case "escape":
        pushText(out, (t as Tokens.Escape).text, marks);
        break;
      case "strong":
        inlinesFrom((t as Tokens.Strong).tokens, { ...marks, bold: true }, objects, out);
        break;
      case "em":
        inlinesFrom((t as Tokens.Em).tokens, { ...marks, italic: true }, objects, out);
        break;
      case "del":
        inlinesFrom((t as Tokens.Del).tokens, { ...marks, strike: true }, objects, out);
        break;
      case "codespan":
        pushText(out, decode((t as Tokens.Codespan).text).replace(/ˋ/g, "`"), { ...marks, font: "Consolas" });
        break;
      case "link":
        inlinesFrom((t as Tokens.Link).tokens, { ...marks, link: decodeURIComponent((t as Tokens.Link).href) }, objects, out);
        break;
      case "image":
        pushText(out, (t as Tokens.Image).text, marks);
        break;
      case "br":
        out.push({ type: "break", kind: "line" });
        break;
      case "html": {
        const raw = (t as Tokens.HTML).raw.trim();
        const ph = PLACEHOLDER.exec(raw);
        if (ph) {
          const obj = objects.get(ph[1]!);
          if (obj && "type" in obj && (obj.type === "opaque" || obj.type === "text" || obj.type === "tab" || obj.type === "break") && !isParagraph(obj as Block)) {
            out.push(obj as Inline);
          }
        } else if (/^<u>$/i.test(raw)) {
          marks = { ...marks, underline: true };
        } else if (/^<\/u>$/i.test(raw)) {
          const { underline: _u, ...rest } = marks;
          marks = rest;
        }
        break;
      }
      default:
        if ("text" in t && typeof t.text === "string") pushText(out, decode(t.text), marks);
    }
  }
}

function decode(s: string): string {
  return s
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&amp;/g, "&");
}

function pushText(out: Inline[], text: string, marks: Marks) {
  if (!text) return;
  const clean: Marks = {};
  for (const [k, v] of Object.entries(marks)) if (v !== undefined && v !== false) (clean as Record<string, unknown>)[k] = v;
  const parts = text.split("\t");
  parts.forEach((p, i) => {
    if (i > 0) out.push({ type: "tab", marks: clean });
    if (p) out.push({ type: "text", text: p, marks: clean });
  });
}

function para(inlines: Inline[], props: Partial<Paragraph> = {}): Paragraph {
  return { type: "paragraph", ...props, inlines: normalize(inlines) };
}

function blocksFrom(tokens: Token[], objects: Map<string, Block | Inline>, out: Block[], ctx: { level: number; quote: boolean }) {
  for (const t of tokens) {
    switch (t.type) {
      case "heading": {
        const h = t as Tokens.Heading;
        const inl: Inline[] = [];
        inlinesFrom(h.tokens, {}, objects, inl);
        out.push(para(inl, { style: `Heading${Math.min(6, h.depth)}` }));
        break;
      }
      case "paragraph": {
        const p = t as Tokens.Paragraph;
        const raw = p.raw.trim();
        const ph = PLACEHOLDER.exec(raw);
        if (ph && objects.get(ph[1]!)) {
          const obj = objects.get(ph[1]!)!;
          out.push(isParagraph(obj as Block) || (obj as Block).type === "opaque" && ph[1]!.startsWith("b") ? (obj as Block) : para([obj as Inline]));
          break;
        }
        const inl: Inline[] = [];
        inlinesFrom(p.tokens, {}, objects, inl);
        out.push(para(inl, ctx.quote ? { style: "Quote" } : {}));
        break;
      }
      case "html": {
        const raw = (t as Tokens.HTML).raw.trim();
        for (const line of raw.split(/\n+/)) {
          const ph = PLACEHOLDER.exec(line.trim());
          const obj = ph ? objects.get(ph[1]!) : undefined;
          if (obj) out.push(ph![1]!.startsWith("b") ? (obj as Block) : para([obj as Inline]));
          else if (line.trim()) out.push(para([{ type: "text", text: line.trim(), marks: {} }]));
        }
        break;
      }
      case "blockquote":
        blocksFrom((t as Tokens.Blockquote).tokens, objects, out, { ...ctx, quote: true });
        break;
      case "code": {
        const c = t as Tokens.Code;
        for (const line of c.text.split("\n")) out.push(para(line ? [{ type: "text", text: line, marks: {} }] : [], { style: "Code" }));
        break;
      }
      case "list": {
        const l = t as Tokens.List;
        for (const item of l.items) {
          const inl: Inline[] = [];
          const nested: Token[] = [];
          for (const it of item.tokens) {
            if (it.type === "list") nested.push(it);
            else if (it.type === "text" || it.type === "paragraph") inlinesFrom((it as Tokens.Text).tokens ?? [it], {}, objects, inl);
          }
          out.push(para(inl, { list: { numId: l.ordered ? DECIMAL_NUM_ID : BULLET_NUM_ID, level: ctx.level } }));
          blocksFrom(nested, objects, out, { ...ctx, level: Math.min(8, ctx.level + 1) });
        }
        break;
      }
      case "table": {
        const tt = t as Tokens.Table;
        const cell = (c: Tokens.TableCell) => {
          const inl: Inline[] = [];
          inlinesFrom(c.tokens, {}, objects, inl);
          return { blocks: [para(inl)], span: 1 };
        };
        const cols = tt.header.length;
        const w = Math.floor(9026 / Math.max(1, cols));
        out.push({
          type: "table",
          grid: Array.from({ length: cols }, () => w),
          borders: true,
          rows: [{ header: true, cells: tt.header.map(cell) }, ...tt.rows.map((r) => ({ cells: r.map(cell) }))],
        });
        break;
      }
      case "hr":
        out.push(para([{ type: "break", kind: "page" }]));
        break;
      default:
        break;
    }
  }
}

export function fromMarkdown(md: string, objects: Map<string, Block | Inline> = new Map()): Block[] {
  const out: Block[] = [];
  blocksFrom(lexer(md, { gfm: true }), objects, out, { level: 0, quote: false });
  return out.length ? out : [para([])];
}
