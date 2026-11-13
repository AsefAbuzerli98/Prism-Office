/**
 * Presentation mode: slide rail with thumbnails, zoomable canvas stage,
 * selection with move/resize/rotate handles, smart guides (snapping),
 * in-place text editing, insert shapes/pictures/tables/text boxes, slide
 * operations, notes, clipboard and undo. Rendering: render.ts.
 */
import { h } from "../core/dom";
import type { HostBridge } from "../core/host";
import type { EngineView } from "../word/editor";
import { createBackend, PageCache, type Backend } from "../word/compositor";
import { evaluate, loadDrawingml } from "./engine";
import type { Paragraph, PresentationModel, RunProps, Slide, SlideNode, TextBody, Xfrm } from "./model";
import {
  applyLayout,
  bounds,
  formatAllText,
  formatParagraphs,
  group,
  materialize,
  newPicture,
  newShape,
  newTable,
  newTextBox,
  reid,
  shapeBody,
  slideFromLayout,
  ungroup,
  zOrder,
} from "./ops";
import { drawSlide, paletteFor, renderSlideToCanvas, type RenderOptions } from "./render";
import { ANIMATION_PRESETS, presetId } from "./anim";
import { TRANSITIONS } from "./transition";
import { decodeXml, parseChartXml, type SlideChart } from "./chart";
import { effectivePara, resolveShape, slideContext } from "./resolve";
import { readEditable, renderEditable, setBlockProps, type EditEnv } from "./textedit";
import { EMU_PER_PT, fitScale, type TextEnv } from "./text";

const ACCENT = "#C2553A";
const HANDLE = 8;
const ROT_OFFSET = 22;
const PAD = 28;
const THUMB_W = 168;

type Drag =
  | { kind: "move"; sx: number; sy: number; orig: Map<number, Xfrm>; moved: boolean }
  | { kind: "resize"; handle: number; id: number; orig: Xfrm; sx: number; sy: number; aspect: boolean }
  | { kind: "rotate"; id: number; orig: Xfrm; cx: number; cy: number }
  | { kind: "marquee"; sx: number; sy: number; x: number; y: number }
  | { kind: "create"; sx: number; sy: number; x: number; y: number; make: (x: Xfrm) => SlideNode };

interface Guide {
  axis: "x" | "y";
  /** Position in EMU. */
  at: number;
}

export interface SlideEditorOptions {
  onDirty?: () => void;
  onState?: () => void;
  onError?: (msg: string) => void;
  /** Start the slideshow at a slide (P4-05). */
  onShow?: (index: number) => void;
  /** Use this model instead of asking the host (tests). */
  model?: PresentationModel;
}

export interface SlideEditorState {
  editing: boolean;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strike: boolean;
  align: string | null;
  bullets: boolean;
  numbering: boolean;
  selection: number;
  slide: number;
  slides: number;
  hidden: boolean;
  layout: string;
  canGroup: boolean;
  canUngroup: boolean;
}

const pt = (emu: number) => emu / EMU_PER_PT;

export class SlideEditor implements EngineView {
  readonly el: HTMLElement;
  readonly ready: Promise<void>;
  model: PresentationModel | null = null;
  current = 0;
  readonly selection = new Set<number>();
  zoom: number | "fit" = "fit";

  private rail: HTMLElement;
  private railList: HTMLElement;
  private stage: HTMLElement;
  /** Composited slide raster (WebGPU/WebGL2/Canvas 2D backend). */
  private canvas: HTMLCanvasElement;
  /** Selection chrome on top (or everything, without a backend). */
  private overlayCanvas: HTMLCanvasElement;
  private backend: Backend | null = null;
  private readonly raster = new PageCache((w, hh) => {
    if (typeof OffscreenCanvas !== "undefined") return new OffscreenCanvas(w, hh);
    const c = document.createElement("canvas");
    c.width = w;
    c.height = hh;
    return c;
  });
  /** Bumped whenever the slide raster must be redrawn. */
  private version = 0;
  private scroll: HTMLElement;
  private spacer: HTMLElement;
  private notes: HTMLTextAreaElement;
  private status: HTMLElement;
  private overlay: HTMLElement | null = null;
  private editing: { id: number; cell?: [number, number]; root: HTMLElement; env: EditEnv; orig: TextBody | undefined } | null = null;
  private drag: Drag | null = null;
  private guides: Guide[] = [];
  private undoStack: string[] = [];
  private redoStack: string[] = [];
  private clip: SlideNode[] = [];
  private images = new Map<string, HTMLImageElement | null>();
  private thumbs: HTMLCanvasElement[] = [];
  private thumbDirty = new Set<number>();
  private pushTimer: ReturnType<typeof setTimeout> | null = null;
  private drawQueued = false;
  private resizeObs: ResizeObserver | null = null;
  private disposed = false;
  /** Animation timeline panel. */
  timelineOpen = false;
  private timeline: HTMLElement | null = null;
  /** Insert tool waiting for a drag on the stage. */
  private pendingInsert: ((x: Xfrm) => SlideNode) | null = null;
  /** Caret/selection inside the text overlay when focus moved to the ribbon. */
  private savedRange: Range | null = null;

  constructor(
    private host: HostBridge,
    private docId: number,
    private opts: SlideEditorOptions = {},
  ) {
    this.railList = h("div", { class: "slide-rail-list", role: "listbox", "aria-label": "Slides", tabindex: "0" });
    this.rail = h(
      "aside",
      { class: "slide-rail" },
      this.railList,
      h("button", { class: "slide-rail-add", title: "New slide (Ctrl+M)", onclick: () => this.execute("newSlide") }, "＋ New slide"),
    );
    this.canvas = h("canvas", { class: "slide-canvas", "aria-hidden": "true" });
    this.overlayCanvas = h("canvas", { class: "slide-canvas", "aria-hidden": "true" });
    this.spacer = h("div", { class: "slide-spacer" });
    this.scroll = h("div", { class: "slide-scroll", tabindex: "0", role: "application", "aria-label": "Slide editor" }, this.spacer);
    this.stage = h("div", { class: "slide-stage" }, this.canvas, this.overlayCanvas, this.scroll);
    this.notes = h("textarea", { class: "slide-notes", placeholder: "Click to add notes", "aria-label": "Speaker notes", spellcheck: "true" });
    this.status = h("div", { class: "slide-status", "aria-live": "polite" });
    this.el = h(
      "div",
      { class: "slide-editor" },
      this.rail,
      h("div", { class: "slide-main" }, this.stage, this.notes),
      this.status,
    );
    this.wire();
    const pref = (globalThis.localStorage?.getItem("prism.compositor") ?? "auto") as "auto" | "webgpu" | "webgl2" | "canvas2d";
    void createBackend(this.canvas, pref)
      .then((b) => {
        this.backend = b;
        this.el.dataset.compositor = b?.kind ?? "none";
        this.queueDraw();
      })
      .catch(() => {});
    this.ready = this.load();
  }

  /** Which compositor backend is active (diagnostics/tests). */
  get compositor(): string {
    return this.backend?.kind ?? "none";
  }

  /** A backend failed at runtime: fall back to the next one. */
  private downgradeCompositor(err: unknown) {
    const order = ["webgpu", "webgl2", "canvas2d"] as const;
    const failed = this.backend?.kind ?? "canvas2d";
    const next = order[order.indexOf(failed) + 1];
    console.warn(`Compositor ${failed} failed, falling back`, err);
    this.backend?.dispose();
    this.backend = null;
    const fresh = h("canvas", { class: "slide-canvas", "aria-hidden": "true" });
    this.canvas.replaceWith(fresh);
    this.canvas = fresh;
    if (!next) return this.queueDraw();
    void createBackend(fresh, next).then((b) => {
      this.backend = b;
      this.el.dataset.compositor = b?.kind ?? "none";
      this.version++;
      this.queueDraw();
    });
  }

  // ---- lifecycle ---------------------------------------------------------------------------

  private async load() {
    const [model] = await Promise.all([this.opts.model ? Promise.resolve(this.opts.model) : this.host.getPresentationModel(this.docId), loadDrawingml()]);
    this.model = model;
    this.current = 0;
    this.rebuildRail();
    this.syncNotes();
    this.queueDraw();
    this.opts.onState?.();
  }

  async flush(): Promise<void> {
    this.commitEdit();
    if (this.pushTimer) {
      clearTimeout(this.pushTimer);
      this.pushTimer = null;
    }
    if (this.model) await this.host.updatePresentationModel(this.docId, this.model);
  }

  focus() {
    if (this.editing) this.editing.root.focus();
    else this.scroll.focus({ preventScroll: true });
  }

  dispose() {
    this.disposed = true;
    this.resizeObs?.disconnect();
    if (this.pushTimer) clearTimeout(this.pushTimer);
  }

  get slide(): Slide | null {
    return this.model?.slides[this.current] ?? null;
  }

  // ---- change tracking -------------------------------------------------------------------------

  /** Snapshot for undo; call before mutating the model. */
  private begin() {
    if (!this.model) return;
    this.undoStack.push(JSON.stringify({ m: this.model, c: this.current }));
    if (this.undoStack.length > 100) this.undoStack.shift();
    this.redoStack = [];
  }

  private changed(allThumbs = false) {
    this.version++;
    this.opts.onDirty?.();
    if (allThumbs) this.model?.slides.forEach((_, i) => this.thumbDirty.add(i));
    else this.thumbDirty.add(this.current);
    if (this.pushTimer) clearTimeout(this.pushTimer);
    this.pushTimer = setTimeout(() => {
      this.pushTimer = null;
      if (this.model && !this.disposed) void this.host.updatePresentationModel(this.docId, this.model).catch((e) => this.opts.onError?.(String(e)));
    }, 500);
    this.queueDraw();
    this.opts.onState?.();
  }

  private restore(snap: string) {
    const { m, c } = JSON.parse(snap) as { m: PresentationModel; c: number };
    this.model = m;
    this.current = Math.min(c, m.slides.length - 1);
    this.selection.clear();
    this.rebuildRail();
    this.syncNotes();
    this.changed(true);
    this.renderTimeline();
  }

  undo() {
    this.commitEdit();
    const s = this.undoStack.pop();
    if (!s || !this.model) return;
    this.redoStack.push(JSON.stringify({ m: this.model, c: this.current }));
    this.restore(s);
  }

  redo() {
    const s = this.redoStack.pop();
    if (!s || !this.model) return;
    this.undoStack.push(JSON.stringify({ m: this.model, c: this.current }));
    this.restore(s);
  }

  // ---- geometry ----------------------------------------------------------------------------------

  /** Zoom (CSS px per point) and slide origin in stage CSS px. */
  view() {
    const m = this.model!;
    const vw = this.scroll.clientWidth || 800;
    const vh = this.scroll.clientHeight || 450;
    const W = pt(m.width);
    const H = pt(m.height);
    const fit = Math.max(0.05, Math.min((vw - PAD * 2) / W, (vh - PAD * 2) / H));
    const k = this.zoom === "fit" ? fit : this.zoom;
    const cw = Math.max(vw, W * k + PAD * 2);
    const ch = Math.max(vh, H * k + PAD * 2);
    return { k, fit, cw, ch, ox: (cw - W * k) / 2 - this.scroll.scrollLeft, oy: (ch - H * k) / 2 - this.scroll.scrollTop, vw, vh };
  }

