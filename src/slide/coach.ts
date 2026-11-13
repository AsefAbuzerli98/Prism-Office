/**
 * Speaker Coach (P4-13): rehearse with live feedback and a report.
 * Measured on this machine from the microphone: pitch (autocorrelation) and
 * its variation (monotone detection), loudness, pauses and — without a
 * transcript — speaking rate from syllable nuclei. With the webview's speech
 * recognition (where available) also words per minute, filler words,
 * repeated phrases and reading the slide aloud.
 */

export interface CoachMetrics {
  /** Seconds of rehearsal. */
  seconds: number;
  /** Words per minute (transcript, or syllable estimate). */
  wpm: number;
  wpmSource: "transcript" | "syllables";
  fillers: Record<string, number>;
  /** Pitch variation in semitones (std dev); < 2 sounds monotone. */
  pitchVariation: number;
  meanPitch: number;
  /** Long silences (> 3 s). */
  longPauses: number;
  /** 0..1 share of slide text read verbatim. */
  reading: number;
  repeated: [string, number][];
  transcript: string;
}

export interface CoachTip {
  kind: "pace" | "filler" | "pitch" | "reading" | "pause" | "repeat" | "good";
  text: string;
}

const FILLERS: Record<string, string[]> = {
  en: ["um", "uh", "erm", "er", "like", "you know", "basically", "actually", "literally", "so yeah", "kind of", "sort of", "i mean"],
  az: ["ee", "eee", "yəni", "hə", "deməli", "bax", "elə", "necə deyim", "ümumiyyətlə"],
  tr: ["ıı", "şey", "yani", "işte", "hani", "aslında"],
  ru: ["э", "ну", "как бы", "типа", "вот", "короче", "значит"],
};

const STOP = new Set("the a an and or of to in on for is are was were be it this that with as at by from we you i our your they he she but not so if then than there here have has had do does did will would can could should about into over also just very more most such these those which what who how when where why all any some each other only own same too its their them his her him us me my mine yours".split(" "));

/** Fundamental frequency (Hz) of a voiced frame, or null (autocorrelation with parabolic refinement). */
export function detectPitch(buf: Float32Array, sampleRate: number): number | null {
  const n = buf.length;
  let rms = 0;
  for (let i = 0; i < n; i++) rms += buf[i]! * buf[i]!;
  rms = Math.sqrt(rms / n);
  if (rms < 0.01) return null;
  const minLag = Math.floor(sampleRate / 500); // 500 Hz
  const maxLag = Math.min(n - 1, Math.floor(sampleRate / 70)); // 70 Hz
  let best = -1;
  let bestCorr = 0;
  const corr = new Float32Array(maxLag + 2);
  for (let lag = minLag; lag <= maxLag + 1; lag++) {
    let s = 0;
    let e1 = 0;
    let e2 = 0;
    for (let i = 0; i + lag < n; i++) {
      s += buf[i]! * buf[i + lag]!;
      e1 += buf[i]! * buf[i]!;
      e2 += buf[i + lag]! * buf[i + lag]!;
    }
    const c = s / Math.sqrt(e1 * e2 || 1);
    corr[lag] = c;
  }
  // first strong peak (avoids octave errors from later multiples)
  for (let lag = minLag + 1; lag <= maxLag; lag++) {
    const c = corr[lag]!;
    if (c > 0.85 && c >= corr[lag - 1]! && c >= corr[lag + 1]!) {
      best = lag;
      bestCorr = c;
      break;
    }
    if (c > bestCorr && c > corr[lag - 1]! && c >= corr[lag + 1]!) {
      bestCorr = c;
      best = lag;
    }
  }
  if (best < 0 || bestCorr < 0.6) return null;
  const a = corr[best - 1]!;
  const b = corr[best]!;
  const c = corr[best + 1]!;
  const shift = (a - c) / (2 * (a - 2 * b + c) || 1);
  return sampleRate / (best + (Number.isFinite(shift) ? shift : 0));
}

/** Count filler words/phrases in a transcript. */
export function countFillers(transcript: string, lang: string): Record<string, number> {
  const list = [...(FILLERS[lang.slice(0, 2)] ?? []), ...(lang.startsWith("en") ? [] : FILLERS.en!.slice(0, 4))];
  const text = ` ${transcript.toLowerCase().replace(/[^\p{L}\p{N}\s']/gu, " ").replace(/\s+/g, " ")} `;
  const out: Record<string, number> = {};
  for (const f of list) {
    const re = new RegExp(`(?<=\\s)${f.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}(?=\\s)`, "gu");
    const n = text.match(re)?.length ?? 0;
    if (n) out[f] = n;
  }
  return out;
}

