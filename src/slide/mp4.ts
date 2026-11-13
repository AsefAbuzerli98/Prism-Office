/**
 * Minimal ISO BMFF (MP4) muxer for WebCodecs output: one video track
 * (H.264 `avc1`/`avcC` or VP9 `vp09`/`vpcC`), an optional audio track
 * (AAC `mp4a`/`esds` or Opus `Opus`/`dOps`) and chapter bookmarks as a
 * Nero `chpl` box (read by ffmpeg/VLC/mpv and most players).
 */

export interface Sample {
  data: Uint8Array;
  /** µs */
  timestamp: number;
  /** µs */
  duration: number;
  key: boolean;
}

export interface VideoTrack {
  codec: "avc1" | "vp09";
  width: number;
  height: number;
  /** avcC payload (H.264) from the encoder's decoderConfig.description. */
  description?: Uint8Array;
  samples: Sample[];
}

export interface AudioTrack {
  codec: "mp4a" | "opus";
  sampleRate: number;
  channels: number;
  /** AudioSpecificConfig (AAC) or OpusHead (Opus) from the encoder, if any. */
  description?: Uint8Array;
  samples: Sample[];
}

export interface Chapter {
  /** ms */
  start: number;
  title: string;
}

// ---- byte writer -----------------------------------------------------------------------------

class W {
  private buf = new Uint8Array(1 << 16);
  len = 0;
  private ensure(n: number) {
    if (this.len + n <= this.buf.length) return;
    let cap = this.buf.length * 2;
    while (cap < this.len + n) cap *= 2;
    const b = new Uint8Array(cap);
    b.set(this.buf.subarray(0, this.len));
    this.buf = b;
  }
  u8(v: number) {
    this.ensure(1);
    this.buf[this.len++] = v & 0xff;
  }
  u16(v: number) {
    this.u8(v >> 8);
    this.u8(v);
  }
  u24(v: number) {
    this.u8(v >> 16);
    this.u16(v);
  }
  u32(v: number) {
    this.u16(Math.floor(v / 65536) & 0xffff);
    this.u16(v & 0xffff);
  }
  u64(v: number) {
    this.u32(Math.floor(v / 4294967296));
    this.u32(v >>> 0);
  }
  bytes(b: Uint8Array) {
    this.ensure(b.length);
    this.buf.set(b, this.len);
    this.len += b.length;
  }
  str(s: string) {
    for (let i = 0; i < s.length; i++) this.u8(s.charCodeAt(i));
  }
  out() {
    return this.buf.slice(0, this.len);
  }
}

function box(type: string, ...parts: (Uint8Array | ((w: W) => void))[]): Uint8Array {
  const w = new W();
  w.u32(0);
  w.str(type);
  for (const p of parts) {
    if (typeof p === "function") p(w);
    else w.bytes(p);
  }
  const b = w.out();
  new DataView(b.buffer).setUint32(0, b.length);
  return b;
}

function full(type: string, version: number, flags: number, ...parts: (Uint8Array | ((w: W) => void))[]): Uint8Array {
  return box(type, (w) => (w.u8(version), w.u24(flags)), ...parts);
}

const MATRIX = [0x00010000, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000];

// ---- track boxes -----------------------------------------------------------------------------

interface TrackLayout {
  id: number;
  timescale: number;
  durations: number[];
  sizes: number[];
  offsets: number[];
  keys: boolean[] | null;
  duration: number;
}

function sttsBox(durations: number[]) {
  const runs: [number, number][] = [];
  for (const d of durations) {
    const last = runs[runs.length - 1];
    if (last && last[1] === d) last[0]++;
    else runs.push([1, d]);
  }
  return full("stts", 0, 0, (w) => {
    w.u32(runs.length);
    for (const [n, d] of runs) (w.u32(n), w.u32(d));
  });
}

function stblBox(stsd: Uint8Array, t: TrackLayout) {
  const big = t.offsets.some((o) => o > 0xffffffff);
  return box(
    "stbl",
    stsd,
    sttsBox(t.durations),
    ...(t.keys && t.keys.some((k) => !k)
      ? [
          full("stss", 0, 0, (w) => {
            const ks = t.keys!.map((k, i) => (k ? i + 1 : 0)).filter(Boolean);
            w.u32(ks.length);
            for (const k of ks) w.u32(k);
          }),
        ]
      : []),
    full("stsc", 0, 0, (w) => (w.u32(1), w.u32(1), w.u32(1), w.u32(1))),
    full("stsz", 0, 0, (w) => {
      w.u32(0);
      w.u32(t.sizes.length);
      for (const s of t.sizes) w.u32(s);
    }),
    big
      ? full("co64", 0, 0, (w) => {
          w.u32(t.offsets.length);
          for (const o of t.offsets) w.u64(o);
        })
      : full("stco", 0, 0, (w) => {
          w.u32(t.offsets.length);
          for (const o of t.offsets) w.u32(o);
        }),
  );
}

