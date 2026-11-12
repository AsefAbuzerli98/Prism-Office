/** Canvas painter for laid-out pages, selection and caret (P2-07). */
import { sameContainer, type ContainerRef, type Frag, type Layout, type Line, type Page } from "./layout";

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

export interface PaintOptions {
  /** Container being edited (headers/footers are dimmed otherwise). */
  active: ContainerRef | null;
  images: (relId: string) => CanvasImageSource | null;
  zoom?: number;
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

function drawShape(ctx: CanvasRenderingContext2D, f: Frag, x: number, y: number) {
  const w = f.width;
  const h = f.height ?? 0;
  ctx.save();
  ctx.lineWidth = 1.5;
  ctx.strokeStyle = `#${f.stroke || "1F2937"}`;
  ctx.fillStyle = f.fill ? `#${f.fill}` : "transparent";
  ctx.beginPath();
  switch (f.preset) {
    case "ellipse":
      ctx.ellipse(x + w / 2, y + h / 2, w / 2 - 1, h / 2 - 1, 0, 0, Math.PI * 2);
      break;
    case "triangle":
      ctx.moveTo(x + w / 2, y + 1);
      ctx.lineTo(x + w - 1, y + h - 1);
      ctx.lineTo(x + 1, y + h - 1);
      ctx.closePath();
      break;
    case "line":
      ctx.moveTo(x, y + h);
      ctx.lineTo(x + w, y);
      break;
    case "rightArrow": {
      const hy = h / 2;
      ctx.moveTo(x, y + hy);
      ctx.lineTo(x + w - 2, y + hy);
      ctx.moveTo(x + w - Math.min(14, w / 3), y + hy - Math.min(8, h / 2));
      ctx.lineTo(x + w - 2, y + hy);
      ctx.lineTo(x + w - Math.min(14, w / 3), y + hy + Math.min(8, h / 2));
      break;
    }
    case "diamond":
      ctx.moveTo(x + w / 2, y + 1);
      ctx.lineTo(x + w - 1, y + h / 2);
      ctx.lineTo(x + w / 2, y + h - 1);
      ctx.lineTo(x + 1, y + h / 2);
      ctx.closePath();
      break;
    default:
      ctx.rect(x + 1, y + 1, w - 2, h - 2);
  }
  if (f.fill) ctx.fill();
  ctx.stroke();
  ctx.restore();
}

function drawFrag(ctx: CanvasRenderingContext2D, f: Frag, line: Line, ox: number, oy: number, opts: PaintOptions) {
  const s = f.style;
  const baseY = oy + line.y + line.baseline + (s.vert === "superscript" ? -s.sizePx * 0.55 : s.vert === "subscript" ? s.sizePx * 0.25 : 0);
  if (s.highlight && f.kind !== "marker" && f.kind !== "image" && f.kind !== "shape") {
    ctx.fillStyle = s.highlight;
    ctx.fillRect(ox + f.x, oy + line.y, f.width, line.height);
  }
  if (f.kind === "image" || f.kind === "shape") {
    const top = oy + line.y + line.baseline - (f.height ?? 0);
    if (f.kind === "shape") return drawShape(ctx, f, ox + f.x, top);
    const img = f.relId ? opts.images(f.relId) : null;
    if (img) ctx.drawImage(img, ox + f.x, top, f.width, f.height ?? 0);
    else {
      ctx.fillStyle = "#f1f5f9";
      ctx.fillRect(ox + f.x, top, f.width, f.height ?? 0);
      ctx.strokeStyle = "#cbd5e1";
      ctx.strokeRect(ox + f.x + 0.5, top + 0.5, f.width - 1, (f.height ?? 0) - 1);
    }
    return;
  }
  if (f.kind === "tab") {
    if (f.leader && f.width > 8) {
      ctx.font = s.font;
      ctx.fillStyle = s.color;
      const unit = ctx.measureText(f.leader + " ").width || 4;
      let x = ox + f.x + 2;
      while (x + unit < ox + f.x + f.width - 2) {
        ctx.fillText(f.leader, x, baseY);
        x += unit;
      }
    }
    return;
  }
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
  const text = f.text.replace(/­/g, "");
  ctx.font = s.font;
  ctx.fillStyle = s.color;
  if (f.rtl) {
    ctx.direction = "rtl";
    ctx.textAlign = "right";
    ctx.fillText(text, ox + f.x + f.width, baseY);
    ctx.direction = "ltr";
    ctx.textAlign = "left";
  } else ctx.fillText(text, ox + f.x, baseY);
  if ((s.underline || s.strike) && text.trim()) {
    const w = f.width - (/\s$/.test(text) ? ctx.measureText(text.match(/\s+$/)![0]).width : 0);
    ctx.fillRect(ox + f.x, s.underline && !s.strike ? baseY + Math.max(1, s.sizePx * 0.08) : baseY - s.sizePx * 0.3, w, Math.max(1, s.sizePx / 16));
  }
}

/** Rasterise one page at page-local coordinates (cached by the compositor). */
export function paintPage(ctx: CanvasRenderingContext2D, layout: Layout, page: Page, opts: PaintOptions) {
  ctx.textBaseline = "alphabetic";
  const editingHeader = opts.active?.type === "header";
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, layout.width, layout.height);
  for (const d of page.decos.filter((d) => d.fill)) {
    ctx.fillStyle = d.fill!;
    ctx.fillRect(d.x, d.y, d.w, d.h);
  }
  for (const b of page.boxes) {
    ctx.fillStyle = "#f8fafc";
    ctx.strokeStyle = "#cbd5e1";
    ctx.setLineDash([4, 3]);
    ctx.fillRect(b.left, b.y, b.right - b.left, b.height);
    ctx.strokeRect(b.left + 0.5, b.y + 0.5, b.right - b.left - 1, b.height - 1);
    ctx.setLineDash([]);
    ctx.fillStyle = "#64748b";
    ctx.font = "13px system-ui, sans-serif";
    ctx.fillText(`⟦${b.label}⟧ — preserved`, b.left + 10, b.y + b.height / 2 + 4);
  }
  for (const line of page.lines) {
    const dim = line.container?.type === "header" && !editingHeader;
    if (dim) ctx.globalAlpha = 0.55;
    for (const f of line.frags) drawFrag(ctx, f, line, 0, 0, opts);
    if (dim) ctx.globalAlpha = 1;
  }
  for (const d of page.decos.filter((d) => d.stroke)) {
    ctx.fillStyle = d.stroke!;
    ctx.fillRect(d.x, d.y, Math.max(d.w, 0.75), Math.max(d.h, 0.75));
  }
}

