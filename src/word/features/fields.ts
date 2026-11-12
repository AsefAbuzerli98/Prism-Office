/** Fields: table of contents, citations and bibliography (P2-11, P2-13). */
import { isParagraph, type Block, type Inline, type Paragraph, type Source, type WordDocument } from "../model";
import { paragraphText } from "../ops";

const run = (inner: string) => `<w:r>${inner}</w:r>`;
export const fieldBegin = (): Inline => ({ type: "opaque", label: "", xml: run('<w:fldChar w:fldCharType="begin"/>') });
export const fieldSeparate = (): Inline => ({ type: "opaque", label: "", xml: run('<w:fldChar w:fldCharType="separate"/>') });
export const fieldEnd = (): Inline => ({ type: "opaque", label: "", xml: run('<w:fldChar w:fldCharType="end"/>') });
export const fieldInstr = (code: string): Inline => ({
  type: "opaque",
  label: "",
  xml: run(`<w:instrText xml:space="preserve"> ${code.replace(/&/g, "&amp;").replace(/</g, "&lt;")} </w:instrText>`),
});

/** Field code text of an instrText opaque inline, or null. */
export function instrOf(i: Inline): string | null {
  if (i.type !== "opaque" || !i.xml.includes("instrText")) return null;
  return i.xml.replace(/<[^>]+>/g, "").replace(/&lt;/g, "<").replace(/&amp;/g, "&").trim();
}
const isBegin = (i: Inline) => i.type === "opaque" && i.xml.includes('fldCharType="begin"');
const isEnd = (i: Inline) => i.type === "opaque" && i.xml.includes('fldCharType="end"');

export interface Heading {
  level: number;
  text: string;
  block: number;
}

export function headings(doc: WordDocument, maxLevel = 3): Heading[] {
  const out: Heading[] = [];
  doc.blocks.forEach((b, i) => {
    if (!isParagraph(b)) return;
    const style = doc.styles.find((s) => s.id === b.style);
    const lvl = b.outlineLevel ?? style?.para.outlineLevel ?? (b.style === "Title" ? undefined : /^Heading(\d)$/.exec(b.style ?? "")?.[1] ? Number(/^Heading(\d)$/.exec(b.style!)![1]) - 1 : undefined);
    if (lvl === undefined || lvl >= maxLevel) return;
    const text = paragraphText(b).replace(/[￼\t\n]/g, " ").trim();
    if (text) out.push({ level: lvl + 1, text, block: i });
  });
  return out;
}

/** TOC paragraphs (field begin in the first, end in the last). */
export function buildToc(items: Heading[], pageOf: (block: number) => number, title = "Contents"): Block[] {
  const code = 'TOC \\o "1-3" \\h \\z \\u';
  const entries: Paragraph[] = items.map((h) => ({
    type: "paragraph",
    style: `TOC${Math.min(3, h.level)}`,
    inlines: [{ type: "text", text: h.text, marks: {} }, { type: "tab", marks: {} }, { type: "text", text: String(pageOf(h.block)), marks: {} }],
  }));
  if (entries.length === 0) entries.push({ type: "paragraph", inlines: [{ type: "text", text: "No headings found.", marks: { italic: true } }] });
  const first = entries[0]!;
  entries[0] = { ...first, inlines: [fieldBegin(), fieldInstr(code), fieldSeparate(), ...first.inlines] };
  const last = entries[entries.length - 1]!;
  entries[entries.length - 1] = { ...last, inlines: [...last.inlines, fieldEnd()] };
  return [{ type: "paragraph", style: "Title", inlines: [{ type: "text", text: title, marks: {} }] }, ...entries];
}

/** [start, end] block indices of the first TOC field, or null. */
export function findToc(blocks: Block[]): [number, number] | null {
  const start = blocks.findIndex((b) => isParagraph(b) && b.inlines.some((i) => instrOf(i)?.startsWith("TOC")));
  if (start < 0) return null;
  for (let j = start; j < blocks.length; j++) {
    const b = blocks[j]!;
    if (isParagraph(b) && b.inlines.some(isEnd)) return [start, j];
  }
  return [start, start];
}

// ---- citations ------------------------------------------------------------------

function lastName(a: string) {
  return a.split(",")[0]!.trim();
}

