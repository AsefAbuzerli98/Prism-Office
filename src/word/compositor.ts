/**
 * GPU page compositor (P2-07).
 *
 * Pages are rasterised once per layout version into offscreen canvases
 * (Canvas 2D, itself GPU-accelerated in WebView2/WKWebView) and composited
 * on screen by a backend: WebGPU when available, WebGL2 otherwise, plain
 * Canvas 2D as the last resort. Scrolling and caret blinking only
 * re-composite cached textures — no text is re-rasterised.
 */

export interface Tile {
  key: string;
  version: number;
  source: HTMLCanvasElement | OffscreenCanvas;
  /** Destination rectangle in CSS pixels (viewport space). */
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Backend {
  readonly kind: "webgpu" | "webgl2" | "canvas2d";
  resize(width: number, height: number, dpr: number): void;
  draw(tiles: Tile[], background: [number, number, number]): void;
  dispose(): void;
}

// ---- Canvas 2D ------------------------------------------------------------------

export class Canvas2DBackend implements Backend {
  readonly kind = "canvas2d" as const;
  private dpr = 1;
  constructor(private readonly canvas: HTMLCanvasElement, private readonly ctx: CanvasRenderingContext2D) {}
  resize(w: number, hgt: number, dpr: number) {
    this.dpr = dpr;
    this.canvas.width = Math.round(w * dpr);
    this.canvas.height = Math.round(hgt * dpr);
  }
  draw(tiles: Tile[], bg: [number, number, number]) {
    const c = this.ctx;
    c.setTransform(1, 0, 0, 1, 0, 0);
    c.fillStyle = `rgb(${bg.map((v) => Math.round(v * 255)).join(",")})`;
    c.fillRect(0, 0, this.canvas.width, this.canvas.height);
    for (const t of tiles) c.drawImage(t.source as CanvasImageSource, t.x * this.dpr, t.y * this.dpr, t.w * this.dpr, t.h * this.dpr);
  }
  dispose() {}
}

// ---- WebGL2 ---------------------------------------------------------------------

const VS = `#version 300 es
in vec2 a_pos; uniform vec4 u_rect; uniform vec2 u_view; out vec2 v_uv;
void main() {
  vec2 p = u_rect.xy + a_pos * u_rect.zw;
  vec2 clip = (p / u_view) * 2.0 - 1.0;
  gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
  v_uv = a_pos;
}`;
const FS = `#version 300 es
precision mediump float; in vec2 v_uv; uniform sampler2D u_tex; out vec4 o;
void main() { o = texture(u_tex, v_uv); }`;

export class WebGL2Backend implements Backend {
  readonly kind = "webgl2" as const;
  private prog: WebGLProgram;
  private textures = new Map<string, { tex: WebGLTexture; version: number }>();
  private loc: { rect: WebGLUniformLocation | null; view: WebGLUniformLocation | null };
  private w = 1;
  private h = 1;

