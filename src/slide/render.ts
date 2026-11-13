/**
 * Canvas 2D slide renderer. World units are points (1/72 in); the caller
 * sets the transform (zoom × devicePixelRatio). Inheritance is resolved by
 * resolve.ts, geometry and colours come from the drawingml WASM.
 */
import { evaluate, Palette } from "./engine";
import type { Background, Cmd, Effects, Fill, Line, ListStyle, ParaProps, PresentationModel, ShapeBody, Slide, SlideNode, Table, Xfrm } from "./model";
import { effectiveBackground, hasText, placeholderPrompt, resolveShape, slideContext, type ResolvedShape, type SlideContext } from "./resolve";
import { drawTextLayout, EMU_PER_PT, fitScale, layoutText, type TextEnv } from "./text";
import { drawChart } from "../sheet/charts";
import type { SlideChart } from "./chart";

export { EMU_PER_PT };

/** Per-shape animation state applied while rendering (slideshow). */
export interface ShapeAnim {
  opacity?: number;
  /** Offset in points. */
  dx?: number;
  dy?: number;
  scale?: number;
  /** Degrees. */
  rotate?: number;
  /** Reveal clip in 0..1 of the box: [x0, y0, x1, y1]. */
  clip?: [number, number, number, number];
  hidden?: boolean;
}

export interface RenderOptions {
  images?: (part: string) => CanvasImageSource | null;
  /** Parsed chart parts (by part name) for chart frames. */
  charts?: (part: string) => SlideChart | null;
  /** Show prompts/outlines of empty placeholders. */
  editing?: boolean;
  slideNumber?: number;
  /** Shape ids not to draw (e.g. the one being text-edited). */
  skip?: Set<number>;
  /** Shape ids whose text is not drawn (text being edited in an overlay). */
  skipText?: Set<number>;
  anim?: (id: number) => ShapeAnim | null;
  /** Draw only the slide's own shapes: no background, master or layout art (Morph layer). */
  shapesOnly?: boolean;
  /** Shrink overflowing normAutofit text (editor live autofit). */
  liveAutofit?: boolean;
}

const palettes = new Map<string, Palette>();
export function paletteFor(ctx: SlideContext): Palette {
  const key = JSON.stringify([ctx.master.theme, ctx.master.clrMap]);
  let p = palettes.get(key);
  if (!p) {
    if (palettes.size > 16) {
      for (const v of palettes.values()) v.dispose();
      palettes.clear();
    }
    p = new Palette(ctx.master);
    palettes.set(key, p);
  }
  return p;
}

interface Env {
  model: PresentationModel;
  ctx: SlideContext;
  palette: Palette;
  opts: RenderOptions;
  level: "slide" | "layout" | "master";
}

const pt = (emu: number) => emu / EMU_PER_PT;

/** Draw one slide; `ctx` must map points to device pixels. */
export function drawSlide(ctx: CanvasRenderingContext2D, model: PresentationModel, slide: Slide, opts: RenderOptions = {}) {
  const sc = slideContext(model, slide);
  const env: Env = { model, ctx: sc, palette: paletteFor(sc), opts, level: "slide" };
  const W = pt(model.width);
  const H = pt(model.height);
  ctx.save();
  ctx.beginPath();
  ctx.rect(0, 0, W, H);
  ctx.clip();
  if (!opts.shapesOnly) {
    drawBackground(ctx, effectiveBackground(slide, sc), W, H, env);
    if (sc.layout.showMasterShapes !== false) {
      for (const n of sc.master.shapes) if (!n.ph) drawNode(ctx, n, { ...env, level: "master" });
    }
    for (const n of sc.layout.shapes) if (!n.ph) drawNode(ctx, n, { ...env, level: "layout" });
  }
  for (const n of slide.shapes) drawNode(ctx, n, env);
  ctx.restore();
}

