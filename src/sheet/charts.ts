/** Canvas renderer for embedded charts (column, bar, line, area, pie, scatter). */
import type { ChartData } from "./engine";
import type { ChartKind } from "./model";

/** Office theme accents, the default series colours in Excel. */
export const SERIES_COLORS = ["#4472C4", "#ED7D31", "#A5A5A5", "#FFC000", "#5B9BD5", "#70AD47", "#264478", "#9E480E", "#636363", "#997300"];

/** "Nice" axis bounds and step for a value range. */
export function niceScale(min: number, max: number, ticks = 5): { lo: number; hi: number; step: number } {
  if (!Number.isFinite(min) || !Number.isFinite(max)) return { lo: 0, hi: 1, step: 0.2 };
  if (min > 0) min = 0;
  if (max < 0) max = 0;
  if (min === max) max = min + 1;
  const raw = (max - min) / ticks;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? 10 * mag;
  return { lo: Math.floor(min / step) * step, hi: Math.ceil(max / step) * step, step };
}

function label(n: number): string {
  const a = Math.abs(n);
  if (a >= 1e9) return `${+(n / 1e9).toFixed(1)}B`;
  if (a >= 1e6) return `${+(n / 1e6).toFixed(1)}M`;
  if (a >= 1e4) return `${+(n / 1e3).toFixed(1)}K`;
  return String(+n.toFixed(6));
}

