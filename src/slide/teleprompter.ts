/**
 * Teleprompter (P4-10): speaker notes scroll in a narrow band at the top
 * centre of the window — right under a laptop's webcam, so the presenter
 * keeps eye contact while reading. Adjustable speed and size, pause,
 * mirror (for beam-splitter glass), and voice-paced auto-pause.
 */
import { h } from "../core/dom";

export interface TeleprompterOptions {
  /** Pixels per second. */
  speed?: number;
  fontSize?: number;
  mirror?: boolean;
  onClose?: () => void;
}

export class Teleprompter {
  readonly el: HTMLElement;
  private text: HTMLElement;
  private viewport: HTMLElement;
  private speedLabel: HTMLElement;
  private playBtn: HTMLButtonElement;
  speed: number;
  fontSize: number;
  mirror: boolean;
  playing = false;
  private raf = 0;
  private last = 0;
  private offset = 0;

  constructor(private readonly opts: TeleprompterOptions = {}) {
    this.speed = opts.speed ?? 40;
    this.fontSize = opts.fontSize ?? 30;
    this.mirror = !!opts.mirror;
    this.text = h("div", { class: "tp-text" });
    this.viewport = h("div", { class: "tp-viewport" }, this.text);
    this.speedLabel = h("span", { class: "tp-speed" });
    const btn = (label: string, title: string, f: () => void) => h("button", { class: "tp-btn", title, onclick: (e: Event) => (e.stopPropagation(), f()) }, label);
    this.playBtn = btn("▶", "Play / pause (Space)", () => this.toggle());
    this.el = h(
      "div",
      { class: "teleprompter", role: "region", "aria-label": "Teleprompter" },
      h("div", { class: "tp-cam", title: "Camera position" }),
      this.viewport,
      h(
        "div",
        { class: "tp-bar" },
        this.playBtn,
        btn("⏮", "Back to top", () => this.reset()),
        btn("−", "Slower", () => this.setSpeed(this.speed - 10)),
        this.speedLabel,
        btn("+", "Faster", () => this.setSpeed(this.speed + 10)),
        btn("A−", "Smaller text", () => this.setSize(this.fontSize - 4)),
        btn("A+", "Larger text", () => this.setSize(this.fontSize + 4)),
        btn("⇋", "Mirror", () => this.setMirror(!this.mirror)),
        btn("✕", "Close teleprompter", () => this.opts.onClose?.()),
      ),
    );
    this.viewport.addEventListener("wheel", (e) => {
      e.preventDefault();
      e.stopPropagation();
      this.scrollBy(e.deltaY);
    });
    this.setSpeed(this.speed);
    this.setSize(this.fontSize);
    this.setMirror(this.mirror);
  }

  setText(notes: string) {
    this.text.textContent = notes || "—";
    this.reset();
  }

  reset() {
    this.offset = 0;
    this.apply();
  }

  scrollBy(px: number) {
    const max = Math.max(0, this.text.scrollHeight - this.viewport.clientHeight * 0.5);
    this.offset = Math.max(0, Math.min(max, this.offset + px));
    this.apply();
  }

  /** Current scroll offset (px), for tests. */
  get position() {
    return this.offset;
  }

  private apply() {
    this.text.style.transform = `translateY(${-this.offset}px)${this.mirror ? " scaleX(-1)" : ""}`;
  }

  setSpeed(v: number) {
    this.speed = Math.max(0, Math.min(400, v));
    this.speedLabel.textContent = `${this.speed}`;
  }

  setSize(v: number) {
    this.fontSize = Math.max(14, Math.min(96, v));
    this.text.style.fontSize = `${this.fontSize}px`;
  }

  setMirror(on: boolean) {
    this.mirror = on;
    this.apply();
  }

  toggle() {
    this.playing ? this.pause() : this.play();
  }

  play() {
    if (this.playing) return;
    this.playing = true;
    this.playBtn.textContent = "⏸";
    this.last = performance.now();
    const tick = (now: number) => {
      if (!this.playing) return;
      const factor = this.voicePaced && !this.speaking ? 0 : 1;
      this.scrollBy(((now - this.last) / 1000) * this.speed * factor);
      this.last = now;
      this.raf = requestAnimationFrame(tick);
    };
    this.raf = requestAnimationFrame(tick);
  }

  pause() {
    this.playing = false;
    this.playBtn.textContent = "▶";
    cancelAnimationFrame(this.raf);
  }

  /** Voice pacing: once fed microphone levels, scroll only while the presenter speaks. */
  voicePaced = false;
  private speaking = true;
  voiceLevel(level: number) {
    this.voicePaced = true;
    this.speaking = level > 0.04;
  }

  dispose() {
    this.pause();
    this.el.remove();
  }
}
