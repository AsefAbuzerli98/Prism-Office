import { h } from "../core/dom";
import { MODES, type Mode } from "../core/modes";

/**
 * Placeholder engine surfaces drawn on a 2D canvas. Phase 2–4 replace these
 * with the WebGPU layout/grid/slide compositors; the mounting contract
 * (one canvas per tab, redraw on resize) stays the same.
 */
export function renderEngineView(mode: Mode): HTMLElement {
  const canvas = h("canvas", { class: "engine-canvas", "aria-label": `${MODES[mode].label} canvas` });
  const wrap = h("div", { class: "engine-view", dataset: { mode } }, canvas);
  const draw = () => {
    const dpr = window.devicePixelRatio || 1;
    const { width, height } = wrap.getBoundingClientRect();
    if (width === 0 || height === 0) return;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    if (mode === "document") drawPage(ctx, width, height);
    else if (mode === "workbook") drawGrid(ctx, width, height);
    else drawSlide(ctx, width, height);
  };
  if (typeof ResizeObserver !== "undefined") new ResizeObserver(draw).observe(wrap);
  requestAnimationFrame(draw);
  return wrap;
}

function drawPage(ctx: CanvasRenderingContext2D, w: number, hgt: number) {
  const pageW = Math.min(794, w - 80); // A4 @ 96dpi
  const pageH = pageW * 1.414;
  const x = (w - pageW) / 2;
  const y = 32;
  ctx.fillStyle = "rgba(0,0,0,.08)";
  ctx.fillRect(x + 3, y + 4, pageW, Math.min(pageH, hgt));
  ctx.fillStyle = "#fff";
  ctx.fillRect(x, y, pageW, Math.min(pageH, hgt));
  const m = pageW * 0.12;
  ctx.fillStyle = MODES.document.accent.base;
  ctx.fillRect(x + m, y + m, 2, 22); // caret
  ctx.fillStyle = "#9ca3af";
  ctx.font = "15px system-ui, sans-serif";
  ctx.fillText("Start typing… (layout engine arrives in Phase 2)", x + m + 10, y + m + 16);
}

function drawGrid(ctx: CanvasRenderingContext2D, w: number, hgt: number) {
  const colW = 96;
  const rowH = 24;
  const hdrW = 48;
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, w, hgt);
  ctx.fillStyle = "#f3f4f6";
  ctx.fillRect(0, 0, w, rowH);
  ctx.fillRect(0, 0, hdrW, hgt);
  ctx.strokeStyle = "#e5e7eb";
  ctx.lineWidth = 1;
  ctx.font = "12px system-ui, sans-serif";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillStyle = "#6b7280";
  for (let c = 0, x = hdrW; x < w; c++, x += colW) {
    ctx.beginPath();
    ctx.moveTo(x + 0.5, 0);
    ctx.lineTo(x + 0.5, hgt);
    ctx.stroke();
    ctx.fillText(columnName(c), x + colW / 2, rowH / 2);
  }
  for (let r = 1, y = rowH; y < hgt; r++, y += rowH) {
    ctx.beginPath();
    ctx.moveTo(0, y + 0.5);
    ctx.lineTo(w, y + 0.5);
    ctx.stroke();
    ctx.fillText(String(r), hdrW / 2, y + rowH / 2);
  }
  ctx.strokeStyle = MODES.workbook.accent.base;
  ctx.lineWidth = 2;
  ctx.strokeRect(hdrW + 1, rowH + 1, colW - 2, rowH - 2); // A1 selection
}

function drawSlide(ctx: CanvasRenderingContext2D, w: number, hgt: number) {
  const railW = 160;
  ctx.fillStyle = "#f3f4f6";
  ctx.fillRect(0, 0, railW, hgt);
  const thumbW = railW - 32;
  ctx.fillStyle = "#fff";
  ctx.fillRect(16, 16, thumbW, (thumbW * 9) / 16);
  ctx.strokeStyle = MODES.presentation.accent.base;
  ctx.lineWidth = 2;
  ctx.strokeRect(16, 16, thumbW, (thumbW * 9) / 16);

  const availW = w - railW - 64;
  const availH = hgt - 64;
  const sw = Math.min(availW, (availH * 16) / 9);
  const sh = (sw * 9) / 16;
  const sx = railW + (w - railW - sw) / 2;
  const sy = (hgt - sh) / 2;
  ctx.fillStyle = "rgba(0,0,0,.08)";
  ctx.fillRect(sx + 3, sy + 4, sw, sh);
  ctx.fillStyle = "#fff";
  ctx.fillRect(sx, sy, sw, sh);
  ctx.fillStyle = "#9ca3af";
  ctx.font = `${Math.max(14, sw / 30)}px system-ui, sans-serif`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText("Click to add title", sx + sw / 2, sy + sh / 2);
}

/** 0 → A, 25 → Z, 26 → AA … */
export function columnName(index: number): string {
  let n = index + 1;
  let s = "";
  while (n > 0) {
    const rem = (n - 1) % 26;
    s = String.fromCharCode(65 + rem) + s;
    n = Math.floor((n - 1) / 26);
  }
  return s;
}
