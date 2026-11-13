/**
 * Slideshow (P4-05/06): full-screen playback with click-step animations,
 * transitions (incl. Morph), auto-advance, black/white screen, jump-to-slide,
 * and a presenter view (current + next slide, notes, timer) that drives an
 * audience window on a second display when the browser allows it (Window
 * Management API placement, plain popup otherwise).
 */
import { h } from "../core/dom";
import { buildSteps, evaluate, type AnimBox, type Step } from "./anim";
import type { PresentationModel, Slide, SlideNode } from "./model";
import { drawSlide, renderSlideToCanvas, type RenderOptions, type ShapeAnim } from "./render";
import { resolveShape, slideContext } from "./resolve";
import { EMU_PER_PT } from "./text";
import { camera, isCameo } from "./camera";
import { Teleprompter } from "./teleprompter";
import { drawMorph, drawTransition, transitionDuration, type Raster } from "./transition";

export interface ShowOptions {
  start?: number;
  images?: (part: string) => CanvasImageSource | null;
  charts?: (part: string) => import("./chart").SlideChart | null;
  onExit?: (slide: number) => void;
  /** Presenter view in this window, audience view in a second window. */
  presenter?: boolean;
  /** Where to mount (default document.body). */
  host?: HTMLElement;
  /** Request browser full screen (default true outside tests). */
  fullscreen?: boolean;
  /** Open the teleprompter in the presenter view. */
  teleprompter?: boolean;
  /** Navigation log: a slide was entered (step -1) or a build step played. */
  onNavigate?: (slide: number, step: number) => void;
  /** Extra painting on the audience canvas (ink): ctx maps slide points. */
  overlay?: (ctx: CanvasRenderingContext2D, slide: number) => void;
}

const pt = (emu: number) => emu / EMU_PER_PT;

interface Surface {
  canvas: HTMLCanvasElement;
  win: Window;
}

export class SlideShow {
  readonly el: HTMLElement;
  /** Indices of the slides that play (hidden slides are skipped). */
  readonly order: number[];
  pos = 0;
  /** Steps played on the current slide (-1 = none). */
  step = -1;
  steps: Step[] = [];
  ended = false;
  screen: "normal" | "black" | "white" = "normal";

  private anim: { step: number; start: number } | null = null;
  private trans: { from: Raster | null; to: Raster; fromSlide: Slide | null; start: number; dur: number } | null = null;
  private raf = 0;
  private advanceTimer: ReturnType<typeof setTimeout> | null = null;
  private jump = "";
  private audience: Surface | null = null;
  private main: HTMLCanvasElement;
  private presenterEls: { current: HTMLCanvasElement; next: HTMLCanvasElement; notes: HTMLElement; timer: HTMLElement; counter: HTMLElement } | null = null;
  private startedAt = Date.now();
  private disposed = false;
  private onKeyBound = (e: KeyboardEvent) => this.onKey(e);
  private onResizeBound = () => this.draw();

