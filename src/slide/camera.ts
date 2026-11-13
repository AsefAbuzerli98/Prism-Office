/**
 * Camera and microphone capture for Cameo, the Recording Studio, Speaker
 * Coach and live subtitles. Uses the webview's media stack — AVFoundation in
 * WKWebView (macOS) and Media Foundation in WebView2 (Windows) — so the OS
 * permission prompts and device lists are the native ones.
 */

export interface CaptureDevice {
  id: string;
  label: string;
  kind: "videoinput" | "audioinput";
}

class Camera {
  private stream: MediaStream | null = null;
  private video: HTMLVideoElement | null = null;
  private starting: Promise<MediaStream> | null = null;
  deviceId: string | undefined;
  error: string | null = null;
  private listeners = new Set<() => void>();

  get active() {
    return !!this.stream;
  }

  onChange(f: () => void) {
    this.listeners.add(f);
    return () => this.listeners.delete(f);
  }

  private emit() {
    for (const f of this.listeners) f();
  }

  async start(deviceId?: string): Promise<MediaStream> {
    if (this.stream && (!deviceId || deviceId === this.deviceId)) return this.stream;
    if (this.starting && !deviceId) return this.starting;
    this.stop();
    if (!navigator.mediaDevices?.getUserMedia) throw new Error("Camera capture is not available in this environment");
    this.starting = navigator.mediaDevices
      .getUserMedia({ video: { deviceId: deviceId ? { exact: deviceId } : undefined, width: { ideal: 1280 }, height: { ideal: 720 } }, audio: false })
      .then(async (s) => {
        this.stream = s;
        this.deviceId = deviceId ?? s.getVideoTracks()[0]?.getSettings().deviceId;
        const v = document.createElement("video");
        v.muted = true;
        v.playsInline = true;
        v.srcObject = s;
        await v.play().catch(() => {});
        this.video = v;
        this.error = null;
        this.emit();
        return s;
      })
      .catch((e: unknown) => {
        this.error = e instanceof Error ? e.message : String(e);
        this.emit();
        throw e;
      })
      .finally(() => {
        this.starting = null;
      });
    return this.starting;
  }

  stop() {
    this.stream?.getTracks().forEach((t) => t.stop());
    this.stream = null;
    this.video = null;
    this.emit();
  }

  /** Current frame source, or null until the first frame arrives. */
  frame(): HTMLVideoElement | null {
    const v = this.video;
    return v && v.readyState >= 2 && v.videoWidth ? v : null;
  }

  async devices(): Promise<CaptureDevice[]> {
    const all = (await navigator.mediaDevices?.enumerateDevices?.()) ?? [];
    return all
      .filter((d) => d.kind === "videoinput" || d.kind === "audioinput")
      .map((d, i) => ({ id: d.deviceId, kind: d.kind as CaptureDevice["kind"], label: d.label || `${d.kind === "videoinput" ? "Camera" : "Microphone"} ${i + 1}` }));
  }
}

/** Shared camera (one device feeds every Cameo on screen). */
export const camera = new Camera();

/** Microphone stream for narration / coach / subtitles. */
export async function microphone(deviceId?: string): Promise<MediaStream> {
  if (!navigator.mediaDevices?.getUserMedia) throw new Error("Microphone capture is not available in this environment");
  return navigator.mediaDevices.getUserMedia({ audio: { deviceId: deviceId ? { exact: deviceId } : undefined, echoCancellation: true, noiseSuppression: true }, video: false });
}

/** Is this node a Cameo (live camera) object? */
export function isCameo(n: object): boolean {
  const p = (n as { prism?: unknown }).prism;
  return !!(p && typeof p === "object" && (p as { cameo?: unknown }).cameo);
}

export function cameoOptions(n: { prism?: unknown }): { mirror: boolean; zoom: number } {
  const c = (n.prism as { cameo?: { mirror?: boolean; zoom?: number } } | undefined)?.cameo ?? {};
  return { mirror: c.mirror !== false, zoom: c.zoom ?? 1 };
}
