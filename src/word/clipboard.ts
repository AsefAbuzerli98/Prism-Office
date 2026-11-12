/** HTML ⇄ blocks for clipboard interop (P2-10). */
import { BULLET_NUM_ID, DECIMAL_NUM_ID, isParagraph, type Block, type Inline, type Marks, type Paragraph } from "./model";
import { normalize, paragraphText } from "./ops";

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const TAGS: Record<string, string> = { Title: "h1", Heading1: "h1", Heading2: "h2", Heading3: "h3", Heading4: "h4", Heading5: "h5", Heading6: "h6", Quote: "blockquote", Code: "pre" };

function inlineHtml(i: Inline): string {
  if (i.type === "tab") return "&emsp;";
  if (i.type === "break") return "<br>";
  if (i.type === "opaque") return i.label ? `<span data-prism-opaque>[${esc(i.label)}]</span>` : "";
  const m = i.marks;
  let s = esc(i.text);
  const css: string[] = [];
  if (m.font) css.push(`font-family:'${esc(m.font)}'`);
  if (m.size) css.push(`font-size:${m.size / 2}pt`);
  if (m.color) css.push(`color:#${m.color}`);
  if (css.length) s = `<span style="${css.join(";")}">${s}</span>`;
  if (m.vertAlign === "superscript") s = `<sup>${s}</sup>`;
  if (m.vertAlign === "subscript") s = `<sub>${s}</sub>`;
  if (m.underline) s = `<u>${s}</u>`;
  if (m.strike) s = `<s>${s}</s>`;
  if (m.italic) s = `<em>${s}</em>`;
  if (m.bold) s = `<strong>${s}</strong>`;
  if (m.link) s = `<a href="${esc(m.link)}">${s}</a>`;
  return s;
}

export function blocksToHtml(blocks: Block[]): string {
  const out: string[] = [];
  let list: { tag: string; level: number }[] = [];
  const closeTo = (level: number) => {
    while (list.length > level) out.push(`</${list.pop()!.tag}>`);
  };
  for (const b of blocks) {
    if (!isParagraph(b)) {
      closeTo(0);
      out.push(`<p data-prism-opaque>[${esc(b.label)}]</p>`);
      continue;
    }
    const body = b.inlines.map(inlineHtml).join("");
    const align = b.align && b.align !== "left" ? ` style="text-align:${b.align}"` : "";
    if (b.list) {
      const tag = b.list.numId === DECIMAL_NUM_ID ? "ol" : "ul";
      closeTo(b.list.level + 1);
      while (list.length < b.list.level + 1) {
        list.push({ tag, level: list.length });
        out.push(`<${tag}>`);
      }
      out.push(`<li${align}>${body}</li>`);
      continue;
    }
    closeTo(0);
    const tag = (b.style && TAGS[b.style]) || "p";
    out.push(`<${tag}${align}>${body || "<br>"}</${tag}>`);
  }
  closeTo(0);
  return out.join("");
}

export function blocksToText(blocks: Block[]): string {
  return blocks.map((b) => (isParagraph(b) ? paragraphText(b).replace(/￼/g, "") : `[${b.label}]`)).join("\n");
}

const BLOCK_TAGS = new Set(["P", "DIV", "H1", "H2", "H3", "H4", "H5", "H6", "LI", "BLOCKQUOTE", "PRE", "TR", "SECTION", "ARTICLE", "HEADER", "FOOTER"]);