  constructor(
    readonly model: PresentationModel,
    private readonly opts: ShowOptions = {},
  ) {
    this.order = model.slides.map((s, i) => (s.hidden && i !== opts.start ? -1 : i)).filter((i) => i >= 0);
    this.pos = Math.max(0, this.order.indexOf(opts.start ?? this.order[0] ?? 0));
    this.main = h("canvas", { class: "show-canvas", "aria-label": "Slide show" });
    if (opts.presenter) {
      const current = h("canvas", { class: "presenter-current" });
      const next = h("canvas", { class: "presenter-next" });
      const notes = h("div", { class: "presenter-notes", tabindex: "0" });
      const timer = h("span", { class: "presenter-timer" });
      const counter = h("span", { class: "presenter-counter" });
      this.presenterEls = { current, next, notes, timer, counter };
      const btn = (label: string, title: string, f: () => void) => h("button", { class: "presenter-btn", title, onclick: f }, label);
      this.el = h(
        "div",
        { class: "presenter", role: "application", "aria-label": "Presenter view", tabindex: "0" },
        h("div", { class: "presenter-main" }, current),
        h("div", { class: "presenter-side" }, h("div", { class: "presenter-label" }, "Next"), next, h("div", { class: "presenter-label" }, "Notes"), notes),
        h(
          "div",
          { class: "presenter-bar" },
          timer,
          counter,
          btn("◀", "Previous (←)", () => this.prev()),
          btn("▶", "Next (→ / Space)", () => this.next()),
          btn("■", "Black screen (B)", () => this.toggleScreen("black")),
          btn("☰", "Teleprompter (T)", () => this.toggleTeleprompter()),
          btn("A+", "Larger notes", () => this.notesSize(1)),
          btn("A−", "Smaller notes", () => this.notesSize(-1)),
          btn("✕", "End show (Esc)", () => this.exit()),
        ),
      );
      current.addEventListener("click", () => this.next());
    } else {
      this.el = h("div", { class: `slideshow${opts.host ? " embedded" : ""}`, role: "application", "aria-label": "Slide show", tabindex: "0" }, this.main);
      this.el.addEventListener("click", (e) => (e.button === 0 ? this.next() : undefined));
      this.el.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        this.prev();
      });
    }
    this.el.addEventListener("wheel", (e) => (e.deltaY > 0 ? this.next() : this.prev()), { passive: true });
  }

  // ---- lifecycle ---------------------------------------------------------------------------

  async start() {
    (this.opts.host ?? document.body).append(this.el);
    document.addEventListener("keydown", this.onKeyBound, true);
    window.addEventListener("resize", this.onResizeBound);
    this.el.focus();
    if (this.model.slides.some((sl) => sl.shapes.some((n) => isCameo(n)))) void camera.start().then(() => this.loop()).catch(() => {});
    if (this.opts.presenter) await this.openAudience();
    else if (this.opts.fullscreen !== false && this.el.requestFullscreen) {
      await this.el.requestFullscreen().catch(() => {});
      document.addEventListener("fullscreenchange", this.onFsChange);
    }
    this.enterSlide(null);
    if (this.opts.teleprompter && this.presenterEls) this.toggleTeleprompter();
    this.tickTimer();
  }

  private onFsChange = () => {
    if (!document.fullscreenElement && !this.disposed) this.exit();
  };

  exit() {
    if (this.disposed) return;
    this.disposed = true;
    cancelAnimationFrame(this.raf);
    if (this.advanceTimer) clearTimeout(this.advanceTimer);
    document.removeEventListener("keydown", this.onKeyBound, true);
    document.removeEventListener("fullscreenchange", this.onFsChange);
    window.removeEventListener("resize", this.onResizeBound);
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => {});
    this.audience?.win.close();
    this.teleprompter?.dispose();
    this.el.remove();
    this.opts.onExit?.(this.order[this.pos] ?? 0);
  }

  get slideIndex(): number {
    return this.order[this.pos] ?? 0;
  }

  get slide(): Slide | null {
    return this.model.slides[this.slideIndex] ?? null;
  }

  // ---- audience window (dual display) ------------------------------------------------------------

  private async openAudience() {
    let features = "popup,width=1280,height=720";
    try {
      const w = window as Window & { getScreenDetails?: () => Promise<{ screens: { left: number; top: number; width: number; height: number; isPrimary: boolean }[]; currentScreen: { left: number } }> };
      if (w.getScreenDetails) {
        const d = await w.getScreenDetails();
        const other = d.screens.find((s) => s.left !== d.currentScreen.left) ?? null;
        if (other) features = `popup,left=${other.left},top=${other.top},width=${other.width},height=${other.height}`;
      }
    } catch {
      /* permission denied: plain popup */
    }
    const win = window.open("", "prism-audience", features);
    if (!win) return;
    win.document.title = "Prism Office — Slide Show";
    win.document.body.style.cssText = "margin:0;background:#000;overflow:hidden";
    const canvas = win.document.createElement("canvas");
    canvas.style.cssText = "display:block;width:100vw;height:100vh;cursor:none";
    win.document.body.append(canvas);
    win.addEventListener("keydown", (e) => this.onKey(e));
    win.addEventListener("resize", () => this.draw());
    canvas.addEventListener("click", () => {
      // full screen needs a gesture inside the audience window
      if (!win.document.fullscreenElement) void win.document.documentElement.requestFullscreen?.().catch(() => {});
      this.next();
    });
    win.addEventListener("beforeunload", () => {
      this.audience = null;
    });
    this.audience = { canvas, win };
  }

  // ---- navigation ----------------------------------------------------------------------------------

  private boxOf = (slide: Slide) => {
    const cache = new Map<number, AnimBox | null>();
    return (id: number): AnimBox | null => {
      if (cache.has(id)) return cache.get(id)!;
      const n = slide.shapes.find((s) => s.id === id) ?? findDeep(slide.shapes, id);
      const x = n ? (n.xfrm ?? resolveShape(n, slideContext(this.model, slide), this.model).xfrm) : null;
      const b = x ? { slideW: pt(this.model.width), slideH: pt(this.model.height), x: pt(x.x), y: pt(x.y), w: pt(x.cx), h: pt(x.cy) } : null;
      cache.set(id, b);
      return b;
    };
  };

  /** Render state for a slide with `step` steps done (t ms into the current one). */
  private states(slide: Slide, steps: Step[], step: number, t: number): Map<number, ShapeAnim> {
    return steps.length ? evaluate(steps, step, t, this.boxOf(slide)) : new Map();
  }

  private enterSlide(from: { slide: Slide; raster: Raster | null } | null) {
    const s = this.slide;
    if (!s) return;
    this.opts.onNavigate?.(this.slideIndex, -1);
    this.steps = buildSteps(s.timing);
    this.step = -1;
    this.anim = null;
    if (this.advanceTimer) clearTimeout(this.advanceTimer);
    const dur = from ? transitionDuration(s.transition) : 0;
    if (dur > 0) {
      const to = this.raster(s, this.steps, -1, 0);
      this.trans = { from: from!.raster, to, fromSlide: from!.slide, start: performance.now(), dur };
    } else this.trans = null;
    if (!this.trans) this.afterEnter();
    this.loop();
    this.updatePresenter();
  }

  private afterEnter() {
    if (this.steps[0]?.auto) this.playStep(0);
    else this.scheduleAdvance();
  }

  private scheduleAdvance() {
    const s = this.slide;
    const after = s?.transition?.advanceAfter;
    if (after === undefined || this.step < this.steps.length - 1) return;
    if (this.advanceTimer) clearTimeout(this.advanceTimer);
    this.advanceTimer = setTimeout(() => this.next(), after);
  }

  private playStep(i: number) {
    this.step = i;
    if (!(i === 0 && this.steps[0]?.auto)) this.opts.onNavigate?.(this.slideIndex, i);
    this.anim = { step: i, start: performance.now() };
    this.loop();
    this.updatePresenter();
  }

  /** Advance: finish what is running, else play the next build step, else go to the next slide. */
  next() {
    if (this.disposed) return;
    if (this.screen !== "normal") return this.toggleScreen(this.screen);
    if (this.trans) {
      this.trans = null;
      this.afterEnter();
      return this.draw();
    }
    if (this.anim) {
      this.anim = null;
      this.scheduleAdvance();
      return this.draw();
    }
    if (this.step + 1 < this.steps.length) return this.playStep(this.step + 1);
    if (this.ended) return this.exit();
    if (this.pos + 1 >= this.order.length) {
      this.ended = true;
      return this.draw();
    }
    const from = this.slide!;
    const raster = this.raster(from, this.steps, this.steps.length, Infinity);
    this.pos++;
    this.enterSlide({ slide: from, raster });
  }

  /** Back one build step, or to the previous slide fully built (no transition). */
  prev() {
    if (this.disposed) return;
    if (this.ended) {
      this.ended = false;
      return this.draw();
    }
    this.trans = null;
    this.anim = null;
    if (this.step >= 0 && !(this.step === 0 && this.steps[0]?.auto)) {
      this.step--;
      this.updatePresenter();
      return this.draw();
    }
    if (this.pos === 0) return this.draw();
    this.pos--;
    this.steps = buildSteps(this.slide?.timing);
    this.step = this.steps.length - 1;
    this.opts.onNavigate?.(this.slideIndex, -1);
    for (let i = 0; i <= this.step; i++) if (!(i === 0 && this.steps[0]?.auto)) this.opts.onNavigate?.(this.slideIndex, i);
    this.updatePresenter();
    this.draw();
  }

  goTo(slideIndex: number) {
    const p = this.order.indexOf(slideIndex);
    if (p < 0) return;
    this.ended = false;
    this.pos = p;
    this.enterSlide(null);
  }

  toggleScreen(mode: "black" | "white") {
    this.screen = this.screen === mode ? "normal" : mode;
    this.draw();
  }

  teleprompter: Teleprompter | null = null;

  toggleTeleprompter() {
    if (this.teleprompter) {
      this.teleprompter.dispose();
      this.teleprompter = null;
      return;
    }
    const tp = new Teleprompter({ onClose: () => this.toggleTeleprompter() });
    tp.setText(this.slide?.notes ?? "");
    this.el.append(tp.el);
    this.teleprompter = tp;
  }

  private notesSize(d: number) {
    const n = this.presenterEls?.notes;
    if (!n) return;
    const cur = parseFloat(getComputedStyle(n).fontSize) || 18;
    n.style.fontSize = `${Math.max(10, Math.min(48, cur + d * 2))}px`;
  }

  private onKey(e: KeyboardEvent) {
    if (this.disposed) return;
    const k = e.key;
    let handled = true;
    if (/^[0-9]$/.test(k)) this.jump += k;
    else if (k === "Enter" && this.jump) {
      this.goTo(Number(this.jump) - 1);
      this.jump = "";
    } else if (["ArrowRight", "ArrowDown", " ", "PageDown", "n", "N", "Enter"].includes(k)) this.next();
    else if (["ArrowLeft", "ArrowUp", "PageUp", "Backspace", "p", "P"].includes(k)) this.prev();
    else if (k === "Home") this.goTo(this.order[0] ?? 0);
    else if (k === "End") this.goTo(this.order[this.order.length - 1] ?? 0);
    else if (k === "b" || k === "B" || k === ".") this.toggleScreen("black");
    else if (k === "w" || k === "W" || k === ",") this.toggleScreen("white");
    else if ((k === "t" || k === "T") && this.presenterEls) this.toggleTeleprompter();
    else if (k === "Escape" || k === "-") this.exit();
    else handled = false;
    if (!/^[0-9]$/.test(k) && k !== "Enter") this.jump = "";
    if (handled) {
      e.preventDefault();
      e.stopPropagation();
    }
  }

  // ---- drawing ---------------------------------------------------------------------------------------

  private renderOpts(index: number, anim?: Map<number, ShapeAnim>): RenderOptions {
    return { images: this.opts.images, charts: this.opts.charts, camera: () => camera.frame(), slideNumber: index + 1, anim: anim ? (id) => anim.get(id) ?? null : undefined };
  }

  /** Raster of a slide in a given build state at the main canvas resolution. */
  private raster(slide: Slide, steps: Step[], step: number, t: number): Raster {
    const target = this.audience?.canvas ?? this.presenterEls?.current ?? this.main;
    const w = Math.max(320, target.width || 1280);
    const c = typeof OffscreenCanvas !== "undefined" ? new OffscreenCanvas(1, 1) : document.createElement("canvas");
    const idx = this.model.slides.indexOf(slide);
    renderSlideToCanvas(c, this.model, slide, w, 1, this.renderOpts(idx, this.states(slide, steps, step, t)));
    return c;
  }

  private loop() {
    cancelAnimationFrame(this.raf);
    const tick = () => {
      if (this.disposed) return;
      const now = performance.now();
      if (this.trans && now - this.trans.start >= this.trans.dur) {
        this.trans = null;
        this.afterEnter();
      }
      if (this.anim) {
        const st = this.steps[this.anim.step];
        if (!st || now - this.anim.start >= st.duration) {
          this.anim = null;
          this.scheduleAdvance();
        }
      }
      this.draw();
      if (this.trans || this.anim || this.liveCameo()) this.raf = requestAnimationFrame(tick);
    };
    this.raf = requestAnimationFrame(tick);
  }

  /** A live camera feeds a Cameo on the current slide (keep repainting). */
  private liveCameo(): boolean {
    return camera.active && !!this.slide?.shapes.some((n) => isCameo(n));
  }

  /** Paint the show into a canvas (letterboxed). */
  private paint(canvas: HTMLCanvasElement, win: Window, slide: Slide | null, anim: Map<number, ShapeAnim> | null, withTransition: boolean) {
    const dpr = win.devicePixelRatio || 1;
    const cw = canvas.clientWidth || win.innerWidth;
    const ch = canvas.clientHeight || win.innerHeight;
    if (!cw || !ch) return;
    if (canvas.width !== Math.round(cw * dpr) || canvas.height !== Math.round(ch * dpr)) {
      canvas.width = Math.round(cw * dpr);
      canvas.height = Math.round(ch * dpr);
    }
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.fillStyle = "#000";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    if (this.screen !== "normal" && canvas !== this.presenterEls?.current) {
      ctx.fillStyle = this.screen === "black" ? "#000" : "#fff";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      return;
    }
    const W = pt(this.model.width);
    const H = pt(this.model.height);
    const k = Math.min(canvas.width / W, canvas.height / H);
    const ox = (canvas.width - W * k) / 2;
    const oy = (canvas.height - H * k) / 2;
    if (this.ended && withTransition) {
      ctx.fillStyle = "#d6d3d1";
      ctx.font = `${Math.round(16 * dpr)}px system-ui, sans-serif`;
      ctx.textAlign = "center";
      ctx.fillText("End of slide show, click to exit.", canvas.width / 2, Math.round(40 * dpr));
      return;
    }
    if (!slide) return;
    const tr = withTransition ? this.trans : null;
    if (tr) {
      const p = Math.min(1, (performance.now() - tr.start) / tr.dur);
      const t = slide.transition!;
      if (t.kind === "morph" && tr.fromSlide) {
        ctx.setTransform(k, 0, 0, k, ox, oy);
        drawMorph(ctx, this.model, tr.fromSlide, slide, p, (n, s) => n.xfrm ?? resolveShape(n, slideContext(this.model, s), this.model).xfrm, this.renderOpts(this.slideIndex));
      } else {
        ctx.translate(ox, oy);
        ctx.beginPath();
        ctx.rect(0, 0, W * k, H * k);
        ctx.clip();
        drawTransition(ctx, t.kind, t.dir, t.option, p, tr.from, tr.to, W * k, H * k);
      }
      return;
    }
    ctx.setTransform(k, 0, 0, k, ox, oy);
    drawSlide(ctx, this.model, slide, this.renderOpts(this.model.slides.indexOf(slide), anim ?? undefined));
    if (withTransition) this.opts.overlay?.(ctx, this.model.slides.indexOf(slide));
    if (canvas === this.main) this.view = { k: k / dpr, ox: ox / dpr, oy: oy / dpr };
  }

  /** Slide-point mapping of the main canvas in CSS px (ink input). */
  view = { k: 1, ox: 0, oy: 0 };

  draw() {
    if (this.disposed) return;
    const slide = this.slide;
    const t = this.anim ? performance.now() - this.anim.start : Infinity;
    const anim = slide ? this.states(slide, this.steps, this.step, t) : null;
    if (this.audience && !this.audience.win.closed) this.paint(this.audience.canvas, this.audience.win, slide, anim, true);
    if (this.presenterEls) {
      this.paint(this.presenterEls.current, window, slide, anim, true);
      const nextIdx = this.step + 1 < this.steps.length ? this.slideIndex : this.order[this.pos + 1];
      const nextSlide = nextIdx !== undefined ? this.model.slides[nextIdx] : undefined;
      if (nextSlide) {
        const ns = buildSteps(nextSlide.timing);
        const nstep = nextIdx === this.slideIndex ? this.step + 1 : ns[0]?.auto ? 0 : -1;
        this.paint(this.presenterEls.next, window, nextSlide, this.states(nextSlide, ns, nstep, Infinity), false);
      } else {
        const c = this.presenterEls.next;
        c.getContext("2d")?.clearRect(0, 0, c.width, c.height);
      }
    } else this.paint(this.main, window, slide, anim, true);
  }

  private updatePresenter() {
    const p = this.presenterEls;
    if (!p) return;
    p.notes.textContent = this.slide?.notes || "No notes for this slide.";
    if (this.teleprompter && this.teleprompter.el.dataset.slide !== String(this.slideIndex)) {
      this.teleprompter.el.dataset.slide = String(this.slideIndex);
      this.teleprompter.setText(this.slide?.notes ?? "");
    }
    p.notes.classList.toggle("empty", !this.slide?.notes);
    const built = this.steps.length ? ` · build ${Math.max(0, this.step + 1)}/${this.steps.length}` : "";
    p.counter.textContent = `Slide ${this.pos + 1} of ${this.order.length}${built}`;
  }

  private tickTimer() {
    const p = this.presenterEls;
    if (!p || this.disposed) return;
    const s = Math.floor((Date.now() - this.startedAt) / 1000);
    const mm = String(Math.floor(s / 60)).padStart(2, "0");
    const ss = String(s % 60).padStart(2, "0");
    p.timer.textContent = `⏱ ${mm}:${ss}   ${new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
    setTimeout(() => this.tickTimer(), 1000);
  }
}

function findDeep(nodes: SlideNode[], id: number): SlideNode | null {
  for (const n of nodes) {
    if (n.id === id) return n;
    if (n.type === "group") {
      const f = findDeep(n.children, id);
      if (f) return f;
    }
  }
  return null;
}

