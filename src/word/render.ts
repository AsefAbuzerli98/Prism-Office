/** Canvas painter for laid-out pages, selection and caret (P2-07, 2D backend). */
import type { Frag, Layout, Line } from "./layout";

export const PAGE_GAP = 24;

export interface Viewport {
  scrollTop: number;
  scrollLeft: number;
  width: number;
  height: number;
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export function pageOrigin(layout: Layout, index: number, viewportWidth: number) {
  return {
    x: Math.max(PAGE_GAP, (viewportWidth - layout.width) / 2),
    y: PAGE_GAP + index * (layout.height + PAGE_GAP),
  };
}

export function totalHeight(layout: Layout) {
  return PAGE_GAP + layout.pages.length * (layout.height + PAGE_GAP);
}

function drawFrag(ctx: CanvasRenderingContext2D, f: Frag, line: Line, ox: number, oy: number) {
  const s = f.style;
  const baseY = oy + line.y + line.baseline + (s.vert === "superscript" ? -s.sizePx * 0.55 : s.vert === "subscript" ? s.sizePx * 0.25 : 0);
  if (s.highlight && f.kind !== "marker") {
    ctx.fillStyle = s.highlight;
    ctx.fillRect(ox + f.x, oy + line.y, f.width, line.height);
  }
  if (f.kind === "tab") return;
  if (f.kind === "opaque") {
    if (!f.text) return;
    ctx.fillStyle = "#f1f5f9";
    ctx.strokeStyle = "#cbd5e1";
    ctx.fillRect(ox + f.x + 1, oy + line.y + 2, f.width - 2, line.height - 4);
    ctx.strokeRect(ox + f.x + 1.5, oy + line.y + 2.5, f.width - 3, line.height - 5);
    ctx.fillStyle = "#64748b";
    ctx.font = s.font;
    ctx.fillText(f.text, ox + f.x + 3, baseY);
    return;
  }
  ctx.font = s.font;
  ctx.fillStyle = s.color;
  ctx.fillText(f.text, ox + f.x, baseY);
  if ((s.underline || s.strike) && f.text.trim()) {
    const w = f.width - (/\s$/.test(f.text) ? ctx.measureText(f.text.match(/\s+$/)![0]).width : 0);
    ctx.fillRect(ox + f.x, s.underline ? baseY + Math.max(1, s.sizePx * 0.08) : baseY - s.sizePx * 0.3, w, Math.max(1, s.sizePx / 16));
  }
}

export function paint(
  ctx: CanvasRenderingContext2D,
  layout: Layout,
  vp: Viewport,
  dpr: number,
  selection: Rect[],
  caret: Rect | null,
  dark: boolean,
) {
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = dark ? "#18181b" : "#eef0f3";
  ctx.fillRect(0, 0, vp.width, vp.height);
  ctx.translate(-vp.scrollLeft, -vp.scrollTop);
  ctx.textBaseline = "alphabetic";
  for (const page of layout.pages) {
    const o = pageOrigin(layout, page.index, vp.width);
    if (o.y + layout.height < vp.scrollTop || o.y > vp.scrollTop + vp.height) continue;
    ctx.fillStyle = "rgba(0,0,0,0.12)";
    ctx.fillRect(o.x + 1, o.y + 2, layout.width, layout.height);
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(o.x, o.y, layout.width, layout.height);
    for (const b of page.boxes) {
      ctx.fillStyle = "#f8fafc";
      ctx.strokeStyle = "#cbd5e1";
      ctx.setLineDash([4, 3]);
      ctx.fillRect(o.x + b.left, o.y + b.y, b.right - b.left, b.height);
      ctx.strokeRect(o.x + b.left + 0.5, o.y + b.y + 0.5, b.right - b.left - 1, b.height - 1);
      ctx.setLineDash([]);
      ctx.fillStyle = "#64748b";
      ctx.font = "13px system-ui, sans-serif";
      ctx.fillText(`⟦${b.label}⟧ — preserved, editing arrives in a later phase`, o.x + b.left + 10, o.y + b.y + b.height / 2 + 4);
    }
    for (const line of page.lines) {
      if (o.y + line.y > vp.scrollTop + vp.height || o.y + line.y + line.height < vp.scrollTop) continue;
      for (const f of line.frags) drawFrag(ctx, f, line, o.x, o.y);
    }
  }
  ctx.fillStyle = "rgba(37, 99, 235, 0.25)";
  for (const r of selection) ctx.fillRect(r.x, r.y, r.w, r.h);
  if (caret) {
    ctx.fillStyle = "#111827";
    ctx.fillRect(Math.round(caret.x), caret.y, 1.5, caret.h);
  }
}