  constructor(private readonly canvas: HTMLCanvasElement, private readonly gl: WebGL2RenderingContext) {
    const sh = (type: number, src: string) => {
      const s = gl.createShader(type)!;
      gl.shaderSource(s, src);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) ?? "shader");
      return s;
    };
    const p = gl.createProgram()!;
    gl.attachShader(p, sh(gl.VERTEX_SHADER, VS));
    gl.attachShader(p, sh(gl.FRAGMENT_SHADER, FS));
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p) ?? "link");
    this.prog = p;
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1]), gl.STATIC_DRAW);
    const aPos = gl.getAttribLocation(p, "a_pos");
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
    this.loc = { rect: gl.getUniformLocation(p, "u_rect"), view: gl.getUniformLocation(p, "u_view") };
  }

  resize(w: number, hgt: number, dpr: number) {
    this.canvas.width = Math.round(w * dpr);
    this.canvas.height = Math.round(hgt * dpr);
    this.w = w;
    this.h = hgt;
  }

  private texture(t: Tile): WebGLTexture {
    const gl = this.gl;
    let e = this.textures.get(t.key);
    if (!e) {
      e = { tex: gl.createTexture()!, version: -1 };
      this.textures.set(t.key, e);
    }
    gl.bindTexture(gl.TEXTURE_2D, e.tex);
    if (e.version !== t.version) {
      gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, t.source as TexImageSource);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      e.version = t.version;
    }
    return e.tex;
  }

  draw(tiles: Tile[], bg: [number, number, number]) {
    const gl = this.gl;
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.clearColor(bg[0], bg[1], bg[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(this.prog);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.uniform2f(this.loc.view, this.w, this.h);
    for (const t of tiles) {
      this.texture(t);
      gl.uniform4f(this.loc.rect, t.x, t.y, t.w, t.h);
      gl.drawArrays(gl.TRIANGLES, 0, 6);
    }
    // evict textures for pages no longer present
    const live = new Set(tiles.map((t) => t.key));
    for (const [k, e] of this.textures) if (!live.has(k) && this.textures.size > 64) { gl.deleteTexture(e.tex); this.textures.delete(k); }
  }

  dispose() {
    for (const e of this.textures.values()) this.gl.deleteTexture(e.tex);
    this.textures.clear();
  }
}

// ---- WebGPU ---------------------------------------------------------------------

const WGSL = `
struct U { rect: vec4f, view: vec2f };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var tex: texture_2d<f32>;
struct V { @builtin(position) pos: vec4f, @location(0) uv: vec2f };
@vertex fn vs(@builtin(vertex_index) i: u32) -> V {
  var q = array<vec2f, 6>(vec2f(0,0), vec2f(1,0), vec2f(0,1), vec2f(0,1), vec2f(1,0), vec2f(1,1));
  let a = q[i];
  let p = u.rect.xy + a * u.rect.zw;
  let c = (p / u.view) * 2.0 - 1.0;
  var o: V; o.pos = vec4f(c.x, -c.y, 0.0, 1.0); o.uv = a; return o;
}
@fragment fn fs(v: V) -> @location(0) vec4f { return textureSample(tex, samp, v.uv); }`;

/* Minimal structural types so this compiles without @webgpu/types. */
type GPUAny = any; // eslint-disable-line @typescript-eslint/no-explicit-any

export class WebGPUBackend implements Backend {
  readonly kind = "webgpu" as const;
  private textures = new Map<string, { tex: GPUAny; version: number; w: number; h: number; bind: GPUAny; ubo: GPUAny }>();
  private w = 1;
  private h = 1;
  /** Set after copyExternalImageToTexture fails once. */
  private cpuUpload = false;
  private constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly device: GPUAny,
    private readonly ctx: GPUAny,
    private readonly format: string,
    private readonly pipeline: GPUAny,
    private readonly sampler: GPUAny,
  ) {}

  static async create(canvas: HTMLCanvasElement): Promise<WebGPUBackend | null> {
    const gpu = (navigator as GPUAny).gpu;
    if (!gpu) return null;
    const adapter = await gpu.requestAdapter();
    if (!adapter) return null;
    const device = await adapter.requestDevice();
    const ctx = canvas.getContext("webgpu" as never) as GPUAny;
    if (!ctx) return null;
    const format = gpu.getPreferredCanvasFormat();
    ctx.configure({ device, format, alphaMode: "premultiplied" });
    const module = device.createShaderModule({ code: WGSL });
    const pipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: { module, entryPoint: "vs" },
      fragment: { module, entryPoint: "fs", targets: [{ format, blend: { color: { srcFactor: "one", dstFactor: "one-minus-src-alpha" }, alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha" } } }] },
      primitive: { topology: "triangle-list" },
    });
    const sampler = device.createSampler({ magFilter: "linear", minFilter: "linear" });
    return new WebGPUBackend(canvas, device, ctx, format, pipeline, sampler);
  }

  resize(w: number, hgt: number, dpr: number) {
    this.canvas.width = Math.round(w * dpr);
    this.canvas.height = Math.round(hgt * dpr);
    this.w = w;
    this.h = hgt;
  }

  private entry(t: Tile) {
    const d = this.device;
    let e = this.textures.get(t.key);
    const sw = (t.source as HTMLCanvasElement).width;
    const sh = (t.source as HTMLCanvasElement).height;
    if (!e || e.w !== sw || e.h !== sh) {
      e?.tex.destroy();
      const tex = d.createTexture({ size: [sw, sh], format: "rgba8unorm", usage: 0x04 | 0x02 | 0x10 /* TEXTURE_BINDING | COPY_DST | RENDER_ATTACHMENT */ });
      const ubo = d.createBuffer({ size: 32, usage: 0x40 | 0x08 /* UNIFORM | COPY_DST */ });
      const bind = d.createBindGroup({
        layout: this.pipeline.getBindGroupLayout(0),
        entries: [{ binding: 0, resource: { buffer: ubo } }, { binding: 1, resource: this.sampler }, { binding: 2, resource: tex.createView() }],
      });
      e = { tex, version: -1, w: sw, h: sh, bind, ubo };
      this.textures.set(t.key, e);
    }
    if (e.version !== t.version) {
      try {
        if (this.cpuUpload) throw new Error("cpu");
        d.queue.copyExternalImageToTexture({ source: t.source }, { texture: e.tex, premultipliedAlpha: true }, [sw, sh]);
      } catch {
        // Some drivers can't import canvases directly; upload the pixels instead.
        this.cpuUpload = true;
        const ctx = (t.source as HTMLCanvasElement).getContext("2d") as CanvasRenderingContext2D | null;
        if (!ctx) throw new Error("page raster has no 2D context");
        const px = ctx.getImageData(0, 0, sw, sh);
        d.queue.writeTexture({ texture: e.tex }, px.data, { bytesPerRow: sw * 4, rowsPerImage: sh }, [sw, sh]);
      }
      e.version = t.version;
    }
    d.queue.writeBuffer(e.ubo, 0, new Float32Array([t.x, t.y, t.w, t.h, this.w, this.h, 0, 0]));
    return e;
  }

  draw(tiles: Tile[], bg: [number, number, number]) {
    const entries = tiles.map((t) => this.entry(t));
    const enc = this.device.createCommandEncoder();
    const pass = enc.beginRenderPass({
      colorAttachments: [{ view: this.ctx.getCurrentTexture().createView(), clearValue: { r: bg[0], g: bg[1], b: bg[2], a: 1 }, loadOp: "clear", storeOp: "store" }],
    });
    pass.setPipeline(this.pipeline);
    for (const e of entries) {
      pass.setBindGroup(0, e.bind);
      pass.draw(6);
    }
    pass.end();
    this.device.queue.submit([enc.finish()]);
    void this.format;
  }

  dispose() {
    for (const e of this.textures.values()) e.tex.destroy();
    this.textures.clear();
  }
}

/**
 * Pick a backend for a canvas. `auto` uses WebGL2 (then Canvas 2D); the
 * WebGPU backend is opt-in (`localStorage["prism.compositor"] = "webgpu"`)
 * until it has been validated on real GPUs — in headless SwiftShader it
 * presents nothing.
 */
export async function createBackend(canvas: HTMLCanvasElement, prefer: "auto" | "webgpu" | "webgl2" | "canvas2d" = "auto"): Promise<Backend | null> {
  if (prefer === "webgpu") {
    try {
      const gpu = await WebGPUBackend.create(canvas);
      if (gpu) return gpu;
    } catch {
      /* fall through */
    }
  }
  if (prefer !== "canvas2d") {
    try {
      const gl = canvas.getContext("webgl2", { premultipliedAlpha: true, alpha: false });
      if (gl) return new WebGL2Backend(canvas, gl);
    } catch {
      /* fall through */
    }
  }
  const ctx = canvas.getContext("2d");
  return ctx ? new Canvas2DBackend(canvas, ctx) : null;
}

/** Page raster cache: re-rasterises a page only when its version changes. */
export class PageCache {
  private pages = new Map<number, { canvas: HTMLCanvasElement | OffscreenCanvas; version: number; scale: number }>();
  rasterised = 0;

  constructor(private readonly make: (w: number, h: number) => HTMLCanvasElement | OffscreenCanvas) {}

  get(index: number, version: number, scale: number, w: number, h: number, paint: (ctx: CanvasRenderingContext2D) => void) {
    let e = this.pages.get(index);
    if (e && e.version === version && e.scale === scale) return e.canvas;
    const canvas = e && e.scale === scale ? e.canvas : this.make(Math.ceil(w * scale), Math.ceil(h * scale));
    const ctx = canvas.getContext("2d") as CanvasRenderingContext2D | null;
    if (ctx) {
      ctx.setTransform(scale, 0, 0, scale, 0, 0);
      ctx.clearRect(0, 0, w, h);
      paint(ctx);
    }
    this.rasterised++;
    e = { canvas, version, scale };
    this.pages.set(index, e);
    return canvas;
  }

  /** Drop pages beyond the document length. */
  trim(count: number) {
    for (const k of [...this.pages.keys()]) if (k >= count) this.pages.delete(k);
  }
}
