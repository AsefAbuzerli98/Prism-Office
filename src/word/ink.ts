/**
 * Pen & stylus input (P2-15): stroke capture, ink-to-shape recognition and
 * handwriting-to-text (via the AI daemon's vision model).
 */
import { h } from "../core/dom";
import type { HostBridge } from "../core/host";
import { handwritingRequest } from "./ai";
import type { WordEditor } from "./editor";
import { EMU_PER_PX, type Inline } from "./model";

export interface Pt {
  x: number;
  y: number;
}

export interface Recognized {
  preset: "line" | "rightArrow" | "triangle" | "rect" | "diamond" | "ellipse";
  w: number;
  h: number;
}

const dist = (a: Pt, b: Pt) => Math.hypot(a.x - b.x, a.y - b.y);

function pathLength(pts: Pt[]) {
  let n = 0;
  for (let i = 1; i < pts.length; i++) n += dist(pts[i - 1]!, pts[i]!);
  return n;
}

/** Ramer–Douglas–Peucker polyline simplification. */
export function simplify(pts: Pt[], eps: number): Pt[] {
  if (pts.length < 3) return pts.slice();
  const a = pts[0]!;
  const b = pts[pts.length - 1]!;
  let idx = 0;
  let max = 0;
  const len = dist(a, b) || 1;
  for (let i = 1; i < pts.length - 1; i++) {
    const p = pts[i]!;
    const d = len > 1e-6 ? Math.abs((b.x - a.x) * (a.y - p.y) - (a.x - p.x) * (b.y - a.y)) / len : dist(p, a);
    if (d > max) {
      max = d;
      idx = i;
    }
  }
  if (max <= eps) return [a, b];
  return [...simplify(pts.slice(0, idx + 1), eps).slice(0, -1), ...simplify(pts.slice(idx), eps)];
}

/** Classify a single stroke as a clean geometric shape, or null. */
export function recognizeShape(pts: Pt[]): Recognized | null {
  if (pts.length < 2) return null;
  const xs = pts.map((p) => p.x);
  const ys = pts.map((p) => p.y);
  const minX = Math.min(...xs);
  const minY = Math.min(...ys);
  const w = Math.max(...xs) - minX;
  const hgt = Math.max(...ys) - minY;
  const diag = Math.hypot(w, hgt);
  if (diag < 8) return null;
  const length = pathLength(pts);
  const closed = dist(pts[0]!, pts[pts.length - 1]!) < Math.max(12, diag * 0.2);
  if (!closed) {
    const straight = dist(pts[0]!, pts[pts.length - 1]!) / length;
    if (straight > 0.92) return { preset: "line", w: Math.max(w, 2), h: Math.max(hgt, 2) };
    // an arrow: a long straight shaft followed by a short hook at the end
    const simp = simplify(pts, diag * 0.08);
    if (simp.length >= 3) {
      const shaft = dist(simp[0]!, simp[1]!);
      if (shaft > diag * 0.6) return { preset: "rightArrow", w: Math.max(w, 10), h: Math.max(hgt, 10) };
    }
    return null;
  }
  // closed shapes: circularity first (radius spread around the centroid)
  const cx = xs.reduce((s, v) => s + v, 0) / xs.length;
  const cy = ys.reduce((s, v) => s + v, 0) / ys.length;
  const rx = w / 2 || 1;
  const ry = hgt / 2 || 1;
  const radial = pts.map((p) => Math.hypot((p.x - cx) / rx, (p.y - cy) / ry));
  const mean = radial.reduce((s, v) => s + v, 0) / radial.length;
  const spread = Math.sqrt(radial.reduce((s, v) => s + (v - mean) ** 2, 0) / radial.length) / mean;
  const ring = [...pts];
  if (dist(ring[0]!, ring[ring.length - 1]!) > 0) ring.push(ring[0]!);
  // RDP needs distinct endpoints: split the ring at the point farthest from its start.
  let far = 0;
  ring.forEach((p, i) => {
    if (dist(p, ring[0]!) > dist(ring[far]!, ring[0]!)) far = i;
  });
  let corners = [...simplify(ring.slice(0, far + 1), diag * 0.07).slice(0, -1), ...simplify(ring.slice(far), diag * 0.07)];
  if (corners.length > 1 && dist(corners[0]!, corners[corners.length - 1]!) < diag * 0.15) corners = corners.slice(0, -1);
  const n = corners.length;
  if (spread < 0.08 && n > 4) return { preset: "ellipse", w, h: hgt };
  if (n === 3) return { preset: "triangle", w, h: hgt };
  if (n === 4) {
    // diamond: vertices sit near the midpoints of the bounding box edges
    const mids = [{ x: minX + w / 2, y: minY }, { x: minX + w, y: minY + hgt / 2 }, { x: minX + w / 2, y: minY + hgt }, { x: minX, y: minY + hgt / 2 }];
    const nearMid = corners.filter((c) => mids.some((m) => dist(c, m) < diag * 0.15)).length;
    return { preset: nearMid >= 3 ? "diamond" : "rect", w, h: hgt };
  }
  if (spread < 0.15) return { preset: "ellipse", w, h: hgt };
  return n > 4 && n <= 6 ? { preset: "rect", w, h: hgt } : null;
}

