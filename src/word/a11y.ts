/** Accessibility checker (P2-16). */
import { contrastRatio } from "../core/theme";
import { effectiveMarks } from "./layout";
import { isParagraph, isTable, type Block, type Paragraph, type WordDocument } from "./model";
import { paragraphText, type Pos } from "./ops";

export interface A11yIssue {
  kind: "alt-text" | "heading-order" | "empty-heading" | "contrast" | "table-header" | "link-text" | "small-text" | "no-headings";
  severity: "error" | "warning" | "tip";
  message: string;
  /** Body position to jump to. */
  pos?: Pos;
  /** For alt-text issues: the picture's relationship id. */
  relId?: string;
}

const HIGHLIGHT_HEX: Record<string, string> = { yellow: "#ffff00", green: "#00ff00", cyan: "#00ffff", magenta: "#ff00ff", blue: "#0000ff", red: "#ff0000", darkBlue: "#000080", darkCyan: "#008080", darkGreen: "#008000", darkMagenta: "#800080", darkRed: "#800000", darkYellow: "#808000", darkGray: "#808080", lightGray: "#c0c0c0", black: "#000000" };
const VAGUE_LINKS = /^(click here|here|link|more|read more|this|this link|bura|burada|ətraflı|daha çox)$/i;

function headingLevel(doc: WordDocument, p: Paragraph): number | null {
  const style = doc.styles.find((s) => s.id === p.style);
  const lvl = p.outlineLevel ?? style?.para.outlineLevel;
  if (lvl !== undefined && lvl < 9) return lvl + 1;
  const m = /^Heading(\d)$/.exec(p.style ?? "");
  return m ? Number(m[1]) : null;
}

export function checkAccessibility(doc: WordDocument): A11yIssue[] {
  const issues: A11yIssue[] = [];
  let prevLevel = 0;
  let headings = 0;
  const visit = (blocks: Block[], top: number | null, bg: string) => {
    blocks.forEach((b, bi) => {
      const block = top ?? bi;
      if (isTable(b)) {
        if (top === null && b.rows.length > 1 && !b.rows[0]?.header) {
          issues.push({ kind: "table-header", severity: "warning", message: "Table has no header row; mark the first row as a header so screen readers can announce columns.", pos: { block, offset: 0 } });
        }
        for (const r of b.rows) for (const c of r.cells) visit(c.blocks, block, c.shading ? `#${c.shading}` : bg);
        return;
      }
      if (!isParagraph(b)) return;
      const text = paragraphText(b).replace(/￼/g, "").trim();
      const lvl = top === null ? headingLevel(doc, b) : null;
      if (lvl !== null) {
        headings++;
        if (!text) issues.push({ kind: "empty-heading", severity: "warning", message: "Empty heading.", pos: { block, offset: 0 } });
        if (prevLevel && lvl > prevLevel + 1) issues.push({ kind: "heading-order", severity: "warning", message: `Heading level jumps from ${prevLevel} to ${lvl}; don't skip levels.`, pos: { block, offset: 0 } });
        prevLevel = lvl;
      }
      let offset = 0;
      for (const i of b.inlines) {
        if (i.type === "image" && !i.descr.trim()) {
          issues.push({ kind: "alt-text", severity: "error", message: "Picture is missing alternative text.", pos: { block, offset }, relId: i.relId });
        }
        if (i.type === "text" && i.text.trim()) {
          const m = effectiveMarks(doc, b, i.marks);
          const fg = `#${(m.color ?? "000000").replace(/^auto$/i, "000000")}`;
          const back = m.highlight ? HIGHLIGHT_HEX[m.highlight] ?? bg : bg;
          const sizePt = (m.size ?? 22) / 2;
          const large = sizePt >= 18 || (sizePt >= 14 && !!m.bold);
          if (/^#[0-9a-f]{6}$/i.test(fg) && /^#[0-9a-f]{6}$/i.test(back)) {
            const ratio = contrastRatio(fg, back);
            const need = large ? 3 : 4.5;
            if (ratio < need) issues.push({ kind: "contrast", severity: "error", message: `Low contrast ${ratio.toFixed(2)}:1 (needs ${need}:1) for “${i.text.trim().slice(0, 30)}”.`, pos: { block, offset } });
          }
          if (sizePt < 9) issues.push({ kind: "small-text", severity: "tip", message: `Text at ${sizePt}pt may be hard to read.`, pos: { block, offset } });
          if (m.link && (VAGUE_LINKS.test(i.text.trim()) || /^https?:\/\/\S{40,}$/.test(i.text.trim()))) {
            issues.push({ kind: "link-text", severity: "warning", message: `Link text “${i.text.trim().slice(0, 40)}” doesn't describe its destination.`, pos: { block, offset } });
          }
        }
        offset += i.type === "text" ? i.text.length : 1;
      }
    });
  };
  visit(doc.blocks, null, "#ffffff");
  const words = doc.blocks.filter(isParagraph).reduce((n, p) => n + (paragraphText(p).match(/\S+/g)?.length ?? 0), 0);
  if (!headings && words > 400) issues.push({ kind: "no-headings", severity: "tip", message: "Long document without headings; add headings to aid navigation." });
  return issues;
}

function syllables(word: string): number {
  const w = word.toLowerCase().replace(/[^a-zəöüğışç]/g, "");
  if (!w) return 0;
  const groups = w.replace(/(?:[^laeiouyəöüı]es|ed|[^laeiouyəöüı]e)$/, "").match(/[aeiouyəöüı]+/g);
  return Math.max(1, groups?.length ?? 1);
}

export interface Readability {
  words: number;
  sentences: number;
  /** Flesch Reading Ease (meaningful for English). */
  flesch: number;
  /** LIX (language-independent): < 30 very easy … > 60 very hard. */
  lix: number;
  grade: "very easy" | "easy" | "standard" | "difficult" | "very difficult";
}

export function readability(doc: WordDocument): Readability {
  const text = doc.blocks.filter(isParagraph).map((p) => paragraphText(p)).join("\n");
  const words = text.match(/[\p{L}\p{N}'’-]+/gu) ?? [];
  const sentences = Math.max(1, (text.match(/[.!?…]+(\s|$)/g) ?? []).length || (words.length ? 1 : 0));
  const n = Math.max(1, words.length);
  const syl = words.reduce((s, w) => s + syllables(w), 0);
  const long = words.filter((w) => w.length > 6).length;
  const flesch = 206.835 - 1.015 * (n / sentences) - 84.6 * (syl / n);
  const lix = n / sentences + (long * 100) / n;
  const grade = lix < 30 ? "very easy" : lix < 40 ? "easy" : lix < 50 ? "standard" : lix < 60 ? "difficult" : "very difficult";
  return { words: words.length, sentences, flesch: Math.round(flesch * 10) / 10, lix: Math.round(lix * 10) / 10, grade };
}
