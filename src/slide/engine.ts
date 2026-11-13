/**
 * Typed wrapper over the DrawingML WASM module (crates/prism-drawingml,
 * built by scripts/build-drawingml-wasm.sh): preset/custom geometry and
 * theme colour resolution, so the canvas renderer matches the Rust side.
 */
import init, { initSync, ColorContext, evaluateGeometry, parseFill, parseLine, presetDefinition, presetNames } from "./wasm/prism_drawingml.js";
import type { Color, EvaluatedGeometry, Fill, GeomDef, Geometry, Line, Master } from "./model";

let ready: Promise<void> | null = null;
let loaded = false;

export function loadDrawingml(): Promise<void> {
  if (loaded) return Promise.resolve();
  ready ??= init().then(() => {
    loaded = true;
  });
  return ready;
}

/** Synchronous init from bytes (tests). */
export function loadDrawingmlSync(bytes: BufferSource): void {
  if (loaded) return;
  initSync({ module: bytes });
  loaded = true;
}

export function isDrawingmlLoaded() {
  return loaded;
}

const geomCache = new Map<string, EvaluatedGeometry>();

/** Evaluate geometry at a size (EMU); results are cached. */
export function evaluate(geom: Geometry, w: number, h: number): EvaluatedGeometry {
  const key = `${JSON.stringify(geom)}|${Math.round(w)}|${Math.round(h)}`;
  let e = geomCache.get(key);
  if (!e) {
    e = JSON.parse(evaluateGeometry(JSON.stringify(geom), Math.max(w, 0), Math.max(h, 0))) as EvaluatedGeometry;
    if (geomCache.size > 4000) geomCache.clear();
    geomCache.set(key, e);
  }
  return e;
}

export function presetDef(name: string): GeomDef {
  return JSON.parse(presetDefinition(name)) as GeomDef;
}

let names: string[] | null = null;
export function allPresetNames(): string[] {
  return (names ??= JSON.parse(presetNames()) as string[]);
}

/** Colours of one master (theme + colour map). */
export class Palette {
  private ctx: ColorContext;
  private cache = new Map<string, string>();
  private fills: (Fill | null)[];
  private bgFills: (Fill | null)[];
  private lines: (Line | null)[];

  constructor(master: Pick<Master, "theme" | "clrMap">) {
    this.ctx = new ColorContext(JSON.stringify(master.theme), JSON.stringify(master.clrMap));
    this.fills = (master.theme.fillStyles ?? []).map((x) => JSON.parse(parseFill(x)) as Fill | null);
    this.bgFills = (master.theme.bgFillStyles ?? []).map((x) => JSON.parse(parseFill(x)) as Fill | null);
    this.lines = (master.theme.lineStyles ?? []).map((x) => JSON.parse(parseLine(x)) as Line | null);
  }

  /** CSS colour; `ph` is the colour substituted for `phClr` (CSS `rgba(...)` or `#hex`). */
  css(c: Color, ph?: string): string {
    const key = JSON.stringify(c) + (ph ?? "");
    let v = this.cache.get(key);
    if (v === undefined) {
      v = this.ctx.resolve(JSON.stringify(c), ph ? toHex(ph) : undefined);
      this.cache.set(key, v);
    }
    return v;
  }

  /** Theme style-matrix fill: idx 1..3 (fillStyleLst) or 1001.. (bgFillStyleLst). */
  styleFill(idx: number): Fill | null {
    if (idx >= 1001) return this.bgFills[idx - 1001] ?? null;
    if (idx <= 0) return null;
    return this.fills[idx - 1] ?? null;
  }

  styleLine(idx: number): Line | null {
    return idx > 0 ? (this.lines[idx - 1] ?? null) : null;
  }

  dispose() {
    this.ctx.free();
  }
}

/** `rgba(r,g,b,a)` / `#rrggbb` → `#RRGGBB`. */
export function toHex(css: string): string {
  if (css.startsWith("#")) return css.slice(0, 7);
  const m = /rgba?\(([\d.]+),\s*([\d.]+),\s*([\d.]+)/.exec(css);
  if (!m) return "#000000";
  return "#" + [m[1], m[2], m[3]].map((v) => Math.round(Number(v)).toString(16).padStart(2, "0")).join("");
}