/** Draw a layout or master itself (layout picker thumbnails). */
export function drawLayout(ctx: CanvasRenderingContext2D, model: PresentationModel, masterIndex: number, layoutIndex: number) {
  const master = model.masters[masterIndex]!;
  const layout = master.layouts[layoutIndex]!;
  const sc: SlideContext = { master, layout, masterIndex };
  const env: Env = { model, ctx: sc, palette: paletteFor(sc), opts: { editing: true }, level: "layout" };
  const W = pt(model.width);
  const H = pt(model.height);
  drawBackground(ctx, layout.background ?? master.background ?? null, W, H, env);
  if (layout.showMasterShapes !== false) for (const n of master.shapes) if (!n.ph) drawNode(ctx, n, { ...env, level: "master" });
  for (const n of layout.shapes) {
    if (n.ph && ["dt", "ftr", "sldNum"].includes(n.ph.kind ?? "")) continue;
    drawNode(ctx, n, env);
  }
}

function drawBackground(ctx: CanvasRenderingContext2D, bg: Background | null, W: number, H: number, env: Env) {
  let fill: Fill | null = bg?.fill ?? null;
  let ph: string | undefined;
  if (!fill && bg?.styleRef) {
    const [idx, color] = bg.styleRef;
    ph = env.palette.css(color);
    fill = env.palette.styleFill(idx) ?? { t: "solid", color };
  }
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, W, H);
  if (fill) {
    ctx.beginPath();
    ctx.rect(0, 0, W, H);
    paintFill(ctx, fill, W, H, env, ph, true);
  }
}

// ---- fills & lines ----------------------------------------------------------------------------

function paintFill(ctx: CanvasRenderingContext2D, fill: Fill, w: number, h: number, env: Env, ph?: string, isBg = false): boolean {
  switch (fill.t) {
    case "none":
    case "group":
      return false;
    case "solid":
      ctx.fillStyle = env.palette.css(fill.color, ph);
      ctx.fill();
      return true;
    case "gradient": {
      let g: CanvasGradient;
      if (fill.path) {
        g = ctx.createRadialGradient(w / 2, h / 2, 0, w / 2, h / 2, Math.hypot(w, h) / 2);
      } else {
        const a = ((fill.angle ?? 0) / 60000) * (Math.PI / 180);
        const c = Math.cos(a);
        const s = Math.sin(a);
        const half = (Math.abs(w * c) + Math.abs(h * s)) / 2;
        g = ctx.createLinearGradient(w / 2 - c * half, h / 2 - s * half, w / 2 + c * half, h / 2 + s * half);
      }
      const stops = [...fill.stops].sort((x, y) => x.pos - y.pos);
      for (const st of stops) g.addColorStop(Math.min(Math.max(st.pos / 100000, 0), 1), env.palette.css(st.color, ph));
      ctx.fillStyle = g;
      ctx.fill();
      return true;
    }
    case "pattern": {
      const fg = env.palette.css(fill.fg, ph);
      const bg = env.palette.css(fill.bg, ph);
      ctx.fillStyle = bg;
      ctx.fill();
      const pat = patternCanvas(fill.prst, fg);
      const p = pat ? ctx.createPattern(pat, "repeat") : null;
      if (p) {
        ctx.fillStyle = p;
        ctx.fill();
      }
      return true;
    }
    case "image": {
      const img = env.opts.images?.(fill.relId);
      if (!img) {
        if (!isBg) {
          ctx.fillStyle = "#e5e7eb";
          ctx.fill();
        }
        return true;
      }
      ctx.save();
      ctx.clip();
      drawImageCropped(ctx, img, fill.crop ?? [0, 0, 0, 0], w, h, fill.tile);
      ctx.restore();
      return true;
    }
  }
}