/** Parse clipboard HTML into blocks (formatting subset: b/i/u/s/a/code/headings/lists). */
export function htmlToBlocks(html: string): Block[] {
  const doc = new DOMParser().parseFromString(html, "text/html");
  const out: Paragraph[] = [];
  let cur: Paragraph | null = null;
  const start = (props: Partial<Paragraph> = {}) => {
    cur = { type: "paragraph", ...props, inlines: [] };
    out.push(cur);
    return cur;
  };
  const ensure = () => cur ?? start();

  const walk = (node: Node, marks: Marks, ctx: { list: ("ul" | "ol")[]; style?: string }) => {
    if (node.nodeType === Node.TEXT_NODE) {
      const raw = (node.textContent ?? "").replace(/[\r\n]+/g, ctx.style === "Code" ? "\n" : " ");
      if (!raw.trim() && !cur) return;
      if (ctx.style === "Code" && raw.includes("\n")) {
        raw.split("\n").forEach((line, i) => {
          if (i > 0) start({ style: "Code" });
          if (line) ensure().inlines.push({ type: "text", text: line, marks });
        });
        return;
      }
      ensure().inlines.push({ type: "text", text: raw, marks });
      return;
    }
    if (!(node instanceof Element)) return;
    const tag = node.tagName;
    if (tag === "STYLE" || tag === "SCRIPT" || tag === "META" || tag === "TITLE") return;
    if (tag === "BR") {
      ensure().inlines.push({ type: "break", kind: "line" });
      return;
    }
    const m: Marks = { ...marks };
    const st = (node as HTMLElement).style;
    if (tag === "B" || tag === "STRONG" || /^(bold|[6-9]00)$/.test(st?.fontWeight ?? "")) m.bold = true;
    if (tag === "I" || tag === "EM" || st?.fontStyle === "italic") m.italic = true;
    if (tag === "U" || (st?.textDecoration ?? "").includes("underline")) m.underline = true;
    if (tag === "S" || tag === "STRIKE" || tag === "DEL" || (st?.textDecoration ?? "").includes("line-through")) m.strike = true;
    if (tag === "CODE" || tag === "KBD") m.font = "Consolas";
    if (tag === "SUP") m.vertAlign = "superscript";
    if (tag === "SUB") m.vertAlign = "subscript";
    if (tag === "A" && node.getAttribute("href")) m.link = node.getAttribute("href")!;
    const newCtx = { ...ctx };
    if (tag === "UL" || tag === "OL") newCtx.list = [...ctx.list, tag === "OL" ? "ol" : "ul"];
    if (/^H[1-6]$/.test(tag)) newCtx.style = `Heading${tag[1]}`;
    if (tag === "BLOCKQUOTE") newCtx.style = "Quote";
    if (tag === "PRE") newCtx.style = "Code";
    if (BLOCK_TAGS.has(tag)) {
      const props: Partial<Paragraph> = {};
      if (newCtx.style) props.style = newCtx.style;
      if (tag === "LI" && ctx.list.length) {
        props.list = { numId: ctx.list[ctx.list.length - 1] === "ol" ? DECIMAL_NUM_ID : BULLET_NUM_ID, level: Math.min(8, ctx.list.length - 1) };
      }
      const align = st?.textAlign;
      if (align === "center" || align === "right" || align === "justify") props.align = align;
      start(props);
      node.childNodes.forEach((c) => walk(c, m, newCtx));
      cur = null;
      return;
    }
    node.childNodes.forEach((c) => walk(c, m, newCtx));
  };
  doc.body.childNodes.forEach((n) => walk(n, {}, { list: [] }));
  const cleaned = out
    .map((p) => {
      const inl = normalize(p.inlines);
      // trim collapsed whitespace at paragraph edges
      const first = inl[0];
      if (first?.type === "text") inl[0] = { ...first, text: first.text.replace(/^\s+/, "") };
      const last = inl[inl.length - 1];
      if (last?.type === "text") inl[inl.length - 1] = { ...last, text: last.text.replace(/\s+$/, "") };
      return { ...p, inlines: normalize(inl) };
    })
    .filter((p, i, arr) => p.inlines.length > 0 || p.style || p.list || (i > 0 && i < arr.length - 1));
  return cleaned.length ? cleaned : [];
}

export function textToBlocks(text: string): Block[] {
  return text
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .map((line) => ({ type: "paragraph" as const, inlines: line ? normalize(line.split("\t").flatMap((t, i): Inline[] => [...(i ? [{ type: "tab" as const, marks: {} }] : []), ...(t ? [{ type: "text" as const, text: t, marks: {} }] : [])])) : [] }));
}
