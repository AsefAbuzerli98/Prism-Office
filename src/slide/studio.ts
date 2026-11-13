/**
 * Recording Studio (P4-09): rehearse and record a take of the slideshow
 * with narration (microphone), camera (Cameo / self view), on-slide ink and
 * the navigation/transition timeline. Tracks are shown on a timeline;
 * the take can be previewed, its slide timings saved into the deck, and
 * exported as video (video.ts).
 */
import { h } from "../core/dom";
import { camera, isCameo, microphone } from "./camera";
import type { PresentationModel } from "./model";
import { SlideShow } from "./show";
import { Teleprompter } from "./teleprompter";
import { drawInk, type InkStroke, type Recording } from "./video";

export interface StudioOptions {
  start?: number;
  images?: (part: string) => CanvasImageSource | null;
  charts?: (part: string) => import("./chart").SlideChart | null;
  /** Previous take to continue/preview. */
  recording?: Recording | null;
  onSaveTimings?: (perSlideMs: Map<number, number>) => void;
  onExport?: (rec: Recording) => void;
  onClose?: (rec: Recording | null) => void;
  /** Start with the teleprompter open. */
  teleprompter?: boolean;
}

type Tool = "none" | "pen" | "highlighter" | "eraser";

const PEN_COLORS = ["#E11D48", "#2563EB", "#16A34A", "#111827", "#FACC15"];

export class RecordingStudio {
  readonly el: HTMLElement;
  private stage: HTMLElement;
  private ink: HTMLCanvasElement;
  private selfView: HTMLVideoElement;
  private timeline: HTMLCanvasElement;
  private clock: HTMLElement;
  private recBtn: HTMLButtonElement;
  private pauseBtn: HTMLButtonElement;
  private status: HTMLElement;
  private show: SlideShow;
  private tp: Teleprompter | null = null;

  state: "idle" | "recording" | "paused" | "preview" = "idle";
  recording: Recording | null;
  private tool: Tool = "none";
  private color = PEN_COLORS[0]!;
  private strokes: InkStroke[] = [];
  private stroke: InkStroke | null = null;
  private t0 = 0;
  private pausedAt = 0;
  private pausedTotal = 0;
  private events: Recording["events"] = [];
  private levels: number[] = [];
  private micStream: MediaStream | null = null;
  private micRec: MediaRecorder | null = null;
  private camRec: MediaRecorder | null = null;
  private micChunks: Blob[] = [];
  private camChunks: Blob[] = [];
  private analyser: AnalyserNode | null = null;
  private audioCtx: AudioContext | null = null;
  private levelTimer: ReturnType<typeof setInterval> | null = null;
  private clockTimer: ReturnType<typeof setInterval> | null = null;
  micOn = true;
  private previewAudio: HTMLAudioElement | null = null;
  private previewRaf = 0;

  constructor(
    readonly model: PresentationModel,
    private readonly opts: StudioOptions = {},
  ) {
    this.recording = opts.recording ?? null;
    this.stage = h("div", { class: "studio-stage" });
    this.ink = h("canvas", { class: "studio-ink" });
    this.selfView = h("video", { class: "studio-self", muted: true, autoplay: true, playsInline: true, hidden: true });
    this.timeline = h("canvas", { class: "studio-timeline", "aria-label": "Recording timeline" });
    this.clock = h("span", { class: "studio-clock" }, "00:00");
    this.status = h("span", { class: "studio-status", "aria-live": "polite" });
    const btn = (label: string, title: string, f: () => void, cls = "") => h("button", { class: `studio-btn ${cls}`, title, onclick: f }, label);
    this.recBtn = btn("● Record", "Start recording (R)", () => void this.toggleRecord(), "rec");
    this.pauseBtn = btn("❚❚", "Pause", () => this.togglePause());
    this.pauseBtn.disabled = true;
    const tools = h(
      "div",
      { class: "studio-tools" },
      ...PEN_COLORS.map((c) => h("button", { class: "studio-swatch", style: `background:${c}`, title: "Pen", onclick: () => this.setTool(c === "#FACC15" ? "highlighter" : "pen", c) })),
      btn("⌫", "Eraser", () => this.setTool("eraser")),
      btn("🖱", "Pointer (no ink)", () => this.setTool("none")),
      btn("🗑", "Clear ink on this slide", () => this.clearInk()),
    );
    this.el = h(
      "div",
      { class: "studio", role: "dialog", "aria-label": "Recording Studio" },
      h(
        "header",
        { class: "studio-head" },
        h("strong", {}, "Recording Studio"),
        btn("📷", "Camera on/off", () => void this.toggleCamera()),
        btn("🎙", "Microphone on/off", () => this.toggleMic()),
        btn("☰", "Teleprompter", () => this.toggleTeleprompter()),
        this.status,
        btn("✕", "Close studio", () => this.close()),
      ),
      h("div", { class: "studio-body" }, this.stage, tools),
      h(
        "footer",
        { class: "studio-foot" },
        this.recBtn,
        this.pauseBtn,
        btn("■ Stop", "Stop", () => void this.stop()),
        this.clock,
        btn("↺ Clear take", "Discard the recording", () => this.discard()),
        btn("▶ Preview", "Play the take", () => void this.preview()),
        btn("⏱ Save timings", "Use the recorded slide timings for auto-advance", () => this.saveTimings()),
        btn("🎞 Export video", "Export the take as MP4", () => this.recording && this.opts.onExport?.(this.recording)),
        this.timeline,
      ),
    );
    this.show = new SlideShow(model, {
      start: opts.start,
      host: this.stage,
      fullscreen: false,
      images: opts.images,
      charts: opts.charts,
      onNavigate: (slide, step) => this.logNav(slide, step),
      overlay: (ctx, slide) => drawInk(ctx, this.strokes, slide, Infinity),
      onExit: () => this.close(),
    });
    this.stage.append(this.ink, this.selfView);
    this.wireInk();
    if (this.recording) this.strokes = this.recording.ink.map((s) => ({ ...s }));
  }

