/**
 * DrawingML text layout for the slide canvas: paragraphs with cascaded
 * properties, wrapping, bullets/numbering, spacing, alignment, anchoring
 * and normAutofit. World units are points.
 */
import type { Palette } from "./engine";
import type { Bullet, Color, Paragraph, ParaProps, Run, RunProps, Spacing, TextBody, Theme } from "./model";
import { effectivePara, mergeRun, type ResolvedShape } from "./resolve";

export const EMU_PER_PT = 12700;
const DEFAULT_SIZE = 1800;

export interface TextEnv {
  palette: Palette;
  theme: Theme;
  /** Default text colour (shape style fontRef), CSS. */
  color?: string;
  slideNumber?: number;
  /** Scale applied on top of the stored autofit (editor shrink-on-overflow). */
  extraScale?: number;
}

export interface Glyphs {
  x: number;
  w: number;
  text: string;
  font: string;
  size: number;
  color: string;
  underline: boolean;
  strike: boolean;
  baselineShift: number;
  highlight: string | null;
  letterSpacing: number;
  link: boolean;
}

export interface TextLine {
  y: number;
  height: number;
  ascent: number;
  items: Glyphs[];
  /** Paragraph index and the bullet (drawn at `bulletX`). */
  para: number;
  bullet: Glyphs | null;
}

export interface TextLayout {
  lines: TextLine[];
  height: number;
  /** Text box (inside insets) in points relative to the shape box. */
  box: { x: number; y: number; w: number; h: number };
  /** Contents overflow the box (before any extra shrink). */
  overflow: boolean;
}

export function fontFamily(face: string | undefined, theme: Theme): string {
  let f = face ?? "+mn-lt";
  if (f.startsWith("+mj")) f = theme.majorFont || "Calibri Light";
  else if (f.startsWith("+mn")) f = theme.minorFont || "Calibri";
  const fallback: Record<string, string> = {
    Calibri: '"Carlito"',
    "Calibri Light": '"Carlito"',
    Cambria: '"Caladea"',
    Arial: '"Liberation Sans"',
    "Times New Roman": '"Liberation Serif"',
    "Courier New": '"Liberation Mono"',
  };
  const fb = fallback[f] ? `, ${fallback[f]}` : "";
  return `"${f}"${fb}, "Segoe UI", system-ui, sans-serif`;
}

function fillColor(fill: RunProps["fill"], env: TextEnv): string | null {
  if (!fill) return null;
  if (fill.t === "solid") return env.palette.css(fill.color);
  if (fill.t === "gradient" && fill.stops[0]) return env.palette.css(fill.stops[0].color);
  if (fill.t === "none") return "rgba(0,0,0,0)";
  return null;
}

function spacingPt(s: Spacing | undefined, size: number): number {
  if (!s) return 0;
  return s.t === "pts" ? s.v / 100 : (s.v / 100000) * size * 1.2;
}

const ROMAN: [number, string][] = [
  [1000, "m"], [900, "cm"], [500, "d"], [400, "cd"], [100, "c"], [90, "xc"],
  [50, "l"], [40, "xl"], [10, "x"], [9, "ix"], [5, "v"], [4, "iv"], [1, "i"],
];

export function autoNumber(scheme: string, n: number): string {
  let core: string;
  if (scheme.startsWith("roman")) {
    let v = n;
    core = "";
    for (const [k, s] of ROMAN) while (v >= k) ((core += s), (v -= k));
    if (scheme.startsWith("romanUc")) core = core.toUpperCase();
  } else if (scheme.startsWith("alpha")) {
    core = "";
    let v = n;
    while (v > 0) {
      v--;
      core = String.fromCharCode(97 + (v % 26)) + core;
      v = Math.floor(v / 26);
    }
    if (scheme.startsWith("alphaUc")) core = core.toUpperCase();
  } else core = String(n);
  if (scheme.endsWith("ParenBoth")) return `(${core})`;
  if (scheme.endsWith("ParenR")) return `${core})`;
  if (scheme.endsWith("Plain")) return core;
  if (scheme.endsWith("Minus")) return `- ${core} -`;
  return `${core}.`;
}

