/**
 * Video export (P4-11): the slideshow is rendered offline frame by frame —
 * transitions, Morph, build animations, recorded ink and narration — and
 * encoded with WebCodecs (H.264, VP9 fallback; AAC, Opus fallback) into an
 * MP4 with one chapter bookmark per slide.
 */
import { buildSteps, evaluate, type AnimBox, type Step } from "./anim";
import type { PresentationModel, Slide } from "./model";
import { muxMp4, type AudioTrack, type Chapter, type Sample, type VideoTrack } from "./mp4";
import { drawSlide, renderSlideToCanvas, type RenderOptions, type ShapeAnim } from "./render";
import { resolveShape, slideContext } from "./resolve";
import { EMU_PER_PT } from "./text";
import { drawMorph, drawTransition, transitionDuration, type Raster } from "./transition";

/** A recorded take (Recording Studio). Times are ms from the start. */
export interface Recording {
  duration: number;
  /** Navigation: slide shown / build step played at `t` (step -1 = slide entered). */
  events: { t: number; slide: number; step: number }[];
  ink: InkStroke[];
  audio: Blob | null;
  camera: Blob | null;
  /** Microphone level per 50 ms (waveform). */
  levels: number[];
}

export interface InkStroke {
  slide: number;
  color: string;
  /** Points. */
  width: number;
  highlighter: boolean;
  /** [x, y] in slide points with the time (ms) each point was drawn. */
  points: [number, number, number][];
  /** Erased at (ms), if the eraser removed it. */
  erased?: number;
}

export type Segment =
  | { kind: "trans"; slide: number; from: number; start: number; dur: number }
  | { kind: "step"; slide: number; step: number; start: number; dur: number }
  | { kind: "hold"; slide: number; step: number; start: number; dur: number };

export interface ExportOptions {
  /** 1080 or 2160 lines. */
  height?: 1080 | 2160;
  fps?: number;
  /** Seconds a slide stays after its last build when there is no recording. */
  slideSeconds?: number;
  /** Pause after each click step (s). */
  stepSeconds?: number;
  images?: (part: string) => CanvasImageSource | null;
  charts?: RenderOptions["charts"];
  recording?: Recording | null;
  onProgress?: (done: number, total: number) => void;
  signal?: AbortSignal;
}

const pt = (emu: number) => emu / EMU_PER_PT;

/** Slide order of the show (hidden slides skipped). */
function showOrder(model: PresentationModel) {
  return model.slides.map((s, i) => (s.hidden ? -1 : i)).filter((i) => i >= 0);
}

/** Timeline from a recording, or from default timings. */
export function buildTimeline(model: PresentationModel, opts: Pick<ExportOptions, "slideSeconds" | "stepSeconds" | "recording">): Segment[] {
  const segs: Segment[] = [];
  const rec = opts.recording;
  if (rec && rec.events.length) {
    let prevSlide = -1;
    rec.events.forEach((e, i) => {
      const end = rec.events[i + 1]?.t ?? rec.duration;
      const slide = model.slides[e.slide];
      if (!slide) return;
      let t = e.t;
      if (e.step < 0) {
        const td = prevSlide >= 0 ? transitionDuration(slide.transition) : 0;
        if (td > 0) {
          segs.push({ kind: "trans", slide: e.slide, from: prevSlide, start: t, dur: Math.min(td, end - t) });
          t += td;
        }
        const steps = buildSteps(slide.timing);
        if (steps[0]?.auto) {
          segs.push({ kind: "step", slide: e.slide, step: 0, start: t, dur: steps[0].duration });
          t += steps[0].duration;
          if (end > t) segs.push({ kind: "hold", slide: e.slide, step: 0, start: t, dur: end - t });
        } else if (end > t) segs.push({ kind: "hold", slide: e.slide, step: -1, start: t, dur: end - t });
        prevSlide = e.slide;
      } else {
        const steps = buildSteps(slide.timing);
        const d = Math.min(steps[e.step]?.duration ?? 0, end - t);
        segs.push({ kind: "step", slide: e.slide, step: e.step, start: t, dur: d });
        if (end > t + d) segs.push({ kind: "hold", slide: e.slide, step: e.step, start: t + d, dur: end - t - d });
      }
    });
    return segs;
  }
  let t = 0;
  let prev = -1;
  const hold = (opts.slideSeconds ?? 5) * 1000;
  const pause = (opts.stepSeconds ?? 1.5) * 1000;
  for (const i of showOrder(model)) {
    const s = model.slides[i]!;
    const td = prev >= 0 ? transitionDuration(s.transition) : 0;
    if (td > 0) {
      segs.push({ kind: "trans", slide: i, from: prev, start: t, dur: td });
      t += td;
    }
    const steps = buildSteps(s.timing);
    let step = -1;
    steps.forEach((st, k) => {
      segs.push({ kind: "step", slide: i, step: k, start: t, dur: st.duration });
      t += st.duration;
      step = k;
      if (k < steps.length - 1) {
        segs.push({ kind: "hold", slide: i, step: k, start: t, dur: pause });
        t += pause;
      }
    });
    const stay = s.transition?.advanceAfter ?? hold;
    segs.push({ kind: "hold", slide: i, step, start: t, dur: stay });
    t += stay;
    prev = i;
  }
  return segs;
}

