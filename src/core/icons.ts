/**
 * Ribbon and shell icons: one colour set on a 24-unit grid (1.6 stroke).
 *
 * Every icon has a hue (`--ic`) chosen by meaning — blue for files and
 * structure, green for adding and data, amber for folders, notes and colour,
 * red for removing, recording and live, teal for media and connections,
 * orange for slides and drawing. Closed shapes take a light wash of the hue
 * (styled in app.css: `rect`, `circle`, `ellipse`, `.f`), solid parts use `.k`,
 * and a few parts carry a second hue (`.c-red`, `.c-amber`, …). Commands whose
 * glyph is a letter (B, I, U, Σ) keep the letter: it is the clearest icon.
 */

/** Stroke-only path. */
const P = (d: string) => `<path d="${d}"/>`;
/** Path with the hue's light wash. */
const F = (d: string) => `<path class="f" d="${d}"/>`;
/** Solid path (fill, no stroke) in an optional second hue. */
const K = (d: string, c = "") => `<path class="k${c ? ` c-${c}` : ""}" d="${d}"/>`;
/** Stroke path in a second hue. */
const S = (d: string, c: string) => `<path class="s c-${c}" d="${d}"/>`;
const R = (x: number, y: number, w: number, h: number, rx = 1, cls = "") =>
  `<rect${cls ? ` class="${cls}"` : ""} x="${x}" y="${y}" width="${w}" height="${h}" rx="${rx}"/>`;
const C = (cx: number, cy: number, r: number, cls = "") => `<circle${cls ? ` class="${cls}"` : ""} cx="${cx}" cy="${cy}" r="${r}"/>`;
/** A letter drawn as text (for A-with-bar style icons). */
const T = (x: number, y: number, s: string, size = 13, cls = "") =>
  `<text${cls ? ` class="${cls}"` : ""} x="${x}" y="${y}" font-size="${size}" font-weight="700" font-family="IBM Plex Sans, system-ui, sans-serif" text-anchor="middle" stroke="none">${s}</text>`;

type Hue = "blue" | "green" | "amber" | "red" | "teal" | "orange" | "pink" | "ink";