  /** Client coordinates → slide EMU. */
  toEmu(clientX: number, clientY: number): [number, number] {
    const r = this.scroll.getBoundingClientRect();
    const v = this.view();
    return [((clientX - r.left - v.ox) / v.k) * EMU_PER_PT, ((clientY - r.top - v.oy) / v.k) * EMU_PER_PT];
  }

  /** Position of a node (inherited for placeholders). */
  xfrmOf(n: SlideNode): Xfrm | null {
    if (n.xfrm) return n.xfrm;
    const s = this.slide;
    if (!s || !this.model) return null;
    return resolveShape(n, slideContext(this.model, s), this.model).xfrm;
  }

  /** Point in a node's local (unrotated) box, EMU. */
  private local(x: Xfrm, px: number, py: number): [number, number] {
    const cx = x.x + x.cx / 2;
    const cy = x.y + x.cy / 2;
    const a = -(((x.rot ?? 0) / 60000) * Math.PI) / 180;
    const dx = px - cx;
    const dy = py - cy;
    return [dx * Math.cos(a) - dy * Math.sin(a) + x.cx / 2, dx * Math.sin(a) + dy * Math.cos(a) + x.cy / 2];
  }

  private hitCtx: CanvasRenderingContext2D | null = null;

  /** Topmost top-level node at a slide point. */
  hit(px: number, py: number): SlideNode | null {
    const s = this.slide;
    if (!s) return null;
    const tol = (6 / this.view().k) * EMU_PER_PT;
    for (let i = s.shapes.length - 1; i >= 0; i--) {
      const n = s.shapes[i]!;
      if (n.hidden) continue;
      const x = this.xfrmOf(n);
      if (!x) continue;
      const [lx, ly] = this.local(x, px, py);
      const thin = x.cx < tol * 2 || x.cy < tol * 2 || n.type === "connector";
      if (thin) {
        // lines: distance to the geometry outline
        const sb = shapeBody(n);
        const ev = evaluate(sb?.geom ?? { t: "preset", name: "rect" }, x.cx, x.cy);
        this.hitCtx ??= (document.createElement("canvas").getContext("2d") as CanvasRenderingContext2D | null) ?? null;
        const c = this.hitCtx;
        if (!c) continue;
        c.lineWidth = tol * 2;
        for (const p of ev.paths) {
          const path = new Path2D();
          for (const cmd of p.cmds) {
            if (cmd.c === "M") path.moveTo(cmd.x, cmd.y);
            else if (cmd.c === "L") path.lineTo(cmd.x, cmd.y);
            else if (cmd.c === "C") path.bezierCurveTo(cmd.x1, cmd.y1, cmd.x2, cmd.y2, cmd.x, cmd.y);
            else if (cmd.c === "Q") path.quadraticCurveTo(cmd.x1, cmd.y1, cmd.x, cmd.y);
            else path.closePath();
          }
          const fx = x.flipH ? x.cx - lx : lx;
          const fy = x.flipV ? x.cy - ly : ly;
          if (c.isPointInStroke(path, fx, fy) || (n.type !== "connector" && c.isPointInPath(path, fx, fy))) return n;
        }
        continue;
      }
      if (lx >= -tol / 2 && ly >= -tol / 2 && lx <= x.cx + tol / 2 && ly <= x.cy + tol / 2) return n;
    }
    return null;
  }

  /** Screen (stage CSS px) corners of a node's box: nw, ne, se, sw. */
  private corners(x: Xfrm): [number, number][] {
    const v = this.view();
    const a = (((x.rot ?? 0) / 60000) * Math.PI) / 180;
    const cx = x.x + x.cx / 2;
    const cy = x.y + x.cy / 2;
    const pts: [number, number][] = [
      [-x.cx / 2, -x.cy / 2],
      [x.cx / 2, -x.cy / 2],
      [x.cx / 2, x.cy / 2],
      [-x.cx / 2, x.cy / 2],
    ];
    return pts.map(([dx, dy]) => [v.ox + pt(cx + dx * Math.cos(a) - dy * Math.sin(a)) * v.k, v.oy + pt(cy + dx * Math.sin(a) + dy * Math.cos(a)) * v.k]);
  }

  /** Handle positions (stage px): 0 nw, 1 n, 2 ne, 3 e, 4 se, 5 s, 6 sw, 7 w, 8 rotate. */
  private handles(x: Xfrm): [number, number][] {
    const [nw, ne, se, sw] = this.corners(x);
    const mid = (p: [number, number], q: [number, number]): [number, number] => [(p[0] + q[0]) / 2, (p[1] + q[1]) / 2];
    const n = mid(nw!, ne!);
    const a = (((x.rot ?? 0) / 60000) * Math.PI) / 180;
    const rot: [number, number] = [n[0] + Math.sin(a) * ROT_OFFSET, n[1] - Math.cos(a) * ROT_OFFSET];
    return [nw!, n, ne!, mid(ne!, se!), se!, mid(se!, sw!), sw!, mid(sw!, nw!), rot];
  }

  private handleAt(clientX: number, clientY: number): number {
    if (this.selection.size !== 1) return -1;
    const n = this.selectedNodes()[0];
    const x = n ? this.xfrmOf(n) : null;
    if (!x) return -1;
    const r = this.scroll.getBoundingClientRect();
    const px = clientX - r.left;
    const py = clientY - r.top;
    const hs = this.handles(x);
    for (let i = hs.length - 1; i >= 0; i--) {
      const [hx, hy] = hs[i]!;
      if (Math.abs(px - hx) <= HANDLE && Math.abs(py - hy) <= HANDLE) return i;
    }
    return -1;
  }

  selectedNodes(): SlideNode[] {
    return this.slide?.shapes.filter((n) => this.selection.has(n.id)) ?? [];
  }

  // ---- drawing -----------------------------------------------------------------------------------

  queueDraw() {
    if (this.drawQueued || this.disposed) return;
    this.drawQueued = true;
    const run = () => {
      this.drawQueued = false;
      this.draw();
      this.updateThumbs();
    };
    if (typeof requestAnimationFrame !== "undefined") requestAnimationFrame(run);
    else setTimeout(run, 0);
  }

  private charts = new Map<string, SlideChart | null>();

  /** Parsed chart part (loaded from the host on first use). */
  chartSource(part: string): SlideChart | null {
    if (this.charts.has(part)) return this.charts.get(part)!;
    this.charts.set(part, null);
    void this.host
      .getSlidePart(this.docId, part)
      .then((d) => {
        if (!d) return;
        this.charts.set(part, parseChartXml(decodeXml(d.data)));
        this.model?.slides.forEach((s, i) => JSON.stringify(s.shapes).includes(part) && this.thumbDirty.add(i));
        this.version++;
        this.queueDraw();
      })
      .catch(() => {});
    return null;
  }

  private renderOpts(): RenderOptions {
    return {
      images: (p) => this.image(p),
      charts: (p) => this.chartSource(p),
      editing: true,
      liveAutofit: true,
      slideNumber: this.current + 1,
      skipText: this.editing ? new Set([this.editing.id]) : undefined,
    };
  }

  draw() {
    const m = this.model;
    const s = this.slide;
    if (!m) return;
    const v = this.view();
    this.spacer.style.width = `${v.cw}px`;
    this.spacer.style.height = `${v.ch}px`;
    const dpr = window.devicePixelRatio || 1;
    const vw = this.scroll.clientWidth;
    const vh = this.scroll.clientHeight;
    if (!vw || !vh) return;
    const W = pt(m.width) * v.k;
    const H = pt(m.height) * v.k;
    const overlay = this.overlayCanvas;
    if (overlay.width !== Math.round(vw * dpr) || overlay.height !== Math.round(vh * dpr)) {
      overlay.width = Math.round(vw * dpr);
      overlay.height = Math.round(vh * dpr);
    }
    for (const c of [this.canvas, overlay]) {
      c.style.width = `${vw}px`;
      c.style.height = `${vh}px`;
    }
    const octx = overlay.getContext("2d");
    if (!octx) return;
    octx.setTransform(dpr, 0, 0, dpr, 0, 0);
    octx.clearRect(0, 0, vw, vh);
    const paintSlide = (ctx: CanvasRenderingContext2D) => {
      if (!s) return;
      try {
        drawSlide(ctx, m, s, this.renderOpts());
      } catch (e) {
        this.opts.onError?.(String(e));
      }
    };
    let composited = false;
    if (this.backend) {
      if (this.canvas.width !== Math.round(vw * dpr) || this.canvas.height !== Math.round(vh * dpr)) this.backend.resize(vw, vh, dpr);
      // keep rasters within GPU texture limits at extreme zoom
      const scale = Math.min(v.k * dpr, 8192 / pt(m.width), 8192 / pt(m.height));
      const source = this.raster.get(this.current, this.version, scale, pt(m.width), pt(m.height), paintSlide);
      try {
        this.backend.draw([{ key: `s${this.current}`, version: this.version * 1000 + Math.round(scale * 100), source, x: v.ox, y: v.oy, w: W, h: H }], [0.906, 0.898, 0.894]);
        composited = true;
      } catch (err) {
        this.downgradeCompositor(err);
      }
    }
    if (composited) {
      // drop shadow outside the slide (the slide itself is on the GPU layer)
      octx.fillStyle = "rgba(0,0,0,.18)";
      octx.fillRect(v.ox + W, v.oy + 3, 2, H);
      octx.fillRect(v.ox + 2, v.oy + H, W, 3);
    } else {
      octx.fillStyle = "#e7e5e4";
      octx.fillRect(0, 0, vw, vh);
      octx.fillStyle = "rgba(0,0,0,.18)";
      octx.fillRect(v.ox + 2, v.oy + 3, W, H);
      octx.save();
      octx.setTransform(dpr * v.k, 0, 0, dpr * v.k, dpr * v.ox, dpr * v.oy);
      paintSlide(octx);
      octx.restore();
    }
    this.drawChrome(octx);
    this.status.textContent = `Slide ${this.current + 1} of ${m.slides.length}  ·  ${Math.round(v.k * 100)}%`;
  }