/** One chapter per slide (title placeholder text, else "Slide n"). */
export function chaptersOf(model: PresentationModel, segs: Segment[]): Chapter[] {
  const out: Chapter[] = [];
  let last = -1;
  for (const s of segs) {
    if (s.slide === last) continue;
    last = s.slide;
    const slide = model.slides[s.slide]!;
    const title = slide.shapes
      .filter((n) => n.ph && ["title", "ctrTitle"].includes(n.ph.kind ?? ""))
      .map((n) => (n.type === "shape" ? (n.text?.paragraphs ?? []).map((p) => (p.runs ?? []).map((r) => (r.t === "break" ? " " : r.text)).join("")).join(" ") : ""))
      .join(" ")
      .trim();
    out.push({ start: s.start, title: title || `Slide ${s.slide + 1}` });
  }
  return out;
}

/** Renders show frames at a time. */
export class FrameRenderer {
  private steps = new Map<number, Step[]>();
  private rasters = new Map<string, Raster>();
  constructor(
    private readonly model: PresentationModel,
    private readonly segs: Segment[],
    private readonly width: number,
    private readonly opts: Pick<ExportOptions, "images" | "charts" | "recording"> & { camera?: () => CanvasImageSource | null },
  ) {}

  get duration() {
    const last = this.segs[this.segs.length - 1];
    return last ? last.start + last.dur : 0;
  }

  private stepsOf(i: number) {
    let s = this.steps.get(i);
    if (!s) this.steps.set(i, (s = buildSteps(this.model.slides[i]?.timing)));
    return s;
  }

  private box(slide: Slide) {
    return (id: number): AnimBox | null => {
      const n = slide.shapes.find((s) => s.id === id);
      const x = n ? (n.xfrm ?? resolveShape(n, slideContext(this.model, slide), this.model).xfrm) : null;
      return x ? { slideW: pt(this.model.width), slideH: pt(this.model.height), x: pt(x.x), y: pt(x.y), w: pt(x.cx), h: pt(x.cy) } : null;
    };
  }

  private states(i: number, step: number, t: number): Map<number, ShapeAnim> {
    const steps = this.stepsOf(i);
    return steps.length ? evaluate(steps, step, t, this.box(this.model.slides[i]!)) : new Map();
  }

  private ro(i: number, anim?: Map<number, ShapeAnim>): RenderOptions {
    return { images: this.opts.images, charts: this.opts.charts, camera: this.opts.camera, slideNumber: i + 1, anim: anim ? (id) => anim.get(id) ?? null : undefined };
  }

  private raster(i: number, step: number): Raster {
    const key = `${i}:${step}`;
    let r = this.rasters.get(key);
    if (!r) {
      r = typeof OffscreenCanvas !== "undefined" ? new OffscreenCanvas(1, 1) : document.createElement("canvas");
      renderSlideToCanvas(r, this.model, this.model.slides[i]!, this.width, 1, this.ro(i, this.states(i, step, Infinity)));
      this.rasters.set(key, r);
    }
    return r;
  }

