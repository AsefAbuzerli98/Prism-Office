/**
 * In-place text editing: a contenteditable overlay mirrors a shape's text
 * body (paragraph defaults on the block, a run's own properties as inline
 * CSS + a JSON copy), and is converted back to runs on every input. Inline
 * formatting applied by the browser (b/i/u, CSS spans, font colour) is read
 * back as run properties, so nothing the model knows is lost.
 */
import type { Palette } from "./engine";
import type { Fill, Paragraph, ParaProps, Run, RunProps, TextBody, Theme } from "./model";
import { effectivePara, type ResolvedShape } from "./resolve";
import { autoNumber, EMU_PER_PT, fontFamily } from "./text";

export interface EditEnv {
  palette: Palette;
  theme: Theme;
  /** CSS pixels per point. */
  scale: number;
  /** Default text colour (shape style), CSS. */
  color?: string;
  /** Autofit font scale (stored × live). */
  fontScale?: number;
}

function css(fill: Fill | undefined, env: EditEnv): string | null {
  if (!fill) return null;
  if (fill.t === "solid") return env.palette.css(fill.color);
  if (fill.t === "gradient" && fill.stops[0]) return env.palette.css(fill.stops[0].color);
  return null;
}

/** CSS declarations for run properties (explicit ones only). */
export function runCss(p: RunProps, env: EditEnv): Record<string, string> {
  const out: Record<string, string> = {};
  const k = env.fontScale ?? 1;
  if (p.size !== undefined) out["font-size"] = `${(p.size / 100) * k * env.scale}px`;
  if (p.bold !== undefined) out["font-weight"] = p.bold ? "700" : "400";
  if (p.italic !== undefined) out["font-style"] = p.italic ? "italic" : "normal";
  const deco = [p.underline && p.underline !== "none" ? "underline" : "", p.strike && p.strike !== "noStrike" ? "line-through" : ""].filter(Boolean).join(" ");
  if (p.underline !== undefined || p.strike !== undefined) out["text-decoration-line"] = deco || "none";
  const c = p.link ? env.palette.css({ base: { t: "scheme", v: "hlink" } }) : css(p.fill, env);
  if (c) out["color"] = c;
  if (p.font) out["font-family"] = fontFamily(p.font, env.theme);
  if (p.baseline) out["vertical-align"] = p.baseline > 0 ? "super" : "sub";
  if (p.cap === "all") out["text-transform"] = "uppercase";
  if (p.cap === "small") out["font-variant"] = "small-caps";
  if (p.highlight) out["background-color"] = env.palette.css(p.highlight);
  return out;
}

function styleString(d: Record<string, string>): string {
  return Object.entries(d)
    .map(([k, v]) => `${k}:${v}`)
    .join(";");
}

function spacingCss(s: ParaProps["spaceBefore"], sizePt: number, env: EditEnv): string {
  if (!s) return "0px";
  const v = s.t === "pts" ? s.v / 100 : (s.v / 100000) * sizePt * 1.2;
  return `${v * env.scale}px`;
}