const SHAPES: Record<string, [Hue, string]> = {
  plus: ["green", P("M12 5v14M5 12h14")],
  folder: ["amber", F("M3.5 7.5a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z") + P("M3.5 10h17")],
  save: ["blue", F("M5 4.5h11l3.5 3.5v11.5h-14.5z") + P("M8 4.5v5h7v-5") + R(8, 13.5, 8, 6, 0.5, "k soft")],
  saveAs: ["blue", F("M5 4.5h9l3 3v4M5 4.5v15h6") + P("M8 4.5v4h5v-4") + S("M14 19.5l1-3 4.5-4.5 2 2-4.5 4.5z", "orange")],
  newDoc: ["blue", F("M6 3.5h8l4 4v13H6z") + P("M14 3.5v4h4") + S("M12 11v6M9 14h6", "green")],
  clipboard: ["amber", F("M7 5.5h10a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1v-13a1 1 0 0 1 1-1z") + R(9, 3.5, 6, 3.5, 1, "k c-ink") + S("M9 11h6M9 14.5h6M9 18h3", "ink")],
  copy: ["blue", R(8.5, 8.5, 10, 11) + P("M15.5 8.5v-4h-10v11h3")],
  cut: ["red", C(6.5, 7, 2.2) + C(6.5, 17, 2.2) + S("M8.3 8.2l11.2 9.3M8.3 15.8l11.2-9.3", "ink")],
  undo: ["blue", P("M8 9.5h7a4.5 4.5 0 0 1 0 9h-4M8 9.5l3.5-3.5M8 9.5l3.5 3.5")],
  redo: ["blue", P("M16 9.5h-7a4.5 4.5 0 0 0 0 9h4M16 9.5l-3.5-3.5M16 9.5l-3.5 3.5")],
  fill: ["ink", F("M6 11l6-6 6 6-6 6z") + K("M18.5 12.5s1.8 2.2 1.8 3.3a1.8 1.8 0 0 1-3.6 0c0-1.1 1.8-3.3 1.8-3.3z", "teal") + R(4, 19, 16, 2.5, 0.5, "k c-amber")],
  fontColor: ["ink", T(12, 15.5, "A", 15, "k") + R(4, 18, 16, 3, 0.5, "k c-red")],
  highlight: ["ink", F("M8 14l7.5-7.5 3 3L11 17H8z") + P("M6 17h2") + R(4, 18.5, 16, 3, 0.5, "k c-amber")],
  grow: ["ink", T(9.5, 18, "A", 15, "k") + S("M17.5 13V5M15 7.5l2.5-2.5 2.5 2.5", "green")],
  shrink: ["ink", T(9.5, 18, "A", 12, "k") + S("M17.5 5v8M15 10.5l2.5 2.5 2.5-2.5", "red")],
  border: ["blue", R(5, 5, 14, 14, 1, "s") + S("M12 5v14M5 12h14", "ink")],
  alignLeft: ["ink", P("M5 6.5h14M5 10.5h9M5 14.5h14M5 18.5h9")],
  alignCenter: ["ink", P("M5 6.5h14M7.5 10.5h9M5 14.5h14M7.5 18.5h9")],
  alignRight: ["ink", P("M5 6.5h14M10 10.5h9M5 14.5h14M10 18.5h9")],
  justify: ["ink", P("M5 6.5h14M5 10.5h14M5 14.5h14M5 18.5h14")],
  wrap: ["ink", P("M5 7h14M5 17h4") + S("M5 12h11a3 3 0 0 1 0 6h-4M14 16l-2 2 2 2", "blue")],
  snowflake: ["teal", P("M12 4v16M5.1 8l13.8 8M5.1 16l13.8-8M10 5.5l2 2 2-2M10 18.5l2-2 2 2")],
  import: ["green", P("M12 4.5v10M8 11l4 4 4-4") + F("M5 15.5v4h14v-4")],
  database: ["teal", `<ellipse cx="12" cy="6.5" rx="6.5" ry="2.5"/>` + F("M5.5 6.5v11c0 1.4 2.9 2.5 6.5 2.5s6.5-1.1 6.5-2.5v-11") + P("M5.5 12c0 1.4 2.9 2.5 6.5 2.5s6.5-1.1 6.5-2.5")],
  refresh: ["green", P("M18.5 9a7 7 0 0 0-12.6-1.5M5.5 15a7 7 0 0 0 12.6 1.5M5.5 4.5v3.5h3.5M18.5 19.5v-3.5h-3.5")],
  code: ["orange", P("M9 7.5l-4.5 4.5 4.5 4.5M15 7.5l4.5 4.5-4.5 4.5") + S("M13 5.5l-2 13", "blue")],
  python: ["blue", F("M12 3.5c-3 0-4.5 1-4.5 3V9h5v1H5.5c-2 0-2.5 2-2.5 3.5S3.7 17 5.5 17h2v-2.5C7.5 13 8.5 12 10 12h5c1.4 0 2.5-1 2.5-2.5v-3c0-2-1.5-3-4.5-3z") + `<path class="f c-amber" d="M12 20.5c3 0 4.5-1 4.5-3V15h-5v-1h7c2 0 2.5-2 2.5-3.5S20.3 7 18.5 7h-2v2.5c0 1.5-1 2.5-2.5 2.5H9c-1.4 0-2.5 1-2.5 2.5v3c0 2 1.5 3 4.5 3z"/>` + C(10, 6, 0.8, "k") + C(14, 18, 0.8, "k c-amber")],
  grid: ["green", R(4.5, 4.5, 15, 15) + R(4.5, 4.5, 15, 5, 1, "k soft") + P("M4.5 9.5h15M4.5 14.5h15M9.5 4.5v15M14.5 4.5v15")],
  pivot: ["green", R(4.5, 4.5, 15, 15) + P("M4.5 9.5h15M9.5 4.5v15") + S("M13 13h3.5v3.5M16.5 13l-3 3", "blue")],
  chart: ["blue", P("M4.5 19.5h15") + R(6, 11, 3, 6.5, 0.5, "k c-blue") + R(10.5, 6, 3, 11.5, 0.5, "k c-green") + R(15, 9, 3, 8.5, 0.5, "k c-amber")],
  sigma: ["green", P("M17 6V5H7l5.5 7L7 19h10v-1")],
  search: ["blue", C(11, 11, 6) + S("M15.5 15.5l4 4", "ink")],
  regex: ["orange", P("M15 4.5v8M11.5 6.5l7 4M11.5 10.5l7-4") + C(6.5, 17, 2, "k c-blue")],
  image: ["teal", R(4.5, 5.5, 15, 13, 1.5) + K("M4.5 17l4.5-5 3.5 3.5 2.5-2.5 4.5 4.5v1.5h-15z", "green") + C(15, 9.5, 1.6, "k c-amber")],
  pen: ["orange", F("M5 19l1-4L16 5l3 3L9 18z") + P("M14 7l3 3")],
  ink: ["orange", F("M5 18l1-4L16 4l3 3L9 17z") + S("M4 21c3-1.5 5 1 8-0.5", "blue")],
  header: ["blue", R(5.5, 4.5, 13, 15) + R(7.5, 6.5, 9, 3, 0.5, "k") + P("M8 13h8M8 16h5")],
  footer: ["blue", R(5.5, 4.5, 13, 15) + P("M8 8h8M8 11h5") + R(7.5, 14.5, 9, 3, 0.5, "k")],
  pageBreak: ["blue", F("M6.5 4.5v5h11v-5") + F("M6.5 19.5v-5h11v5") + S("M4 12h2M8 12h2M12 12h2M16 12h2M20 12h.5", "red")],
  margins: ["blue", R(5.5, 4.5, 13, 15) + S("M8.5 4.5v15M15.5 4.5v15", "orange")],
  rotate: ["blue", R(8, 9, 8, 11, 1) + S("M6.5 9.5a7 7 0 0 1 9-5.5M15.5 4v3h-3", "orange")],
  columns: ["blue", P("M5 6.5h6M5 10.5h6M5 14.5h6M5 18.5h6M13 6.5h6M13 10.5h6M13 14.5h6M13 18.5h4")],
  hyphen: ["blue", P("M4.5 9h6M4.5 15h10") + S("M14 9h5.5", "orange")],
  rtl: ["ink", P("M19 6.5h-14M19 10.5h-9M19 14.5h-14") + S("M9 18.5l-4-2 4-2M5 16.5h9", "blue")],
  footnote: ["blue", P("M5 8h9M5 12h9M5 16h6") + S("M16.5 5l1.5-1v5", "orange") + S("M5 20h14", "ink")],
  quote: ["blue", F("M5.5 7.5h4v4.5c0 2-1.5 4-3.5 4.5M5.5 12h4") + F("M14 7.5h4v4.5c0 2-1.5 4-3.5 4.5M14 12h4")],
  book: ["amber", F("M5.5 5.5h5a1.5 1.5 0 0 1 1.5 1.5v12a1.5 1.5 0 0 0-1.5-1.5h-5z") + F("M18.5 5.5h-5a1.5 1.5 0 0 0-1.5 1.5v12a1.5 1.5 0 0 1 1.5-1.5h5z")],
  list: ["ink", P("M9.5 6.5h9.5M9.5 12h9.5M9.5 17.5h9.5") + C(5.5, 6.5, 1.3, "k c-blue") + C(5.5, 12, 1.3, "k c-blue") + C(5.5, 17.5, 1.3, "k c-blue")],
  numbered: ["ink", P("M10 6.5h9M10 12h9M10 17.5h9") + S("M4.5 5l1.5-1v5M4.5 10.5h2.5l-2.5 3.5h2.5", "blue")],
  toc: ["blue", P("M5 6.5h14M8 10.5h11M8 14.5h11M5 18.5h14")],
  styles: ["blue", R(4, 4.5, 7, 7, 1) + R(13, 4.5, 7, 7, 1, "k") + R(4, 13.5, 7, 7, 1, "k c-amber") + R(13, 13.5, 7, 7, 1)],
  font: ["blue", T(8.5, 17, "A", 15, "k c-ink") + T(16.5, 17, "a", 11, "k")],
  eye: ["blue", F("M3.5 12s3-6 8.5-6 8.5 6 8.5 6-3 6-8.5 6-8.5-6-8.5-6z") + C(12, 12, 2.5, "k")],
  eyeOff: ["ink", P("M3.5 12s3-6 8.5-6c1.6 0 3 .5 4.2 1.2M20.5 12s-3 6-8.5 6c-1.6 0-3-.5-4.2-1.2") + S("M5 19l14-14", "red")],
  check: ["green", P("M5 12.5l4.5 4.5 9.5-10")],
  checks: ["green", P("M3.5 12.5l4 4 8.5-9M11 16.5l.5.5 9-9.5")],
  x: ["red", P("M6.5 6.5l11 11M17.5 6.5l-11 11")],
  xs: ["red", P("M4 7l8 8M12 7l-8 8M12.5 7l8 8M20.5 7l-8 8")],
  arrowRight: ["blue", P("M5 12h14M13.5 6.5l5.5 5.5-5.5 5.5")],
  track: ["orange", F("M5 17l1-4L16 3l3 3L9 16z") + S("M5 20.5h14", "red")],
  markup: ["blue", R(5, 4.5, 14, 15) + S("M8 9h8", "red") + S("M8 13h5", "green") + P("M8 16.5h8")],
  comment: ["amber", F("M5 6.5a1.5 1.5 0 0 1 1.5-1.5h11a1.5 1.5 0 0 1 1.5 1.5v8a1.5 1.5 0 0 1-1.5 1.5h-7l-4 3.5V16a1.5 1.5 0 0 1-1.5-1.5z") + S("M8.5 9h7M8.5 12h4.5", "ink")],
  comments: ["amber", F("M4.5 5.5h10v7h-5l-3 2.5v-2.5h-2z") + `<path class="f c-blue" d="M14.5 9.5h5v7h-2v2.5l-3-2.5h-5v-2.5"/>`],
  sparkle: ["pink", F("M12 4.5l1.6 4.4 4.4 1.6-4.4 1.6L12 16.5l-1.6-4.4L6 10.5l4.4-1.6z") + K("M18 15.5l.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7z", "amber")],
  gear: ["ink", C(12, 12, 2.75) + P("M12 4v2.2M12 17.8V20M4 12h2.2M17.8 12H20M6.3 6.3l1.6 1.6M16.1 16.1l1.6 1.6M6.3 17.7l1.6-1.6M16.1 7.9l1.6-1.6")],
  markdown: ["ink", R(3.5, 6, 17, 12, 1.5) + P("M6.5 15V9l2.5 3 2.5-3v6") + S("M16 9v6M14 13l2 2 2-2", "blue")],
  accessibility: ["blue", C(12, 12, 8.5) + C(12, 7.5, 1.3, "k") + P("M7.5 10h9M12 10v3.5l-2.5 4M12 13.5l2.5 4")],
  slide: ["orange", R(4, 5.5, 16, 11) + P("M12 16.5v3M8.5 19.5h7")],
  slidePlus: ["orange", R(4, 5.5, 16, 11) + S("M12 8.5v5M9.5 11h5", "green")],
  slideSize: ["orange", R(4, 6, 16, 12) + S("M7 12V9h3M17 12v3h-3", "blue")],
  layout: ["orange", R(4, 5.5, 16, 13) + R(4, 5.5, 16, 4, 1, "k soft") + P("M4 9.5h16M10 9.5v9")],
  trash: ["red", P("M5 7h14M10 7V5h4v2") + F("M7 7l1 12.5h8L17 7") + P("M10.5 10.5v6M13.5 10.5v6")],
  layers: ["blue", F("M12 4.5l8 4-8 4-8-4z") + P("M4 12.5l8 4 8-4M4 16.5l8 4 8-4")],
  shapes: ["orange", C(8.5, 8.5, 3.5, "k c-blue") + R(12.5, 12.5, 7, 7, 1, "k") + K("M4.5 19.5l3.5-6 3.5 6z", "green")],
  textBox: ["blue", R(4, 5, 16, 14, 1, "s") + T(12, 16, "A", 11, "k c-ink")],
  halfFill: ["orange", R(5, 5, 14, 14, 1.5) + K("M12 5h5.5A1.5 1.5 0 0 1 19 6.5v11a1.5 1.5 0 0 1-1.5 1.5H12z")],
  palette: ["orange", F("M12 4.5a7.5 7.5 0 0 0 0 15c1 0 1.5-.6 1.5-1.4 0-1.1-1-1.4-1-2.4 0-.8.6-1.2 1.4-1.2h2a3.6 3.6 0 0 0 3.6-3.6c0-3.6-3.4-6.4-7.5-6.4z") + C(8.5, 11, 1.1, "k c-red") + C(11, 8, 1.1, "k c-amber") + C(15, 8.5, 1.1, "k c-blue")],
  transition: ["blue", R(3.5, 7, 8, 10, 1, "k c-orange") + P("M14.5 12h6M18 9l2.5 3-2.5 3")],
  morph: ["blue", C(7, 12, 3, "k c-orange") + R(14, 8, 7, 8, 1, "k") + P("M10.5 12h2")],
  fade: ["blue", R(4, 6, 16, 12) + K("M4 7a1 1 0 0 1 1-1h4v12H5a1 1 0 0 1-1-1z") + P("M14 6v12")],
  clock: ["blue", C(12, 12, 7.5) + S("M12 8v4.5l3 2", "ink")],
  play: ["green", K("M8 5.5v13l10-6.5z")],
  playFrom: ["green", P("M5.5 5.5v13") + K("M9.5 5.5v13l9-6.5z")],
  monitor: ["blue", R(3.5, 5, 17, 11) + R(13.5, 5, 7, 11, 1, "k soft") + P("M9 19.5h6M12 16v3.5")],
  broadcast: ["red", C(12, 12, 1.9, "k") + P("M8.5 8.5a5 5 0 0 0 0 7M15.5 8.5a5 5 0 0 1 0 7M6 6a8.5 8.5 0 0 0 0 12M18 6a8.5 8.5 0 0 1 0 12")],
  mic: ["blue", R(9.5, 4, 5, 10, 2.5) + P("M6.5 11.5a5.5 5.5 0 0 0 11 0M12 17v3")],
  cameo: ["pink", C(12, 12, 7.5) + C(12, 10.5, 2.5, "k") + P("M7.5 17.5c1-2 2.6-3 4.5-3s3.5 1 4.5 3")],
  hexagon: ["pink", F("M12 4l7 4v8l-7 4-7-4V8z")],
  mirror: ["pink", P("M12 4.5v15") + F("M9.5 7l-5 5 5 5z") + K("M14.5 7l5 5-5 5z")],
  video: ["teal", R(4, 7, 11, 10, 1.5) + F("M15 10.5l5-3v9l-5-3z")],
  record: ["red", C(12, 12, 7.5) + C(12, 12, 3.5, "k")],
  lines: ["blue", R(4, 4.5, 16, 15, 1.5) + P("M7 8.5h10M7 12h10M7 15.5h6")],
  film: ["blue", R(4, 5, 16, 14) + P("M8 5v14M16 5v14M4 9h4M4 15h4M16 9h4M16 15h4") + K("M10.5 9.5v5l4-2.5z", "red")],
  outline: ["orange", R(5, 4, 14, 13, 2, "s") + R(4, 19, 16, 2.5, 0.5, "k c-blue")],
  tableTools: ["green", R(4.5, 4.5, 15, 15) + P("M4.5 9.5h15M9.5 9.5v10") + S("M14 14l4 4M18 14l-4 4", "ink")],
  resize: ["teal", R(4.5, 9.5, 10, 10, 1) + S("M14 4.5h5.5V10M19.5 4.5l-6 6", "blue")],
  newSheet: ["green", R(4.5, 4.5, 15, 15) + P("M4.5 9.5h15M9.5 4.5v15") + S("M14.5 12.5v5M12 15h5", "green")],
  percent: ["ink", P("M18 6L6 18") + C(7.5, 7.5, 2.2, "s") + C(16.5, 16.5, 2.2, "s")],
  numFormat: ["ink", P("M10 4.5l-2 15M16 4.5l-2 15M5 9h14M4.5 15h14")],
  subscript: ["ink", T(9, 15, "x", 15, "k") + T(17.5, 20, "2", 9, "k c-blue")],
  superscript: ["ink", T(9, 19, "x", 15, "k") + T(17.5, 10, "2", 9, "k c-blue")],
  clearFormat: ["ink", T(9, 15, "A", 14, "k") + `<path class="f c-pink" d="M13.5 17.5l4-4 3 3-3 3h-2.5z"/>` + S("M11 20.5h8", "pink")],
  changeCase: ["ink", T(8, 17, "A", 14, "k") + T(16.5, 17, "a", 11, "k c-blue")],
  indent: ["ink", P("M11 6.5h8M11 10.5h8M11 14.5h8M5 18.5h14") + S("M4.5 8l3.5 2.5-3.5 2.5", "blue")],
  outdent: ["ink", P("M11 6.5h8M11 10.5h8M11 14.5h8M5 18.5h14") + S("M8 8l-3.5 2.5L8 13", "blue")],
  lineSpacing: ["ink", P("M11 6.5h8M11 10.5h8M11 14.5h8M11 18.5h8") + S("M6 5v15M4 7l2-2 2 2M4 18l2 2 2-2", "blue")],
  replace: ["blue", C(8.5, 8.5, 4.5) + S("M12 12l2.5 2.5", "ink") + S("M14 19.5h6M17.5 17l2.5 2.5-2.5 2.5", "green")],
  selectAll: ["blue", `<rect class="s" x="4" y="4" width="16" height="16" rx="1.5" stroke-dasharray="2.5 2.5"/>` + R(8, 8, 8, 8, 1)],
  brush: ["amber", F("M14 4l6 6-5 5-6-6z") + `<path class="f c-orange" d="M9 9l6 6-2 2c-1.5 1.5-3 1.5-4 .5L7 16c-1-1-1-2.5.5-4z"/>` + S("M6.5 17.5L4 20", "ink")],
  brushPaste: ["amber", F("M14 4l6 6-5 5-6-6z") + `<path class="k c-orange" d="M9 9l6 6-2 2c-1.5 1.5-3 1.5-4 .5L7 16c-1-1-1-2.5.5-4z"/>` + S("M6.5 17.5L4 20", "ink")],
  keyboard: ["ink", R(3, 6.5, 18, 11, 2) + P("M6.5 10h1M10 10h1M13.5 10h1M17 10h.5M7 14h10")],
};