let measureCtx: CanvasRenderingContext2D | null | undefined;
function measurer(): CanvasRenderingContext2D | null {
  if (measureCtx === undefined) {
    const c = typeof OffscreenCanvas !== "undefined" ? new OffscreenCanvas(4, 4) : typeof document !== "undefined" ? document.createElement("canvas") : null;
    measureCtx = (c?.getContext("2d") as CanvasRenderingContext2D | null) ?? null;
  }
  return measureCtx;
}

/** Measure width (points) of text in a CSS font whose size is in points. */
export function measure(text: string, font: string, letterSpacing = 0): number {
  const m = measurer();
  if (!m) {
    // no canvas (tests): average glyph ≈ half the font size
    const size = parseFloat(/([\d.]+)px/.exec(font)?.[1] ?? "18");
    return text.length * size * 0.5 + letterSpacing * text.length;
  }
  m.font = font;
  return m.measureText(text).width + letterSpacing * text.length;
}

interface Piece {
  text: string;
  props: RunProps;
  field?: boolean;
  brk?: boolean;
}

function runText(r: Run, env: TextEnv): string {
  if (r.t === "break") return "";
  if (r.t === "field") {
    if (r.kind === "slidenum" && env.slideNumber !== undefined) return String(env.slideNumber);
    if (r.kind.startsWith("datetime")) return new Date().toLocaleDateString();
  }
  return r.text;
}

function styleOf(props: RunProps, env: TextEnv, scale: number): Omit<Glyphs, "x" | "w" | "text"> {
  const size = ((props.size ?? DEFAULT_SIZE) / 100) * scale;
  const small = props.cap === "small";
  const sup = props.baseline ?? 0;
  const eff = sup !== 0 ? size * 0.67 : size;
  const font = `${props.italic ? "italic " : ""}${small ? "small-caps " : ""}${props.bold ? "bold " : ""}${eff}px ${fontFamily(props.font, env.theme)}`;
  return {
    font,
    size: eff,
    // hyperlinks always take the theme's hlink colour (classic PowerPoint behaviour)
    color: props.link ? env.palette.css({ base: { t: "scheme", v: "hlink" } }) : (fillColor(props.fill, env) ?? env.color ?? "#000000"),
    underline: (props.underline !== undefined && props.underline !== "none") || !!props.link,
    strike: props.strike !== undefined && props.strike !== "noStrike",
    baselineShift: (-sup / 100000) * size,
    highlight: props.highlight ? env.palette.css(props.highlight) : null,
    letterSpacing: ((props.spacing ?? 0) / 100) * scale,
    link: !!props.link,
  };
}

function transformCase(t: string, props: RunProps) {
  return props.cap === "all" ? t.toUpperCase() : t;
}