/** In-text citation, APA-like: (Smith, 2020) / (Smith & Doe, 2020) / (Smith et al., 2020). */
export function citationText(s: Source | undefined, tag: string): string {
  if (!s) return `(${tag})`;
  const names = s.authors.map(lastName);
  const who = names.length === 0 ? s.title || s.tag : names.length === 1 ? names[0] : names.length === 2 ? `${names[0]} & ${names[1]}` : `${names[0]} et al.`;
  return `(${who}${s.year ? `, ${s.year}` : ""})`;
}

/** Bibliography entry, APA-like. */
export function bibliographyEntry(s: Source): string {
  const authors = s.authors
    .map((a) => {
      const [last, first] = a.split(",").map((x) => x.trim());
      return first ? `${last}, ${first.split(/\s+/).map((n) => n[0] + ".").join(" ")}` : last;
    })
    .join(", ");
  const parts = [authors || s.title, s.year ? `(${s.year}).` : "(n.d.).", authors ? `${s.title}.` : "", s.publisher ? `${s.publisher}.` : "", s.url];
  return parts.filter(Boolean).join(" ");
}

export function citationField(s: Source | undefined, tag: string): Inline[] {
  return [fieldBegin(), fieldInstr(`CITATION ${tag} \\l 1033`), fieldSeparate(), { type: "text", text: citationText(s, tag), marks: {} }, fieldEnd()];
}

export function buildBibliography(sources: Source[], title = "References"): Block[] {
  const sorted = [...sources].sort((a, b) => (a.authors[0] ?? a.title).localeCompare(b.authors[0] ?? b.title));
  const entries: Paragraph[] = sorted.map((s) => ({ type: "paragraph", style: "Bibliography", inlines: [{ type: "text", text: bibliographyEntry(s), marks: {} }] }));
  if (!entries.length) entries.push({ type: "paragraph", style: "Bibliography", inlines: [{ type: "text", text: "There are no sources in the current document.", marks: { italic: true } }] });
  entries[0] = { ...entries[0]!, inlines: [fieldBegin(), fieldInstr("BIBLIOGRAPHY"), fieldSeparate(), ...entries[0]!.inlines] };
  const last = entries[entries.length - 1]!;
  entries[entries.length - 1] = { ...last, inlines: [...last.inlines, fieldEnd()] };
  return [{ type: "paragraph", style: "Heading1", inlines: [{ type: "text", text: title, marks: {} }] }, ...entries];
}

/** Refresh citation results and the bibliography / TOC fields. */
export function updateFields(doc: WordDocument, pageOf: (block: number) => number): WordDocument {
  const sources = doc.sources ?? [];
  // citations: replace the result text between separate and end
  let blocks = doc.blocks.map((b) => {
    if (!isParagraph(b)) return b;
    const inl = b.inlines.slice();
    for (let i = 0; i < inl.length; i++) {
      const code = instrOf(inl[i]!);
      const m = code && /^CITATION\s+(\S+)/.exec(code);
      if (!m) continue;
      const sep = inl.findIndex((x, k) => k > i && x.type === "opaque" && x.xml.includes('fldCharType="separate"'));
      const end = inl.findIndex((x, k) => k > sep && isEnd(x));
      if (sep < 0 || end < 0) continue;
      const src = sources.find((s) => s.tag === m[1]);
      inl.splice(sep + 1, end - sep - 1, { type: "text", text: citationText(src, m[1]!), marks: {} });
    }
    return { ...b, inlines: inl };
  });
  // bibliography
  const bibStart = blocks.findIndex((b) => isParagraph(b) && b.inlines.some((i) => instrOf(i)?.startsWith("BIBLIOGRAPHY")));
  if (bibStart >= 0) {
    let end = bibStart;
    while (end < blocks.length && !(isParagraph(blocks[end]!) && (blocks[end] as Paragraph).inlines.some(isEnd))) end++;
    const fresh = buildBibliography(sources).slice(1);
    blocks = [...blocks.slice(0, bibStart), ...fresh, ...blocks.slice(Math.min(end, blocks.length - 1) + 1)];
  }
  // table of contents (keep a preceding title paragraph)
  const toc = findToc(blocks);
  if (toc) {
    const fresh = buildToc(headings({ ...doc, blocks }), pageOf).slice(1);
    blocks = [...blocks.slice(0, toc[0]), ...fresh, ...blocks.slice(toc[1] + 1)];
  }
  return { ...doc, blocks };
}

export { isBegin };