  segmentAt(t: number): Segment | null {
    let lo = 0;
    let hi = this.segs.length - 1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      const s = this.segs[mid]!;
      if (t < s.start) hi = mid - 1;
      else if (t >= s.start + s.dur) lo = mid + 1;
      else return s;
    }
    return this.segs[this.segs.length - 1] ?? null;
  }

  /** Draw the frame at `t` ms into a canvas of `w`×`h` device pixels. */
  draw(ctx: CanvasRenderingContext2D, t: number, w: number, h: number) {
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.fillStyle = "#000";
    ctx.fillRect(0, 0, w, h);
    const seg = this.segmentAt(t);
    if (!seg) return;
    const W = pt(this.model.width);
    const H = pt(this.model.height);
    const k = Math.min(w / W, h / H);
    const ox = (w - W * k) / 2;
    const oy = (h - H * k) / 2;
    const slide = this.model.slides[seg.slide]!;
    if (seg.kind === "trans") {
      const p = Math.min(1, (t - seg.start) / Math.max(1, seg.dur));
      const tr = slide.transition!;
      const fromSteps = this.stepsOf(seg.from);
      if (tr.kind === "morph") {
        ctx.setTransform(k, 0, 0, k, ox, oy);
        drawMorph(ctx, this.model, this.model.slides[seg.from]!, slide, p, (n, s) => n.xfrm ?? resolveShape(n, slideContext(this.model, s), this.model).xfrm, this.ro(seg.slide));
      } else {
        ctx.save();
        ctx.translate(ox, oy);
        ctx.beginPath();
        ctx.rect(0, 0, W * k, H * k);
        ctx.clip();
        drawTransition(ctx, tr.kind, tr.dir, tr.option, p, this.raster(seg.from, fromSteps.length - 1), this.raster(seg.slide, -1), W * k, H * k);
        ctx.restore();
      }
    } else {
      const local = seg.kind === "step" ? t - seg.start : Infinity;
      ctx.setTransform(k, 0, 0, k, ox, oy);
      drawSlide(ctx, this.model, slide, this.ro(seg.slide, this.states(seg.slide, seg.step, local)));
    }
    // recorded ink on top (slide points)
    const ink = this.opts.recording?.ink ?? [];
    if (ink.length) {
      ctx.setTransform(k, 0, 0, k, ox, oy);
      drawInk(ctx, ink, seg.slide, t);
    }
  }
}

/** Draw ink strokes of a slide as they were at time `t`. */
export function drawInk(ctx: CanvasRenderingContext2D, strokes: InkStroke[], slide: number, t: number) {
  ctx.save();
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  for (const s of strokes) {
    if (s.slide !== slide || (s.erased !== undefined && s.erased <= t)) continue;
    const pts = s.points.filter((p) => p[2] <= t);
    if (pts.length < 1) continue;
    ctx.strokeStyle = s.color;
    ctx.globalAlpha = s.highlighter ? 0.35 : 1;
    ctx.lineWidth = s.width;
    ctx.beginPath();
    pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)));
    if (pts.length === 1) ctx.lineTo(pts[0]![0] + 0.01, pts[0]![1]);
    ctx.stroke();
  }
  ctx.restore();
}

/** Pick the best supported encoder configuration. */
async function videoConfig(width: number, height: number, fps: number): Promise<{ config: VideoEncoderConfig; codec: "avc1" | "vp09" }> {
  const bitrate = height >= 2160 ? 30e6 : 10e6;
  const tries: [string, "avc1" | "vp09"][] = height >= 2160 ? [["avc1.640033", "avc1"], ["avc1.640034", "avc1"], ["vp09.00.51.08", "vp09"]] : [["avc1.640028", "avc1"], ["avc1.4d0028", "avc1"], ["vp09.00.40.08", "vp09"]];
  for (const [codec, kind] of tries) {
    const config: VideoEncoderConfig = { codec, width, height, bitrate, framerate: fps, latencyMode: "quality", ...(kind === "avc1" ? { avc: { format: "avc" } } : {}) } as VideoEncoderConfig;
    try {
      if ((await VideoEncoder.isConfigSupported(config)).supported) return { config, codec: kind };
    } catch {
      /* next */
    }
  }
  throw new Error("This system has no H.264 or VP9 video encoder for WebCodecs");
}

/** Encode recorded narration (any format the webview decodes) as AAC or Opus. */
async function encodeAudio(blob: Blob, duration: number, signal?: AbortSignal): Promise<AudioTrack | null> {
  if (typeof AudioEncoder === "undefined") return null;
  const sampleRate = 48000;
  const ctx = new OfflineAudioContext(1, Math.max(1, Math.ceil((duration / 1000) * sampleRate)), sampleRate);
  const buf = await ctx.decodeAudioData(await blob.arrayBuffer());
  const src = ctx.createBufferSource();
  src.buffer = buf;
  src.connect(ctx.destination);
  src.start();
  const pcm = (await ctx.startRendering()).getChannelData(0);
  let codec: "mp4a" | "opus" = "mp4a";
  let config: AudioEncoderConfig = { codec: "mp4a.40.2", sampleRate, numberOfChannels: 1, bitrate: 128000 };
  if (!(await AudioEncoder.isConfigSupported(config).catch(() => ({ supported: false }))).supported) {
    codec = "opus";
    config = { codec: "opus", sampleRate, numberOfChannels: 1, bitrate: 96000 };
    if (!(await AudioEncoder.isConfigSupported(config).catch(() => ({ supported: false }))).supported) return null;
  }
  const samples: Sample[] = [];
  let description: Uint8Array | undefined;
  const enc = new AudioEncoder({
    output: (chunk, meta) => {
      const data = new Uint8Array(chunk.byteLength);
      chunk.copyTo(data);
      samples.push({ data, timestamp: chunk.timestamp, duration: chunk.duration ?? 20000, key: true });
      const d = meta?.decoderConfig?.description;
      if (d && !description) description = new Uint8Array(d instanceof ArrayBuffer ? d : (d as ArrayBufferView).buffer.slice(0));
    },
    error: (e) => {
      throw e;
    },
  });
  enc.configure(config);
  const frame = 4800; // 100 ms
  for (let i = 0; i < pcm.length; i += frame) {
    if (signal?.aborted) throw new DOMException("Export cancelled", "AbortError");
    const part = pcm.subarray(i, Math.min(pcm.length, i + frame));
    const ad = new AudioData({ format: "f32-planar", sampleRate, numberOfFrames: part.length, numberOfChannels: 1, timestamp: Math.round((i / sampleRate) * 1e6), data: part.slice() });
    enc.encode(ad);
    ad.close();
  }
  await enc.flush();
  enc.close();
  return { codec, sampleRate, channels: 1, description, samples };
}