export function shapeInline(r: Recognized, stroke = "1F2937"): Inline {
  return { type: "shape", preset: r.preset, cx: Math.round(Math.max(r.w, 4) * EMU_PER_PX), cy: Math.round(Math.max(r.h, 4) * EMU_PER_PX), stroke, xml: "" };
}

/** Render strokes to a white-background PNG (base64) for handwriting recognition. */
export function strokesToPng(strokes: Pt[][]): string | null {
  const all = strokes.flat();
  if (!all.length) return null;
  const minX = Math.min(...all.map((p) => p.x)) - 12;
  const minY = Math.min(...all.map((p) => p.y)) - 12;
  const w = Math.max(...all.map((p) => p.x)) - minX + 12;
  const hh = Math.max(...all.map((p) => p.y)) - minY + 12;
  const c = document.createElement("canvas");
  c.width = Math.ceil(w);
  c.height = Math.ceil(hh);
  const ctx = c.getContext("2d");
  if (!ctx) return null;
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, c.width, c.height);
  ctx.strokeStyle = "#000";
  ctx.lineWidth = 3;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  for (const s of strokes) {
    ctx.beginPath();
    s.forEach((p, i) => (i ? ctx.lineTo(p.x - minX, p.y - minY) : ctx.moveTo(p.x - minX, p.y - minY)));
    ctx.stroke();
  }
  return c.toDataURL("image/png").split(",")[1] ?? null;
}

/** Full-editor ink overlay with Shapes / Text / Cancel actions. */
export function openInkOverlay(editor: WordEditor, host: HostBridge) {
  editor.el.querySelector(".ink-layer")?.remove();
  const strokes: Pt[][] = [];
  const canvas = h("canvas", { class: "ink-canvas", "aria-label": "Ink surface" });
  const status = h("span", { class: "muted small" }, "Draw with a pen, finger or mouse.");
  const close = () => layer.remove();
  const toShapes = () => {
    const shapes = strokes.map((s) => recognizeShape(s)).filter((r): r is Recognized => !!r);
    if (!shapes.length) return void (status.textContent = "No shapes recognised — try a cleaner stroke.");
    for (const r of shapes) editor.execute("insertShape", JSON.stringify(shapeInline(r)));
    close();
  };
  const toText = async () => {
    const png = strokesToPng(strokes);
    if (!png) return;
    status.textContent = "Recognising handwriting…";
    try {
      const res = await host.aiComplete(handwritingRequest(png));
      const text = res.text.trim();
      if (!text) return void (status.textContent = "No legible handwriting found.");
      editor.insertText(text, "insert");
      close();
    } catch (e) {
      status.textContent = String(e instanceof Error ? e.message : e);
    }
  };
  const bar = h(
    "div",
    { class: "ink-bar" },
    status,
    h("button", { class: "btn btn-quiet", onclick: toShapes }, "Convert to shapes"),
    h("button", { class: "btn btn-quiet", onclick: () => void toText() }, "Convert to text"),
    h("button", { class: "btn btn-quiet", onclick: () => { strokes.length = 0; draw(); } }, "Clear"),
    h("button", { class: "btn", onclick: close }, "Done"),
  );
  const layer = h("div", { class: "ink-layer" }, canvas, bar);
  editor.el.append(layer);
  const ctx = canvas.getContext("2d");
  const fit = () => {
    canvas.width = layer.clientWidth;
    canvas.height = layer.clientHeight;
    draw();
  };
  const draw = () => {
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.strokeStyle = "#2563eb";
    ctx.lineWidth = 2.5;
    ctx.lineCap = "round";
    for (const s of strokes) {
      ctx.beginPath();
      s.forEach((p, i) => (i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y)));
      ctx.stroke();
    }
  };
  let cur: Pt[] | null = null;
  canvas.addEventListener("pointerdown", (e) => {
    canvas.setPointerCapture?.(e.pointerId);
    cur = [{ x: e.offsetX, y: e.offsetY }];
    strokes.push(cur);
  });
  canvas.addEventListener("pointermove", (e) => {
    if (!cur) return;
    for (const ev of e.getCoalescedEvents?.() ?? [e]) cur.push({ x: ev.offsetX, y: ev.offsetY });
    draw();
  });
  canvas.addEventListener("pointerup", () => (cur = null));
  requestAnimationFrame(fit);
  return { strokes, toShapes, toText, close };
}