  async open(host: HTMLElement = document.body) {
    host.append(this.el);
    await this.show.start();
    this.layoutInk();
    window.addEventListener("resize", this.onResize);
    document.addEventListener("keydown", this.onKey, true);
    this.drawTimeline();
    if (this.opts.teleprompter) this.toggleTeleprompter();
    if (camera.active) this.attachSelfView();
  }

  private onResize = () => {
    this.layoutInk();
    this.drawTimeline();
  };

  private onKey = (e: KeyboardEvent) => {
    if (e.key === "r" || e.key === "R") {
      e.preventDefault();
      e.stopPropagation();
      void this.toggleRecord();
    } else if (e.key === "Escape") {
      e.stopPropagation();
      e.preventDefault();
      if (this.tool !== "none") this.setTool("none");
      else this.close();
    }
  };

  // ---- devices -----------------------------------------------------------------------------------

  private attachSelfView() {
    // self view only when no Cameo shows the camera on the slide
    const cameo = this.model.slides.some((s) => s.shapes.some((n) => isCameo(n)));
    const v = camera.frame();
    this.selfView.hidden = cameo || !camera.active;
    if (!cameo && camera.active && v) this.selfView.srcObject = (v as HTMLVideoElement).srcObject;
  }

  private async toggleCamera() {
    if (camera.active) camera.stop();
    else await camera.start().catch((e) => this.say(`Camera: ${e instanceof Error ? e.message : e}`));
    setTimeout(() => this.attachSelfView(), 300);
  }

  private toggleMic() {
    this.micOn = !this.micOn;
    this.say(this.micOn ? "Microphone on" : "Microphone off");
  }

  private toggleTeleprompter() {
    if (this.tp) {
      this.tp.dispose();
      this.tp = null;
      return;
    }
    this.tp = new Teleprompter({ onClose: () => this.toggleTeleprompter() });
    this.tp.setText(this.model.slides[this.show.slideIndex]?.notes ?? "");
    this.el.append(this.tp.el);
  }

  private say(msg: string) {
    this.status.textContent = msg;
  }

  // ---- recording -----------------------------------------------------------------------------------

  /** ms since recording started, paused time excluded. */
  now(): number {
    if (this.state === "idle" || this.state === "preview") return 0;
    const end = this.state === "paused" ? this.pausedAt : performance.now();
    return end - this.t0 - this.pausedTotal;
  }

  private logNav(slide: number, step: number) {
    if (this.tp && step < 0) this.tp.setText(this.model.slides[slide]?.notes ?? "");
    if (this.state !== "recording") return;
    this.events.push({ t: Math.round(this.now()), slide, step });
    this.drawTimeline();
  }

  async toggleRecord() {
    if (this.state === "recording" || this.state === "paused") return this.stop();
    await this.record();
  }

