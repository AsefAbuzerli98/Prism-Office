/**
 * Animation timeline for the slideshow: the flat effect list of a slide
 * (prism_slide::anim) is scheduled into click steps the way PowerPoint's
 * main sequence plays them, and evaluated at a time into per-shape render
 * state (opacity, offset, scale, rotation, reveal clip).
 */
import type { Effect, Timing } from "./model";
import type { ShapeAnim } from "./render";

export interface Scheduled {
  effect: Effect;
  /** ms from the start of the step. */
  begin: number;
  end: number;
}

export interface Step {
  /** Plays without a click (effects before the first onClick, PowerPoint behaviour). */
  auto: boolean;
  items: Scheduled[];
  /** ms until the last effect of the step ends. */
  duration: number;
}

/** Group effects into click steps with begin/end times. */
export function buildSteps(timing: Timing | undefined): Step[] {
  const steps: Step[] = [];
  let cur: Step | null = null;
  let prevBegin = 0;
  for (const e of timing?.effects ?? []) {
    const dur = Math.max(0, e.duration) * Math.max(1, e.repeat ?? 1);
    if (e.trigger === "onClick" || !cur) {
      cur = { auto: e.trigger !== "onClick" && steps.length === 0, items: [], duration: 0 };
      steps.push(cur);
      prevBegin = 0;
    }
    let begin: number;
    if (e.trigger === "afterPrev") begin = cur.duration + (e.delay ?? 0);
    else if (e.trigger === "withPrev") begin = prevBegin + (e.delay ?? 0);
    else begin = e.delay ?? 0;
    const item = { effect: e, begin, end: begin + dur };
    cur.items.push(item);
    cur.duration = Math.max(cur.duration, item.end);
    prevBegin = begin;
  }
  return steps;
}

const clamp01 = (v: number) => (v < 0 ? 0 : v > 1 ? 1 : v);
const easeOut = (p: number) => 1 - (1 - p) * (1 - p) * (1 - p);
const easeInOut = (p: number) => (p < 0.5 ? 4 * p * p * p : 1 - Math.pow(-2 * p + 2, 3) / 2);

function bounce(p: number) {
  const n = 7.5625;
  const d = 2.75;
  if (p < 1 / d) return n * p * p;
  if (p < 2 / d) return n * (p -= 1.5 / d) * p + 0.75;
  if (p < 2.5 / d) return n * (p -= 2.25 / d) * p + 0.9375;
  return n * (p -= 2.625 / d) * p + 0.984375;
}

