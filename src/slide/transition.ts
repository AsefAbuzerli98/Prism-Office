/**
 * Slide transitions for the slideshow. Classic transitions composite two
 * slide rasters; Morph matches shapes across the two slides (by name, then
 * picture, then text, then placeholder) and interpolates position, size,
 * rotation and fill colour, fading unmatched shapes in and out.
 */
import { Palette, toHex } from "./engine";
import type { Fill, PresentationModel, Slide, SlideNode, Transition, Xfrm } from "./model";
import { drawSlide, EMU_PER_PT, paletteFor, type RenderOptions } from "./render";
import { slideContext } from "./resolve";

export type Raster = HTMLCanvasElement | OffscreenCanvas;

const ease = (p: number) => (p < 0.5 ? 2 * p * p : 1 - Math.pow(-2 * p + 2, 2) / 2);

/** Default duration (ms) PowerPoint uses when the file gives none. */
export function transitionDuration(t: Transition | undefined): number {
  if (!t || t.kind === "cut" || t.kind === "none") return 0;
  return t.duration ?? (t.kind === "morph" ? 2000 : 700);
}

/** Deterministic pseudo-random in [0,1) for block dissolves. */
function rnd(i: number) {
  const x = Math.sin(i * 12.9898) * 43758.5453;
  return x - Math.floor(x);
}

/**
 * Draw transition frame `p` (0..1) between rasters `a` (from) and `b` (to)
 * into `ctx` covering `w`×`h` device pixels.
 */
export function drawTransition(ctx: CanvasRenderingContext2D, kind: string, dir: string | undefined, option: string | undefined, p: number, a: Raster | null, b: Raster, w: number, h: number) {
  const e = ease(p);
  const img = (r: Raster | null, x = 0, y = 0, ww = w, hh = h) => {
    if (r) ctx.drawImage(r as CanvasImageSource, x, y, ww, hh);
    else {
      ctx.fillStyle = "#000";
      ctx.fillRect(x, y, ww, hh);
    }
  };
  const clipped = (path: () => void) => {
    ctx.save();
    ctx.beginPath();
    path();
    ctx.clip();
    img(b);
    ctx.restore();
  };
  // direction vector of motion for push/cover/pull/wipe (where the new slide comes from)
  const d = dir ?? "l";
  const vx = d.includes("l") ? 1 : d.includes("r") ? -1 : 0;
  const vy = d.includes("u") ? 1 : d.includes("d") ? -1 : 0;
  ctx.save();
  switch (kind) {
    case "cut":
      img(p < 0.5 ? a : b);
      break;
    case "push":
      img(a, -vx * w * e, -vy * h * e);
      img(b, vx * w * (1 - e), vy * h * (1 - e));
      break;
    case "cover":
      img(a);
      img(b, vx * w * (1 - e), vy * h * (1 - e));
      break;
    case "pull":
    case "uncover":
      img(b);
      img(a, -vx * w * e, -vy * h * e);
      break;
    case "wipe": {
      img(a);
      const x0 = vx > 0 ? w * (1 - e) : 0;
      const y0 = vy > 0 ? h * (1 - e) : 0;
      clipped(() => ctx.rect(vx ? x0 : 0, vy ? y0 : 0, vx ? w * e : w, vy ? h * e : h));
      break;
    }
    case "split":
    case "doors":
    case "window": {
      img(a);
      const vert = (dir ?? "horz") === "vert" || kind === "doors" || kind === "window";
      const inward = option === "in" || dir === "in";
      const f = inward ? e : e;
      if (vert) clipped(() => ctx.rect(w / 2 - (w / 2) * f, 0, w * f, h));
      else clipped(() => ctx.rect(0, h / 2 - (h / 2) * f, w, h * f));
      break;
    }
    case "circle":
    case "ripple":
    case "vortex":
      img(a);
      clipped(() => ctx.arc(w / 2, h / 2, (Math.hypot(w, h) / 2) * e, 0, Math.PI * 2));
      break;
    case "diamond":
      img(a);
      clipped(() => {
        const r = (w + h) * 0.5 * e;
        ctx.moveTo(w / 2, h / 2 - r);
        ctx.lineTo(w / 2 + r, h / 2);
        ctx.lineTo(w / 2, h / 2 + r);
        ctx.lineTo(w / 2 - r, h / 2);
      });
      break;
    case "plus":
      img(a);
      clipped(() => {
        ctx.rect(w / 2 - (w / 2) * e, h / 2 - (h / 6) * e * 3, w * e, (h / 3) * e * 3);
        ctx.rect(w / 2 - (w / 6) * e * 3, h / 2 - (h / 2) * e, (w / 3) * e * 3, h * e);
      });
      break;
    case "wedge":
    case "wheel":
    case "clock":
      img(a);
      clipped(() => {
        ctx.moveTo(w / 2, h / 2);
        ctx.arc(w / 2, h / 2, Math.hypot(w, h), -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * e);
        ctx.closePath();
      });
      break;
    case "blinds":
    case "comb":
    case "strips":
    case "randomBar":
    case "shred": {
      img(a);
      const n = kind === "randomBar" ? 40 : 10;
      const vert = (dir ?? "horz") === "vert";
      clipped(() => {
        for (let i = 0; i < n; i++) {
          const f = kind === "randomBar" ? (rnd(i) < e ? 1 : 0) : e;
          if (vert) ctx.rect((w / n) * i, 0, (w / n) * f, h);
          else ctx.rect(0, (h / n) * i, w, (h / n) * f);
        }
      });
      break;
    }
    case "checker":
    case "dissolve":
    case "random":
    case "glitter":
    case "honeycomb": {
      img(a);
      const cols = kind === "checker" ? 12 : 32;
      const rows = Math.max(1, Math.round((cols * h) / w));
      clipped(() => {
        for (let r = 0; r < rows; r++)
          for (let c = 0; c < cols; c++) {
            const t = kind === "checker" ? ((c + (r % 2) * 0.5) / cols) * 0.5 + 0.5 * e : rnd(r * cols + c);
            if (kind === "checker" ? e * 2 > (c % 2 === r % 2 ? 0 : 1) && t <= e + 0.5 : t < e) ctx.rect((w / cols) * c, (h / rows) * r, w / cols + 0.5, h / rows + 0.5);
          }
      });
      break;
    }
    case "zoom":
    case "gallery":
    case "flythrough":
      img(a);
      ctx.globalAlpha = e;
      {
        const s = 0.6 + 0.4 * e;
        img(b, (w * (1 - s)) / 2, (h * (1 - s)) / 2, w * s, h * s);
      }
      break;
    case "newsflash":
      img(a);
      ctx.translate(w / 2, h / 2);
      ctx.rotate((1 - e) * Math.PI * 4);
      ctx.scale(e || 0.001, e || 0.001);
      img(b, -w / 2, -h / 2);
      break;
    case "flash":
    case "reveal":
    case "fade":
    default: {
      const via = kind === "flash" ? "#fff" : kind === "reveal" || option === "thruBlk" ? "#000" : null;
      if (via) {
        if (p < 0.5) img(a);
        else img(b);
        ctx.globalAlpha = 1 - Math.abs(p - 0.5) * 2;
        ctx.fillStyle = via;
        ctx.fillRect(0, 0, w, h);
      } else {
        img(a);
        ctx.globalAlpha = e;
        img(b);
      }
      break;
    }
  }
  ctx.restore();
}