  async record() {
    this.stopPreview();
    this.events = [];
    this.levels = [];
    this.strokes = [];
    this.micChunks = [];
    this.camChunks = [];
    this.pausedTotal = 0;
    if (this.micOn) {
      try {
        this.micStream = await microphone();
        this.micRec = new MediaRecorder(this.micStream, { mimeType: pickMime(["audio/webm;codecs=opus", "audio/mp4", "audio/webm"]) });
        this.micRec.ondataavailable = (e) => e.data.size && this.micChunks.push(e.data);
        this.audioCtx = new AudioContext();
        this.analyser = this.audioCtx.createAnalyser();
        this.analyser.fftSize = 1024;
        this.audioCtx.createMediaStreamSource(this.micStream).connect(this.analyser);
      } catch (e) {
        this.say(`Microphone unavailable: ${e instanceof Error ? e.message : e}`);
        this.micRec = null;
      }
    }
    const camStream = camera.active ? (camera.frame() as HTMLVideoElement | null)?.srcObject : null;
    if (camStream instanceof MediaStream) {
      try {
        this.camRec = new MediaRecorder(camStream, { mimeType: pickMime(["video/webm;codecs=vp9", "video/webm", "video/mp4"]), videoBitsPerSecond: 4e6 });
        this.camRec.ondataavailable = (e) => e.data.size && this.camChunks.push(e.data);
      } catch {
        this.camRec = null;
      }
    }
    this.t0 = performance.now();
    this.state = "recording";
    this.micRec?.start(1000);
    this.camRec?.start(1000);
    // restart the current slide so the take begins cleanly
    this.show.goTo(this.show.slideIndex);
    this.levelTimer = setInterval(() => this.sampleLevel(), 50);
    this.clockTimer = setInterval(() => this.tickClock(), 250);
    this.recBtn.textContent = "■ Stop";
    this.recBtn.classList.add("on");
    this.pauseBtn.disabled = false;
    this.say("Recording…");
  }

  private sampleLevel() {
    if (this.state !== "recording" || !this.analyser) return;
    const buf = new Float32Array(this.analyser.fftSize);
    this.analyser.getFloatTimeDomainData(buf);
    let sum = 0;
    for (const v of buf) sum += v * v;
    const rms = Math.min(1, Math.sqrt(sum / buf.length) * 4);
    this.levels.push(rms);
    this.tp?.voiceLevel(rms);
    if (this.levels.length % 10 === 0) this.drawTimeline();
  }