  /** Selection boxes, handles, guides, marquee (stage px). */
  private drawChrome(ctx: CanvasRenderingContext2D) {
    const v = this.view();
    ctx.save();
    ctx.lineWidth = 1;
    for (const n of this.selectedNodes()) {
      const x = this.xfrmOf(n);
      if (!x) continue;
      const cs = this.corners(x);
      ctx.strokeStyle = ACCENT;
      ctx.setLineDash(this.editing?.id === n.id ? [4, 3] : []);
      ctx.beginPath();
      cs.forEach(([px, py], i) => (i ? ctx.lineTo(px, py) : ctx.moveTo(px, py)));
      ctx.closePath();
      ctx.stroke();
    }
    ctx.setLineDash([]);
    if (this.selection.size === 1 && !this.editing) {
      const n = this.selectedNodes()[0];
      const x = n ? this.xfrmOf(n) : null;
      if (x) {
        const hs = this.handles(x);
        ctx.beginPath();
        ctx.moveTo(...(hs[1] as [number, number]));
        ctx.lineTo(...(hs[8] as [number, number]));
        ctx.strokeStyle = ACCENT;
        ctx.stroke();
        hs.forEach(([hx, hy], i) => {
          ctx.beginPath();
          if (i === 8) ctx.arc(hx, hy, 5, 0, Math.PI * 2);
          else ctx.rect(hx - 4, hy - 4, 8, 8);
          ctx.fillStyle = "#fff";
          ctx.fill();
          ctx.strokeStyle = ACCENT;
          ctx.stroke();
        });
      }
    }
    if (this.guides.length) {
      ctx.strokeStyle = "#e11d48";
      ctx.setLineDash([4, 3]);
      for (const g of this.guides) {
        ctx.beginPath();
        if (g.axis === "x") {
          const sx = v.ox + pt(g.at) * v.k;
          ctx.moveTo(sx, 0);
          ctx.lineTo(sx, v.vh);
        } else {
          const sy = v.oy + pt(g.at) * v.k;
          ctx.moveTo(0, sy);
          ctx.lineTo(v.vw, sy);
        }
        ctx.stroke();
      }
    }
    const d = this.drag;
    if (d && (d.kind === "marquee" || d.kind === "create")) {
      const x0 = v.ox + pt(Math.min(d.sx, d.x)) * v.k;
      const y0 = v.oy + pt(Math.min(d.sy, d.y)) * v.k;
      const w = (pt(Math.abs(d.x - d.sx)) * v.k) | 0;
      const hh = (pt(Math.abs(d.y - d.sy)) * v.k) | 0;
      ctx.setLineDash(d.kind === "marquee" ? [3, 3] : []);
      ctx.strokeStyle = ACCENT;
      ctx.fillStyle = "rgba(194,85,58,.08)";
      ctx.fillRect(x0, y0, w, hh);
      ctx.strokeRect(x0 + 0.5, y0 + 0.5, w, hh);
    }
    ctx.restore();
  }

  private image(part: string): HTMLImageElement | null {
    if (this.images.has(part)) {
      const img = this.images.get(part)!;
      return img && img.complete && img.naturalWidth ? img : null;
    }
    this.images.set(part, null);
    void this.host
      .getSlidePart(this.docId, part)
      .then((d) => {
        if (!d || typeof Image === "undefined") return;
        const img = new Image();
        img.onload = () => {
          this.model?.slides.forEach((s, i) => {
            if (JSON.stringify(s.shapes).includes(part) || JSON.stringify(s.background ?? {}).includes(part)) this.thumbDirty.add(i);
          });
          if (JSON.stringify(this.model?.masters ?? []).includes(part)) this.model?.slides.forEach((_, i) => this.thumbDirty.add(i));
          this.version++;
          this.queueDraw();
        };
        img.src = `data:${d.mime};base64,${d.data}`;
        this.images.set(part, img);
      })
      .catch(() => {});
    return null;
  }

  // ---- animation timeline ------------------------------------------------------------------------

  /** Side panel listing the slide's effects: order, trigger, duration, delay, remove. */
  renderTimeline() {
    if (!this.timelineOpen) {
      this.timeline?.remove();
      this.timeline = null;
      return;
    }
    if (!this.timeline) {
      this.timeline = h("aside", { class: "slide-timeline", "aria-label": "Animation timeline" });
      this.stage.append(this.timeline);
    }
    const s = this.slide;
    const effects = s?.timing?.effects ?? [];
    const nameOf = (id: number) => s?.shapes.find((n) => n.id === id)?.name ?? `#${id}`;
    const edit = (f: () => void) => {
      if (!s?.timing) return;
      this.begin();
      f();
      this.changed();
      this.renderTimeline();
    };
    let click = 0;
    const rows = effects.map((e, i) => {
      if (e.trigger === "onClick") click++;
      const icon = { entr: "↗", exit: "↘", emph: "✦", path: "∿" }[e.class] ?? "•";
      return h(
        "li",
        { class: `tl-row tl-${e.class}${this.selection.has(e.shape) ? " sel" : ""}` },
        h("span", { class: "tl-num" }, e.trigger === "onClick" ? String(click) : ""),
        h("span", { class: "tl-icon", title: e.class }, icon),
        h("span", { class: "tl-name", title: `${e.preset} — ${nameOf(e.shape)}`, onclick: () => (this.selection.clear(), this.selection.add(e.shape), this.queueDraw(), this.renderTimeline()) }, `${e.preset} · ${nameOf(e.shape)}`),
        h(
          "select",
          { class: "tl-trigger", "aria-label": "Start", onchange: (ev: Event) => edit(() => (e.trigger = (ev.target as HTMLSelectElement).value)) },
          ...[
            ["onClick", "On click"],
            ["withPrev", "With previous"],
            ["afterPrev", "After previous"],
          ].map(([v, l]) => h("option", { value: v!, selected: e.trigger === v }, l!)),
        ),
        h("input", { class: "tl-num-input tl-dur", type: "number", min: "0", step: "0.1", value: String(e.duration / 1000), title: "Duration (s)", "aria-label": "Duration in seconds", onchange: (ev: Event) => edit(() => (e.duration = Math.max(1, Math.round(Number((ev.target as HTMLInputElement).value) * 1000)))) }),
        h("input", { class: "tl-num-input tl-delay", type: "number", min: "0", step: "0.1", value: String((e.delay ?? 0) / 1000), title: "Delay (s)", "aria-label": "Delay in seconds", onchange: (ev: Event) => edit(() => (e.delay = Math.max(0, Math.round(Number((ev.target as HTMLInputElement).value) * 1000)))) }),
        h("button", { class: "tl-btn", title: "Move up", disabled: i === 0, onclick: () => edit(() => effects.splice(i - 1, 0, ...effects.splice(i, 1))) }, "↑"),
        h("button", { class: "tl-btn", title: "Move down", disabled: i === effects.length - 1, onclick: () => edit(() => effects.splice(i + 1, 0, ...effects.splice(i, 1))) }, "↓"),
        h("button", { class: "tl-btn", title: "Remove", onclick: () => edit(() => {
          effects.splice(i, 1);
          if (!effects.length) delete s!.timing;
        }) }, "✕"),
      );
    });
    const tr = s?.transition;
    this.timeline.replaceChildren(
      h("div", { class: "tl-head" }, h("strong", {}, "Animations"), h("button", { class: "tl-btn", title: "Close", onclick: () => this.execute("timeline") }, "✕")),
      effects.length ? h("ol", { class: "tl-list" }, ...rows) : h("p", { class: "tl-empty" }, "Select a shape and choose Add Animation."),
      h(
        "div",
        { class: "tl-trans" },
        h("strong", {}, "Transition: "),
        tr ? `${tr.kind}${tr.dir ? ` (${tr.dir})` : ""}` : "none",
        tr
          ? h("label", {}, " · ", h("input", { class: "tl-num-input", type: "number", min: "0", step: "0.1", value: String((tr.duration ?? 700) / 1000), "aria-label": "Transition duration in seconds", onchange: (ev: Event) => this.execute("transitionDuration", (ev.target as HTMLInputElement).value) }), " s")
          : "",
        h("label", {}, " · auto after ", h("input", { class: "tl-num-input", type: "number", min: "0", step: "1", value: tr?.advanceAfter !== undefined ? String(tr.advanceAfter / 1000) : "", placeholder: "—", "aria-label": "Advance after seconds", onchange: (ev: Event) => this.execute("advanceAfter", (ev.target as HTMLInputElement).value) }), " s"),
      ),
      h("button", { class: "tl-play", onclick: () => this.execute("show", "current") }, "▶ Preview from this slide"),
    );
  }

  /** Picture element for a part (slideshow shares the editor's image cache). */
  imageSource(part: string): CanvasImageSource | null {
    return this.image(part);
  }

  // ---- rail --------------------------------------------------------------------------------------

  private rebuildRail() {
    const m = this.model;
    if (!m) return;
    this.thumbs = m.slides.map(() => h("canvas", { class: "slide-thumb-canvas" }));
    this.railList.replaceChildren(
      ...m.slides.map((s, i) =>
        h(
          "div",
          {
            class: `slide-thumb${i === this.current ? " active" : ""}${s.hidden ? " hidden-slide" : ""}`,
            role: "option",
            "aria-selected": String(i === this.current),
            draggable: "true",
            dataset: { index: String(i) },
            onclick: () => this.goTo(i),
            ondblclick: () => this.opts.onShow?.(i),
            ondragstart: (e: DragEvent) => {
              e.dataTransfer?.setData("text/x-prism-slide", String(i));
              if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
            },
            ondragover: (e: DragEvent) => {
              if (e.dataTransfer?.types.includes("text/x-prism-slide")) {
                e.preventDefault();
                (e.currentTarget as HTMLElement).classList.add("drop");
              }
            },
            ondragleave: (e: DragEvent) => (e.currentTarget as HTMLElement).classList.remove("drop"),
            ondrop: (e: DragEvent) => {
              e.preventDefault();
              (e.currentTarget as HTMLElement).classList.remove("drop");
              const from = Number(e.dataTransfer?.getData("text/x-prism-slide"));
              if (Number.isFinite(from)) this.moveSlide(from, i);
            },
          },
          h("span", { class: "slide-thumb-num" }, String(i + 1), s.transition || s.timing?.effects.length ? h("span", { class: "slide-thumb-fx", title: "Transition or animations" }, "★") : ""),
          this.thumbs[i]!,
        ),
      ),
    );
    m.slides.forEach((_, i) => this.thumbDirty.add(i));
    this.queueDraw();
  }

  private updateThumbs() {
    const m = this.model;
    if (!m) return;
    for (const i of this.thumbDirty) {
      const c = this.thumbs[i];
      const s = m.slides[i];
      if (!c || !s) continue;
      try {
        renderSlideToCanvas(c, m, s, THUMB_W, Math.min(window.devicePixelRatio || 1, 2), { images: (p) => this.image(p), charts: (p) => this.chartSource(p), slideNumber: i + 1 });
      } catch {
        /* a broken shape must not break the rail */
      }
      c.style.width = `${THUMB_W}px`;
    }
    this.thumbDirty.clear();
    [...this.railList.children].forEach((el, i) => {
      el.classList.toggle("active", i === this.current);
      el.setAttribute("aria-selected", String(i === this.current));
      el.classList.toggle("hidden-slide", !!m.slides[i]?.hidden);
    });
  }

