/**
 * RTF ⇄ blocks for clipboard interop (P2-10). Supports the subset Office,
 * LibreOffice and macOS TextEdit put on the clipboard: paragraphs, line
 * breaks, tabs, bold/italic/underline/strike, font size, colour table,
 * super/subscript, \uN unicode escapes and \'hh code-page bytes (cp1252).
 * Destinations such as fonttbl, stylesheet, pict, info are skipped.
 */
import { isParagraph, isTable, type Block, type Inline, type Marks, type Paragraph } from "./model";
import { normalize } from "./ops";

const SKIP = new Set(["fonttbl", "stylesheet", "info", "pict", "colortbl", "listtable", "listoverridetable", "themedata", "colorschememapping", "latentstyles", "datastore", "xmlnstbl", "rsidtbl", "generator", "header", "footer", "object", "fldinst", "bkmkstart", "bkmkend", "filetbl", "revtbl", "pgdsctbl", "mmathPr"]);
const CP1252: Record<number, string> = { 0x80: "€", 0x82: "‚", 0x84: "„", 0x85: "…", 0x91: "‘", 0x92: "’", 0x93: "“", 0x94: "”", 0x95: "•", 0x96: "–", 0x97: "—", 0x99: "™" };

interface State {
  marks: Marks;
  skip: boolean;
  uc: number;
}

export function rtfToBlocks(rtf: string): Block[] {
  if (!rtf.trimStart().startsWith("{\\rtf")) return [];
  const colors: string[] = [];
  const ct = /\{\\colortbl;?([^}]*)\}/.exec(rtf);
  if (ct) {
    for (const entry of ct[1]!.split(";")) {
      const r = /\\red(\d+)/.exec(entry);
      const g = /\\green(\d+)/.exec(entry);
      const b = /\\blue(\d+)/.exec(entry);
      if (r && g && b) colors.push([r, g, b].map((m) => Number(m[1]).toString(16).padStart(2, "0")).join("").toUpperCase());
      else if (entry.trim() === "" && colors.length === 0) continue;
    }
  }
  const out: Paragraph[] = [];
  let cur: Inline[] = [];
  let align: Paragraph["align"];
  const stack: State[] = [];
  let st: State = { marks: {}, skip: false, uc: 1 };
  let pendingSkip = 0;
  const pushText = (t: string) => {
    if (st.skip || !t) return;
    if (pendingSkip > 0) {
      const n = Math.min(pendingSkip, t.length);
      t = t.slice(n);
      pendingSkip -= n;
      if (!t) return;
    }
    cur.push({ type: "text", text: t, marks: { ...st.marks } });
  };
  const endPara = () => {
    out.push({ type: "paragraph", ...(align ? { align } : {}), inlines: normalize(cur) });
    cur = [];
  };
  let i = 0;
  let starDest = false;
  while (i < rtf.length) {
    const c = rtf[i]!;
    if (c === "{") {
      stack.push(st);
      st = { ...st, marks: { ...st.marks } };
      i++;
      starDest = false;
      continue;
    }
    if (c === "}") {
      st = stack.pop() ?? st;
      i++;
      continue;
    }
    if (c === "\r" || c === "\n") {
      i++;
      continue;
    }
    if (c !== "\\") {
      let j = i;
      while (j < rtf.length && !"\\{}\r\n".includes(rtf[j]!)) j++;
      pushText(rtf.slice(i, j));
      i = j;
      continue;
    }
    // control sequence
    const next = rtf[i + 1] ?? "";
    if (next === "\\" || next === "{" || next === "}") {
      pushText(next);
      i += 2;
      continue;
    }
    if (next === "'") {
      const code = parseInt(rtf.slice(i + 2, i + 4), 16);
      pushText(CP1252[code] ?? String.fromCharCode(code));
      i += 4;
      continue;
    }
    if (next === "*") {
      starDest = true;
      i += 2;
      continue;
    }
    if (next === "~") {
      pushText("\u00A0");
      i += 2;
      continue;
    }
    if (next === "-") {
      pushText("\u00AD");
      i += 2;
      continue;
    }
    const m = /^\\([a-zA-Z]+)(-?\d+)? ?/.exec(rtf.slice(i, i + 40));
    if (!m) {
      i += 2;
      continue;
    }
    i += m[0].length;
    const word = m[1]!;
    const n = m[2] !== undefined ? Number(m[2]) : undefined;
    if (starDest || SKIP.has(word)) {
      st.skip = true;
      starDest = false;
      continue;
    }
    if (st.skip) continue;
    switch (word) {
      case "par":
        endPara();
        break;
      case "pard":
        align = undefined;
        break;
      case "line":
        cur.push({ type: "break", kind: "line" });
        break;
      case "page":
        cur.push({ type: "break", kind: "page" });
        break;
      case "tab":
        cur.push({ type: "tab", marks: { ...st.marks } });
        break;
      case "qc":
        align = "center";
        break;
      case "qr":
        align = "right";
        break;
      case "qj":
        align = "justify";
        break;
      case "ql":
        align = undefined;
        break;
      case "b":
        st.marks.bold = n === 0 ? undefined : true;
        break;
      case "i":
        st.marks.italic = n === 0 ? undefined : true;
        break;
      case "ul":
        st.marks.underline = n === 0 ? undefined : true;
        break;
      case "ulnone":
        st.marks.underline = undefined;
        break;
      case "strike":
        st.marks.strike = n === 0 ? undefined : true;
        break;
      case "super":
        st.marks.vertAlign = "superscript";
        break;
      case "sub":
        st.marks.vertAlign = "subscript";
        break;
      case "nosupersub":
        st.marks.vertAlign = undefined;
        break;
      case "fs":
        if (n) st.marks.size = n;
        break;
      case "cf":
        st.marks.color = n ? colors[n - 1] : undefined;
        break;
      case "plain":
        st.marks = {};
        break;
      case "uc":
        st.uc = n ?? 1;
        break;
      case "u":
        if (n !== undefined) {
          pushText(String.fromCharCode(n < 0 ? n + 65536 : n));
          pendingSkip = st.uc;
        }
        break;
      case "emdash":
        pushText("—");
        break;
      case "endash":
        pushText("–");
        break;
      case "bullet":
        pushText("•");
        break;
      case "lquote":
        pushText("‘");
        break;
      case "rquote":
        pushText("’");
        break;
      case "ldblquote":
        pushText("“");
        break;
      case "rdblquote":
        pushText("”");
        break;
    }
  }
  if (cur.length) endPara();
  // clean undefined marks
  return out.map((p) => ({
    ...p,
    inlines: normalize(
      p.inlines.map((x) => (x.type === "text" || x.type === "tab" ? { ...x, marks: Object.fromEntries(Object.entries(x.marks).filter(([, v]) => v !== undefined)) as Marks } : x)),
    ),
  }));
}