function patternCanvas(prst: string, fg: string): HTMLCanvasElement | OffscreenCanvas | null {
  if (typeof document === "undefined" && typeof OffscreenCanvas === "undefined") return null;
  const c = typeof OffscreenCanvas !== "undefined" ? new OffscreenCanvas(8, 8) : document.createElement("canvas");
  c.width = c.height = 8;
  const x = c.getContext("2d") as CanvasRenderingContext2D | null;
  if (!x) return null;
  x.fillStyle = fg;
  x.strokeStyle = fg;
  if (/Diag|diag/.test(prst)) {
    x.beginPath();
    if (/dn|Dn/.test(prst)) (x.moveTo(0, 0), x.lineTo(8, 8));
    else (x.moveTo(0, 8), x.lineTo(8, 0));
    x.stroke();
  } else if (/horz|Horz/.test(prst)) x.fillRect(0, 0, 8, 1);
  else if (/vert|Vert/.test(prst)) x.fillRect(0, 0, 1, 8);
  else if (/cross|Grid|grid/.test(prst)) (x.fillRect(0, 0, 8, 1), x.fillRect(0, 0, 1, 8));
  else {
    const pct = Number(/pct(\d+)/.exec(prst)?.[1] ?? 50);
    const n = Math.max(1, Math.round((pct / 100) * 64));
    for (let i = 0; i < n; i++) {
      const k = (i * 37) % 64;
      x.fillRect(k % 8, Math.floor(k / 8), 1, 1);
    }
  }
  return c;
}

function drawImageCropped(ctx: CanvasRenderingContext2D, img: CanvasImageSource, crop: number[], w: number, h: number, tile?: boolean) {
  const iw = (img as HTMLImageElement).naturalWidth || (img as HTMLCanvasElement).width || 1;
  const ih = (img as HTMLImageElement).naturalHeight || (img as HTMLCanvasElement).height || 1;
  if (tile) {
    const p = ctx.createPattern(img, "repeat");
    if (p) {
      ctx.fillStyle = p;
      ctx.fillRect(0, 0, w, h);
    }
    return;
  }
  const [l, t, r, b] = crop.map((c) => c / 100000) as [number, number, number, number];
  const sx = iw * l;
  const sy = ih * t;
  const sw = iw * (1 - l - r);
  const sh = ih * (1 - t - b);
  if (sw <= 0 || sh <= 0) return;
  // negative crop (image smaller than frame) is handled by the same mapping
  const dx = (Math.max(0, -l) * w) / (1 - l - r);
  const dy = (Math.max(0, -t) * h) / (1 - t - b);
  ctx.drawImage(img, Math.max(0, sx), Math.max(0, sy), Math.min(iw, sw), Math.min(ih, sh), dx, dy, w - dx - (Math.max(0, -r) * w) / (1 - l - r), h - dy - (Math.max(0, -b) * h) / (1 - t - b));
}

const DASHES: Record<string, number[]> = {
  dash: [4, 3],
  sysDash: [3, 1],
  sysDot: [1, 1],
  dot: [1, 3],
  dashDot: [4, 3, 1, 3],
  lgDash: [8, 3],
  lgDashDot: [8, 3, 1, 3],
  lgDashDotDot: [8, 3, 1, 3, 1, 3],
  sysDashDot: [3, 1, 1, 1],
  sysDashDotDot: [3, 1, 1, 1, 1, 1],
};

function strokeStyle(ctx: CanvasRenderingContext2D, line: Line, env: Env, ph?: string): boolean {
  const f = line.fill;
  if (!f || f.t === "none") return false;
  if (f.t === "solid") ctx.strokeStyle = env.palette.css(f.color, ph);
  else if (f.t === "gradient" && f.stops[0]) ctx.strokeStyle = env.palette.css(f.stops[0].color, ph);
  else return false;
  const w = Math.max(pt(line.width ?? 9525), 0.25);
  ctx.lineWidth = w;
  ctx.setLineDash((DASHES[line.dash ?? ""] ?? []).map((d) => d * w));
  ctx.lineCap = line.cap === "rnd" ? "round" : line.cap === "sq" ? "square" : "butt";
  ctx.lineJoin = line.join === "bevel" ? "bevel" : line.join === "miter" ? "miter" : "round";
  return true;
}