/** Command id → icon shape. Commands not listed keep their text glyph. */
const FOR: Record<string, string> = {
  "file.new": "newDoc", "file.open": "folder", "file.save": "save", "file.saveAs": "saveAs",
  "xl.paste": "clipboard", "xl.copy": "copy", "xl.cut": "cut", "xl.undo": "undo", "xl.redo": "redo",
  "xl.fill": "fill", "xl.color": "fontColor", "xl.border": "border", "xl.alignLeft": "alignLeft", "xl.alignCenter": "alignCenter", "xl.alignRight": "alignRight",
  "xl.wrap": "wrap", "xl.freeze": "snowflake", "xl.addSheet": "newSheet", "xl.importCsv": "import", "xl.importSql": "database", "xl.refresh": "refresh",
  "xl.groupby": "sigma", "xl.pivotby": "pivot", "xl.regex": "regex", "xl.xlookup": "search", "xl.py": "python", "xl.pyEnv": "gear", "xl.pivotTable": "pivot", "xl.chart": "chart",
  "xl.percent": "percent", "xl.numfmt": "numFormat",
  "doc.undo": "undo", "doc.redo": "redo", "doc.copyFormat": "brush", "doc.pasteFormat": "brushPaste",
  "doc.subscript": "subscript", "doc.superscript": "superscript", "doc.grow": "grow", "doc.shrink": "shrink",
  "doc.highlight": "highlight", "doc.color": "fontColor", "doc.changeCase": "changeCase", "doc.clearFormat": "clearFormat",
  "doc.outdent": "outdent", "doc.indent": "indent", "doc.alignLeft": "alignLeft", "doc.alignCenter": "alignCenter",
  "doc.alignRight": "alignRight", "doc.justify": "justify", "doc.lineSpacing": "lineSpacing",
  "doc.find": "search", "doc.replace": "replace", "doc.selectAll": "selectAll",
  "doc.font": "font", "doc.bullets": "list", "doc.numbering": "numbered", "doc.align": "alignLeft", "doc.styles": "styles",
  "doc.table": "grid", "doc.tableTools": "tableTools", "doc.picture": "image", "doc.pictureTools": "resize", "doc.ink": "ink",
  "doc.header": "header", "doc.footer": "footer", "doc.pageBreak": "pageBreak", "doc.margins": "margins", "doc.orientation": "rotate", "doc.columns": "columns",
  "doc.hyphenation": "hyphen", "doc.rtl": "rtl", "doc.footnote": "footnote", "doc.endnote": "footnote",
  "doc.citation": "quote", "doc.sources": "book", "doc.bibliography": "list", "doc.toc": "toc", "doc.updateFields": "refresh",
  "doc.trackChanges": "track", "doc.markup": "markup", "doc.accept": "check", "doc.reject": "x", "doc.acceptAll": "checks", "doc.rejectAll": "xs",
  "doc.nextChange": "arrowRight", "doc.comment": "comment", "doc.comments": "comments", "doc.accessibility": "accessibility", "doc.markdown": "markdown",
  "doc.agent": "sparkle", "doc.rewrite": "pen", "doc.aiSettings": "gear",
  "ppt.newSlide": "slidePlus", "ppt.layout": "layout", "ppt.duplicateSlide": "copy", "ppt.deleteSlide": "trash", "ppt.hideSlide": "eyeOff",
  "ppt.font": "font", "ppt.grow": "grow", "ppt.shrink": "shrink", "ppt.color": "fontColor",
  "ppt.alignLeft": "alignLeft", "ppt.alignCenter": "alignCenter", "ppt.alignRight": "alignRight", "ppt.justify": "justify", "ppt.bullets": "list", "ppt.numbering": "numbered",
  "ppt.fill": "fill", "ppt.outline": "outline", "ppt.arrange": "layers", "ppt.textBox": "textBox", "ppt.shapes": "shapes", "ppt.picture": "image", "ppt.table": "grid",
  "ppt.slideSize": "slideSize", "ppt.background": "halfFill", "ppt.theme": "palette", "ppt.generate": "sparkle", "ppt.transition": "transition",
  "ppt.morph": "morph", "ppt.fade": "fade", "ppt.animate": "sparkle", "ppt.timeline": "clock", "ppt.showStart": "play", "ppt.showCurrent": "playFrom",
  "ppt.presenter": "monitor", "ppt.live": "broadcast", "ppt.coach": "mic", "ppt.cameo": "cameo", "ppt.cameoShape": "hexagon", "ppt.cameoMirror": "mirror",
  "ppt.camera": "video", "ppt.record": "record", "ppt.teleprompter": "lines", "ppt.export": "film",
};

