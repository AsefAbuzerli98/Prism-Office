/**
 * Live Presenting (P4-14): the audience scans a QR code and follows the
 * slides on their phones with live subtitles. Speech is recognised in the
 * webview (where available), final sentences are translated with the AI
 * service into the languages viewers picked, and pushed to the host's LAN
 * server (src-tauri/src/live.rs).
 */
import { h } from "../core/dom";
import type { AiRequest, HostBridge } from "../core/host";
import { getLocale } from "../core/i18n";
import { errorText, overlay } from "../word/ai-panel";
import type { PresentationModel } from "./model";
import { encodeQr, qrSvg } from "./qr";
import { renderSlideToCanvas, type RenderOptions } from "./render";
import { SlideShow, type ShowOptions } from "./show";

export function translateRequest(text: string, langs: string[]): AiRequest {
  return {
    system: 'You translate live presentation subtitles. Answer with one JSON object mapping each requested language code to the translation, e.g. {"de": "…"} — no commentary.',
    prompt: `Languages: ${langs.join(", ")}\nText: ${text}`,
    maxTokens: 1200,
    effort: "low",
  };
}

export function parseTranslations(text: string, langs: string[]): Record<string, string> {
  const a = text.indexOf("{");
  const b = text.lastIndexOf("}");
  if (a < 0 || b <= a) return {};
  try {
    const raw = JSON.parse(text.slice(a, b + 1)) as Record<string, unknown>;
    const out: Record<string, string> = {};
    for (const l of langs) if (typeof raw[l] === "string") out[l] = raw[l] as string;
    return out;
  } catch {
    return {};
  }
}

type Recognition = {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  onresult: ((e: { resultIndex: number; results: ArrayLike<{ isFinal: boolean; 0: { transcript: string } }> }) => void) | null;
  onend: (() => void) | null;
  onerror: ((e: unknown) => void) | null;
  start(): void;
  stop(): void;
};

export class LiveSession {
  private rec: Recognition | null = null;
  private active = false;
  private statusTimer: ReturnType<typeof setInterval> | null = null;
  languages: string[] = [];
  viewers = 0;
  subtitleEl: HTMLElement | null = null;
  onStatus?: () => void;

  constructor(
    private readonly host: HostBridge,
    private readonly model: PresentationModel,
    private readonly render: Pick<RenderOptions, "images" | "charts">,
  ) {}

  async start() {
    const info = await this.host.liveStart();
    this.active = true;
    this.statusTimer = setInterval(async () => {
      const st = await this.host.liveStatus().catch(() => null);
      if (!st) return;
      this.languages = st.languages.filter((l) => l && l !== getLocale().slice(0, 2));
      this.viewers = st.viewers;
      this.onStatus?.();
    }, 2000);
    return info;
  }

  /** Push the slide image the audience sees. */
  async pushSlide(index: number) {
    if (!this.active) return;
    const slide = this.model.slides[index];
    if (!slide) return;
    const c = typeof OffscreenCanvas !== "undefined" ? new OffscreenCanvas(1, 1) : document.createElement("canvas");
    renderSlideToCanvas(c, this.model, slide, 1280, 1, { ...this.render, slideNumber: index + 1 });
    const blob = c instanceof OffscreenCanvas ? await c.convertToBlob({ type: "image/png" }) : await new Promise<Blob | null>((r) => (c as HTMLCanvasElement).toBlob(r, "image/png"));
    if (!blob) return;
    const visible = this.model.slides.filter((s) => !s.hidden).length;
    await this.host.liveSlide(new Uint8Array(await blob.arrayBuffer()), index, visible).catch(() => {});
  }