/** Build the editable DOM for a text body inside `root`. */
export function renderEditable(root: HTMLElement, text: TextBody | undefined, res: ResolvedShape, env: EditEnv) {
  root.replaceChildren();
  const paras = text?.paragraphs?.length ? text.paragraphs : [{} as Paragraph];
  const counters = new Map<number, number>();
  const k = env.fontScale ?? 1;
  for (const p of paras) {
    const pp = effectivePara(res.lists, p.props);
    const base = pp.run ?? {};
    const size = (base.size ?? 1800) / 100;
    const div = document.createElement("div");
    div.dataset.pp = JSON.stringify(p.props ?? {});
    if (p.end) div.dataset.end = JSON.stringify(p.end);
    const lvl = p.props?.level ?? 0;
    const lineH = pp.lineSpacing?.t === "pts" ? `${(pp.lineSpacing.v / 100) * env.scale}px` : String(1.2 * (pp.lineSpacing?.t === "pct" ? pp.lineSpacing.v / 100000 : 1));
    const block: Record<string, string> = {
      "font-family": fontFamily(base.font, env.theme),
      "font-size": `${size * k * env.scale}px`,
      "font-weight": base.bold ? "700" : "400",
      "font-style": base.italic ? "italic" : "normal",
      color: (base.fill ? css(base.fill, env) : null) ?? env.color ?? "#000",
      "text-align": { l: "left", ctr: "center", r: "right", just: "justify", dist: "justify" }[pp.align ?? "l"] ?? "left",
      "line-height": lineH,
      "margin-top": spacingCss(pp.spaceBefore, size * k, env),
      "margin-bottom": spacingCss(pp.spaceAfter, size * k, env),
      "padding-left": `${((pp.marginLeft ?? 0) / EMU_PER_PT) * env.scale}px`,
      "text-indent": `${((pp.indent ?? 0) / EMU_PER_PT) * env.scale}px`,
    };
    const bu = pp.bullet;
    if (bu && bu.t !== "none") {
      let label = "";
      if (bu.t === "char") label = bu.ch;
      else {
        const n = (counters.get(lvl) ?? (bu.start ?? 1) - 1) + 1;
        counters.set(lvl, n);
        label = autoNumber(bu.scheme, n);
      }
      div.dataset.bullet = label + " ";
    } else counters.delete(lvl);
    div.setAttribute("style", styleString(block));
    for (const r of p.runs ?? []) {
      if (r.t === "break") {
        div.append(document.createElement("br"));
        continue;
      }
      const span = document.createElement("span");
      span.dataset.r = JSON.stringify(r.props ?? {});
      if (r.t === "field") span.dataset.field = JSON.stringify({ kind: r.kind, id: r.id });
      span.setAttribute("style", styleString(runCss(r.props ?? {}, env)));
      span.textContent = r.text;
      div.append(span);
    }
    if (!div.childNodes.length) div.append(document.createElement("br"));
    root.append(div);
  }
}