  goTo(i: number) {
    const m = this.model;
    if (!m || !m.slides.length) return;
    this.commitEdit();
    this.current = Math.max(0, Math.min(i, m.slides.length - 1));
    this.version++;
    this.selection.clear();
    this.syncNotes();
    this.queueDraw();
    this.updateThumbs();
    this.railList.children[this.current]?.scrollIntoView?.({ block: "nearest" });
    this.renderTimeline();
    this.opts.onState?.();
  }

  private moveSlide(from: number, to: number) {
    const m = this.model;
    if (!m || from === to || from < 0 || from >= m.slides.length) return;
    this.begin();
    const [s] = m.slides.splice(from, 1);
    m.slides.splice(to, 0, s!);
    this.current = to;
    this.rebuildRail();
    this.changed(true);
  }

  private syncNotes() {
    this.notes.value = this.slide?.notes ?? "";
  }

  // ---- events ------------------------------------------------------------------------------------

  private wire() {
    if (typeof ResizeObserver !== "undefined") {
      this.resizeObs = new ResizeObserver(() => this.queueDraw());
      this.resizeObs.observe(this.stage);
    }
    this.scroll.addEventListener("scroll", () => {
      this.positionOverlay();
      this.queueDraw();
    });
    this.scroll.addEventListener("pointerdown", (e) => this.onDown(e));
    this.scroll.addEventListener("pointermove", (e) => this.onMove(e));
    this.scroll.addEventListener("pointerup", (e) => this.onUp(e));
    this.scroll.addEventListener("dblclick", (e) => this.onDouble(e));
    this.scroll.addEventListener("keydown", (e) => this.onKey(e));
    this.scroll.addEventListener(
      "wheel",
      (e) => {
        if (!e.ctrlKey && !e.metaKey) return;
        e.preventDefault();
        this.setZoom(this.view().k * (e.deltaY < 0 ? 1.1 : 1 / 1.1));
      },
      { passive: false },
    );
    this.el.addEventListener("paste", (e) => this.onPaste(e as ClipboardEvent));
    this.el.addEventListener("copy", (e) => {
      if (this.editing || !this.selection.size) return;
      this.copy();
      (e as ClipboardEvent).clipboardData?.setData("text/plain", this.selectedNodes().map((n) => this.nodeText(n)).join("\n"));
      e.preventDefault();
    });
    this.notes.addEventListener("focus", () => this.begin());
    this.notes.addEventListener("input", () => {
      const s = this.slide;
      if (!s) return;
      s.notes = this.notes.value;
      this.changed();
    });
    this.railList.addEventListener("keydown", (e) => {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        this.goTo(this.current + (e.key === "ArrowDown" ? 1 : -1));
      } else if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        this.execute("deleteSlide");
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "d") {
        e.preventDefault();
        this.execute("duplicateSlide");
      }
    });
  }

  private nodeText(n: SlideNode): string {
    const b = shapeBody(n);
    return (b?.text?.paragraphs ?? []).map((p) => (p.runs ?? []).map((r) => (r.t === "break" ? "\n" : r.text)).join("")).join("\n");
  }

  private onDown(e: PointerEvent) {
    if (!this.model || !this.slide || e.button !== 0) return;
    if (this.editing) this.commitEdit();
    this.scroll.focus({ preventScroll: true });
    const [px, py] = this.toEmu(e.clientX, e.clientY);
    if (this.pendingInsert) {
      this.drag = { kind: "create", sx: px, sy: py, x: px, y: py, make: this.pendingInsert };
      this.pendingInsert = null;
      this.scroll.style.cursor = "";
      this.scroll.setPointerCapture?.(e.pointerId);
      return;
    }
    const hnd = this.handleAt(e.clientX, e.clientY);
    if (hnd >= 0) {
      const n = this.selectedNodes()[0]!;
      this.begin();
      const x = materialize(this.model, this.slide, n);
      if (!x) return;
      if (hnd === 8) this.drag = { kind: "rotate", id: n.id, orig: { ...x }, cx: x.x + x.cx / 2, cy: x.y + x.cy / 2 };
      else this.drag = { kind: "resize", handle: hnd, id: n.id, orig: { ...x }, sx: px, sy: py, aspect: n.type === "picture" };
      this.scroll.setPointerCapture?.(e.pointerId);
      return;
    }
    const n = this.hit(px, py);
    if (n) {
      if (e.shiftKey || e.ctrlKey || e.metaKey) {
        if (this.selection.has(n.id)) this.selection.delete(n.id);
        else this.selection.add(n.id);
      } else if (!this.selection.has(n.id)) {
        this.selection.clear();
        this.selection.add(n.id);
      }
      const orig = new Map<number, Xfrm>();
      for (const s of this.selectedNodes()) {
        const x = this.xfrmOf(s);
        if (x) orig.set(s.id, { ...x });
      }
      this.drag = { kind: "move", sx: px, sy: py, orig, moved: false };
    } else {
      if (!e.shiftKey) this.selection.clear();
      this.drag = { kind: "marquee", sx: px, sy: py, x: px, y: py };
    }
    this.scroll.setPointerCapture?.(e.pointerId);
    this.queueDraw();
    this.opts.onState?.();
  }

  private onMove(e: PointerEvent) {
    const d = this.drag;
    const m = this.model;
    const s = this.slide;
    if (!d || !m || !s) {
      const hnd = this.handleAt(e.clientX, e.clientY);
      const cursors = ["nwse-resize", "ns-resize", "nesw-resize", "ew-resize", "nwse-resize", "ns-resize", "nesw-resize", "ew-resize", "grab"];
      if (this.pendingInsert) this.scroll.style.cursor = "crosshair";
      else if (hnd >= 0) this.scroll.style.cursor = cursors[hnd]!;
      else {
        const [px, py] = this.toEmu(e.clientX, e.clientY);
        this.scroll.style.cursor = this.hit(px, py) ? "move" : "default";
      }
      return;
    }
    const [px, py] = this.toEmu(e.clientX, e.clientY);
    const snapTol = (6 / this.view().k) * EMU_PER_PT;
    if (d.kind === "move") {
      let dx = px - d.sx;
      let dy = py - d.sy;
      if (!d.moved && Math.hypot(dx, dy) < snapTol / 2) return;
      if (!d.moved) {
        this.begin();
        for (const n of this.selectedNodes()) materialize(m, s, n);
        d.moved = true;
      }
      if (e.shiftKey) {
        if (Math.abs(dx) > Math.abs(dy)) dy = 0;
        else dx = 0;
      }
      // smart guides: snap the moving bounds to slide and other shapes
      this.guides = [];
      if (!e.altKey) {
        let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
        for (const x of d.orig.values()) {
          const b = bounds(x);
          x0 = Math.min(x0, b.x);
          y0 = Math.min(y0, b.y);
          x1 = Math.max(x1, b.x + b.w);
          y1 = Math.max(y1, b.y + b.h);
        }
        const xs = [0, m.width / 2, m.width];
        const ys = [0, m.height / 2, m.height];
        for (const n of s.shapes) {
          if (this.selection.has(n.id) || n.hidden) continue;
          const x = this.xfrmOf(n);
          if (!x) continue;
          const b = bounds(x);
          xs.push(b.x, b.x + b.w / 2, b.x + b.w);
          ys.push(b.y, b.y + b.h / 2, b.y + b.h);
        }
        const snap = (edges: number[], cands: number[], axis: "x" | "y", delta: number) => {
          let best: [number, number] | null = null;
          for (const e0 of edges) for (const c of cands) {
            const diff = c - (e0 + delta);
            if (Math.abs(diff) <= snapTol && (!best || Math.abs(diff) < Math.abs(best[0]))) best = [diff, c];
          }
          if (!best) return delta;
          const at = best[1];
          this.guides.push({ axis, at });
          return delta + best[0];
        };
        dx = snap([x0, (x0 + x1) / 2, x1], xs, "x", dx);
        dy = snap([y0, (y0 + y1) / 2, y1], ys, "y", dy);
      }
      for (const n of this.selectedNodes()) {
        const o = d.orig.get(n.id);
        if (!o || !n.xfrm) continue;
        n.xfrm.x = Math.round(o.x + dx);
        n.xfrm.y = Math.round(o.y + dy);
      }
      this.version++;
      this.queueDraw();
    } else if (d.kind === "resize") {
      const n = s.shapes.find((x) => x.id === d.id);
      if (!n?.xfrm) return;
      n.xfrm = { ...n.xfrm, ...resizeXfrm(d.orig, d.handle, px - d.sx, py - d.sy, d.aspect !== e.shiftKey && d.handle % 2 === 0) };
      this.version++;
      this.queueDraw();
    } else if (d.kind === "rotate") {
      const n = s.shapes.find((x) => x.id === d.id);
      if (!n?.xfrm) return;
      let deg = (Math.atan2(py - d.cy, px - d.cx) * 180) / Math.PI + 90;
      if (e.shiftKey) deg = Math.round(deg / 15) * 15;
      else {
        const r = Math.round(deg / 90) * 90;
        if (Math.abs(r - deg) < 3) deg = r;
      }
      n.xfrm.rot = Math.round((((deg % 360) + 360) % 360) * 60000);
      this.version++;
      this.queueDraw();
    } else {
      d.x = px;
      d.y = py;
      if (d.kind === "marquee") {
        const [x0, x1] = [Math.min(d.sx, px), Math.max(d.sx, px)];
        const [y0, y1] = [Math.min(d.sy, py), Math.max(d.sy, py)];
        if (!e.shiftKey) this.selection.clear();
        for (const n of s.shapes) {
          const x = this.xfrmOf(n);
          if (!x || n.hidden) continue;
          const b = bounds(x);
          if (b.x >= x0 && b.y >= y0 && b.x + b.w <= x1 && b.y + b.h <= y1) this.selection.add(n.id);
        }
      }
      this.queueDraw();
    }
  }

  private onUp(e: PointerEvent) {
    const d = this.drag;
    this.drag = null;
    this.guides = [];
    this.scroll.releasePointerCapture?.(e.pointerId);
    if (!d) return;
    if (d.kind === "create" && this.slide) {
      let w = Math.abs(d.x - d.sx);
      let hh = Math.abs(d.y - d.sy);
      const click = w < EMU_PER_PT * 4 && hh < EMU_PER_PT * 4;
      if (click) {
        w = 1828800;
        hh = 1371600;
      }
      const x0 = click ? d.sx - w / 2 : Math.min(d.sx, d.x);
      const y0 = click ? d.sy - hh / 2 : Math.min(d.sy, d.y);
      this.insertNode(d.make({ x: Math.round(x0), y: Math.round(y0), cx: Math.round(w), cy: Math.round(hh) }));
      return;
    }
    if ((d.kind === "move" && d.moved) || d.kind === "resize" || d.kind === "rotate") this.changed();
    else {
      this.queueDraw();
      this.opts.onState?.();
    }
  }

  private onDouble(e: MouseEvent) {
    const [px, py] = this.toEmu(e.clientX, e.clientY);
    const n = this.hit(px, py);
    if (!n) return;
    if (n.type === "picture" || (n.ph?.kind === "pic" && !shapeBody(n)?.text)) {
      if (n.ph?.kind === "pic" || n.type === "picture") void this.replacePicture(n);
      return;
    }
    if (n.type === "frame" && n.kind === "table" && n.table && n.xfrm) {
      const [lx, ly] = this.local(n.xfrm, px, py);
      let r = 0;
      let acc = 0;
      for (; r < n.table.rows.length - 1; r++) {
        acc += n.table.rows[r]![0];
        if (ly < acc) break;
      }
      let c = 0;
      acc = 0;
      for (; c < n.table.cols.length - 1; c++) {
        acc += n.table.cols[c]!;
        if (lx < acc) break;
      }
      this.selection.clear();
      this.selection.add(n.id);
      this.startEdit(n.id, { cell: [r, c] });
      return;
    }
    if (shapeBody(n)) {
      this.selection.clear();
      this.selection.add(n.id);
      this.startEdit(n.id, { caretAt: [e.clientX, e.clientY] });
    }
  }

  private onKey(e: KeyboardEvent) {
    if (!this.model || this.editing) return;
    const mod = e.ctrlKey || e.metaKey;
    const k = e.key;
    const nudge = (dx: number, dy: number) => {
      if (!this.selection.size || !this.slide) return false;
      this.begin();
      for (const n of this.selectedNodes()) {
        const x = materialize(this.model!, this.slide, n);
        if (!x) continue;
        x.x += dx;
        x.y += dy;
      }
      this.changed();
      return true;
    };
    const step = (e.shiftKey ? 10 : 1) * EMU_PER_PT * (e.altKey ? 0.1 : 1);
    let handled = true;
    if (mod && k.toLowerCase() === "z") e.shiftKey ? this.redo() : this.undo();
    else if (mod && k.toLowerCase() === "y") this.redo();
    else if (mod && k.toLowerCase() === "a") this.execute("selectAll");
    else if (mod && k.toLowerCase() === "c") this.copy();
    else if (mod && k.toLowerCase() === "x") this.execute("cut");
    else if (mod && k.toLowerCase() === "v") handled = false; // paste event
    else if (mod && k.toLowerCase() === "d") this.execute("duplicate");
    else if (mod && k.toLowerCase() === "g") this.execute(e.shiftKey ? "ungroup" : "group");
    else if (mod && k.toLowerCase() === "m") this.execute("newSlide");
    else if (mod && k.toLowerCase() === "b") this.execute("bold");
    else if (mod && k.toLowerCase() === "i") this.execute("italic");
    else if (mod && k.toLowerCase() === "u") this.execute("underline");
    else if (mod && k === "]") this.execute(e.shiftKey ? "bringToFront" : "bringForward");
    else if (mod && k === "[") this.execute(e.shiftKey ? "sendToBack" : "sendBackward");
    else if (k === "Delete" || k === "Backspace") this.selection.size ? this.execute("delete") : this.execute("deleteSlide");
    else if (k === "ArrowLeft") handled = nudge(-step, 0) || (this.goTo(this.current - 1), true);
    else if (k === "ArrowRight") handled = nudge(step, 0) || (this.goTo(this.current + 1), true);
    else if (k === "ArrowUp") handled = nudge(0, -step) || (this.goTo(this.current - 1), true);
    else if (k === "ArrowDown") handled = nudge(0, step) || (this.goTo(this.current + 1), true);
    else if (k === "PageUp") this.goTo(this.current - 1);
    else if (k === "PageDown") this.goTo(this.current + 1);
    else if (k === "Home") this.goTo(0);
    else if (k === "End") this.goTo(this.model.slides.length - 1);
    else if (k === "Escape") {
      this.pendingInsert = null;
      this.selection.clear();
      this.queueDraw();
      this.opts.onState?.();
    } else if (k === "F5") this.opts.onShow?.(e.shiftKey ? this.current : 0);
    else if ((k === "Enter" || k === "F2") && this.selection.size === 1) {
      const n = this.selectedNodes()[0]!;
      if (shapeBody(n)) this.startEdit(n.id, { selectAll: k === "F2" });
    } else if (k === "Tab" && this.slide?.shapes.length) {
      const ids = this.slide.shapes.map((s) => s.id);
      const cur = ids.indexOf([...this.selection][0] ?? -1);
      const next = ids[(cur + (e.shiftKey ? -1 : 1) + ids.length) % ids.length]!;
      this.selection.clear();
      this.selection.add(next);
      this.queueDraw();
      this.opts.onState?.();
    } else if (k.length === 1 && !mod && this.selection.size === 1) {
      // typing over a selected text shape replaces its text (PowerPoint behaviour)
      const n = this.selectedNodes()[0]!;
      if (shapeBody(n)) {
        this.startEdit(n.id, { replaceWith: k });
      } else handled = false;
    } else handled = false;
    if (handled) e.preventDefault();
  }

  private onPaste(e: ClipboardEvent) {
    if (this.editing || !this.slide || !this.model) return;
    const dt = e.clipboardData;
    const file = dt ? [...dt.files].find((f) => f.type.startsWith("image/")) : undefined;
    e.preventDefault();
    if (file) {
      const r = new FileReader();
      r.onload = () => {
        const b64 = String(r.result).split(",")[1] ?? "";
        void this.insertImageData(b64);
      };
      r.readAsDataURL(file);
      return;
    }
    const text = dt?.getData("text/plain") ?? "";
    if (this.clip.length && (!text || text === this.clip.map((n) => this.nodeText(n)).join("\n"))) {
      this.paste();
      return;
    }
    if (text) {
      const m = this.model;
      const box = newTextBox(this.slide, { x: Math.round(m.width * 0.1), y: Math.round(m.height * 0.4), cx: Math.round(m.width * 0.8), cy: 457200 });
      const b = shapeBody(box)!;
      b.text = { body: b.text?.body, paragraphs: text.split(/\r?\n/).map((l) => ({ runs: l ? [{ t: "text", text: l }] : [] })) };
      this.insertNode(box);
    }
  }

  // ---- text editing ------------------------------------------------------------------------------

  private textOf(n: SlideNode, cell?: [number, number]): TextBody | undefined {
    if (cell && n.type === "frame") return n.table?.rows[cell[0]]?.[1][cell[1]]?.text;
    return shapeBody(n)?.text;
  }

  private setTextOf(n: SlideNode, t: TextBody, cell?: [number, number]) {
    if (cell && n.type === "frame") {
      const c = n.table?.rows[cell[0]]?.[1][cell[1]];
      if (c) c.text = t;
      return;
    }
    const b = shapeBody(n);
    if (b) b.text = t;
  }

  startEdit(id: number, o: { selectAll?: boolean; replaceWith?: string; caretAt?: [number, number]; cell?: [number, number] } = {}) {
    const m = this.model;
    const s = this.slide;
    const n = s?.shapes.find((x) => x.id === id);
    if (!m || !s || !n) return;
    this.commitEdit();
    this.begin();
    const x = this.xfrmOf(n);
    if (!x) return;
    const sc = slideContext(m, s);
    const palette = paletteFor(sc);
    let res = resolveShape(n, sc, m);
    const sb = shapeBody(n);
    const color = sb?.style?.font?.[1] ? palette.css(sb.style.font[1]) : undefined;
    let fontScale = 1;
    if (res.body.autofit?.t === "normal") {
      fontScale = (res.body.autofit.fontScale ?? 100000) / 100000;
      const tenv: TextEnv = { palette, theme: sc.master.theme, color };
      fontScale *= fitScale(sb?.text, res, pt(x.cx), pt(x.cy), tenv);
    }
    if (o.cell) res = { ...res, body: { anchor: "t" }, lists: [m.defaultTextStyle ?? {}] };
    const env: EditEnv = { palette, theme: sc.master.theme, scale: this.view().k, color, fontScale };
    const root = h("div", { class: "slide-text-edit", contenteditable: "true", spellcheck: "true", role: "textbox", "aria-multiline": "true" });
    const orig = this.textOf(n, o.cell);
    const text = o.replaceWith !== undefined ? { ...(orig ?? {}), paragraphs: [{ ...(orig?.paragraphs?.[0] ?? {}), runs: [{ t: "text" as const, text: o.replaceWith, props: firstRunProps(orig) }] }] } : orig;
    renderEditable(root, text, res, env);
    const box = h("div", { class: "slide-text-box" }, root);
    this.overlay = box;
    this.editing = { id, cell: o.cell, root, env, orig };
    this.stage.append(box);
    this.positionOverlay();
    root.addEventListener("input", () => this.syncEdit());
    root.addEventListener("keydown", (e) => {
      const mod = e.ctrlKey || e.metaKey;
      if (e.key === "Escape") {
        e.preventDefault();
        this.commitEdit();
        this.scroll.focus();
      } else if (mod && ["b", "i", "u"].includes(e.key.toLowerCase())) {
        e.preventDefault();
        this.execute({ b: "bold", i: "italic", u: "underline" }[e.key.toLowerCase()]!);
      } else if (e.key === "Tab" && !o.cell) {
        e.preventDefault();
        this.execute(e.shiftKey ? "outdent" : "indent");
      }
      e.stopPropagation();
    });
    root.addEventListener("blur", () => {
      const sel = document.getSelection();
      if (sel?.rangeCount && root.contains(sel.anchorNode)) this.savedRange = sel.getRangeAt(0).cloneRange();
    });
    root.addEventListener("keyup", () => this.opts.onState?.());
    root.addEventListener("mouseup", () => this.opts.onState?.());
    root.addEventListener("paste", (e) => {
      // plain text only: keep the model's formatting
      e.preventDefault();
      const t = e.clipboardData?.getData("text/plain") ?? "";
      document.execCommand("insertText", false, t);
    });
    root.focus();
    const sel = document.getSelection();
    if (sel) {
      const r = document.createRange();
      if (o.selectAll) r.selectNodeContents(root);
      else if (o.caretAt && (document as Document & { caretRangeFromPoint?: (x: number, y: number) => Range | null }).caretRangeFromPoint) {
        const cr = (document as Document & { caretRangeFromPoint: (x: number, y: number) => Range | null }).caretRangeFromPoint(o.caretAt[0], o.caretAt[1]);
        if (cr && root.contains(cr.startContainer)) r.setStart(cr.startContainer, cr.startOffset);
        else (r.selectNodeContents(root), r.collapse(false));
      } else {
        r.selectNodeContents(root);
        r.collapse(false);
      }
      sel.removeAllRanges();
      sel.addRange(r);
    }
    if (o.replaceWith !== undefined) this.syncEdit();
    this.version++;
    this.queueDraw();
    this.opts.onState?.();
  }

  private positionOverlay() {
    const ed = this.editing;
    const box = this.overlay;
    const n = ed ? this.slide?.shapes.find((x) => x.id === ed.id) : null;
    if (!ed || !box || !n || !this.model) return;
    const x = this.xfrmOf(n);
    if (!x) return;
    const v = this.view();
    let rx = 0, ry = 0, rw = x.cx, rh = x.cy;
    if (ed.cell && n.type === "frame" && n.table) {
      const t = n.table;
      rx = t.cols.slice(0, ed.cell[1]).reduce((a, b) => a + b, 0);
      ry = t.rows.slice(0, ed.cell[0]).reduce((a, r) => a + r[0], 0);
      const cell = t.rows[ed.cell[0]]?.[1][ed.cell[1]];
      rw = t.cols.slice(ed.cell[1], ed.cell[1] + (cell?.gridSpan ?? 1)).reduce((a, b) => a + b, 0);
      rh = t.rows[ed.cell[0]]?.[0] ?? rh;
    } else {
      const sb = shapeBody(n);
      const ev = evaluate(sb?.geom ?? { t: "preset", name: "rect" }, x.cx, x.cy);
      [rx, ry, rw, rh] = ev.textRect;
    }
    const sc = slideContext(this.model, this.slide!);
    const res = ed.cell ? null : resolveShape(n, sc, this.model);
    const ins = res?.body.insets ?? [null, null, null, null];
    const [l, t, r, b] = [ins[0] ?? 91440, ins[1] ?? 45720, ins[2] ?? 91440, ins[3] ?? 45720].map((e) => pt(e) * v.k);
    const anchor = ed.cell ? (shapeBody(n) ? "t" : (this.textOf(n, ed.cell)?.body?.anchor ?? "t")) : (res?.body.anchor ?? "t");
    const W = pt(x.cx) * v.k;
    const H = pt(x.cy) * v.k;
    Object.assign(box.style, {
      left: `${v.ox + pt(x.x) * v.k}px`,
      top: `${v.oy + pt(x.y) * v.k}px`,
      width: `${W}px`,
      height: `${H}px`,
      transform: `rotate(${(x.rot ?? 0) / 60000}deg)`,
    });
    Object.assign(ed.root.style, {
      left: `${pt(rx) * v.k}px`,
      top: `${pt(ry) * v.k}px`,
      width: `${pt(rw) * v.k}px`,
      minHeight: `${pt(rh) * v.k}px`,
      padding: `${t}px ${r}px ${b}px ${l}px`,
      justifyContent: anchor === "ctr" ? "center" : anchor === "b" ? "flex-end" : "flex-start",
    });
    if (ed.env.scale !== v.k) {
      // zoom changed: re-render at the new scale, keeping the model text
      const text = readEditable(ed.root, ed.orig, ed.env);
      ed.env = { ...ed.env, scale: v.k };
      const res2 = res ?? { xfrm: null, geom: null, fill: null, line: null, body: {}, lists: [this.model.defaultTextStyle ?? {}], styleKind: "other" as const };
      renderEditable(ed.root, text, res2, ed.env);
    }
  }

  private syncEdit() {
    const ed = this.editing;
    const n = ed ? this.slide?.shapes.find((x) => x.id === ed.id) : null;
    if (!ed || !n) return;
    this.setTextOf(n, readEditable(ed.root, ed.orig, ed.env), ed.cell);
    this.changed();
  }

  /** Leave text editing (keeps the edit; undo reverts the whole session). */
  commitEdit() {
    const ed = this.editing;
    if (!ed) return;
    this.syncEdit();
    this.editing = null;
    this.savedRange = null;
    this.overlay?.remove();
    this.overlay = null;
    // an untouched session leaves no undo step
    const n = this.slide?.shapes.find((x) => x.id === ed.id);
    if (n && JSON.stringify(this.textOf(n, ed.cell) ?? null) === JSON.stringify(ed.orig ?? null)) this.undoStack.pop();
    this.version++;
    this.queueDraw();
    this.opts.onState?.();
  }

  get isEditingText() {
    return !!this.editing;
  }

  // ---- inserting -----------------------------------------------------------------------------------

  private insertNode(n: SlideNode) {
    const s = this.slide;
    if (!s) return;
    this.begin();
    s.shapes.push(n);
    this.selection.clear();
    this.selection.add(n.id);
    this.changed();
    if (shapeBody(n)?.textBox) this.startEdit(n.id);
  }

  async insertImageData(b64: string) {
    const m = this.model;
    const s = this.slide;
    if (!m || !s) return;
    const part = await this.host.addSlideImage(this.docId, b64);
    const { w, h: ih } = await imageSize(b64);
    let cx = w * 9525;
    let cy = ih * 9525;
    const maxW = m.width * 0.8;
    const maxH = m.height * 0.8;
    const f = Math.min(1, maxW / cx, maxH / cy);
    cx = Math.round(cx * f);
    cy = Math.round(cy * f);
    // an empty picture placeholder takes the picture
    const ph = this.selectedNodes().find((n) => n.ph && (n.ph.kind === "pic" || n.ph.kind === "obj" || n.ph.kind === undefined) && !this.nodeText(n));
    if (ph) {
      const x = materialize(m, s, ph);
      if (x) {
        this.begin();
        const pic = { ...newPicture(s, part, x), id: ph.id, name: ph.name, ph: ph.ph };
        s.shapes[s.shapes.indexOf(ph)] = pic as SlideNode;
        this.changed();
        return;
      }
    }
    this.insertNode(newPicture(s, part, { x: Math.round((m.width - cx) / 2), y: Math.round((m.height - cy) / 2), cx, cy }));
  }

  private async replacePicture(n: SlideNode) {
    const b64 = await this.host.pickImage();
    if (!b64 || !this.slide || !this.model) return;
    if (n.type === "picture") {
      const part = await this.host.addSlideImage(this.docId, b64);
      this.begin();
      n.image = { t: "image", relId: part };
      this.changed();
    } else {
      this.selection.clear();
      this.selection.add(n.id);
      await this.insertImageData(b64);
    }
  }

  // ---- clipboard -----------------------------------------------------------------------------------

  copy() {
    const s = this.slide;
    if (!s || !this.model) return;
    this.clip = structuredClone(this.selectedNodes());
    // placeholders become free shapes on paste, at their current position
    for (const n of this.clip) {
      const orig = s.shapes.find((x) => x.id === n.id);
      if (n.ph && orig) {
        n.xfrm = this.xfrmOf(orig) ?? undefined;
      }
    }
  }

  paste() {
    const s = this.slide;
    if (!s || !this.clip.length) return;
    this.begin();
    const fresh = reid(this.clip, s);
    const same = this.clip.some((c) => s.shapes.some((x) => x.id === c.id && JSON.stringify(x.xfrm) === JSON.stringify(c.xfrm)));
    for (const n of fresh) {
      if (n.ph && !s.shapes.some((x) => x.ph && x.ph.kind === n.ph!.kind && x.ph.idx === n.ph!.idx)) {
        /* keep as placeholder when the slide lacks one */
      } else delete n.ph;
      if (same && n.xfrm) {
        n.xfrm.x += 182880;
        n.xfrm.y += 182880;
      }
    }
    s.shapes.push(...fresh);
    this.selection.clear();
    for (const n of fresh) this.selection.add(n.id);
    this.clip = structuredClone(fresh);
    this.changed();
  }

  // ---- commands ------------------------------------------------------------------------------------

  setZoom(k: number | "fit") {
    this.zoom = k === "fit" ? "fit" : Math.max(0.1, Math.min(k, 8));
    this.queueDraw();
    requestAnimationFrame?.(() => this.positionOverlay());
  }

  /** Apply run formatting to the editing selection or to the selected shapes. */
  private formatRuns(cmd: "bold" | "italic" | "underline" | "strikeThrough", prop: (p: RunProps, on: boolean) => RunProps, isOn: (p: RunProps) => boolean) {
    if (this.editing) {
      document.execCommand("styleWithCSS", false, "true");
      document.execCommand(cmd);
      this.syncEdit();
      return;
    }
    const nodes = this.selectedNodes();
    if (!nodes.length) return;
    const on = !nodes.every((n) => (shapeBody(n)?.text?.paragraphs ?? []).every((p) => (p.runs ?? []).every((r) => isOn(r.props ?? {}))));
    this.begin();
    for (const n of nodes) formatAllText(n, (p) => prop(p, on));
    this.changed();
  }

  private formatSize(arg: string) {
    const cur = this.currentSize();
    const steps = [8, 9, 10, 10.5, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 40, 44, 48, 54, 60, 66, 72, 80, 88, 96];
    let next: number;
    if (arg === "+") next = steps.find((s) => s > cur) ?? cur + 8;
    else if (arg === "-") next = [...steps].reverse().find((s) => s < cur) ?? Math.max(1, cur - 1);
    else next = Number(arg) || cur;
    if (this.editing) {
      const ed = this.editing;
      document.execCommand("styleWithCSS", false, "false");
      document.execCommand("fontSize", false, "7");
      for (const f of [...ed.root.querySelectorAll("font[size='7']")] as HTMLElement[]) {
        const span = document.createElement("span");
        span.style.fontSize = `${next * (ed.env.fontScale ?? 1) * ed.env.scale}px`;
        span.append(...f.childNodes);
        f.replaceWith(span);
      }
      this.syncEdit();
      return;
    }
    const nodes = this.selectedNodes();
    if (!nodes.length) return;
    this.begin();
    for (const n of nodes) formatAllText(n, (p) => ({ ...p, size: Math.round(next * 100) }));
    this.changed();
  }

  /** Font size (pt) at the caret or of the first selected shape. */
  currentSize(): number {
    if (this.editing) {
      const sel = document.getSelection();
      const node = sel?.anchorNode;
      const el = node instanceof HTMLElement ? node : node?.parentElement;
      if (el && this.editing.root.contains(el)) {
        const px = parseFloat(getComputedStyle(el).fontSize);
        if (px) return Math.round((px / this.editing.env.scale / (this.editing.env.fontScale ?? 1)) * 2) / 2;
      }
    }
    const n = this.selectedNodes()[0];
    const s = this.slide;
    if (!n || !s || !this.model) return 18;
    const res = resolveShape(n, slideContext(this.model, s), this.model);
    const p0 = shapeBody(n)?.text?.paragraphs?.[0];
    const r0 = p0?.runs?.find((r) => r.t !== "break");
    const sz = r0?.props?.size ?? p0?.end?.size;
    if (sz) return sz / 100;
    const lvl = p0?.props?.level ?? 0;
    for (const ls of [...res.lists].reverse()) {
      const v = ls.levels?.[lvl]?.run?.size ?? ls.default?.run?.size;
      if (v) return v / 100;
    }
    return 18;
  }

  private formatColor(hex: string) {
    if (this.editing) {
      document.execCommand("styleWithCSS", false, "true");
      document.execCommand("foreColor", false, hex);
      this.syncEdit();
      return;
    }
    const nodes = this.selectedNodes();
    if (!nodes.length) return;
    this.begin();
    const v = hex.replace("#", "").toUpperCase();
    for (const n of nodes) formatAllText(n, (p) => ({ ...p, fill: { t: "solid", color: { base: { t: "srgb", v } } } }));
    this.changed();
  }

  private formatPara(f: (p: Paragraph) => void, block: (pp: NonNullable<Paragraph["props"]>) => NonNullable<Paragraph["props"]>) {
    if (this.editing) {
      const ed = this.editing;
      setBlockProps(ed.root, block);
      const text = readEditable(ed.root, ed.orig, ed.env);
      const n = this.slide?.shapes.find((x) => x.id === ed.id);
      if (!n || !this.model || !this.slide) return;
      this.setTextOf(n, text, ed.cell);
      // re-render the editable with the new paragraph props, caret to the end
      const sc = slideContext(this.model, this.slide);
      const res = ed.cell ? { xfrm: null, geom: null, fill: null, line: null, body: {}, lists: [this.model.defaultTextStyle ?? {}], styleKind: "other" as const } : resolveShape(n, sc, this.model);
      const caret = caretOffset(ed.root);
      renderEditable(ed.root, text, res, ed.env);
      setCaretOffset(ed.root, caret);
      this.changed();
      return;
    }
    const nodes = this.selectedNodes();
    if (!nodes.length) return;
    this.begin();
    for (const n of nodes) formatParagraphs(n, f);
    this.changed();
  }

  private setFill(hex: string) {
    const nodes = this.selectedNodes();
    if (!nodes.length || !this.slide) return;
    this.begin();
    for (const n of nodes) {
      const b = shapeBody(n);
      if (b) b.fill = hex ? { t: "solid", color: { base: { t: "srgb", v: hex.replace("#", "").toUpperCase() } } } : { t: "none" };
    }
    this.changed();
  }

  private setOutline(hex: string, width?: number) {
    const nodes = this.selectedNodes();
    if (!nodes.length) return;
    this.begin();
    for (const n of nodes) {
      if (n.type !== "shape" && n.type !== "connector" && n.type !== "picture") continue;
      const line = { ...(n.line ?? {}) };
      if (hex === "none") line.fill = { t: "none" };
      else if (hex) line.fill = { t: "solid", color: { base: { t: "srgb", v: hex.replace("#", "").toUpperCase() } } };
      if (width !== undefined) line.width = width;
      n.line = line;
    }
    this.changed();
  }

  private newSlide(layoutPart?: string) {
    const m = this.model;
    if (!m) return;
    const master = m.masters[0]!;
    const cur = this.slide;
    let layout = layoutPart ? m.masters.flatMap((ms) => ms.layouts).find((l) => l.part === layoutPart) : undefined;
    if (!layout && cur) {
      // like PowerPoint: after a title slide, use "Title and Content"; otherwise the current layout
      const cl = slideContext(m, cur).layout;
      layout = cl.kind === "title" ? (master.layouts.find((l) => l.kind === "obj") ?? cl) : cl;
    }
    layout ??= master.layouts.find((l) => l.kind === "obj") ?? master.layouts[0];
    if (!layout) return;
    this.commitEdit();
    this.begin();
    const s = slideFromLayout(layout);
    m.slides.splice(this.current + 1, 0, s);
    this.current += 1;
    this.selection.clear();
    this.rebuildRail();
    this.syncNotes();
    this.changed(true);
  }

  /** Give focus back to the text overlay with its last selection (ribbon clicks steal it). */
  private refocusEdit() {
    const ed = this.editing;
    if (!ed || document.activeElement === ed.root) return;
    ed.root.focus();
    if (this.savedRange && ed.root.contains(this.savedRange.startContainer)) {
      const sel = document.getSelection();
      sel?.removeAllRanges();
      sel?.addRange(this.savedRange);
    }
  }

  execute(command: string, arg?: string): boolean {
    this.refocusEdit();
    const m = this.model;
    const s = this.slide;
    if (!m) return false;
    const sel = () => this.selectedNodes();
    switch (command) {
      case "undo":
        this.undo();
        return true;
      case "redo":
        this.redo();
        return true;
      case "selectAll":
        if (!s) return true;
        this.selection.clear();
        for (const n of s.shapes) this.selection.add(n.id);
        this.queueDraw();
        this.opts.onState?.();
        return true;
      case "copy":
        this.copy();
        return true;
      case "cut":
        this.copy();
        return this.execute("delete");
      case "paste":
        this.paste();
        return true;
      case "duplicate":
        this.copy();
        this.paste();
        return true;
      case "delete": {
        if (!s || !this.selection.size) return true;
        this.begin();
        // deleting a filled placeholder empties it first (PowerPoint keeps the prompt)
        s.shapes = s.shapes.filter((n) => !this.selection.has(n.id));
        this.selection.clear();
        this.changed();
        return true;
      }
      case "bold":
        this.formatRuns("bold", (p, on) => ({ ...p, bold: on }), (p) => !!p.bold);
        return true;
      case "italic":
        this.formatRuns("italic", (p, on) => ({ ...p, italic: on }), (p) => !!p.italic);
        return true;
      case "underline":
        this.formatRuns("underline", (p, on) => ({ ...p, underline: on ? "sng" : "none" }), (p) => !!p.underline && p.underline !== "none");
        return true;
      case "strike":
        this.formatRuns("strikeThrough", (p, on) => ({ ...p, strike: on ? "sngStrike" : "noStrike" }), (p) => !!p.strike && p.strike !== "noStrike");
        return true;
      case "fontSize":
        this.formatSize(arg ?? "+");
        return true;
      case "grow":
        this.formatSize("+");
        return true;
      case "shrink":
        this.formatSize("-");
        return true;
      case "color":
        this.formatColor(arg ?? "#C00000");
        return true;
      case "font":
        if (!arg) return true;
        if (this.editing) {
          document.execCommand("styleWithCSS", false, "true");
          document.execCommand("fontName", false, arg);
          this.syncEdit();
        } else if (sel().length) {
          this.begin();
          for (const n of sel()) formatAllText(n, (p) => ({ ...p, font: arg }));
          this.changed();
        }
        return true;
      case "align": {
        const a = arg ?? "l";
        this.formatPara(
          (p) => (p.props = { ...p.props, align: a }),
          (pp) => ({ ...pp, align: a }),
        );
        return true;
      }
      case "alignLeft":
      case "alignCenter":
      case "alignRight":
      case "justify":
        return this.execute("align", { alignLeft: "l", alignCenter: "ctr", alignRight: "r", justify: "just" }[command]);
      case "bullets":
      case "numbering": {
        const want = command === "bullets" ? "char" : "autoNum";
        const st = this.state();
        const off = command === "bullets" ? st.bullets : st.numbering;
        const bu = off ? { t: "none" as const } : want === "char" ? { t: "char" as const, ch: "•" } : { t: "autoNum" as const, scheme: "arabicPeriod" };
        const apply = (pp: NonNullable<Paragraph["props"]>) => ({ ...pp, bullet: bu, ...(off ? { marginLeft: 0, indent: 0 } : { marginLeft: 342900 * ((pp.level ?? 0) + 1), indent: -342900 }) });
        this.formatPara((p) => (p.props = apply(p.props ?? {})), apply);
        return true;
      }
      case "indent":
      case "outdent": {
        const d = command === "indent" ? 1 : -1;
        const f = (pp: NonNullable<Paragraph["props"]>) => ({ ...pp, level: Math.max(0, Math.min(8, (pp.level ?? 0) + d)) });
        this.formatPara((p) => (p.props = f(p.props ?? {})), f);
        return true;
      }
      case "fill":
        this.setFill(arg ?? "");
        return true;
      case "outline":
        this.setOutline(arg ?? "none");
        return true;
      case "outlineWidth":
        this.setOutline("", Math.round(Number(arg ?? 1) * EMU_PER_PT));
        return true;
      case "bringForward":
      case "sendBackward":
      case "bringToFront":
      case "sendToBack":
        if (!s || !this.selection.size) return true;
        this.begin();
        zOrder(s, this.selection, ({ bringForward: "forward", sendBackward: "backward", bringToFront: "front", sendToBack: "back" } as const)[command]);
        this.changed();
        return true;
      case "group": {
        if (!s || this.selection.size < 2) return true;
        this.begin();
        const g = group(s, this.selection, m);
        if (g) {
          this.selection.clear();
          this.selection.add(g.id);
        }
        this.changed();
        return true;
      }
      case "ungroup": {
        if (!s) return true;
        const gs = sel().filter((n) => n.type === "group");
        if (!gs.length) return true;
        this.begin();
        this.selection.clear();
        for (const g of gs) for (const k of ungroup(s, g.id)) this.selection.add(k.id);
        this.changed();
        return true;
      }
      case "insertTextBox":
        if (!s) return true;
        this.pendingInsert = (x) => newTextBox(s, { ...x, cy: Math.max(x.cy, 369332) });
        this.scroll.style.cursor = "crosshair";
        this.scroll.focus();
        return true;
      case "insertShape":
        if (!s) return true;
        this.pendingInsert = (x) => newShape(s, arg ?? "rect", x);
        this.scroll.style.cursor = "crosshair";
        this.scroll.focus();
        return true;
      case "insertTable": {
        if (!s) return true;
        const [r, c] = (arg ?? "3x3").split("x").map(Number) as [number, number];
        const w = Math.round(m.width * 0.7);
        const hh = (r || 3) * 370840;
        this.insertNode(newTable(s, r || 3, c || 3, { x: Math.round((m.width - w) / 2), y: Math.round((m.height - hh) / 2), cx: w, cy: hh }));
        return true;
      }
      case "insertImage":
        void this.host.pickImage().then((b64) => (b64 ? this.insertImageData(b64) : undefined)).catch((e) => this.opts.onError?.(String(e)));
        return true;
      case "newSlide":
        this.newSlide(arg);
        return true;
      case "duplicateSlide": {
        if (!s) return true;
        this.commitEdit();
        this.begin();
        const copy = structuredClone(s);
        delete copy.part;
        m.slides.splice(this.current + 1, 0, copy);
        this.current += 1;
        this.rebuildRail();
        this.changed(true);
        return true;
      }
      case "deleteSlide": {
        if (!s || m.slides.length <= 1) return true;
        this.commitEdit();
        this.begin();
        m.slides.splice(this.current, 1);
        this.current = Math.min(this.current, m.slides.length - 1);
        this.selection.clear();
        this.rebuildRail();
        this.syncNotes();
        this.changed(true);
        return true;
      }
      case "hideSlide":
        if (!s) return true;
        this.begin();
        s.hidden = !s.hidden;
        this.changed();
        return true;
      case "moveSlideUp":
        this.moveSlide(this.current, this.current - 1);
        return true;
      case "moveSlideDown":
        this.moveSlide(this.current, this.current + 1);
        return true;
      case "layout": {
        if (!s || !arg) return true;
        const layout = m.masters.flatMap((ms) => ms.layouts).find((l) => l.part === arg);
        if (!layout) return true;
        this.commitEdit();
        this.begin();
        m.slides[this.current] = applyLayout(s, layout);
        this.selection.clear();
        this.changed();
        return true;
      }
      case "background": {
        if (!s) return true;
        this.begin();
        const all = arg?.startsWith("all:");
        const hex = (all ? arg!.slice(4) : (arg ?? "")).replace("#", "").toUpperCase();
        const bg = hex ? { fill: { t: "solid" as const, color: { base: { t: "srgb" as const, v: hex } } } } : undefined;
        for (const t of all ? m.slides : [s]) t.background = bg;
        this.changed(!!all);
        return true;
      }
      case "slideSize": {
        this.begin();
        const [w, hh] = arg === "4:3" ? [9144000, 6858000] : [12192000, 6858000];
        const sx = w / m.width;
        if (sx !== 1) {
          // scale horizontal positions of all content (like PowerPoint's "Ensure fit" for width)
          const scaleX = (nodes: SlideNode[]) => {
            for (const n of nodes) if (n.xfrm) ((n.xfrm.x = Math.round(n.xfrm.x * sx)), (n.xfrm.cx = Math.round(n.xfrm.cx * sx)));
          };
          for (const ms of m.masters) {
            scaleX(ms.shapes);
            for (const l of ms.layouts) scaleX(l.shapes);
          }
          for (const sl of m.slides) scaleX(sl.shapes);
        }
        m.width = w;
        m.height = hh;
        this.changed(true);
        return true;
      }
      case "zoomIn":
        this.setZoom(this.view().k * 1.25);
        return true;
      case "zoomOut":
        this.setZoom(this.view().k / 1.25);
        return true;
      case "zoomFit":
        this.setZoom("fit");
        return true;
      case "prevSlide":
        this.goTo(this.current - 1);
        return true;
      case "nextSlide":
        this.goTo(this.current + 1);
        return true;
      case "show":
        this.commitEdit();
        this.opts.onShow?.(arg === "current" ? this.current : 0);
        return true;
      case "transition": {
        if (!s) return true;
        const [kind, dir] = (arg ?? "fade").split(":");
        const all = arg?.endsWith(":all");
        this.begin();
        for (const t of all ? m.slides : [s]) {
          if (kind === "none") delete t.transition;
          else {
            const def = TRANSITIONS.find((x) => x.kind === kind);
            t.transition = { ...(t.transition ?? {}), kind: kind!, dir: dir && dir !== "all" ? dir : def?.dir, duration: t.transition?.kind === kind ? t.transition?.duration : kind === "morph" ? 2000 : 700, advanceClick: t.transition?.advanceClick ?? true };
            if (!t.transition.dir) delete t.transition.dir;
          }
        }
        this.rebuildRail();
        this.changed(!!all);
        return true;
      }
      case "transitionDuration": {
        if (!s?.transition) return true;
        this.begin();
        s.transition.duration = Math.max(0, Math.round(Number(arg) * 1000));
        this.changed();
        return true;
      }
      case "advanceAfter": {
        if (!s) return true;
        this.begin();
        s.transition ??= { kind: "cut", advanceClick: true };
        if (arg === "" || arg === undefined) delete s.transition.advanceAfter;
        else s.transition.advanceAfter = Math.round(Number(arg) * 1000);
        this.changed();
        return true;
      }
      case "animate": {
        if (!s || !this.selection.size) return true;
        const [cls, preset, sub] = (arg ?? "entr:fade").split(":");
        const def = ANIMATION_PRESETS.find((p) => p.class === cls && p.preset === preset);
        this.begin();
        s.timing ??= { effects: [] };
        for (const n of this.selectedNodes()) {
          s.timing.effects.push({
            shape: n.id,
            class: cls!,
            preset: preset!,
            presetId: presetId(cls!, preset!),
            subtype: sub ? Number(sub) : (def?.subtype ?? 0),
            trigger: "onClick",
            delay: 0,
            duration: cls === "emph" ? 1000 : preset === "appear" ? 1 : 500,
          });
        }
        this.rebuildRail();
        this.changed();
        this.renderTimeline();
        return true;
      }
      case "clearAnimations": {
        if (!s?.timing) return true;
        this.begin();
        const ids = this.selection.size ? this.selection : new Set(s.shapes.map((n) => n.id));
        s.timing.effects = s.timing.effects.filter((e) => !ids.has(e.shape));
        if (!s.timing.effects.length) delete s.timing;
        this.rebuildRail();
        this.changed();
        this.renderTimeline();
        return true;
      }
      case "timeline":
        this.timelineOpen = !this.timelineOpen;
        this.renderTimeline();
        this.queueDraw();
        return true;
      default:
        return false;
    }
  }

  state(): SlideEditorState {
    const s = this.slide;
    const nodes = this.selectedNodes();
    const m = this.model;
    let bold = false, italic = false, underline = false, strike = false;
    let align: string | null = null;
    let bullets = false, numbering = false;
    if (this.editing && typeof document.queryCommandState === "function") {
      try {
        bold = document.queryCommandState("bold");
        italic = document.queryCommandState("italic");
        underline = document.queryCommandState("underline");
        strike = document.queryCommandState("strikeThrough");
      } catch {
        /* jsdom */
      }
    }
    const n0 = nodes[0];
    const sb = n0 ? shapeBody(n0) : null;
    const p0 = sb?.text?.paragraphs?.[0];
    if (n0 && s && m) {
      const res = resolveShape(n0, slideContext(m, s), m);
      const eff = effectivePara(res.lists, p0?.props);
      const bu = eff.bullet;
      bullets = bu?.t === "char";
      numbering = bu?.t === "autoNum";
      align = eff.align ?? "l";
      if (!this.editing) {
        const runs = (sb?.text?.paragraphs ?? []).flatMap((p) => p.runs ?? []).filter((r) => r.t !== "break");
        const all = (f: (p: RunProps) => boolean) => runs.length > 0 && runs.every((r) => f(r.props ?? {}));
        bold = all((p) => !!p.bold);
        italic = all((p) => !!p.italic);
        underline = all((p) => !!p.underline && p.underline !== "none");
        strike = all((p) => !!p.strike && p.strike !== "noStrike");
      }
    }
    return {
      editing: !!this.editing,
      bold,
      italic,
      underline,
      strike,
      align,
      bullets,
      numbering,
      selection: nodes.length,
      slide: this.current,
      slides: m?.slides.length ?? 0,
      hidden: !!s?.hidden,
      layout: s?.layout ?? "",
      canGroup: nodes.length > 1,
      canUngroup: nodes.some((n) => n.type === "group"),
    };
  }
}