/** SVG markup for an icon shape name. */
export function iconSvg(name: string, size = 18): string | null {
  const s = SHAPES[name];
  if (!s) return null;
  return `<svg xmlns="http://www.w3.org/2000/svg" class="ic ic-${s[0]}" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${s[1]}</svg>`;
}

/** Whether a command has a drawn icon (otherwise it shows its letter glyph). */
export function hasIcon(id: string): boolean {
  return !!FOR[id];
}

/** Icon element for a command: the colour icon, or its letter glyph. */
export function commandIcon(id: string, glyph: string, size = 22): HTMLElement {
  const span = document.createElement("span");
  span.className = "cmd-icon";
  const svg = FOR[id] ? iconSvg(FOR[id]!, size) : null;
  if (svg) span.innerHTML = svg;
  else {
    const key = id.split(".").pop()!;
    span.textContent = key === "strike" ? "S" : glyph;
    span.classList.add("glyph", `glyph-${key}`);
  }
  return span;
}

export function iconNames(): string[] {
  return Object.keys(SHAPES);
}

/** The Prism Office mark: a prism whose lit face takes the current mode's hue. */
export function prismMark(size = 18): HTMLElement {
  const s = document.createElement("span");
  s.className = "prism-mark";
  s.setAttribute("aria-hidden", "true");
  s.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 24 24"><path d="M12 3 21 19H12z" fill="var(--accent)"/><path d="M12 3 3 19h18z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/><path d="M12 3v16" stroke="currentColor" stroke-width="1.6"/></svg>`;
  return s;
}