  /** Start speech recognition → subtitles (+ translations). */
  startSubtitles(lang = getLocale()): boolean {
    const g = globalThis as unknown as { SpeechRecognition?: new () => Recognition; webkitSpeechRecognition?: new () => Recognition };
    const Rec = g.SpeechRecognition ?? g.webkitSpeechRecognition;
    if (!Rec) return false;
    const r = new Rec();
    r.lang = lang === "az" ? "az-AZ" : lang;
    r.continuous = true;
    r.interimResults = true;
    r.onresult = (e) => {
      let interim = "";
      for (let i = e.resultIndex; i < e.results.length; i++) {
        const res = e.results[i]!;
        if (res.isFinal) void this.subtitle(res[0].transcript.trim(), true);
        else interim += res[0].transcript;
      }
      if (interim) void this.subtitle(interim.trim(), false);
    };
    r.onerror = () => {};
    r.onend = () => {
      if (this.active) {
        try {
          r.start();
        } catch {
          /* running */
        }
      }
    };
    r.start();
    this.rec = r;
    return true;
  }

  async subtitle(text: string, final: boolean) {
    if (!text) return;
    if (this.subtitleEl) {
      this.subtitleEl.textContent = text;
      this.subtitleEl.classList.toggle("interim", !final);
    }
    let translations: Record<string, string> = {};
    if (final && this.languages.length) {
      try {
        const res = await this.host.aiComplete(translateRequest(text, this.languages));
        translations = parseTranslations(res.text, this.languages);
      } catch {
        /* AI off or offline: viewers get the original */
      }
    }
    await this.host.liveSubtitle(text, translations, final).catch(() => {});
  }

  async stop() {
    this.active = false;
    this.rec?.stop();
    this.rec = null;
    if (this.statusTimer) clearInterval(this.statusTimer);
    await this.host.liveStop().catch(() => {});
  }
}

/** QR dialog, then the slideshow with audience sync and subtitles. */
export async function presentLive(root: HTMLElement, host: HostBridge, model: PresentationModel, start: number, show: Omit<ShowOptions, "start" | "onNavigate" | "onExit">, onError: (m: string) => void) {
  const session = new LiveSession(host, model, show);
  let info;
  try {
    info = await session.start();
  } catch (e) {
    return onError(errorText(e));
  }
  const qr = encodeQr(info.url, "M");
  const viewers = h("p", { class: "muted small" }, "Waiting for the audience…");
  session.onStatus = () => (viewers.textContent = `${session.viewers} viewer(s) connected${session.languages.length ? ` · subtitles in: ${session.languages.join(", ")}` : ""}`);
  const subs = h("input", { type: "checkbox", checked: true, "aria-label": "Live subtitles" });
  const qrBox = h("div", { class: "live-qr" });
  qrBox.innerHTML = qrSvg(qr, 260);
  const begin = h(
    "button",
    {
      class: "btn",
      onclick: async () => {
        el.remove();
        const s = new SlideShow(model, {
          ...show,
          start,
          onNavigate: (slide, step) => {
            if (step < 0) void session.pushSlide(slide);
          },
          onExit: () => void session.stop(),
        });
        await s.start();
        if (subs.checked) {
          const line = h("div", { class: "live-subtitle", "aria-live": "polite" });
          s.el.append(line);
          session.subtitleEl = line;
          if (!session.startSubtitles()) line.textContent = "Speech recognition is not available in this webview — slides are shared without subtitles.";
        }
        // Q shows the join code on screen during the show
        const join = h("div", { class: "live-join", hidden: true });
        join.innerHTML = qrSvg(qr, 320);
        join.append(h("p", {}, info.url));
        s.el.append(join);
        s.el.addEventListener("keydown", (e) => {
          if (e.key === "q" || e.key === "Q") join.hidden = !join.hidden;
        });
      },
    },
    "Start presenting",
  );
  const el = overlay(
    root,
    "Present live",
    h("p", {}, "Your audience scans this code (same Wi-Fi network) to follow the slides with live subtitles in their language."),
    qrBox,
    h("p", { class: "live-url" }, info.url),
    viewers,
    h("label", { class: "radio" }, subs, h("span", {}, "Live subtitles (speech recognition; translated with the AI service)")),
    h("div", { class: "dialog-actions" }, begin, h("button", { class: "btn btn-quiet", onclick: () => (el.remove(), void session.stop()) }, "Cancel")),
  );
}