function hexOf(cssColor: string): string | null {
  const m = /rgba?\(\s*(\d+)[ ,]+(\d+)[ ,]+(\d+)/.exec(cssColor);
  if (m) return [m[1], m[2], m[3]].map((v) => Number(v).toString(16).padStart(2, "0")).join("").toUpperCase();
  const h = /^#([0-9a-f]{6})$/i.exec(cssColor.trim());
  if (h) return h[1]!.toUpperCase();
  const s = /^#([0-9a-f])([0-9a-f])([0-9a-f])$/i.exec(cssColor.trim());
  if (s) return (s[1]! + s[1] + s[2] + s[2] + s[3] + s[3]).toUpperCase();
  return null;
}

/** Inline CSS on an element → run property overrides (only where it differs from `known`). */
function cssOverrides(el: HTMLElement, props: RunProps, env: EditEnv): RunProps {
  const known = runCss(props, env);
  const out: RunProps = { ...props };
  const st = el.style;
  const get = (name: string) => st.getPropertyValue(name).trim();
  // browsers normalise what they store (#hex → rgb(), 700 → bold, quotes in families)
  const norm = (name: string, v: string) => {
    if (!v) return "";
    if (name.includes("color")) return hexOf(v) ?? v;
    if (name === "font-size") return String(Math.round(parseFloat(v) * 100) / 100);
    if (name === "font-weight") return /bold|[6-9]00/.test(v) ? "bold" : "normal";
    if (name === "font-family") return v.split(",")[0]!.replace(/["']/g, "").trim();
    return v.trim();
  };
  const changed = (name: string) => {
    const v = get(name);
    return v !== "" && norm(name, v) !== norm(name, known[name] ?? "");
  };
  if (changed("font-weight")) out.bold = /bold|[6-9]00/.test(get("font-weight"));
  if (changed("font-style")) out.italic = get("font-style") === "italic";
  const deco = get("text-decoration-line") || get("text-decoration");
  if (deco && deco !== (known["text-decoration-line"] ?? "")) {
    out.underline = deco.includes("underline") ? (props.underline && props.underline !== "none" ? props.underline : "sng") : "none";
    out.strike = deco.includes("line-through") ? "sngStrike" : props.strike ? "noStrike" : undefined;
    if (out.strike === undefined) delete out.strike;
  }
  if (changed("color")) {
    const hx = hexOf(get("color"));
    if (hx) out.fill = { t: "solid", color: { base: { t: "srgb", v: hx } } };
  }
  if (changed("font-size")) {
    const px = parseFloat(get("font-size"));
    if (px > 0) out.size = Math.round((px / env.scale / (env.fontScale ?? 1)) * 100);
  }
  if (changed("font-family")) {
    const f = get("font-family").split(",")[0]!.replace(/["']/g, "").trim();
    if (f) out.font = f;
  }
  if (changed("background-color")) {
    const hx = hexOf(get("background-color"));
    if (hx) out.highlight = { base: { t: "srgb", v: hx } };
  }
  return out;
}

function sameProps(a: RunProps, b: RunProps) {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** Read the edited DOM back into a text body (body props and list style kept from `orig`). */
export function readEditable(root: HTMLElement, orig: TextBody | undefined, env: EditEnv): TextBody {
  const paras: Paragraph[] = [];
  const blocks: { el: HTMLElement | null; nodes: Node[] }[] = [];
  let loose: Node[] = [];
  for (const n of [...root.childNodes]) {
    if (n instanceof HTMLElement && (n.tagName === "DIV" || n.tagName === "P")) {
      if (loose.length) (blocks.push({ el: null, nodes: loose }), (loose = []));
      blocks.push({ el: n, nodes: [...n.childNodes] });
    } else loose.push(n);
  }
  if (loose.length) blocks.push({ el: null, nodes: loose });
  for (const b of blocks) {
    const props: ParaProps | undefined = b.el?.dataset.pp ? (JSON.parse(b.el.dataset.pp) as ParaProps) : undefined;
    const end: RunProps | undefined = b.el?.dataset.end ? (JSON.parse(b.el.dataset.end) as RunProps) : undefined;
    const runs: Run[] = [];
    const push = (text: string, p: RunProps, field?: string) => {
      if (!text) return;
      const last = runs[runs.length - 1];
      if (!field && last && last.t === "text" && sameProps(last.props ?? {}, p)) last.text += text;
      else if (field) {
        const f = JSON.parse(field) as { kind: string; id: string };
        runs.push({ t: "field", kind: f.kind, id: f.id, text, props: p });
      } else runs.push({ t: "text", text, props: p });
    };
    const visit = (n: Node, p: RunProps, field?: string) => {
      if (n.nodeType === Node.TEXT_NODE) return push((n.textContent ?? "").replace(/​/g, ""), p, field);
      if (!(n instanceof HTMLElement)) return;
      if (n.tagName === "BR") {
        runs.push({ t: "break", props: p });
        return;
      }
      if (n.tagName === "DIV" || n.tagName === "P") {
        // nested block (paste): treat as a line break
        if (runs.length) runs.push({ t: "break", props: p });
      }
      let q = p;
      if (n.dataset.r !== undefined) q = JSON.parse(n.dataset.r) as RunProps;
      const tag = n.tagName;
      if (tag === "B" || tag === "STRONG") q = { ...q, bold: true };
      if (tag === "I" || tag === "EM") q = { ...q, italic: true };
      if (tag === "U") q = { ...q, underline: "sng" };
      if (tag === "S" || tag === "STRIKE" || tag === "DEL") q = { ...q, strike: "sngStrike" };
      if (tag === "FONT") {
        const c = n.getAttribute("color");
        const hx = c ? hexOf(c) : null;
        if (hx) q = { ...q, fill: { t: "solid", color: { base: { t: "srgb", v: hx } } } };
      }
      if (n.getAttribute("style")) q = cssOverrides(n, q, env);
      const f = n.dataset.field ?? field;
      for (const c of [...n.childNodes]) visit(c, q, f);
    };
    for (const n of b.nodes) visit(n, {}, undefined);
    // the browser keeps a trailing <br> in empty/ending lines
    while (runs.length && runs[runs.length - 1]!.t === "break") runs.pop();
    for (const r of runs) if (r.props && Object.keys(r.props).length === 0) delete r.props;
    const para: Paragraph = {};
    if (props && Object.keys(props).length) para.props = props;
    if (runs.length) para.runs = runs;
    if (end) para.end = end;
    paras.push(para);
  }
  if (!paras.length) paras.push({});
  return { ...(orig ?? {}), paragraphs: paras };
}

/** Set a paragraph property on the blocks touched by the current selection. */
export function setBlockProps(root: HTMLElement, f: (p: ParaProps) => ParaProps) {
  const sel = root.ownerDocument.getSelection();
  const blocks = [...root.children] as HTMLElement[];
  let touched = blocks.filter((b) => sel && sel.rangeCount && sel.getRangeAt(0).intersectsNode(b));
  if (!touched.length) touched = blocks;
  for (const b of touched) {
    const p = b.dataset.pp ? (JSON.parse(b.dataset.pp) as ParaProps) : {};
    b.dataset.pp = JSON.stringify(f(p));
  }
}