/** Share of the slide's text (as 3-word phrases) that was spoken verbatim. */
export function readingScore(transcript: string, slideText: string): number {
  const words = (s: string) => s.toLowerCase().match(/[\p{L}\p{N}']+/gu) ?? [];
  const sw = words(slideText);
  if (sw.length < 6) return 0;
  const spoken = new Set<string>();
  const tw = words(transcript);
  for (let i = 0; i + 2 < tw.length; i++) spoken.add(`${tw[i]} ${tw[i + 1]} ${tw[i + 2]}`);
  let hit = 0;
  let total = 0;
  for (let i = 0; i + 2 < sw.length; i++) {
    total++;
    if (spoken.has(`${sw[i]} ${sw[i + 1]} ${sw[i + 2]}`)) hit++;
  }
  return total ? hit / total : 0;
}

/** Most repeated content words (≥ 4 times). */
export function repeatedWords(transcript: string): [string, number][] {
  const counts = new Map<string, number>();
  for (const w of transcript.toLowerCase().match(/[\p{L}']{4,}/gu) ?? []) if (!STOP.has(w)) counts.set(w, (counts.get(w) ?? 0) + 1);
  return [...counts].filter(([, n]) => n >= 4).sort((a, b) => b[1] - a[1]).slice(0, 5);
}

function std(xs: number[]) {
  if (xs.length < 2) return 0;
  const m = xs.reduce((a, b) => a + b, 0) / xs.length;
  return Math.sqrt(xs.reduce((a, b) => a + (b - m) * (b - m), 0) / xs.length);
}

/** Tips from metrics (live and in the report). */
export function tips(m: CoachMetrics): CoachTip[] {
  const out: CoachTip[] = [];
  if (m.seconds >= 10) {
    if (m.wpm > 175) out.push({ kind: "pace", text: `You're speaking fast (${Math.round(m.wpm)} wpm). Slow down a little — about 130–160 wpm is easy to follow.` });
    else if (m.wpm > 0 && m.wpm < 100) out.push({ kind: "pace", text: `Your pace is slow (${Math.round(m.wpm)} wpm). Try to keep the energy up.` });
  }
  const fillerTotal = Object.values(m.fillers).reduce((a, b) => a + b, 0);
  if (fillerTotal >= 3) {
    const top = Object.entries(m.fillers).sort((a, b) => b[1] - a[1])[0]!;
    out.push({ kind: "filler", text: `${fillerTotal} filler words — most often “${top[0]}” (${top[1]}×). A short pause works better.` });
  }
  if (m.seconds >= 15 && m.pitchVariation > 0 && m.pitchVariation < 2) out.push({ kind: "pitch", text: "Your voice sounds monotone. Vary your pitch to stress key points." });
  if (m.reading > 0.5) out.push({ kind: "reading", text: "You're reading the slide text. Talk about the points instead of reciting them." });
  if (m.longPauses >= 3) out.push({ kind: "pause", text: `${m.longPauses} long pauses. Rehearse the transitions between slides.` });
  for (const [w, n] of m.repeated.slice(0, 2)) out.push({ kind: "repeat", text: `You said “${w}” ${n} times — consider a synonym.` });
  if (!out.length && m.seconds >= 20) out.push({ kind: "good", text: "Great delivery — steady pace, varied pitch and no filler words." });
  return out;
}

type Recognition = {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  onresult: ((e: { resultIndex: number; results: ArrayLike<{ isFinal: boolean; 0: { transcript: string } }> }) => void) | null;
  onerror: ((e: unknown) => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
};

/** Live rehearsal session. */
export class SpeakerCoach {
  private ctx: AudioContext | null = null;
  private analyser: AnalyserNode | null = null;
  private timer: ReturnType<typeof setInterval> | null = null;
  private stream: MediaStream | null = null;
  private rec: Recognition | null = null;
  private started = 0;
  private pitches: number[] = [];
  private silenceSince = 0;
  private pauses = 0;
  private syllables = 0;
  private voicedSec = 0;
  private lastEnergy = 0;
  private rising = false;
  private finals: string[] = [];
  private interim = "";
  private slideTexts: string[] = [];
  private readingMax = 0;
  /** Live level 0..1 (meters, voice-paced teleprompter). */
  level = 0;
  readonly hasTranscript: boolean;
  onTip?: (tip: CoachTip) => void;
  private lastTip = new Map<string, number>();

  constructor(private readonly lang = "en-US") {
    const g = globalThis as unknown as { SpeechRecognition?: new () => Recognition; webkitSpeechRecognition?: new () => Recognition };
    this.hasTranscript = !!(g.SpeechRecognition ?? g.webkitSpeechRecognition);
  }

  async start(stream: MediaStream) {
    this.stream = stream;
    this.started = performance.now();
    this.silenceSince = this.started;
    this.ctx = new AudioContext();
    this.analyser = this.ctx.createAnalyser();
    this.analyser.fftSize = 2048;
    this.ctx.createMediaStreamSource(stream).connect(this.analyser);
    this.timer = setInterval(() => this.sample(), 50);
    const g = globalThis as unknown as { SpeechRecognition?: new () => Recognition; webkitSpeechRecognition?: new () => Recognition };
    const Rec = g.SpeechRecognition ?? g.webkitSpeechRecognition;
    if (Rec) {
      try {
        const r = new Rec();
        r.lang = this.lang;
        r.continuous = true;
        r.interimResults = true;
        r.onresult = (e) => {
          this.interim = "";
          for (let i = e.resultIndex; i < e.results.length; i++) {
            const res = e.results[i]!;
            if (res.isFinal) this.finals.push(res[0].transcript);
            else this.interim += res[0].transcript;
          }
          this.checkLive();
        };
        r.onerror = () => {};
        r.onend = () => {
          if (this.timer) {
            try {
              r.start();
            } catch {
              /* already running */
            }
          }
        };
        r.start();
        this.rec = r;
      } catch {
        this.rec = null;
      }
    }
  }

  /** Text of the slide being presented (reading detection). */
  setSlide(text: string) {
    this.slideTexts.push(text);
    this.slideStart = this.finals.length;
  }

  private slideStart = 0;

  private sample() {
    const a = this.analyser;
    if (!a || !this.ctx) return;
    const buf = new Float32Array(a.fftSize);
    a.getFloatTimeDomainData(buf);
    let e = 0;
    for (const v of buf) e += v * v;
    const rms = Math.sqrt(e / buf.length);
    this.level = Math.min(1, rms * 4);
    const now = performance.now();
    const voiced = rms > 0.015;
    if (voiced) {
      if (now - this.silenceSince > 3000) this.pauses++;
      this.silenceSince = now;
      this.voicedSec += 0.05;
      const f0 = detectPitch(buf, this.ctx.sampleRate);
      if (f0) this.pitches.push(f0);
    }
    // syllable nuclei: energy peaks
    if (rms > this.lastEnergy * 1.15 && rms > 0.02) this.rising = true;
    else if (this.rising && rms < this.lastEnergy * 0.85) {
      this.syllables++;
      this.rising = false;
    }
    this.lastEnergy = rms;
    if (Math.round((now - this.started) / 50) % 100 === 0) this.checkLive();
  }

  get transcript(): string {
    return [...this.finals, this.interim].join(" ").replace(/\s+/g, " ").trim();
  }

  metrics(): CoachMetrics {
    const seconds = (performance.now() - this.started) / 1000;
    const words = this.transcript.split(/\s+/).filter(Boolean).length;
    const useTranscript = this.hasTranscript && words > 5;
    // ~1.5 syllables per word in English-like speech
    const wpm = useTranscript ? (words / Math.max(seconds, 1)) * 60 : (this.syllables / 1.5 / Math.max(seconds, 1)) * 60;
    const semis = this.pitches.map((f) => 12 * Math.log2(f / 100));
    const slideText = this.slideTexts[this.slideTexts.length - 1] ?? "";
    this.readingMax = Math.max(this.readingMax, readingScore(this.finals.slice(this.slideStart).join(" "), slideText));
    return {
      seconds,
      wpm,
      wpmSource: useTranscript ? "transcript" : "syllables",
      fillers: countFillers(this.transcript, this.lang),
      pitchVariation: std(semis),
      meanPitch: this.pitches.length ? this.pitches.reduce((a, b) => a + b, 0) / this.pitches.length : 0,
      longPauses: this.pauses,
      reading: this.readingMax,
      repeated: repeatedWords(this.transcript),
      transcript: this.transcript,
    };
  }

  private checkLive() {
    const m = this.metrics();
    const now = performance.now();
    for (const t of tips(m)) {
      if (t.kind === "good" || t.kind === "repeat") continue;
      if (now - (this.lastTip.get(t.kind) ?? -Infinity) < 30000) continue;
      this.lastTip.set(t.kind, now);
      this.onTip?.(t);
    }
  }

  stop(): CoachMetrics {
    const m = this.metrics();
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    this.rec?.stop();
    this.rec = null;
    this.stream?.getTracks().forEach((t) => t.stop());
    void this.ctx?.close();
    this.ctx = null;
    return m;
  }
}