// ---- Morph -------------------------------------------------------------------------------------

function nodeText(n: SlideNode): string {
  if (n.type !== "shape" && n.type !== "connector") return "";
  return (n.text?.paragraphs ?? []).map((p) => (p.runs ?? []).map((r) => (r.t === "break" ? "\n" : r.text)).join("")).join("\n").trim();
}

function key(n: SlideNode) {
  return n.name.startsWith("!!") ? n.name : null;
}

/** Pairs of shapes Morph animates between `a` and `b` (indices into the shape lists). */
export function morphPairs(a: SlideNode[], b: SlideNode[]): [number, number][] {
  const used = new Set<number>();
  const pairs: [number, number][] = [];
  const tryMatch = (f: (x: SlideNode, y: SlideNode) => boolean) => {
    a.forEach((x, i) => {
      if (pairs.some((p) => p[0] === i)) return;
      const j = b.findIndex((y, jj) => !used.has(jj) && f(x, y));
      if (j >= 0) {
        used.add(j);
        pairs.push([i, j]);
      }
    });
  };
  tryMatch((x, y) => !!key(x) && key(x) === key(y));
  tryMatch((x, y) => x.type === y.type && x.name === y.name && x.name !== "");
  tryMatch((x, y) => x.type === "picture" && y.type === "picture" && x.image.t === "image" && y.image.t === "image" && x.image.relId === y.image.relId);
  tryMatch((x, y) => x.type === y.type && nodeText(x) !== "" && nodeText(x) === nodeText(y));
  tryMatch((x, y) => !!x.ph && !!y.ph && x.ph.kind === y.ph.kind && x.ph.idx === y.ph.idx);
  return pairs;
}

function lerp(a: number, b: number, t: number) {
  return a + (b - a) * t;
}

function lerpXfrm(a: Xfrm, b: Xfrm, t: number): Xfrm {
  // rotate the short way round
  let ra = a.rot ?? 0;
  const rb = b.rot ?? 0;
  if (rb - ra > 10800000) ra += 21600000;
  if (ra - rb > 10800000) ra -= 21600000;
  return { ...b, x: Math.round(lerp(a.x, b.x, t)), y: Math.round(lerp(a.y, b.y, t)), cx: Math.round(lerp(a.cx, b.cx, t)), cy: Math.round(lerp(a.cy, b.cy, t)), rot: Math.round(lerp(ra, rb, t)) };
}