/** Slide size and a shape's box (points) for directional effects. */
export interface AnimBox {
  slideW: number;
  slideH: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

/** PowerPoint direction subtypes: 1 top, 2 right, 4 bottom, 8 left (+ corners). */
function fromDir(sub: number, box: AnimBox, k: number): { dx: number; dy: number } {
  let dx = 0;
  let dy = 0;
  if (sub & 1) dy = -(box.y + box.h) * k;
  if (sub & 4 || sub === 0) dy = (box.slideH - box.y) * k;
  if (sub & 8) dx = -(box.x + box.w) * k;
  if (sub & 2) dx = (box.slideW - box.x) * k;
  return { dx, dy };
}

function wipeClip(sub: number, e: number): [number, number, number, number] {
  switch (sub) {
    case 1:
      return [0, 0, 1, e]; // from top
    case 2:
      return [1 - e, 0, 1, 1]; // from right
    case 8:
      return [0, 0, e, 1]; // from left
    default:
      return [0, 1 - e, 1, 1]; // from bottom
  }
}

/** Visual state of an entrance at progress `p` (0 hidden → 1 fully in). Exits use 1 - p. */
function entrance(preset: string, sub: number, p: number, box: AnimBox): ShapeAnim {
  const e = easeOut(p);
  switch (preset) {
    case "appear":
      return p > 0 ? {} : { hidden: true };
    case "fly": {
      const d = fromDir(sub, box, 1 - e);
      return { dx: d.dx, dy: d.dy };
    }
    case "float":
      return { dy: 30 * (1 - e), opacity: e };
    case "bounce": {
      const b = bounce(p);
      return { dy: -(box.y + box.h) * (1 - b) };
    }
    case "wipe":
      return { clip: wipeClip(sub, e) };
    case "split":
      return sub === 26 || sub === 42 ? { clip: [0, 0.5 - e / 2, 1, 0.5 + e / 2] } : { clip: [0.5 - e / 2, 0, 0.5 + e / 2, 1] };
    case "peek":
      return { clip: wipeClip(sub || 4, e), ...fromDir(sub || 4, { ...box, slideW: box.x + box.w * 2, slideH: box.y + box.h * 2 }, 0) };
    case "zoom":
    case "box":
    case "circle":
    case "diamond":
    case "stretch":
      return { scale: 0.05 + 0.95 * e, opacity: e };
    case "swivel":
    case "wheel":
      return { rotate: (1 - e) * (preset === "wheel" ? -90 : 30), opacity: e };
    default:
      // fade, dissolve, blinds, checkerboard, strips and other wipes: crossfade
      return { opacity: e };
  }
}

function emphasis(preset: string, p: number): ShapeAnim {
  switch (preset) {
    case "pulse":
      return { scale: 1 + 0.08 * Math.sin(Math.PI * p) };
    case "spin":
      return { rotate: 360 * easeInOut(p) };
    case "grow":
      return { scale: 1 + 0.5 * easeOut(p) };
    case "teeter":
      return { rotate: 6 * Math.sin(4 * Math.PI * p) * (1 - p) };
    case "transparency":
      return { opacity: 1 - 0.5 * easeOut(p) };
    case "blink":
      return { opacity: p > 0 && p < 1 && Math.floor(p * 4) % 2 === 0 ? 0 : 1 };
    default:
      return { scale: 1 + 0.08 * Math.sin(Math.PI * p) };
  }
}

/** State an emphasis leaves behind once done. */
function emphasisAfter(preset: string): ShapeAnim | null {
  if (preset === "grow") return { scale: 1.5 };
  if (preset === "transparency") return { opacity: 0.5 };
  return null;
}

/** Point at fraction `p` along a motion path (slide-relative units). */
export function pathPoint(path: string, p: number): [number, number] {
  const tok = path.trim().split(/[\s,]+/);
  const pts: [number, number][] = [];
  let cur: [number, number] = [0, 0];
  for (let i = 0; i < tok.length; ) {
    const c = tok[i++]!;
    const num = () => Number(tok[i++] ?? 0);
    if (c === "M" || c === "m" || c === "L" || c === "l") {
      cur = [num(), num()];
      pts.push(cur);
    } else if (c === "C" || c === "c") {
      const c1: [number, number] = [num(), num()];
      const c2: [number, number] = [num(), num()];
      const end: [number, number] = [num(), num()];
      const s = cur;
      for (let k = 1; k <= 16; k++) {
        const t = k / 16;
        const mt = 1 - t;
        pts.push([
          mt * mt * mt * s[0] + 3 * mt * mt * t * c1[0] + 3 * mt * t * t * c2[0] + t * t * t * end[0],
          mt * mt * mt * s[1] + 3 * mt * mt * t * c1[1] + 3 * mt * t * t * c2[1] + t * t * t * end[1],
        ]);
      }
      cur = end;
    } else if (c === "Z" || c === "z") {
      if (pts[0]) pts.push(pts[0]);
    } else if (c === "E" || c === "e") break;
    else if (!Number.isNaN(Number(c))) {
      // implicit lineto
      i--;
      cur = [num(), num()];
      pts.push(cur);
    }
  }
  if (pts.length < 2) return pts[0] ?? [0, 0];
  const seg: number[] = [];
  let total = 0;
  for (let i = 1; i < pts.length; i++) {
    const d = Math.hypot(pts[i]![0] - pts[i - 1]![0], pts[i]![1] - pts[i - 1]![1]);
    seg.push(d);
    total += d;
  }
  let want = clamp01(p) * total;
  for (let i = 0; i < seg.length; i++) {
    if (want <= seg[i]! || i === seg.length - 1) {
      const t = seg[i]! ? want / seg[i]! : 0;
      const a = pts[i]!;
      const b = pts[i + 1]!;
      return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    }
    want -= seg[i]!;
  }
  return pts[pts.length - 1]!;
}

function combine(a: ShapeAnim, b: ShapeAnim): ShapeAnim {
  return {
    hidden: a.hidden || b.hidden || undefined,
    opacity: a.opacity !== undefined || b.opacity !== undefined ? (a.opacity ?? 1) * (b.opacity ?? 1) : undefined,
    dx: a.dx !== undefined || b.dx !== undefined ? (a.dx ?? 0) + (b.dx ?? 0) : undefined,
    dy: a.dy !== undefined || b.dy !== undefined ? (a.dy ?? 0) + (b.dy ?? 0) : undefined,
    scale: a.scale !== undefined || b.scale !== undefined ? (a.scale ?? 1) * (b.scale ?? 1) : undefined,
    rotate: a.rotate !== undefined || b.rotate !== undefined ? (a.rotate ?? 0) + (b.rotate ?? 0) : undefined,
    clip: b.clip ?? a.clip,
  };
}

/**
 * Render state of every animated shape with `step` steps fully played and
 * the current step at `t` ms (step index `step`; pass t = Infinity when idle
 * at the end of a step, or step = -1 before anything ran).
 */
export function evaluate(steps: Step[], step: number, t: number, box: (shapeId: number) => AnimBox | null): Map<number, ShapeAnim> {
  const out = new Map<number, ShapeAnim>();
  const persistent = new Map<number, ShapeAnim>();
  /** Accumulated end points of finished motion paths. */
  const pathBase = new Map<number, [number, number]>();
  // initial visibility: shapes whose first effect is an entrance start hidden
  const seen = new Set<number>();
  for (const s of steps)
    for (const it of s.items) {
      const id = it.effect.shape;
      if (seen.has(id)) continue;
      seen.add(id);
      if (it.effect.class === "entr") persistent.set(id, { hidden: true });
    }
  steps.forEach((s, si) => {
    if (si > step) return;
    const now = si < step ? Infinity : t;
    for (const it of s.items) {
      const e = it.effect;
      const b = box(e.shape);
      if (!b) continue;
      if (now < it.begin) continue; // not started: earlier state stays
      const span = Math.max(1, it.end - it.begin);
      const p = clamp01((now - it.begin) / span);
      const done = p >= 1;
      const reps = Math.max(1, e.repeat ?? 1);
      const local = done ? 1 : (p * reps) % 1;
      const base = persistent.get(e.shape) ?? {};
      if (e.class === "entr") {
        if (done) persistent.set(e.shape, stripHidden(base, true));
        else persistent.set(e.shape, combine(stripHidden(base, true), entrance(e.preset, e.subtype ?? 0, local, b)));
      } else if (e.class === "exit") {
        if (done) persistent.set(e.shape, { hidden: true });
        else persistent.set(e.shape, combine(stripHidden(base, false), entrance(e.preset, e.subtype ?? 0, 1 - local, b)));
      } else if (e.class === "emph") {
        const after = emphasisAfter(e.preset);
        if (done) persistent.set(e.shape, after ? combine(cleanTransient(base), after) : cleanTransient(base));
        else persistent.set(e.shape, combine(cleanTransient(base), emphasis(e.preset, local)));
      } else if (e.class === "path" && e.path) {
        const [px, py] = pathPoint(e.path, done ? 1 : easeInOut(p));
        const pb = pathBase.get(e.shape) ?? [0, 0];
        const ox = pb[0] + px * b.slideW;
        const oy = pb[1] + py * b.slideH;
        persistent.set(e.shape, { ...cleanTransient(base), dx: ox, dy: oy });
        if (done) pathBase.set(e.shape, [ox, oy]);
      }
    }
  });
  for (const [id, st] of persistent) out.set(id, st);
  return out;
}

function stripHidden(a: ShapeAnim, visible: boolean): ShapeAnim {
  const { hidden: _h, ...rest } = a;
  return visible ? rest : a;
}

/** Drop in-flight entrance visuals, keep persistent ones (offsets from paths, grow, transparency). */
function cleanTransient(a: ShapeAnim): ShapeAnim {
  const { clip: _c, ...rest } = a;
  return rest;
}

/** Effects a ribbon "Add Animation" offers. */
export const ANIMATION_PRESETS: { class: "entr" | "exit" | "emph"; preset: string; label: string; subtype?: number }[] = [
  { class: "entr", preset: "appear", label: "Appear" },
  { class: "entr", preset: "fade", label: "Fade" },
  { class: "entr", preset: "fly", label: "Fly In", subtype: 4 },
  { class: "entr", preset: "float", label: "Float In" },
  { class: "entr", preset: "wipe", label: "Wipe", subtype: 8 },
  { class: "entr", preset: "split", label: "Split", subtype: 21 },
  { class: "entr", preset: "zoom", label: "Zoom" },
  { class: "entr", preset: "bounce", label: "Bounce" },
  { class: "entr", preset: "swivel", label: "Swivel" },
  { class: "emph", preset: "pulse", label: "Pulse" },
  { class: "emph", preset: "spin", label: "Spin" },
  { class: "emph", preset: "grow", label: "Grow/Shrink" },
  { class: "emph", preset: "teeter", label: "Teeter" },
  { class: "emph", preset: "transparency", label: "Transparency" },
  { class: "exit", preset: "fade", label: "Fade Out" },
  { class: "exit", preset: "fly", label: "Fly Out", subtype: 4 },
  { class: "exit", preset: "zoom", label: "Zoom Out" },
];

/** PowerPoint preset id for a class/preset (mirrors prism_slide::anim::preset_id). */
export function presetId(cls: string, name: string): number {
  const map: Record<string, number> = { appear: 1, fly: 2, wipe: 22, split: 16, zoom: 53, float: 42, bounce: 26, wheel: 21, swivel: 55, fade: 10 };
  if (cls === "emph") return ({ grow: 6, spin: 8, transparency: 9, pulse: 26, teeter: 32 } as Record<string, number>)[name] ?? 26;
  if (cls === "path") return 0;
  return map[name] ?? 10;
}