function tracePath(ctx: CanvasRenderingContext2D, cmds: Cmd[]) {
  ctx.beginPath();
  for (const c of cmds) {
    switch (c.c) {
      case "M":
        ctx.moveTo(pt(c.x), pt(c.y));
        break;
      case "L":
        ctx.lineTo(pt(c.x), pt(c.y));
        break;
      case "C":
        ctx.bezierCurveTo(pt(c.x1), pt(c.y1), pt(c.x2), pt(c.y2), pt(c.x), pt(c.y));
        break;
      case "Q":
        ctx.quadraticCurveTo(pt(c.x1), pt(c.y1), pt(c.x), pt(c.y));
        break;
      case "Z":
        ctx.closePath();
        break;
    }
  }
}

function endPoints(cmds: Cmd[]): { start: [number, number, number, number]; end: [number, number, number, number] } | null {
  const pts: [number, number][] = [];
  for (const c of cmds) {
    if (c.c === "Z") continue;
    if (c.c === "C") pts.push([c.x2, c.y2], [c.x, c.y]);
    else if (c.c === "Q") pts.push([c.x1, c.y1], [c.x, c.y]);
    else pts.push([c.x, c.y]);
  }
  if (pts.length < 2) return null;
  const a = pts[0]!;
  const b = pts.find((p) => p[0] !== a[0] || p[1] !== a[1]) ?? pts[1]!;
  const z = pts[pts.length - 1]!;
  const y = [...pts].reverse().find((p) => p[0] !== z[0] || p[1] !== z[1]) ?? pts[pts.length - 2]!;
  return { start: [pt(a[0]), pt(a[1]), pt(b[0]), pt(b[1])], end: [pt(z[0]), pt(z[1]), pt(y[0]), pt(y[1])] };
}

function drawArrow(ctx: CanvasRenderingContext2D, kind: string, size: [string | undefined, string | undefined], seg: [number, number, number, number], lw: number) {
  if (kind === "none") return;
  const [x, y, px, py] = seg;
  const ang = Math.atan2(y - py, x - px);
  const k = (s: string | undefined) => (s === "sm" ? 2 : s === "lg" ? 5 : 3);
  const len = Math.max(lw, 1) * k(size[1]);
  const wid = Math.max(lw, 1) * k(size[0]);
  ctx.save();
  ctx.translate(x, y);
  ctx.rotate(ang);
  ctx.setLineDash([]);
  ctx.fillStyle = ctx.strokeStyle;
  ctx.beginPath();
  if (kind === "oval") ctx.ellipse(0, 0, len / 2, wid / 2, 0, 0, Math.PI * 2);
  else if (kind === "diamond") (ctx.moveTo(len / 2, 0), ctx.lineTo(0, wid / 2), ctx.lineTo(-len / 2, 0), ctx.lineTo(0, -wid / 2));
  else if (kind === "arrow") {
    ctx.moveTo(-len, -wid / 2);
    ctx.lineTo(0, 0);
    ctx.lineTo(-len, wid / 2);
    ctx.stroke();
    ctx.restore();
    return;
  } else if (kind === "stealth") (ctx.moveTo(0, 0), ctx.lineTo(-len, -wid / 2), ctx.lineTo(-len * 0.6, 0), ctx.lineTo(-len, wid / 2));
  else (ctx.moveTo(0, 0), ctx.lineTo(-len, -wid / 2), ctx.lineTo(-len, wid / 2));
  ctx.closePath();
  ctx.fill();
  ctx.restore();
}

function applyShadow(ctx: CanvasRenderingContext2D, fx: Effects | undefined, env: Env, scaleHint: number) {
  const s = fx?.shadow;
  if (!s || s.inner) return;
  const a = (s.dir / 60000) * (Math.PI / 180);
  const m = ctx.getTransform();
  const k = Math.hypot(m.a, m.b) || scaleHint;
  ctx.shadowColor = env.palette.css(s.color);
  ctx.shadowBlur = pt(s.blur) * k;
  ctx.shadowOffsetX = Math.cos(a) * pt(s.dist) * k;
  ctx.shadowOffsetY = Math.sin(a) * pt(s.dist) * k;
}

function clearShadow(ctx: CanvasRenderingContext2D) {
  ctx.shadowColor = "transparent";
  ctx.shadowBlur = ctx.shadowOffsetX = ctx.shadowOffsetY = 0;
}

// ---- nodes -------------------------------------------------------------------------------------