function hexLerp(a: string, b: string, t: number): string {
  const pa = toHex(a).slice(1);
  const pb = toHex(b).slice(1);
  const c = (i: number) => Math.round(lerp(parseInt(pa.slice(i, i + 2), 16), parseInt(pb.slice(i, i + 2), 16), t));
  return [c(0), c(2), c(4)].map((v) => v.toString(16).padStart(2, "0")).join("").toUpperCase();
}

function solidCss(f: Fill | undefined, pal: Palette): string | null {
  return f?.t === "solid" ? pal.css(f.color) : null;
}

/** Inherited position of a node on its slide. */
type XfrmOf = (n: SlideNode, slide: Slide) => Xfrm | null;

/** Draw one Morph frame at progress `p` from slide `a` to `b` (ctx in points). */
export function drawMorph(ctx: CanvasRenderingContext2D, model: PresentationModel, a: Slide, b: Slide, p: number, xfrmOf: XfrmOf, opts: RenderOptions = {}) {
  const t = ease(p);
  const pairs = morphPairs(a.shapes, b.shapes);
  const pa = paletteFor(slideContext(model, a));
  const pb = paletteFor(slideContext(model, b));
  // backgrounds and master/layout art crossfade
  drawSlide(ctx, model, { ...a, shapes: [] }, opts);
  ctx.save();
  ctx.globalAlpha = t;
  drawSlide(ctx, model, { ...b, shapes: [] }, opts);
  ctx.restore();

  const shapes: SlideNode[] = [];
  const alpha = new Map<number, number>();
  let id = 1;
  const add = (n: SlideNode, op: number) => {
    const c = { ...n, id: id++ } as SlideNode;
    shapes.push(c);
    alpha.set(c.id, op);
    return c;
  };
  const matchedA = new Set(pairs.map((x) => x[0]));
  const matchedB = new Set(pairs.map((x) => x[1]));
  a.shapes.forEach((n, i) => {
    if (!matchedA.has(i)) add(n, 1 - t);
  });
  // b order decides stacking of morphing shapes
  b.shapes.forEach((nb, j) => {
    if (!matchedB.has(j)) {
      add(nb, t);
      return;
    }
    const i = pairs.find((x) => x[1] === j)![0];
    const na = a.shapes[i]!;
    const xa = xfrmOf(na, a);
    const xb = xfrmOf(nb, b);
    if (!xa || !xb) {
      add(nb, t);
      return;
    }
    const sameGeom = JSON.stringify((na as { geom?: unknown }).geom ?? null) === JSON.stringify((nb as { geom?: unknown }).geom ?? null);
    const sameText = nodeText(na) === nodeText(nb);
    const x = lerpXfrm(xa, xb, t);
    if (sameGeom) {
      const textless = sameText ? nb : ({ ...nb, text: undefined } as SlideNode);
      const c = add({ ...textless, xfrm: x } as SlideNode, 1) as SlideNode & { fill?: Fill };
      const fa = solidCss((na as { fill?: Fill }).fill, pa);
      const fb = solidCss((nb as { fill?: Fill }).fill, pb);
      if (fa && fb && fa !== fb) c.fill = { t: "solid", color: { base: { t: "srgb", v: hexLerp(fa, fb, t) } } };
      if (!sameText && (nb.type === "shape" || nb.type === "connector")) {
        // text crossfades inside the moving shape
        const bare = { fill: { t: "none" }, line: { fill: { t: "none" } }, effects: undefined, style: undefined };
        add({ ...na, ...bare, xfrm: x } as SlideNode, 1 - t);
        add({ ...nb, ...bare, xfrm: x } as SlideNode, t);
      }
    } else {
      add({ ...na, xfrm: x } as SlideNode, 1 - t);
      add({ ...nb, xfrm: x } as SlideNode, t);
    }
  });
  const scene: Slide = { ...b, shapes };
  drawSlide(ctx, model, scene, { ...opts, shapesOnly: true, anim: (sid) => ({ opacity: alpha.get(sid) ?? 1 }) });
}

export const EMU = EMU_PER_PT;

/** Transitions offered in the ribbon. */
export const TRANSITIONS: { kind: string; label: string; dir?: string }[] = [
  { kind: "none", label: "None" },
  { kind: "morph", label: "Morph" },
  { kind: "fade", label: "Fade" },
  { kind: "push", label: "Push", dir: "u" },
  { kind: "wipe", label: "Wipe", dir: "r" },
  { kind: "split", label: "Split", dir: "vert" },
  { kind: "cover", label: "Cover", dir: "l" },
  { kind: "pull", label: "Uncover", dir: "l" },
  { kind: "zoom", label: "Zoom" },
  { kind: "circle", label: "Shape" },
  { kind: "wheel", label: "Clock" },
  { kind: "dissolve", label: "Dissolve" },
  { kind: "blinds", label: "Blinds", dir: "horz" },
  { kind: "checker", label: "Checkerboard" },
  { kind: "randomBar", label: "Random Bars", dir: "horz" },
  { kind: "newsflash", label: "Newsflash" },
  { kind: "flash", label: "Flash" },
  { kind: "cut", label: "Cut" },
];
