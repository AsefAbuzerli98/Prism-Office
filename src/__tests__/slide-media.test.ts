import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { buildSlides, parseDeck } from "../slide/ai";
import { countFillers, detectPitch, readingScore, repeatedWords, tips } from "../slide/coach";
import { loadDrawingmlSync } from "../slide/engine";
import { parseTranslations } from "../slide/live";
import { blankPresentation } from "../slide/model";
import { muxMp4 } from "../slide/mp4";
import { slideText } from "../slide/ops";
import { dataOrder, encodeQr, formatBits, rsEncode } from "../slide/qr";
import { buildTimeline, chaptersOf } from "../slide/video";

beforeAll(() => {
  loadDrawingmlSync(readFileSync(resolve(__dirname, "../slide/wasm/prism_drawingml_bg.wasm")));
  HTMLCanvasElement.prototype.getContext = (() => null) as never;
});

describe("QR code", () => {
  it("matches the ISO 18004 Reed-Solomon and format-information vectors", () => {
    // "HELLO WORLD", version 1-M (ISO/IEC 18004 annex / Thonky tutorial)
    expect(rsEncode([32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17], 10)).toEqual([196, 35, 39, 119, 235, 215, 231, 226, 93, 23]);
    expect(formatBits("M", 0).toString(2).padStart(15, "0")).toBe("101010000010010");
    expect(formatBits("L", 0).toString(2).padStart(15, "0")).toBe("111011111000100");
    expect(formatBits("H", 7).toString(2).padStart(15, "0")).toBe("000100000111011");
  });

  it("encodes a URL that an independent reader decodes back", () => {
    const url = "http://192.168.1.20:54321/?t=0123456789abcdef0123456789abcdef";
    const qr = encodeQr(url, "M");
    const n = qr.size;
    expect(n).toBe(17 + 4 * qr.version);
    // finder patterns
    for (const [ox, oy] of [[0, 0], [n - 7, 0], [0, n - 7]] as const) {
      expect(qr.modules[oy]![ox]).toBe(true);
      expect(qr.modules[oy + 1]![ox + 1]).toBe(false);
      expect(qr.modules[oy + 3]![ox + 3]).toBe(true);
    }
    // read format info (first copy) → level and mask
    const fmt = [0, 1, 2, 3, 4, 5].map((i) => qr.modules[i]![8]).concat([qr.modules[7]![8]!, qr.modules[8]![8]!, qr.modules[8]![7]!], [9, 10, 11, 12, 13, 14].map((i) => qr.modules[8]![14 - i]!));
    const bits = fmt.reduce((a, b, i) => a | ((b ? 1 : 0) << i), 0);
    expect(bits).toBe(formatBits("M", qr.mask));
    // unmask data modules and rebuild codewords
    const masks = [(x: number, y: number) => (x + y) % 2 === 0, (_: number, y: number) => y % 2 === 0, (x: number) => x % 3 === 0, (x: number, y: number) => (x + y) % 3 === 0, (x: number, y: number) => (Math.floor(y / 2) + Math.floor(x / 3)) % 2 === 0, (x: number, y: number) => ((x * y) % 2) + ((x * y) % 3) === 0, (x: number, y: number) => (((x * y) % 2) + ((x * y) % 3)) % 2 === 0, (x: number, y: number) => (((x + y) % 2) + ((x * y) % 3)) % 2 === 0];
    const stream = dataOrder(qr.version).map(([x, y]) => qr.modules[y]![x] !== masks[qr.mask]!(x, y));
    const cw: number[] = [];
    for (let i = 0; i + 8 <= stream.length; i += 8) cw.push(stream.slice(i, i + 8).reduce((a, b) => (a << 1) | (b ? 1 : 0), 0));
    // version 4-M: 2 blocks of 32 data + 18 EC codewords, interleaved
    expect(qr.version).toBe(4);
    const blocks = [[], []] as number[][];
    for (let i = 0; i < 64; i++) blocks[i % 2]!.push(cw[i]!);
    const ecs = [[], []] as number[][];
    for (let i = 0; i < 36; i++) ecs[i % 2]!.push(cw[64 + i]!);
    expect(rsEncode(blocks[0]!, 18)).toEqual(ecs[0]);
    expect(rsEncode(blocks[1]!, 18)).toEqual(ecs[1]);
    const data = [...blocks[0]!, ...blocks[1]!];
    const bitStr = data.map((b) => b.toString(2).padStart(8, "0")).join("");
    expect(bitStr.slice(0, 4)).toBe("0100");
    const len = parseInt(bitStr.slice(4, 12), 2);
    const bytes = Array.from({ length: len }, (_, i) => parseInt(bitStr.slice(12 + i * 8, 20 + i * 8), 2));
    expect(new TextDecoder().decode(new Uint8Array(bytes))).toBe(url);
  });
});

describe("MP4 muxer", () => {
  it("writes ftyp/mdat/moov with sample tables and Nero chapters", () => {
    const frame = (key: boolean, t: number) => ({ data: new Uint8Array([0, 0, 0, 2, key ? 0x65 : 0x41, 0x88]), timestamp: t, duration: 33333, key });
    const mp4 = muxMp4(
      { codec: "avc1", width: 1920, height: 1080, description: new Uint8Array([1, 0x64, 0, 0x28, 0xff, 0xe0, 0]), samples: [frame(true, 0), frame(false, 33333), frame(false, 66666)] },
      { codec: "opus", sampleRate: 48000, channels: 1, samples: [{ data: new Uint8Array([1, 2, 3]), timestamp: 0, duration: 20000, key: true }] },
      [{ start: 0, title: "Intro" }, { start: 1500, title: "Agenda" }],
    );
    const dv = new DataView(mp4.buffer);
    const boxes: string[] = [];
    for (let p = 0; p < mp4.length; ) {
      const size = dv.getUint32(p);
      boxes.push(String.fromCharCode(...mp4.subarray(p + 4, p + 8)));
      p += size;
    }
    expect(boxes).toEqual(["ftyp", "mdat", "moov"]);
    const text = new TextDecoder("latin1").decode(mp4);
    for (const b of ["mvhd", "trak", "tkhd", "mdhd", "hdlr", "stsd", "avc1", "avcC", "stts", "stss", "stsz", "stco", "Opus", "dOps", "udta", "chpl", "Intro", "Agenda"]) expect(text).toContain(b);
    // first video sample sits right after the mdat header
    const mdatAt = dv.getUint32(0);
    expect(mp4[mdatAt + 8 + 4]).toBe(0x65);
  });
});