function applyXfrm(ctx: CanvasRenderingContext2D, x: Xfrm, anim: ShapeAnim | null) {
  const w = pt(x.cx);
  const h = pt(x.cy);
  ctx.translate(pt(x.x) + w / 2 + (anim?.dx ?? 0), pt(x.y) + h / 2 + (anim?.dy ?? 0));
  const rot = (x.rot ?? 0) / 60000 + (anim?.rotate ?? 0);
  if (rot) ctx.rotate((rot * Math.PI) / 180);
  if (anim?.scale !== undefined) ctx.scale(anim.scale, anim.scale);
  ctx.scale(x.flipH ? -1 : 1, x.flipV ? -1 : 1);
  ctx.translate(-w / 2, -h / 2);
}

export function drawNode(ctx: CanvasRenderingContext2D, node: SlideNode, env: Env) {
  if (node.hidden || env.opts.skip?.has(node.id)) return;
  const anim = env.level === "slide" ? (env.opts.anim?.(node.id) ?? null) : null;
  if (anim?.hidden || anim?.opacity === 0) return;
  ctx.save();
  if (anim?.opacity !== undefined) ctx.globalAlpha *= anim.opacity;
  try {
    switch (node.type) {
      case "group": {
        const x = node.xfrm;
        if (x) {
          applyXfrm(ctx, x, anim);
          const ch = x.child ?? [x.x, x.y, x.cx, x.cy];
          const sx = ch[2] ? x.cx / ch[2] : 1;
          const sy = ch[3] ? x.cy / ch[3] : 1;
          ctx.scale(sx, sy);
          ctx.translate(-pt(ch[0]), -pt(ch[1]));
        }
        if (anim?.clip) clipAnim(ctx, anim.clip, x);
        for (const c of node.children) drawNode(ctx, c, { ...env, opts: { ...env.opts, anim: undefined } });
        break;
      }
      case "shape":
      case "connector":
        drawShape(ctx, node, env, anim);
        break;
      case "picture":
        drawPicture(ctx, node, env, anim);
        break;
      case "frame":
        drawFrame(ctx, node, env, anim);
        break;
      default:
        break;
    }
  } finally {
    ctx.restore();
  }
}

function clipAnim(ctx: CanvasRenderingContext2D, c: [number, number, number, number], x: Xfrm | undefined | null) {
  if (!x) return;
  const w = pt(x.cx);
  const h = pt(x.cy);
  ctx.beginPath();
  ctx.rect(c[0] * w, c[1] * h, (c[2] - c[0]) * w, (c[3] - c[1]) * h);
  ctx.clip();
}

function textEnv(env: Env, style: ShapeBody["style"]): TextEnv {
  const font = style?.font;
  return {
    palette: env.palette,
    theme: env.ctx.master.theme,
    color: font?.[1] ? env.palette.css(font[1]) : undefined,
    slideNumber: env.opts.slideNumber,
  };
}