/** Per-frame overlay: selection, caret and the active container outline. */
export function paintOverlay(ctx: CanvasRenderingContext2D, layout: Layout, vp: Viewport, dpr: number, selection: Rect[], caret: Rect | null, active: ContainerRef | null) {
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, vp.width, vp.height);
  ctx.translate(-vp.scrollLeft, -vp.scrollTop);
  if (active && active.type !== "cell") {
    for (const page of layout.pages) {
      const o = pageOrigin(layout, page.index, vp.width);
      for (const a of page.areas.filter((a) => sameContainer(a.container, active))) {
        ctx.strokeStyle = "#2563eb";
        ctx.setLineDash([5, 4]);
        ctx.strokeRect(o.x + a.x - 4.5, o.y + a.y - 4.5, a.w + 9, a.h + 9);
        ctx.setLineDash([]);
      }
    }
  }
  ctx.fillStyle = "rgba(37, 99, 235, 0.25)";
  for (const r of selection) ctx.fillRect(r.x, r.y, r.w, r.h);
  if (caret) {
    ctx.fillStyle = "#111827";
    ctx.fillRect(Math.round(caret.x), caret.y, 1.5, caret.h);
  }
}

/** Single-canvas painter (pages + overlay), used when no compositor is available. */
export function paint(
  ctx: CanvasRenderingContext2D,
  layout: Layout,
  vp: Viewport,
  dpr: number,
  selection: Rect[],
  caret: Rect | null,
  dark: boolean,
  opts: PaintOptions = { active: null, images: () => null },
) {
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = dark ? "#18181b" : "#eef0f3";
  ctx.fillRect(0, 0, vp.width, vp.height);
  for (const page of layout.pages) {
    const o = pageOrigin(layout, page.index, vp.width);
    if (o.y + layout.height < vp.scrollTop || o.y > vp.scrollTop + vp.height) continue;
    ctx.setTransform(dpr, 0, 0, dpr, (o.x - vp.scrollLeft) * dpr, (o.y - vp.scrollTop) * dpr);
    ctx.fillStyle = "rgba(0,0,0,0.12)";
    ctx.fillRect(1, 2, layout.width, layout.height);
    paintPage(ctx, layout, page, opts);
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.translate(-vp.scrollLeft, -vp.scrollTop);
  ctx.fillStyle = "rgba(37, 99, 235, 0.25)";
  for (const r of selection) ctx.fillRect(r.x, r.y, r.w, r.h);
  if (caret) {
    ctx.fillStyle = "#111827";
    ctx.fillRect(Math.round(caret.x), caret.y, 1.5, caret.h);
  }
}