/** Caret position as a character offset (paragraph breaks count as one). */
function caretOffset(root: HTMLElement): number {
  const sel = document.getSelection();
  if (!sel?.rangeCount || !root.contains(sel.anchorNode)) return -1;
  const r = document.createRange();
  r.selectNodeContents(root);
  r.setEnd(sel.getRangeAt(0).endContainer, sel.getRangeAt(0).endOffset);
  const frag = r.cloneContents();
  const blocks = frag.querySelectorAll("div").length;
  return (frag.textContent ?? "").length + Math.max(0, blocks - 1);
}

function setCaretOffset(root: HTMLElement, off: number) {
  const sel = document.getSelection();
  if (!sel) return;
  const r = document.createRange();
  if (off < 0) {
    r.selectNodeContents(root);
    r.collapse(false);
  } else {
    let left = off;
    let placed = false;
    const blocks = [...root.children];
    for (let b = 0; b < blocks.length && !placed; b++) {
      const walker = document.createTreeWalker(blocks[b]!, NodeFilter.SHOW_TEXT);
      let t: Node | null;
      let last: Node | null = null;
      while ((t = walker.nextNode())) {
        last = t;
        const len = t.textContent?.length ?? 0;
        if (left <= len) {
          r.setStart(t, left);
          placed = true;
          break;
        }
        left -= len;
      }
      if (!placed) {
        if (left === 0 || b === blocks.length - 1) {
          if (last) r.setStart(last, last.textContent?.length ?? 0);
          else r.setStart(blocks[b]!, 0);
          placed = true;
        } else left -= 1;
      }
    }
    if (!placed) (r.selectNodeContents(root), r.collapse(false));
    r.collapse(true);
  }
  sel.removeAllRanges();
  sel.addRange(r);
}