function drawShape(ctx: CanvasRenderingContext2D, node: Extract<SlideNode, { type: "shape" | "connector" }>, env: Env, anim: ShapeAnim | null) {
  const res = resolveShape(node, env.ctx, env.model, env.level);
  const x = res.xfrm;
  if (!x) return;
  const w = pt(x.cx);
  const h = pt(x.cy);
  applyXfrm(ctx, x, anim);
  if (anim?.clip) clipAnim(ctx, anim.clip, x);
  const geom = res.geom ?? { t: "preset", name: "rect" };
  const ev = evaluate(geom, x.cx, x.cy);
  const isConn = node.type === "connector";
  let fill = isConn ? null : res.fill;
  let fillPh: string | undefined;
  if (!fill && !isConn && node.style?.fill && node.style.fill.idx > 0) {
    fillPh = node.style.fill.color ? env.palette.css(node.style.fill.color) : undefined;
    fill = env.palette.styleFill(node.style.fill.idx) ?? (node.style.fill.color ? { t: "solid", color: node.style.fill.color } : null);
  }
  let line = res.line;
  let linePh: string | undefined;
  if (node.style?.line && node.style.line.idx > 0) {
    linePh = node.style.line.color ? env.palette.css(node.style.line.color) : undefined;
    const base = env.palette.styleLine(node.style.line.idx) ?? {};
    line = { ...base, ...(line ?? {}), fill: line?.fill ?? base.fill ?? (node.style.line.color ? { t: "solid", color: node.style.line.color } : undefined) };
  }
  const eff = node.effects;
  for (const p of ev.paths) {
    tracePath(ctx, p.cmds);
    if (fill && p.fill !== "none") {
      applyShadow(ctx, eff, env, 1);
      if (paintFill(ctx, fill, w, h, env, fillPh)) {
        clearShadow(ctx);
        if (p.fill !== "norm") {
          ctx.fillStyle = { lighten: "rgba(255,255,255,.4)", lightenLess: "rgba(255,255,255,.2)", darken: "rgba(0,0,0,.4)", darkenLess: "rgba(0,0,0,.2)" }[p.fill];
          ctx.fill();
        }
      }
      clearShadow(ctx);
    }
  }
  if (line) {
    for (const p of ev.paths) {
      if (!p.stroke) continue;
      tracePath(ctx, p.cmds);
      if (!fill) applyShadow(ctx, eff, env, 1);
      if (strokeStyle(ctx, line, env, linePh)) {
        ctx.stroke();
        clearShadow(ctx);
        if (line.head || line.tail) {
          const ends = endPoints(p.cmds);
          const lw = ctx.lineWidth;
          if (ends && line.head) drawArrow(ctx, line.head.type, [line.head.w, line.head.len], ends.start, lw);
          if (ends && line.tail) drawArrow(ctx, line.tail.type, [line.tail.w, line.tail.len], ends.end, lw);
        }
      }
      clearShadow(ctx);
    }
  }
  drawShapeText(ctx, node, res, w, h, env, ev.textRect.map(pt) as [number, number, number, number], x);
}

function drawShapeText(ctx: CanvasRenderingContext2D, node: ShapeBody, res0: ResolvedShape, w: number, h: number, env: Env, rect: [number, number, number, number], x: Xfrm) {
  if (env.opts.skipText?.has(node.id)) return;
  let res = res0;
  // undo flips for text (PowerPoint never mirrors text; flipV turns it 180°)
  if (x.flipH || x.flipV) {
    ctx.translate(w / 2, h / 2);
    ctx.scale(x.flipH ? -1 : 1, x.flipV ? -1 : 1);
    if (x.flipV) ctx.rotate(Math.PI);
    ctx.translate(-w / 2, -h / 2);
  }
  const vert = res.body.vert;
  if (vert === "vert" || vert === "eaVert" || vert === "vert270") {
    ctx.translate(w / 2, h / 2);
    ctx.rotate(vert === "vert270" ? -Math.PI / 2 : Math.PI / 2);
    ctx.translate(-h / 2, -w / 2);
    [w, h] = [h, w];
    rect = [rect[1], rect[0], rect[3], rect[2]];
  }
  const tenv = textEnv(env, node.style);
  // the shape style's font colour beats colours inherited from the base text style
  if (tenv.color) res = { ...res, lists: [withoutFill(res.lists[0] ?? {}), ...res.lists.slice(1)] };
  const empty = !hasText(node.text);
  if (empty) {
    if (env.opts.editing && env.level === "slide" && node.ph) {
      ctx.save();
      ctx.setLineDash([3, 2]);
      ctx.lineWidth = 0.75;
      ctx.strokeStyle = "#9ca3af";
      ctx.strokeRect(0, 0, w, h);
      ctx.restore();
      const prompt = placeholderPrompt(node.ph);
      if (prompt) {
        const lay = layoutText({ paragraphs: [{ runs: [{ t: "text", text: prompt }] }] }, res, w, h, { ...tenv, color: "#8a8f98" }, rect);
        drawTextLayout(ctx, lay);
      }
    }
    return;
  }
  let extra = 1;
  if (env.opts.liveAutofit && res.body.autofit?.t === "normal") {
    const lay0 = layoutText(node.text, res, w, h, tenv, rect);
    if (lay0.overflow) extra = fitScale(node.text, res, w, h, tenv);
  }
  const lay = layoutText(node.text, res, w, h, { ...tenv, extraScale: extra }, rect);
  drawTextLayout(ctx, lay);
}