export function drawChart(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, kind: ChartKind, title: string, data: ChartData, selected = false) {
  ctx.save();
  ctx.beginPath();
  ctx.rect(x, y, w, h);
  ctx.clip();
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(x, y, w, h);
  ctx.strokeStyle = selected ? "#059669" : "#d1d5db";
  ctx.lineWidth = selected ? 2 : 1;
  ctx.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);
  ctx.textBaseline = "middle";
  let top = y + 10;
  if (title) {
    ctx.fillStyle = "#374151";
    ctx.font = "600 15px Calibri, Carlito, system-ui, sans-serif";
    ctx.textAlign = "center";
    ctx.fillText(title, x + w / 2, y + 18, w - 20);
    top = y + 36;
  }
  const series = data.series.filter((s) => s.values.length);
  // legend
  ctx.font = "11px Calibri, Carlito, system-ui, sans-serif";
  const legendItems = kind === "pie" ? data.categories : series.map((s) => s.name);
  const legendH = legendItems.length ? 20 : 0;
  let lx = x + 12;
  const ly = y + h - 12;
  ctx.textAlign = "left";
  legendItems.slice(0, 12).forEach((name, i) => {
    ctx.fillStyle = SERIES_COLORS[i % SERIES_COLORS.length]!;
    ctx.fillRect(lx, ly - 4, 8, 8);
    ctx.fillStyle = "#4b5563";
    ctx.fillText(name, lx + 11, ly);
    lx += 22 + ctx.measureText(name).width;
  });
  const bottom = y + h - 8 - legendH;
  if (!series.length) {
    ctx.fillStyle = "#9ca3af";
    ctx.textAlign = "center";
    ctx.fillText("No numeric data", x + w / 2, (top + bottom) / 2);
    ctx.restore();
    return;
  }
  if (kind === "pie") {
    const vals = series[0]!.values.map((v) => Math.max(0, v ?? 0));
    const total = vals.reduce((a, b) => a + b, 0) || 1;
    const r = Math.max(10, Math.min(w - 24, bottom - top) / 2 - 4);
    const cx = x + w / 2;
    const cy = (top + bottom) / 2;
    let a0 = -Math.PI / 2;
    vals.forEach((v, i) => {
      const a1 = a0 + (v / total) * Math.PI * 2;
      ctx.beginPath();
      ctx.moveTo(cx, cy);
      ctx.arc(cx, cy, r, a0, a1);
      ctx.closePath();
      ctx.fillStyle = SERIES_COLORS[i % SERIES_COLORS.length]!;
      ctx.fill();
      ctx.strokeStyle = "#fff";
      ctx.lineWidth = 1.5;
      ctx.stroke();
      a0 = a1;
    });
    ctx.restore();
    return;
  }
  const all = series.flatMap((s) => s.values.filter((v): v is number => v !== null));
  const scatter = kind === "scatter";
  const horizontal = kind === "bar";
  const { lo, hi, step } = niceScale(Math.min(...all), Math.max(...all));
  ctx.font = "11px Calibri, Carlito, system-ui, sans-serif";
  const axisW = Math.max(...[lo, hi].map((v) => ctx.measureText(label(v)).width)) + 10;
  const catW = horizontal ? Math.min(w * 0.3, Math.max(...data.categories.map((c) => ctx.measureText(c).width), 10) + 10) : 0;
  const px0 = x + 10 + (horizontal ? catW : axisW);
  const px1 = x + w - 14;
  const py0 = top + 4;
  const py1 = bottom - (horizontal ? 18 : 16);
  if (px1 - px0 < 20 || py1 - py0 < 20) {
    ctx.restore();
    return;
  }
  const valPos = (v: number) => (horizontal ? px0 + ((v - lo) / (hi - lo)) * (px1 - px0) : py1 - ((v - lo) / (hi - lo)) * (py1 - py0));
  // gridlines + value labels
  ctx.strokeStyle = "#e5e7eb";
  ctx.lineWidth = 1;
  ctx.fillStyle = "#6b7280";
  for (let v = lo; v <= hi + step / 2; v += step) {
    const p = Math.round(valPos(v)) + 0.5;
    ctx.beginPath();
    if (horizontal) {
      ctx.moveTo(p, py0);
      ctx.lineTo(p, py1);
      ctx.textAlign = "center";
      ctx.fillText(label(v), p, py1 + 10);
    } else {
      ctx.moveTo(px0, p);
      ctx.lineTo(px1, p);
      ctx.textAlign = "right";
      ctx.fillText(label(v), px0 - 5, p);
    }
    ctx.stroke();
  }
  const n = Math.max(1, ...series.map((s) => s.values.length));
  const band = (horizontal ? py1 - py0 : px1 - px0) / n;
  // category labels
  ctx.fillStyle = "#4b5563";
  const every = Math.max(1, Math.ceil((n * 40) / (horizontal ? py1 - py0 : px1 - px0)));
  for (let i = 0; i < n; i += scatter ? n : every) {
    const c = data.categories[i] ?? String(i + 1);
    if (horizontal) {
      ctx.textAlign = "right";
      ctx.fillText(c, px0 - 5, py0 + band * (i + 0.5), catW - 6);
    } else if (!scatter) {
      ctx.textAlign = "center";
      ctx.fillText(c, px0 + band * (i + 0.5), py1 + 10, band * every);
    }
  }
  const zero = valPos(Math.max(lo, Math.min(hi, 0)));
  series.forEach((s, si) => {
    const color = SERIES_COLORS[si % SERIES_COLORS.length]!;
    ctx.fillStyle = color;
    ctx.strokeStyle = color;
    if (kind === "column" || kind === "bar") {
      const gap = band * 0.3;
      const bw = (band - gap) / series.length;
      s.values.forEach((v, i) => {
        if (v === null) return;
        const p = valPos(v);
        const off = band * i + gap / 2 + bw * si;
        if (horizontal) ctx.fillRect(Math.min(zero, p), py0 + off, Math.abs(p - zero), bw);
        else ctx.fillRect(px0 + off, Math.min(zero, p), bw, Math.abs(p - zero));
      });
    } else {
      const pts = s.values.map((v, i) => (v === null ? null : ([scatter ? px0 + ((i + 0.5) / n) * (px1 - px0) : px0 + band * (i + 0.5), valPos(v)] as const)));
      if (kind === "area") {
        ctx.globalAlpha = 0.55;
        ctx.beginPath();
        let started = false;
        let lastX = px0;
        pts.forEach((p) => {
          if (!p) return;
          if (!started) {
            ctx.moveTo(p[0], zero);
            started = true;
          }
          ctx.lineTo(p[0], p[1]);
          lastX = p[0];
        });
        ctx.lineTo(lastX, zero);
        ctx.closePath();
        ctx.fill();
        ctx.globalAlpha = 1;
      }
      if (!scatter) {
        ctx.lineWidth = 2.25;
        ctx.beginPath();
        let pen = false;
        for (const p of pts) {
          if (!p) {
            pen = false;
            continue;
          }
          if (pen) ctx.lineTo(p[0], p[1]);
          else ctx.moveTo(p[0], p[1]);
          pen = true;
        }
        ctx.stroke();
      }
      if (kind !== "area") {
        for (const p of pts) {
          if (!p) continue;
          ctx.beginPath();
          ctx.arc(p[0], p[1], scatter ? 3.5 : 2.5, 0, Math.PI * 2);
          ctx.fill();
        }
      }
    }
  });
  // axis line
  ctx.strokeStyle = "#9ca3af";
  ctx.lineWidth = 1;
  ctx.beginPath();
  if (horizontal) {
    ctx.moveTo(Math.round(zero) + 0.5, py0);
    ctx.lineTo(Math.round(zero) + 0.5, py1);
  } else {
    ctx.moveTo(px0, Math.round(zero) + 0.5);
    ctx.lineTo(px1, Math.round(zero) + 0.5);
  }
  ctx.stroke();
  ctx.restore();
}