  private tickClock() {
    const s = Math.floor(this.now() / 1000);
    this.clock.textContent = `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
  }

  togglePause() {
    if (this.state === "recording") {
      this.state = "paused";
      this.pausedAt = performance.now();
      this.micRec?.pause();
      this.camRec?.pause();
      this.pauseBtn.textContent = "▶";
      this.say("Paused");
    } else if (this.state === "paused") {
      this.pausedTotal += performance.now() - this.pausedAt;
      this.state = "recording";
      this.micRec?.resume();
      this.camRec?.resume();
      this.pauseBtn.textContent = "❚❚";
      this.say("Recording…");
    }
  }

  async stop() {
    if (this.state !== "recording" && this.state !== "paused") return;
    if (this.state === "paused") this.togglePause();
    const duration = Math.round(this.now());
    this.state = "idle";
    const stopRec = (r: MediaRecorder | null) => (r && r.state !== "inactive" ? new Promise<void>((res) => ((r.onstop = () => res()), r.stop())) : Promise.resolve());
    await Promise.all([stopRec(this.micRec), stopRec(this.camRec)]);
    this.micStream?.getTracks().forEach((t) => t.stop());
    this.micStream = null;
    void this.audioCtx?.close();
    this.audioCtx = null;
    if (this.levelTimer) clearInterval(this.levelTimer);
    if (this.clockTimer) clearInterval(this.clockTimer);
    this.recording = {
      duration,
      events: this.events,
      ink: this.strokes,
      audio: this.micChunks.length ? new Blob(this.micChunks, { type: this.micRec?.mimeType || "audio/webm" }) : null,
      camera: this.camChunks.length ? new Blob(this.camChunks, { type: this.camRec?.mimeType || "video/webm" }) : null,
      levels: this.levels,
    };
    this.micRec = this.camRec = null;
    this.recBtn.textContent = "● Record";
    this.recBtn.classList.remove("on");
    this.pauseBtn.disabled = true;
    this.say(`Recorded ${(duration / 1000).toFixed(1)} s`);
    this.drawTimeline();
  }

  discard() {
    if (this.state === "recording" || this.state === "paused") return;
    this.recording = null;
    this.strokes = [];
    this.events = [];
    this.levels = [];
    this.show.draw();
    this.drawTimeline();
    this.say("Take cleared");
  }

  /** Recorded time per slide (ms), for auto-advance timings. */
  slideTimings(): Map<number, number> {
    const out = new Map<number, number>();
    const rec = this.recording;
    if (!rec) return out;
    const enters = rec.events.filter((e) => e.step < 0);
    enters.forEach((e, i) => {
      const end = enters[i + 1]?.t ?? rec.duration;
      out.set(e.slide, (out.get(e.slide) ?? 0) + Math.max(0, end - e.t));
    });
    return out;
  }

  saveTimings() {
    const m = this.slideTimings();
    if (!m.size) return this.say("Record a take first");
    this.opts.onSaveTimings?.(m);
    this.say(`Saved timings for ${m.size} slide(s)`);
  }

  // ---- preview ---------------------------------------------------------------------------------------

  async preview() {
    const rec = this.recording;
    if (!rec || this.state !== "idle") return;
    this.state = "preview";
    this.strokes = [];
    if (rec.audio) {
      this.previewAudio = new Audio(URL.createObjectURL(rec.audio));
      void this.previewAudio.play().catch(() => {});
    }
    const start = performance.now();
    let ev = 0;
    const tick = () => {
      if (this.state !== "preview") return;
      const t = performance.now() - start;
      while (ev < rec.events.length && rec.events[ev]!.t <= t) {
        const e = rec.events[ev++]!;
        if (e.step < 0) this.show.goTo(e.slide);
        else this.show.next();
      }
      this.strokes = rec.ink.map((s) => ({ ...s, points: s.points.filter((p) => p[2] <= t) })).filter((s) => s.points.length && (s.erased === undefined || s.erased > t));
      this.show.draw();
      this.playhead = t;
      this.drawTimeline();
      if (t < rec.duration) this.previewRaf = requestAnimationFrame(tick);
      else this.stopPreview();
    };
    this.previewRaf = requestAnimationFrame(tick);
  }

  private playhead = -1;

  private stopPreview() {
    if (this.state !== "preview") return;
    cancelAnimationFrame(this.previewRaf);
    this.previewAudio?.pause();
    this.previewAudio = null;
    this.state = "idle";
    this.playhead = -1;
    if (this.recording) this.strokes = this.recording.ink.map((s) => ({ ...s }));
    this.show.draw();
  }

  // ---- ink ----------------------------------------------------------------------------------------------

  setTool(t: Tool, color?: string) {
    this.tool = t;
    if (color) this.color = color;
    this.ink.style.pointerEvents = t === "none" ? "none" : "auto";
    this.ink.style.cursor = t === "eraser" ? "cell" : t === "none" ? "" : "crosshair";
  }

  private layoutInk() {
    const r = this.stage.getBoundingClientRect();
    this.ink.style.width = `${r.width}px`;
    this.ink.style.height = `${r.height}px`;
  }

  private toSlide(e: PointerEvent): [number, number] {
    const r = this.stage.getBoundingClientRect();
    const v = this.show.view;
    return [(e.clientX - r.left - v.ox) / v.k, (e.clientY - r.top - v.oy) / v.k];
  }

  private wireInk() {
    this.ink.style.pointerEvents = "none";
    this.ink.addEventListener("pointerdown", (e) => {
      if (this.tool === "none") return;
      this.ink.setPointerCapture(e.pointerId);
      const [x, y] = this.toSlide(e);
      const t = this.now();
      if (this.tool === "eraser") return this.eraseAt(x, y, t);
      this.stroke = { slide: this.show.slideIndex, color: this.color, width: this.tool === "highlighter" ? 14 : 3, highlighter: this.tool === "highlighter", points: [[x, y, t]] };
      this.strokes.push(this.stroke);
      this.show.draw();
    });
    this.ink.addEventListener("pointermove", (e) => {
      if (this.tool === "eraser" && e.buttons) {
        const [x, y] = this.toSlide(e);
        return this.eraseAt(x, y, this.now());
      }
      if (!this.stroke) return;
      const [x, y] = this.toSlide(e);
      this.stroke.points.push([x, y, this.now()]);
      this.show.draw();
    });
    this.ink.addEventListener("pointerup", () => {
      this.stroke = null;
      this.drawTimeline();
    });
  }

  private eraseAt(x: number, y: number, t: number) {
    const slide = this.show.slideIndex;
    for (const s of this.strokes) {
      if (s.slide !== slide || s.erased !== undefined) continue;
      if (s.points.some((p) => Math.hypot(p[0] - x, p[1] - y) < 10)) s.erased = t;
    }
    if (this.state === "idle") this.strokes = this.strokes.filter((s) => s.erased === undefined);
    this.show.draw();
  }

  private clearInk() {
    const slide = this.show.slideIndex;
    const t = this.now();
    for (const s of this.strokes) if (s.slide === slide && s.erased === undefined) s.erased = t;
    if (this.state === "idle") this.strokes = this.strokes.filter((s) => s.slide !== slide);
    this.show.draw();
  }

  // ---- timeline ------------------------------------------------------------------------------------------

  drawTimeline() {
    const c = this.timeline;
    const dpr = window.devicePixelRatio || 1;
    const w = c.clientWidth || 600;
    const hh = c.clientHeight || 72;
    c.width = Math.round(w * dpr);
    c.height = Math.round(hh * dpr);
    const ctx = c.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = "#1c1917";
    ctx.fillRect(0, 0, w, hh);
    const rec = this.state === "recording" || this.state === "paused" ? { duration: this.now(), events: this.events, levels: this.levels, ink: this.strokes, camera: this.camRec ? true : null } : this.recording;
    const lanes = ["Slides", "Mic", "Camera", "Ink"];
    const laneH = hh / lanes.length;
    ctx.font = "10px system-ui, sans-serif";
    ctx.textBaseline = "middle";
    lanes.forEach((l, i) => {
      ctx.fillStyle = "#78716c";
      ctx.fillText(l, 4, laneH * i + laneH / 2);
    });
    if (!rec || !rec.duration) return;
    const x0 = 52;
    const scale = (w - x0 - 6) / Math.max(rec.duration, 1000);
    const X = (t: number) => x0 + t * scale;
    // slides lane: segments per slide entry, ticks per build step
    const enters = rec.events.filter((e) => e.step < 0);
    enters.forEach((e, i) => {
      const end = enters[i + 1]?.t ?? rec.duration;
      ctx.fillStyle = i % 2 ? "#9a3f29" : "#C2553A";
      ctx.fillRect(X(e.t), 2, Math.max(1, (end - e.t) * scale - 1), laneH - 4);
      ctx.fillStyle = "#fff";
      ctx.fillText(String(e.slide + 1), X(e.t) + 3, laneH / 2);
    });
    ctx.fillStyle = "#fde68a";
    for (const e of rec.events) if (e.step >= 0) ctx.fillRect(X(e.t), 2, 1.5, laneH - 4);
    // mic waveform
    ctx.fillStyle = "#22c55e";
    rec.levels.forEach((v, i) => {
      const hgt = Math.max(1, v * (laneH - 4));
      ctx.fillRect(X(i * 50), laneH + laneH / 2 - hgt / 2, Math.max(1, 50 * scale), hgt);
    });
    // camera lane
    if (rec.camera) {
      ctx.fillStyle = "#3b82f6";
      ctx.fillRect(X(0), laneH * 2 + 3, rec.duration * scale, laneH - 6);
    }
    // ink lane
    ctx.fillStyle = "#e11d48";
    for (const s of rec.ink) {
      const a = s.points[0]?.[2] ?? 0;
      const b = s.points[s.points.length - 1]?.[2] ?? a;
      ctx.fillRect(X(a), laneH * 3 + 3, Math.max(2, (b - a) * scale), laneH - 6);
    }
    if (this.playhead >= 0) {
      ctx.fillStyle = "#fff";
      ctx.fillRect(X(this.playhead), 0, 1.5, hh);
    }
  }

  close() {
    if (!this.el.isConnected) return;
    if (this.state === "recording" || this.state === "paused") void this.stop().then(() => this.close());
    if (this.state === "recording" || this.state === "paused") return;
    this.stopPreview();
    window.removeEventListener("resize", this.onResize);
    document.removeEventListener("keydown", this.onKey, true);
    this.tp?.dispose();
    this.el.remove();
    this.show.exit();
    this.opts.onClose?.(this.recording);
  }
}

function pickMime(list: string[]): string {
  if (typeof MediaRecorder === "undefined") return "";
  return list.find((m) => MediaRecorder.isTypeSupported(m)) ?? "";
}