/** A list style with run fills removed (so a contextual default colour applies). */
function withoutFill(ls: ListStyle): ListStyle {
  const strip = (p: ParaProps | null | undefined) => (p ? { ...p, run: p.run ? { ...p.run, fill: undefined } : undefined } : p);
  return { default: strip(ls.default) ?? undefined, levels: ls.levels?.map((l) => strip(l) ?? null) };
}

function drawPicture(ctx: CanvasRenderingContext2D, node: Extract<SlideNode, { type: "picture" }>, env: Env, anim: ShapeAnim | null) {
  const res = resolveShape(node, env.ctx, env.model, env.level);
  const x = res.xfrm;
  if (!x) return;
  const w = pt(x.cx);
  const h = pt(x.cy);
  applyXfrm(ctx, x, anim);
  if (anim?.clip) clipAnim(ctx, anim.clip, x);
  const geom = node.geom ?? { t: "preset" as const, name: "rect" };
  const ev = evaluate(geom, x.cx, x.cy);
  const outline = ev.paths[0];
  if (outline) tracePath(ctx, outline.cmds);
  else (ctx.beginPath(), ctx.rect(0, 0, w, h));
  applyShadow(ctx, node.effects, env, 1);
  const img = node.image.t === "image" ? env.opts.images?.(node.image.relId) : null;
  if (img) {
    ctx.save();
    // a fill casts the shadow; the clipped image then covers it
    if (node.effects?.shadow) {
      ctx.fillStyle = "#fff";
      ctx.fill();
    }
    clearShadow(ctx);
    ctx.clip();
    drawImageCropped(ctx, img, node.image.t === "image" ? (node.image.crop ?? [0, 0, 0, 0]) : [0, 0, 0, 0], w, h, node.image.t === "image" && node.image.tile);
    ctx.restore();
  } else {
    clearShadow(ctx);
    ctx.fillStyle = "#eef0f3";
    ctx.fill();
    if (env.opts.editing || node.media) {
      ctx.fillStyle = "#9ca3af";
      ctx.font = `${Math.min(14, h / 4)}px system-ui, sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(node.media ? "▶ media" : node.ph ? placeholderPrompt(node.ph) : "picture", w / 2, h / 2);
    }
  }
  if (res.line && outline) {
    tracePath(ctx, outline.cmds);
    if (strokeStyle(ctx, res.line, env)) ctx.stroke();
  }
}

function drawFrame(ctx: CanvasRenderingContext2D, node: Extract<SlideNode, { type: "frame" }>, env: Env, anim: ShapeAnim | null) {
  const res = resolveShape(node, env.ctx, env.model, env.level);
  const x = res.xfrm;
  if (!x) return;
  applyXfrm(ctx, x, anim);
  if (anim?.clip) clipAnim(ctx, anim.clip, x);
  const w = pt(x.cx);
  const h = pt(x.cy);
  if (node.kind === "table" && node.table) {
    drawTable(ctx, node.table, env);
    return;
  }
  const chart = node.kind === "chart" && node.relId ? env.opts.charts?.(node.relId) : null;
  if (chart) {
    drawChart(ctx, 0, 0, w, h, chart.kind, chart.title, chart.data);
    return;
  }
  ctx.fillStyle = "rgba(148,163,184,.12)";
  ctx.fillRect(0, 0, w, h);
  ctx.strokeStyle = "#cbd5e1";
  ctx.lineWidth = 0.75;
  ctx.strokeRect(0, 0, w, h);
  ctx.fillStyle = "#64748b";
  ctx.font = `${Math.max(8, Math.min(16, h / 6))}px system-ui, sans-serif`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  const label = node.kind === "chart" ? "Chart" : node.kind === "diagram" ? "SmartArt" : node.kind === "ole" ? "Embedded object" : node.kind;
  ctx.fillText(label, w / 2, h / 2);
}

function drawTable(ctx: CanvasRenderingContext2D, t: Table, env: Env) {
  const colX: number[] = [0];
  for (const c of t.cols) colX.push(colX[colX.length - 1]! + pt(c));
  const rowY: number[] = [0];
  for (const [rh] of t.rows) rowY.push(rowY[rowY.length - 1]! + pt(rh));
  const styled = !!t.styleId;
  const accent = { base: { t: "scheme" as const, v: "accent1" } };
  const tenvBase: TextEnv = { palette: env.palette, theme: env.ctx.master.theme, slideNumber: env.opts.slideNumber };
  t.rows.forEach(([, cells], ri) => {
    cells.forEach((cell, ci) => {
      if (cell.hMerge || cell.vMerge) return;
      const x0 = colX[ci] ?? 0;
      const y0 = rowY[ri] ?? 0;
      const x1 = colX[Math.min(ci + (cell.gridSpan ?? 1), t.cols.length)] ?? x0;
      const y1 = rowY[Math.min(ri + (cell.rowSpan ?? 1), t.rows.length)] ?? y0;
      const cw = x1 - x0;
      const ch = y1 - y0;
      ctx.save();
      ctx.translate(x0, y0);
      ctx.beginPath();
      ctx.rect(0, 0, cw, ch);
      const header = styled && t.firstRow && ri === 0;
      let fill = cell.fill ?? null;
      if (!fill && styled) {
        fill = header
          ? { t: "solid", color: accent }
          : { t: "solid", color: { base: accent.base, mods: [{ name: "tint", val: t.bandRow && (ri - (t.firstRow ? 1 : 0)) % 2 === 0 ? 40000 : 20000 }] } };
      }
      if (fill) paintFill(ctx, fill, cw, ch, env);
      const res: ResolvedShape = { xfrm: null, geom: null, fill: null, line: null, body: { anchor: "t", ...(cell.text.body ?? {}) }, lists: [header ? withoutFill(env.model.defaultTextStyle ?? {}) : (env.model.defaultTextStyle ?? {}), cell.text.listStyle ?? {}], styleKind: "other" };
      const tenv = header ? { ...tenvBase, color: "#ffffff" } : tenvBase;
      const text = header ? { ...cell.text, paragraphs: cell.text.paragraphs?.map((p) => ({ ...p, props: { ...p.props, run: { bold: true, ...p.props?.run } } })) } : cell.text;
      drawTextLayout(ctx, layoutText(text, res, cw, ch, tenv));
      // borders: l, r, t, b
      const b = cell.borders;
      const edges: [number, number, number, number][] = [
        [0, 0, 0, ch],
        [cw, 0, cw, ch],
        [0, 0, cw, 0],
        [0, ch, cw, ch],
      ];
      edges.forEach((e, i) => {
        const ln = b?.[i] ?? (styled ? { width: 12700, fill: { t: "solid", color: { base: { t: "scheme", v: "bg1" } } } } : null);
        if (!ln) return;
        if (!strokeStyle(ctx, ln as Line, env)) return;
        ctx.beginPath();
        ctx.moveTo(e[0], e[1]);
        ctx.lineTo(e[2], e[3]);
        ctx.stroke();
      });
      ctx.restore();
    });
  });
}

/** Render a slide into a canvas of the given CSS width (thumbnails, export). */
export function renderSlideToCanvas(canvas: HTMLCanvasElement | OffscreenCanvas, model: PresentationModel, slide: Slide, cssWidth: number, dpr = 1, opts: RenderOptions = {}) {
  const W = pt(model.width);
  const H = pt(model.height);
  const k = (cssWidth / W) * dpr;
  canvas.width = Math.max(1, Math.round(W * k));
  canvas.height = Math.max(1, Math.round(H * k));
  const ctx = canvas.getContext("2d") as CanvasRenderingContext2D | null;
  if (!ctx) return;
  ctx.setTransform(k, 0, 0, k, 0, 0);
  drawSlide(ctx, model, slide, opts);
}