/** Lay out a text body inside a shape box of `w`×`h` points. */
export function layoutText(text: TextBody | undefined, res: ResolvedShape, w: number, h: number, env: TextEnv, textRect?: [number, number, number, number]): TextLayout {
  const b = res.body;
  const ins = b.insets ?? [null, null, null, null];
  const [l, t, r, bt] = [ins[0] ?? 91440, ins[1] ?? 45720, ins[2] ?? 91440, ins[3] ?? 45720].map((v) => v / EMU_PER_PT) as [number, number, number, number];
  const [rx, ry, rw, rh] = textRect ?? [0, 0, w, h];
  const box = { x: rx + l, y: ry + t, w: Math.max(rw - l - r, 1), h: Math.max(rh - t - bt, 0) };
  const wrap = b.wrap !== "none";
  const af = b.autofit;
  const storedScale = af?.t === "normal" ? (af.fontScale ?? 100000) / 100000 : 1;
  const lnRed = af?.t === "normal" ? (af.lineReduction ?? 0) / 100000 : 0;
  const scale = storedScale * (env.extraScale ?? 1);
  const paras = text?.paragraphs?.length ? text.paragraphs : [{ runs: [] } as Paragraph];

  const lines: TextLine[] = [];
  let y = 0;
  const counters = new Map<number, number>();
  paras.forEach((p, pi) => {
    const pp: ParaProps = effectivePara(res.lists, p.props);
    const lvl = p.props?.level ?? 0;
    const baseRun = pp.run ?? {};
    const pieces: Piece[] = (p.runs ?? []).map((run) => ({
      text: transformCase(runText(run, env), mergeRun(baseRun, run.props)),
      props: mergeRun(baseRun, run.props),
      field: run.t === "field",
      brk: run.t === "break",
    }));
    const endProps = mergeRun(baseRun, p.end);
    const empty = !pieces.some((x) => x.text.length > 0);
    const firstProps = pieces.find((x) => x.text.length > 0)?.props ?? endProps;
    const paraSize = ((firstProps.size ?? DEFAULT_SIZE) / 100) * scale;
    const lnPct = pp.lineSpacing?.t === "pct" ? pp.lineSpacing.v / 100000 : 1;
    const lineH = (base: number) => (pp.lineSpacing?.t === "pts" ? (pp.lineSpacing.v / 100) * scale : base * 1.2 * lnPct * (1 - lnRed));
    y += spacingPt(pp.spaceBefore, paraSize) * (pp.spaceBefore?.t === "pts" ? scale : 1);

    // bullet
    let bullet: Glyphs | null = null;
    const bu: Bullet | undefined = pp.bullet;
    if (bu && bu.t !== "none" && !empty) {
      let txt = "";
      if (bu.t === "char") txt = bu.ch;
      else {
        const n = (counters.get(lvl) ?? (bu.start ?? 1) - 1) + 1;
        counters.set(lvl, n);
        txt = autoNumber(bu.scheme, n);
      }
      const st = styleOf(firstProps, env, scale);
      if (bu.t === "char") {
        const sz = bu.size ? (st.size * bu.size) / 100000 : st.size;
        st.font = `${sz}px ${bu.font ? `"${bu.font}", ` : ""}${fontFamily(firstProps.font, env.theme)}`;
        st.size = sz;
        if (bu.color) st.color = env.palette.css(bu.color as Color);
      }
      st.underline = st.strike = false;
      bullet = { ...st, x: 0, w: measure(txt, st.font), text: txt };
    } else if (!bu || bu.t === "none") counters.delete(lvl);

    const marL = (pp.marginLeft ?? 0) / EMU_PER_PT;
    const indent = (pp.indent ?? 0) / EMU_PER_PT;
    const firstX = marL + indent;
    if (bullet) {
      bullet.x = firstX;
    }
    const firstTextX = bullet ? Math.max(marL, firstX + bullet.w + 2) : firstX;
    const align = pp.align ?? "l";

    // break into words
    type Tok = { text: string; props: RunProps; space: boolean; brk?: boolean };
    const toks: Tok[] = [];
    for (const pc of pieces) {
      if (pc.brk) {
        toks.push({ text: "", props: pc.props, space: false, brk: true });
        continue;
      }
      for (const part of pc.text.split(/(\s+)/)) if (part) toks.push({ text: part, props: pc.props, space: /^\s+$/.test(part) });
    }

    let cur: Glyphs[] = [];
    let x = firstTextX;
    let lineStart = firstTextX;
    let first = true;
    const avail = () => box.w - lineStart;
    const flush = (endOfPara: boolean) => {
      // trim trailing spaces
      while (cur.length && /^\s+$/.test(cur[cur.length - 1]!.text)) cur.pop();
      const maxSize = Math.max(cur.reduce((m, g) => Math.max(m, g.size), 0), cur.length ? 0 : ((endProps.size ?? firstProps.size ?? DEFAULT_SIZE) / 100) * scale);
      const height = lineH(maxSize);
      const used = cur.length ? cur[cur.length - 1]!.x + cur[cur.length - 1]!.w - lineStart : 0;
      const slack = box.w - lineStart - used;
      let dx = 0;
      if (align === "ctr") dx = slack / 2;
      else if (align === "r") dx = slack;
      else if ((align === "just" || align === "dist") && !endOfPara && cur.length > 1) {
        const gaps = cur.filter((g) => /^\s+$/.test(g.text)).length;
        if (gaps > 0) {
          const per = slack / gaps;
          let acc = 0;
          for (const g of cur) {
            g.x += acc;
            if (/^\s+$/.test(g.text)) acc += per;
          }
        }
      }
      for (const g of cur) g.x += dx;
      lines.push({ y, height, ascent: maxSize * 0.92 + (height - maxSize * 1.2) / 2 + maxSize * 0.1, items: cur, para: pi, bullet: first ? (bullet ? { ...bullet, x: bullet.x + (align === "ctr" ? dx : align === "r" ? dx : 0) } : null) : null });
      y += height;
      cur = [];
      first = false;
      lineStart = marL;
      x = marL;
    };
    for (const tk of toks) {
      if (tk.brk) {
        flush(false);
        continue;
      }
      const st = styleOf(tk.props, env, scale);
      let wdt = measure(tk.text, st.font, st.letterSpacing);
      if (wrap && !tk.space && x + wdt > box.w + 0.01 && cur.some((g) => !/^\s+$/.test(g.text))) {
        flush(false);
      }
      if (tk.space && cur.length === 0 && !first) continue;
      if (wrap && !tk.space && wdt > avail()) {
        // break an over-long word by characters
        let rest = tk.text;
        while (rest.length) {
          let n = rest.length;
          while (n > 1 && measure(rest.slice(0, n), st.font, st.letterSpacing) > box.w - x) n--;
          const chunk = rest.slice(0, n);
          const cw = measure(chunk, st.font, st.letterSpacing);
          cur.push({ ...st, x, w: cw, text: chunk });
          x += cw;
          rest = rest.slice(n);
          if (rest.length) flush(false);
        }
        continue;
      }
      cur.push({ ...st, x, w: wdt, text: tk.text });
      x += wdt;
      wdt = 0;
    }
    flush(true);
    y += spacingPt(pp.spaceAfter, paraSize) * (pp.spaceAfter?.t === "pts" ? scale : 1);
  });
  const height = y;
  const overflow = height > box.h + 0.5;
  let oy = box.y;
  const anchor = b.anchor ?? "t";
  if (anchor === "ctr") oy = box.y + (box.h - height) / 2;
  else if (anchor === "b") oy = box.y + box.h - height;
  for (const ln of lines) ln.y += oy;
  return { lines, height, box, overflow };
}

