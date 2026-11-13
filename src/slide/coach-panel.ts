/** Rehearse with Speaker Coach: slideshow + live hints + report. */
import { h } from "../core/dom";
import { getLocale } from "../core/i18n";
import { errorText, overlay } from "../word/ai-panel";
import { microphone } from "./camera";
import { SpeakerCoach, tips, type CoachMetrics } from "./coach";
import type { PresentationModel } from "./model";
import { slideText } from "./ops";
import { SlideShow, type ShowOptions } from "./show";

export async function rehearseWithCoach(root: HTMLElement, model: PresentationModel, start: number, show: Omit<ShowOptions, "start" | "onExit" | "onNavigate">, onError: (msg: string) => void) {
  let stream: MediaStream;
  try {
    stream = await microphone();
  } catch (e) {
    return onError(`Microphone: ${errorText(e)}`);
  }
  const lang = getLocale() === "az" ? "az-AZ" : getLocale();
  const coach = new SpeakerCoach(lang);
  const hint = h("div", { class: "coach-hint", role: "status", "aria-live": "polite", hidden: true });
  const meter = h("div", { class: "coach-meter" }, h("span", {}));
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  coach.onTip = (t) => {
    hint.textContent = t.text;
    hint.hidden = false;
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(() => (hint.hidden = true), 6000);
  };
  const s = new SlideShow(model, {
    ...show,
    start,
    onNavigate: (slide, step) => {
      if (step < 0) coach.setSlide(slideText(model.slides[slide]!));
    },
    onExit: () => {
      clearInterval(meterTimer);
      openReport(root, coach.stop(), coach.hasTranscript);
    },
  });
  await coach.start(stream);
  await s.start();
  s.el.append(hint, meter);
  const meterTimer = setInterval(() => ((meter.firstElementChild as HTMLElement).style.width = `${Math.round(coach.level * 100)}%`), 100);
}

export function openReport(root: HTMLElement, m: CoachMetrics, transcriptAvailable: boolean) {
  const mins = Math.floor(m.seconds / 60);
  const secs = Math.round(m.seconds % 60);
  const fillerTotal = Object.values(m.fillers).reduce((a, b) => a + b, 0);
  const row = (label: string, value: string, note = "") => h("tr", {}, h("th", {}, label), h("td", {}, value), h("td", { class: "muted small" }, note));
  const advice = tips(m);
  const el = overlay(
    root,
    "Rehearsal report",
    h(
      "table",
      { class: "coach-table" },
      row("Duration", `${mins}:${String(secs).padStart(2, "0")}`),
      row("Pace", m.wpm ? `${Math.round(m.wpm)} words/min` : "—", m.wpmSource === "syllables" ? "estimated from syllables" : "from the transcript"),
      row("Pitch", m.meanPitch ? `${Math.round(m.meanPitch)} Hz, variation ${m.pitchVariation.toFixed(1)} semitones` : "—", m.pitchVariation && m.pitchVariation < 2 ? "monotone" : ""),
      row("Filler words", transcriptAvailable ? String(fillerTotal) : "—", transcriptAvailable ? Object.entries(m.fillers).map(([k, v]) => `${k} ×${v}`).join(", ") : "needs speech recognition"),
      row("Long pauses", String(m.longPauses)),
      row("Reading slides", transcriptAvailable ? `${Math.round(m.reading * 100)}%` : "—"),
    ),
    h("ul", { class: "coach-tips" }, ...advice.map((t) => h("li", { class: `tip-${t.kind}` }, t.text))),
    m.transcript ? h("details", {}, h("summary", {}, "Transcript"), h("p", { class: "coach-transcript" }, m.transcript)) : h("p", { class: "muted small" }, transcriptAvailable ? "No speech was recognised." : "Speech recognition is not available in this webview; pace is estimated from syllables and filler words are not counted."),
    h("div", { class: "dialog-actions" }, h("button", { class: "btn", onclick: () => el.remove() }, "Close")),
  );
}