const escRtf = (s: string) =>
  Array.from(s)
    .map((ch) => {
      const c = ch.codePointAt(0)!;
      if (ch === "\\" || ch === "{" || ch === "}") return `\\${ch}`;
      if (ch === "\t") return "\\tab ";
      if (c > 127) return c > 0xffff ? Array.from(String.fromCodePoint(c)).map(() => "?").join("") : `\\u${c > 32767 ? c - 65536 : c}?`;
      return ch;
    })
    .join("");

export function blocksToRtf(blocks: Block[]): string {
  const colors: string[] = [];
  const color = (hex: string) => {
    let i = colors.indexOf(hex);
    if (i < 0) i = colors.push(hex) - 1;
    return i + 1;
  };
  const paras: string[] = [];
  const emit = (bs: Block[]) => {
    for (const b of bs) {
      if (isTable(b)) {
        for (const r of b.rows) emit(r.cells.flatMap((c) => c.blocks));
        continue;
      }
      if (!isParagraph(b)) continue;
      const q = b.align === "center" ? "\\qc" : b.align === "right" ? "\\qr" : b.align === "justify" ? "\\qj" : "\\ql";
      const body = b.inlines
        .map((i) => {
          if (i.type === "tab") return "\\tab ";
          if (i.type === "break") return i.kind === "page" ? "\\page " : "\\line ";
          if (i.type !== "text") return "";
          const m = i.marks;
          const f = [m.bold && "\\b", m.italic && "\\i", m.underline && "\\ul", m.strike && "\\strike", m.vertAlign === "superscript" && "\\super", m.vertAlign === "subscript" && "\\sub", m.size && `\\fs${m.size}`, m.color && `\\cf${color(m.color)}`]
            .filter(Boolean)
            .join("");
          return f ? `{${f} ${escRtf(i.text)}}` : escRtf(i.text);
        })
        .join("");
      paras.push(`\\pard${q} ${body}\\par`);
    }
  };
  emit(blocks);
  const ctbl = colors.length ? `{\\colortbl;${colors.map((c) => `\\red${parseInt(c.slice(0, 2), 16)}\\green${parseInt(c.slice(2, 4), 16)}\\blue${parseInt(c.slice(4, 6), 16)};`).join("")}}` : "";
  return `{\\rtf1\\ansi\\ansicpg1252\\deff0{\\fonttbl{\\f0 Calibri;}}${ctbl}\\fs22 ${paras.join("\n")}}`;
}