function trakBox(t: TrackLayout, movieTimescale: number, handler: "vide" | "soun", stsd: Uint8Array, width = 0, height = 0) {
  const movieDur = Math.round((t.duration / t.timescale) * movieTimescale);
  return box(
    "trak",
    full("tkhd", 0, 3, (w) => {
      w.u32(0);
      w.u32(0);
      w.u32(t.id);
      w.u32(0);
      w.u32(movieDur);
      w.u32(0);
      w.u32(0);
      w.u16(0);
      w.u16(0);
      w.u16(handler === "soun" ? 0x0100 : 0);
      w.u16(0);
      for (const m of MATRIX) w.u32(m);
      w.u32(width * 65536);
      w.u32(height * 65536);
    }),
    box(
      "mdia",
      full("mdhd", 0, 0, (w) => {
        w.u32(0);
        w.u32(0);
        w.u32(t.timescale);
        w.u32(t.duration);
        w.u16(0x55c4); // 'und'
        w.u16(0);
      }),
      full("hdlr", 0, 0, (w) => {
        w.u32(0);
        w.str(handler);
        w.u32(0);
        w.u32(0);
        w.u32(0);
        w.str(handler === "vide" ? "Prism Video" : "Prism Audio");
        w.u8(0);
      }),
      box(
        "minf",
        handler === "vide" ? full("vmhd", 0, 1, (w) => (w.u16(0), w.u16(0), w.u16(0), w.u16(0))) : full("smhd", 0, 0, (w) => (w.u16(0), w.u16(0))),
        box("dinf", full("dref", 0, 0, (w) => w.u32(1), full("url ", 0, 1))),
        stblBox(stsd, t),
      ),
    ),
  );
}

function visualEntry(type: string, width: number, height: number, ...children: Uint8Array[]) {
  return box(type, (w) => {
    for (let i = 0; i < 6; i++) w.u8(0);
    w.u16(1); // data reference index
    w.u16(0);
    w.u16(0);
    w.u32(0);
    w.u32(0);
    w.u32(0);
    w.u16(width);
    w.u16(height);
    w.u32(0x00480000);
    w.u32(0x00480000);
    w.u32(0);
    w.u16(1);
    const name = "Prism Office";
    w.u8(name.length);
    w.str(name);
    for (let i = name.length; i < 31; i++) w.u8(0);
    w.u16(0x18);
    w.u16(0xffff);
  }, ...children);
}

function audioEntry(type: string, channels: number, sampleRate: number, ...children: Uint8Array[]) {
  return box(type, (w) => {
    for (let i = 0; i < 6; i++) w.u8(0);
    w.u16(1);
    w.u32(0);
    w.u32(0);
    w.u16(channels);
    w.u16(16);
    w.u16(0);
    w.u16(0);
    w.u32(sampleRate * 65536 > 0xffffffff ? 0 : sampleRate * 65536);
  }, ...children);
}

/** esds descriptor with the AAC AudioSpecificConfig. */
function esds(asc: Uint8Array, bitrate: number) {
  const desc = (tag: number, body: Uint8Array) => {
    const w = new W();
    w.u8(tag);
    // 4-byte expandable length
    const n = body.length;
    w.u8(0x80 | ((n >> 21) & 0x7f));
    w.u8(0x80 | ((n >> 14) & 0x7f));
    w.u8(0x80 | ((n >> 7) & 0x7f));
    w.u8(n & 0x7f);
    w.bytes(body);
    return w.out();
  };
  const dsi = desc(0x05, asc);
  const dcd = (() => {
    const w = new W();
    w.u8(0x40); // AAC
    w.u8(0x15); // audio stream
    w.u24(0);
    w.u32(bitrate);
    w.u32(bitrate);
    w.bytes(dsi);
    return desc(0x04, w.out());
  })();
  const sl = desc(0x06, new Uint8Array([0x02]));
  const es = (() => {
    const w = new W();
    w.u16(2); // ES id
    w.u8(0);
    w.bytes(dcd);
    w.bytes(sl);
    return desc(0x03, w.out());
  })();
  return full("esds", 0, 0, es);
}

/** Opus-in-ISOBMFF `dOps` box (from an OpusHead, or defaults). */
function dOps(channels: number, sampleRate: number, head?: Uint8Array) {
  let preSkip = 312;
  if (head && head.length >= 19 && String.fromCharCode(...head.subarray(0, 8)) === "OpusHead") preSkip = head[10]! | (head[11]! << 8);
  return box("dOps", (w) => {
    w.u8(0);
    w.u8(channels);
    w.u16(preSkip);
    w.u32(sampleRate);
    w.u16(0);
    w.u8(0);
  });
}

/** AudioSpecificConfig for AAC-LC when the encoder gives none. */
function defaultAsc(sampleRate: number, channels: number) {
  const rates = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];
  const idx = Math.max(0, rates.indexOf(sampleRate));
  return new Uint8Array([(2 << 3) | (idx >> 1), ((idx & 1) << 7) | (channels << 3)]);
}

function chpl(chapters: Chapter[]) {
  return box("chpl", (w) => {
    w.u32(0x01000000);
    w.u32(0);
    const list = chapters.slice(0, 255);
    w.u8(list.length);
    for (const c of list) {
      w.u64(Math.round(c.start * 10000)); // 100 ns units
      const bytes = new TextEncoder().encode(c.title).subarray(0, 255);
      w.u8(bytes.length);
      w.bytes(bytes);
    }
  });
}