/** Render and encode the presentation; resolves to MP4 bytes. */
export async function exportVideo(model: PresentationModel, opts: ExportOptions = {}): Promise<Uint8Array> {
  if (typeof VideoEncoder === "undefined") throw new Error("Video export needs WebCodecs (not available in this webview)");
  const height = opts.height ?? 1080;
  const width = Math.round((height * model.width) / model.height / 2) * 2;
  const fps = opts.fps ?? 30;
  const segs = buildTimeline(model, opts);
  // camera track of a recording drives Cameo shapes
  let camVideo: HTMLVideoElement | null = null;
  if (opts.recording?.camera && typeof document !== "undefined") {
    camVideo = document.createElement("video");
    camVideo.muted = true;
    camVideo.src = URL.createObjectURL(opts.recording.camera);
    await new Promise((r) => ((camVideo!.onloadeddata = r), (camVideo!.onerror = r)));
  }
  const fr = new FrameRenderer(model, segs, width, { ...opts, camera: () => (camVideo && camVideo.readyState >= 2 ? camVideo : null) });
  const total = Math.max(1, Math.ceil((fr.duration / 1000) * fps));
  const { config, codec } = await videoConfig(width, height, fps);
  const samples: Sample[] = [];
  let description: Uint8Array | undefined;
  let failure: unknown = null;
  const enc = new VideoEncoder({
    output: (chunk, meta) => {
      const data = new Uint8Array(chunk.byteLength);
      chunk.copyTo(data);
      samples.push({ data, timestamp: chunk.timestamp, duration: chunk.duration ?? Math.round(1e6 / fps), key: chunk.type === "key" });
      const d = meta?.decoderConfig?.description;
      if (d && !description) description = new Uint8Array(d instanceof ArrayBuffer ? d.slice(0) : (d as ArrayBufferView).buffer.slice(0));
    },
    error: (e) => (failure = e),
  });
  enc.configure(config);
  const canvas = new OffscreenCanvas(width, height);
  const ctx = canvas.getContext("2d") as unknown as CanvasRenderingContext2D;
  for (let i = 0; i < total; i++) {
    if (opts.signal?.aborted) {
      enc.close();
      throw new DOMException("Export cancelled", "AbortError");
    }
    if (failure) throw failure;
    const t = (i * 1000) / fps;
    if (camVideo) {
      camVideo.currentTime = t / 1000;
      await new Promise((r) => {
        camVideo!.onseeked = r;
        setTimeout(r, 200);
      });
    }
    fr.draw(ctx, t, width, height);
    const frame = new VideoFrame(canvas, { timestamp: Math.round((i * 1e6) / fps), duration: Math.round(1e6 / fps) });
    enc.encode(frame, { keyFrame: i % (fps * 2) === 0 });
    frame.close();
    while (enc.encodeQueueSize > 6) await new Promise((r) => setTimeout(r, 2));
    if (i % 10 === 0) opts.onProgress?.(i, total);
  }
  await enc.flush();
  enc.close();
  if (failure) throw failure;
  if (camVideo) URL.revokeObjectURL(camVideo.src);
  const video: VideoTrack = { codec, width, height, description, samples };
  const audio = opts.recording?.audio ? await encodeAudio(opts.recording.audio, fr.duration, opts.signal).catch(() => null) : null;
  opts.onProgress?.(total, total);
  return muxMp4(video, audio, chaptersOf(model, segs));
}