/** Shrink factor so text fits (normAutofit), in steps like PowerPoint. */
export function fitScale(text: TextBody | undefined, res: ResolvedShape, w: number, h: number, env: TextEnv): number {
  for (let s = 1; s >= 0.25; s -= 0.075) {
    const lay = layoutText(text, res, w, h, { ...env, extraScale: s });
    if (!lay.overflow) return s;
  }
  return 0.25;
}

export function drawTextLayout(ctx: CanvasRenderingContext2D, lay: TextLayout) {
  ctx.textBaseline = "alphabetic";
  ctx.textAlign = "left";
  for (const ln of lay.lines) {
    const base = ln.y + ln.ascent;
    const all = ln.bullet ? [ln.bullet, ...ln.items] : ln.items;
    for (const g of all) {
      const gx = lay.box.x + g.x;
      if (g.highlight) {
        ctx.fillStyle = g.highlight;
        ctx.fillRect(gx, ln.y, g.w, ln.height);
      }
      ctx.font = g.font;
      ctx.fillStyle = g.color;
      if ("letterSpacing" in ctx) (ctx as unknown as { letterSpacing: string }).letterSpacing = `${g.letterSpacing}px`;
      ctx.fillText(g.text, gx, base + g.baselineShift);
      if (g.underline || g.strike) {
        ctx.fillRect(gx, base + g.baselineShift + (g.underline ? g.size * 0.12 : -g.size * 0.3), g.w, Math.max(g.size / 16, 0.5));
      }
    }
  }
  if ("letterSpacing" in ctx) (ctx as unknown as { letterSpacing: string }).letterSpacing = "0px";
}