/** Build a complete MP4 file. */
export function muxMp4(video: VideoTrack, audio: AudioTrack | null, chapters: Chapter[] = []): Uint8Array {
  const VTS = 90000;
  const MOVIE_TS = 1000;
  const ftyp = box("ftyp", (w) => {
    w.str("isom");
    w.u32(0x200);
    for (const b of ["isom", "iso2", video.codec === "avc1" ? "avc1" : "iso6", "mp41"]) w.str(b);
  });
  // mdat: video samples, then audio samples
  const vSizes = video.samples.map((s) => s.data.length);
  const aSizes = audio?.samples.map((s) => s.data.length) ?? [];
  const payload = [...vSizes, ...aSizes].reduce((a, b) => a + b, 0);
  const large = payload + 16 > 0xffffffff;
  const mdatHeader = large ? 16 : 8;
  let off = ftyp.length + mdatHeader;
  const vOffsets = vSizes.map((s) => {
    const o = off;
    off += s;
    return o;
  });
  const aOffsets = aSizes.map((s) => {
    const o = off;
    off += s;
    return o;
  });
  const vDur = video.samples.map((s, i) => {
    const next = video.samples[i + 1];
    const d = next ? next.timestamp - s.timestamp : s.duration;
    return Math.max(1, Math.round((d * VTS) / 1e6));
  });
  const vTrack: TrackLayout = { id: 1, timescale: VTS, durations: vDur, sizes: vSizes, offsets: vOffsets, keys: video.samples.map((s) => s.key), duration: vDur.reduce((a, b) => a + b, 0) };
  const vStsd = full(
    "stsd",
    0,
    0,
    (w) => w.u32(1),
    video.codec === "avc1"
      ? visualEntry("avc1", video.width, video.height, box("avcC", video.description ?? new Uint8Array()))
      : visualEntry("vp09", video.width, video.height, full("vpcC", 1, 0, (w) => {
          w.u8(0); // profile
          w.u8(40); // level 4.0
          w.u8((8 << 4) | (1 << 1)); // 8-bit, 4:2:0 colocated
          w.u8(1); // BT.709 primaries
          w.u8(1);
          w.u8(1);
          w.u16(0);
        })),
  );
  const traks = [trakBox(vTrack, MOVIE_TS, "vide", vStsd, video.width, video.height)];
  let movieDur = Math.round((vTrack.duration / VTS) * MOVIE_TS);
  if (audio && audio.samples.length) {
    const ats = audio.sampleRate;
    const aDur = audio.samples.map((s) => Math.max(1, Math.round((s.duration * ats) / 1e6)));
    const aTrack: TrackLayout = { id: 2, timescale: ats, durations: aDur, sizes: aSizes, offsets: aOffsets, keys: null, duration: aDur.reduce((a, b) => a + b, 0) };
    const entry = audio.codec === "mp4a" ? audioEntry("mp4a", audio.channels, ats, esds(audio.description ?? defaultAsc(ats, audio.channels), 128000)) : audioEntry("Opus", audio.channels, 48000, dOps(audio.channels, ats, audio.description));
    const aStsd = full("stsd", 0, 0, (w) => w.u32(1), entry);
    traks.push(trakBox(aTrack, MOVIE_TS, "soun", aStsd));
    movieDur = Math.max(movieDur, Math.round((aTrack.duration / ats) * MOVIE_TS));
  }
  const mvhd = full("mvhd", 0, 0, (w) => {
    w.u32(0);
    w.u32(0);
    w.u32(MOVIE_TS);
    w.u32(movieDur);
    w.u32(0x00010000);
    w.u16(0x0100);
    w.u16(0);
    w.u32(0);
    w.u32(0);
    for (const m of MATRIX) w.u32(m);
    for (let i = 0; i < 6; i++) w.u32(0);
    w.u32(traks.length + 1);
  });
  const udta = chapters.length ? [box("udta", chpl(chapters))] : [];
  const moov = box("moov", mvhd, ...traks, ...udta);
  // assemble
  const total = ftyp.length + mdatHeader + payload + moov.length;
  const out = new Uint8Array(total);
  let p = 0;
  out.set(ftyp, p);
  p += ftyp.length;
  const dv = new DataView(out.buffer);
  if (large) {
    dv.setUint32(p, 1);
    out.set([0x6d, 0x64, 0x61, 0x74], p + 4);
    dv.setUint32(p + 8, Math.floor((payload + 16) / 4294967296));
    dv.setUint32(p + 12, (payload + 16) >>> 0);
  } else {
    dv.setUint32(p, payload + 8);
    out.set([0x6d, 0x64, 0x61, 0x74], p + 4);
  }
  p += mdatHeader;
  for (const s of video.samples) {
    out.set(s.data, p);
    p += s.data.length;
  }
  for (const s of audio?.samples ?? []) {
    out.set(s.data, p);
    p += s.data.length;
  }
  out.set(moov, p);
  return out;
}