function firstRunProps(t: TextBody | undefined): RunProps | undefined {
  const r = t?.paragraphs?.[0]?.runs?.find((x) => x.t !== "break");
  return r?.props ?? t?.paragraphs?.[0]?.end;
}

/** Resize an xfrm by dragging handle `hd` by (dx, dy) EMU in slide space; opposite edge stays put. */
export function resizeXfrm(o: Xfrm, hd: number, dx: number, dy: number, keepAspect: boolean): Xfrm {
  const a = (((o.rot ?? 0) / 60000) * Math.PI) / 180;
  // pointer delta in the shape's local frame
  const lx = dx * Math.cos(a) + dy * Math.sin(a);
  const ly = -dx * Math.sin(a) + dy * Math.cos(a);
  let l = 0, t = 0, r = o.cx, b = o.cy;
  if ([0, 6, 7].includes(hd)) l += lx;
  if ([2, 3, 4].includes(hd)) r += lx;
  if ([0, 1, 2].includes(hd)) t += ly;
  if ([4, 5, 6].includes(hd)) b += ly;
  const min = EMU_PER_PT;
  if (r - l < min) [l, r] = [0, 6, 7].includes(hd) ? [r - min, r] : [l, l + min];
  if (b - t < min) [t, b] = [0, 1, 2].includes(hd) ? [b - min, b] : [t, t + min];
  if (keepAspect && o.cx > 0 && o.cy > 0) {
    const ratio = o.cx / o.cy;
    const w = r - l;
    const hh = b - t;
    if (w / hh > ratio) {
      const nh = w / ratio;
      if ([0, 1, 2].includes(hd)) t = b - nh;
      else b = t + nh;
    } else {
      const nw = hh * ratio;
      if ([0, 6, 7].includes(hd)) l = r - nw;
      else r = l + nw;
    }
  }
  const w = r - l;
  const hh = b - t;
  // new centre: local offset rotated back into slide space
  const ocx = o.cx / 2;
  const ocy = o.cy / 2;
  const ncx = (l + r) / 2 - ocx;
  const ncy = (t + b) / 2 - ocy;
  const cx = o.x + ocx + ncx * Math.cos(a) - ncy * Math.sin(a);
  const cy = o.y + ocy + ncx * Math.sin(a) + ncy * Math.cos(a);
  return { ...o, x: Math.round(cx - w / 2), y: Math.round(cy - hh / 2), cx: Math.round(w), cy: Math.round(hh) };
}

/** Pixel size of a base64 picture. */
function imageSize(b64: string): Promise<{ w: number; h: number }> {
  return new Promise((resolve) => {
    if (typeof Image === "undefined") return resolve({ w: 640, h: 480 });
    const img = new Image();
    img.onload = () => resolve({ w: img.naturalWidth || 640, h: img.naturalHeight || 480 });
    img.onerror = () => resolve({ w: 640, h: 480 });
    img.src = `data:image/*;base64,${b64}`;
  });
}