describe("video timeline", () => {
  it("lays out transitions, builds and holds, one chapter per slide", () => {
    const m = blankPresentation();
    m.slides[0]!.shapes[0] = { ...m.slides[0]!.shapes[0]!, type: "shape", text: { paragraphs: [{ runs: [{ t: "text", text: "Hello" }] }] } } as never;
    m.slides.push({ layout: m.slides[0]!.layout, shapes: [], notes: "", transition: { kind: "fade", duration: 500, advanceClick: true }, timing: { effects: [{ shape: 2, class: "entr", preset: "fade", presetId: 10, trigger: "onClick", duration: 400 }] } });
    const segs = buildTimeline(m, { slideSeconds: 2, stepSeconds: 1 });
    expect(segs.map((s) => [s.kind, s.slide, s.start, s.dur])).toEqual([
      ["hold", 0, 0, 2000],
      ["trans", 1, 2000, 500],
      ["step", 1, 2500, 400],
      ["hold", 1, 2900, 2000],
    ]);
    expect(chaptersOf(m, segs)).toEqual([
      { start: 0, title: "Hello" },
      { start: 2000, title: "Slide 2" },
    ]);
    // from a recording
    const rec = buildTimeline(m, { recording: { duration: 6000, events: [{ t: 0, slide: 0, step: -1 }, { t: 3000, slide: 1, step: -1 }, { t: 4000, slide: 1, step: 0 }], ink: [], audio: null, camera: null, levels: [] } });
    expect(rec.map((s) => [s.kind, s.start, s.dur])).toEqual([
      ["hold", 0, 3000],
      ["trans", 3000, 500],
      ["hold", 3500, 500],
      ["step", 4000, 400],
      ["hold", 4400, 1600],
    ]);
  });
});

describe("AI deck generator", () => {
  it("parses an outline and lays it out on the deck's layouts", () => {
    const deck = parseDeck('Here you go:\n{"title":"Plan","slides":[{"layout":"title","title":"Q3 Plan","subtitle":"Team sync","notes":"Welcome."},{"layout":"content","title":"Goals","bullets":["Ship v1",{"text":"Docs","level":1}],"notes":"Our goals."},{"layout":"two","title":"Now vs next","left":["A"],"right":["B"]},{"layout":"weird","title":"Fallback"}]}');
    expect(deck.slides.map((s) => s.layout)).toEqual(["title", "content", "two", "content"]);
    const m = blankPresentation();
    const slides = buildSlides(m, deck);
    expect(slides).toHaveLength(4);
    expect(slideText(slides[0]!)).toBe("Q3 Plan\nTeam sync");
    expect(slideText(slides[1]!)).toBe("Goals\nShip v1\nDocs");
    expect(slides[1]!.notes).toBe("Our goals.");
    const goals = slides[1]!.shapes.find((n) => n.ph?.idx === 1)!;
    expect(goals.type === "shape" && goals.text!.paragraphs![1]!.props?.level).toBe(1);
    expect(slideText(slides[2]!)).toBe("Now vs next\nA\nB");
    expect(() => parseDeck("no json")).toThrow();
  });
});

describe("Speaker Coach", () => {
  it("detects pitch, fillers, reading and repetition", () => {
    const sr = 48000;
    const buf = new Float32Array(2048).map((_, i) => 0.3 * Math.sin((2 * Math.PI * 180 * i) / sr));
    expect(detectPitch(buf, sr)!).toBeCloseTo(180, -0.5);
    expect(detectPitch(new Float32Array(2048), sr)).toBeNull();
    expect(countFillers("So um I think, uh, we basically like um ship it", "en-US")).toEqual({ um: 2, uh: 1, like: 1, basically: 1 });
    expect(countFillers("Yəni biz, ee, bunu edəcəyik yəni", "az")).toEqual({ ee: 1, yəni: 2 });
    expect(readingScore("today our revenue grew by twenty percent in europe", "Revenue grew by twenty percent in Europe this quarter")).toBeGreaterThan(0.5);
    expect(readingScore("let me tell you a story", "Revenue grew by twenty percent in Europe")).toBe(0);
    expect(repeatedWords("product product product product launch launch")[0]).toEqual(["product", 4]);
    const t = tips({ seconds: 60, wpm: 200, wpmSource: "transcript", fillers: { um: 4 }, pitchVariation: 1.2, meanPitch: 150, longPauses: 0, reading: 0.7, repeated: [], transcript: "" });
    expect(t.map((x) => x.kind)).toEqual(["pace", "filler", "pitch", "reading"]);
  });
});

describe("live subtitles", () => {
  it("parses translations for the requested languages only", () => {
    expect(parseTranslations('{"de":"Hallo","fr":"Salut","xx":1}', ["de", "fr", "es"])).toEqual({ de: "Hallo", fr: "Salut" });
    expect(parseTranslations("sorry", ["de"])).toEqual({});
  });
});
