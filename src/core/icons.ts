/**
 * Ribbon and shell icons: one outline set (24-unit grid, 1.5 stroke,
 * currentColor) so every command reads in the same hand. Commands whose
 * glyph is a letter (B, I, U, Σ, %) keep the letter — it is the clearest icon.
 */

const P = (d: string) => `<path d="${d}"/>`;

const SHAPES: Record<string, string> = {
  plus: P("M12 5v14M5 12h14"),
  folder: P("M3.5 7.5a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"),
  save: P("M5 4.5h11l3.5 3.5v11.5h-14.5zM8 4.5v5h7v-5M8 19.5v-6h8v6"),
  saveAs: P("M5 4.5h9l3 3v4M8 4.5v4h5v-4M5 4.5v15h6M14 19.5l1-3 4.5-4.5 2 2-4.5 4.5z"),
  clipboard: P("M9 4.5h6v2.5h-6zM9 5.5h-2.5v14h11v-14h-2.5"),
  copy: P("M8.5 8.5h10v11h-10zM15.5 8.5v-4h-10v11h3"),
  cut: P("M6.5 7a2 2 0 1 0 0 .01M6.5 17a2 2 0 1 0 0 .01M8.3 8.2l11.2 9.3M8.3 15.8l11.2-9.3"),
  undo: P("M8 9.5h7a4.5 4.5 0 0 1 0 9h-4M8 9.5l3.5-3.5M8 9.5l3.5 3.5"),
  redo: P("M16 9.5h-7a4.5 4.5 0 0 0 0 9h4M16 9.5l-3.5-3.5M16 9.5l-3.5 3.5"),
  fill: P("M6 19.5h12") + `<rect x="7" y="5" width="10" height="10" rx="1.5"/>`,
  border: `<rect x="5" y="5" width="14" height="14" rx="1"/>` + P("M12 5v14M5 12h14"),
  alignLeft: P("M5 6.5h14M5 10.5h9M5 14.5h14M5 18.5h9"),
  alignCenter: P("M5 6.5h14M7.5 10.5h9M5 14.5h14M7.5 18.5h9"),
  alignRight: P("M5 6.5h14M10 10.5h9M5 14.5h14M10 18.5h9"),
  justify: P("M5 6.5h14M5 10.5h14M5 14.5h14M5 18.5h14"),
  wrap: P("M5 7h14M5 12h11a3 3 0 0 1 0 6h-4M14 16l-2 2 2 2M5 17h4"),
  pin: P("M9 4.5h6l-1 5 3 3v1.5h-10v-1.5l3-3zM12 14v5.5"),
  import: P("M12 4.5v10M8 11l4 4 4-4M5 15.5v4h14v-4"),
  database: `<ellipse cx="12" cy="6.5" rx="6.5" ry="2.5"/>` + P("M5.5 6.5v11c0 1.4 2.9 2.5 6.5 2.5s6.5-1.1 6.5-2.5v-11M5.5 12c0 1.4 2.9 2.5 6.5 2.5s6.5-1.1 6.5-2.5"),
  refresh: P("M18.5 9a7 7 0 0 0-12.6-1.5M5.5 15a7 7 0 0 0 12.6 1.5M5.5 4.5v3.5h3.5M18.5 19.5v-3.5h-3.5"),
  code: P("M9 7.5l-4.5 4.5 4.5 4.5M15 7.5l4.5 4.5-4.5 4.5M13 5.5l-2 13"),
  grid: `<rect x="4.5" y="4.5" width="15" height="15" rx="1"/>` + P("M4.5 9.5h15M4.5 14.5h15M9.5 4.5v15M14.5 4.5v15"),
  chart: P("M5 19.5h14M7.5 16.5v-5M12 16.5v-10M16.5 16.5v-7"),
  image: `<rect x="4.5" y="5.5" width="15" height="13" rx="1.5"/>` + P("M4.5 15.5l4-4 3.5 3.5 2.5-2.5 5 5") + `<circle cx="15" cy="9.5" r="1.25"/>`,
  pen: P("M5 19l1-4L16 5l3 3L9 18zM14 7l3 3"),
  header: `<rect x="5.5" y="4.5" width="13" height="15" rx="1"/>` + P("M8 7.5h8"),
  footer: `<rect x="5.5" y="4.5" width="13" height="15" rx="1"/>` + P("M8 16.5h8"),
  pageBreak: P("M6.5 4.5v5h11v-5M6.5 19.5v-5h11v5M4 12h2M8 12h2M12 12h2M16 12h2M20 12h.5"),
  margins: `<rect x="5.5" y="4.5" width="13" height="15" rx="1"/>` + P("M8.5 4.5v15M15.5 4.5v15"),
  rotate: P("M17.5 8.5a6.5 6.5 0 1 0 1 6M18.5 4.5v4h-4"),
  columns: P("M5 6.5h6M5 10.5h6M5 14.5h6M5 18.5h6M13 6.5h6M13 10.5h6M13 14.5h6M13 18.5h4"),
  rtl: P("M19 6.5h-14M19 10.5h-9M19 14.5h-14M9 18.5l-4-2 4-2"),
  quote: P("M6 16.5c2 0 3.5-1.5 3.5-3.5v-5.5h-4v4.5h3M14.5 16.5c2 0 3.5-1.5 3.5-3.5v-5.5h-4v4.5h3"),
  book: P("M5.5 5.5h5a1.5 1.5 0 0 1 1.5 1.5v12a1.5 1.5 0 0 0-1.5-1.5h-5zM18.5 5.5h-5a1.5 1.5 0 0 0-1.5 1.5v12a1.5 1.5 0 0 1 1.5-1.5h5z"),
  list: P("M9.5 6.5h9.5M9.5 12h9.5M9.5 17.5h9.5M5 6.5h.5M5 12h.5M5 17.5h.5"),
  toc: P("M5 6.5h14M8 10.5h11M8 14.5h11M5 18.5h14"),
  eye: P("M3.5 12s3-6 8.5-6 8.5 6 8.5 6-3 6-8.5 6-8.5-6-8.5-6z") + `<circle cx="12" cy="12" r="2.5"/>`,
  eyeOff: P("M3.5 12s3-6 8.5-6c1.6 0 3 .5 4.2 1.2M20.5 12s-3 6-8.5 6c-1.6 0-3-.5-4.2-1.2M5 19l14-14"),
  check: P("M5 12.5l4.5 4.5 9.5-10"),
  checks: P("M3.5 12.5l4 4 8.5-9M11 16.5l.5.5 9-9.5"),
  x: P("M6.5 6.5l11 11M17.5 6.5l-11 11"),
  xs: P("M4 7l8 8M12 7l-8 8M12.5 7l8 8M20.5 7l-8 8"),
  arrowRight: P("M5 12h14M13.5 6.5l5.5 5.5-5.5 5.5"),
  comment: P("M5 6.5a1.5 1.5 0 0 1 1.5-1.5h11a1.5 1.5 0 0 1 1.5 1.5v8a1.5 1.5 0 0 1-1.5 1.5h-7l-4 3.5v-3.5h0a1.5 1.5 0 0 1-1.5-1.5z"),
  comments: P("M4.5 5.5h10v7h-5l-3 2.5v-2.5h-2zM14.5 9.5h5v7h-2v2.5l-3-2.5h-5v-2.5"),
  person: `<circle cx="12" cy="6.5" r="2"/>` + P("M5.5 10h13M12 10v4.5l-3 5M12 14.5l3 5"),
  sparkle: P("M12 4.5l1.6 4.4 4.4 1.6-4.4 1.6L12 16.5l-1.6-4.4L6 10.5l4.4-1.6zM18 15.5l.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7z"),
  gear: `<circle cx="12" cy="12" r="2.75"/>` + P("M12 4v2.2M12 17.8V20M4 12h2.2M17.8 12H20M6.3 6.3l1.6 1.6M16.1 16.1l1.6 1.6M6.3 17.7l1.6-1.6M16.1 7.9l1.6-1.6"),
  slide: `<rect x="4" y="5.5" width="16" height="11" rx="1"/>` + P("M12 16.5v3M8.5 19.5h7"),
  slidePlus: `<rect x="4" y="5.5" width="16" height="11" rx="1"/>` + P("M12 8.5v5M9.5 11h5"),
  layout: `<rect x="4" y="5.5" width="16" height="13" rx="1"/>` + P("M4 9.5h16M10 9.5v9"),
  trash: P("M5 7h14M10 7V5h4v2M7 7l1 12.5h8L17 7M10.5 10.5v6M13.5 10.5v6"),
  layers: P("M12 4.5l8 4-8 4-8-4zM4 12.5l8 4 8-4M4 16.5l8 4 8-4"),
  shapes: `<circle cx="8.5" cy="8.5" r="3.5"/>` + `<rect x="12.5" y="12.5" width="7" height="7" rx="1"/>` + P("M5 19.5l3.5-6 3.5 6z"),
  frame: `<rect x="4" y="6" width="16" height="12" rx="1"/>` + P("M8 6v12M4 10h4"),
  halfFill: `<rect x="5" y="5" width="14" height="14" rx="1.5"/>` + P("M12 5v14M12 9h7M12 13h7M12 17h7"),
  palette: P("M12 4.5a7.5 7.5 0 0 0 0 15c1 0 1.5-.6 1.5-1.4 0-1.1-1-1.4-1-2.4 0-.8.6-1.2 1.4-1.2h2a3.6 3.6 0 0 0 3.6-3.6c0-3.6-3.4-6.4-7.5-6.4z") + `<circle cx="8.5" cy="11" r=".8"/><circle cx="11" cy="8" r=".8"/><circle cx="15" cy="8.5" r=".8"/>`,
  transition: `<rect x="3.5" y="7" width="8" height="10" rx="1"/>` + P("M14.5 12h6M18 9l2.5 3-2.5 3"),
  morph: `<circle cx="7" cy="12" r="3"/>` + `<rect x="14" y="8" width="7" height="8" rx="1"/>` + P("M10.5 12h2"),
  fade: `<rect x="4" y="6" width="16" height="12" rx="1"/>` + P("M9 6v12M14 6v12"),
  clock: `<circle cx="12" cy="12" r="7.5"/>` + P("M12 8v4.5l3 2"),
  play: P("M8 5.5v13l10-6.5z"),
  playFrom: P("M5.5 5.5v13M9.5 5.5v13l9-6.5z"),
  monitor: `<rect x="3.5" y="5" width="17" height="11" rx="1"/>` + P("M9 19.5h6M12 16v3.5M14 5v11"),
  broadcast: `<circle cx="12" cy="12" r="1.75"/>` + P("M8.5 8.5a5 5 0 0 0 0 7M15.5 8.5a5 5 0 0 1 0 7M6 6a8.5 8.5 0 0 0 0 12M18 6a8.5 8.5 0 0 1 0 12"),
  mic: `<rect x="9.5" y="4" width="5" height="10" rx="2.5"/>` + P("M6.5 11.5a5.5 5.5 0 0 0 11 0M12 17v3"),
  cameo: `<circle cx="12" cy="12" r="7.5"/>` + `<circle cx="12" cy="10.5" r="2.5"/>` + P("M7.5 17.5c1-2 2.6-3 4.5-3s3.5 1 4.5 3"),
  hexagon: P("M12 4l7 4v8l-7 4-7-4V8z"),
  mirror: P("M12 4.5v15M9.5 7l-5 5 5 5zM14.5 7l5 5-5 5z"),
  video: `<rect x="4" y="7" width="11" height="10" rx="1.5"/>` + P("M15 10.5l5-3v9l-5-3"),
  record: `<circle cx="12" cy="12" r="7.5"/><circle cx="12" cy="12" r="3.5" fill="currentColor"/>`,
  lines: P("M5 7h14M5 11h14M5 15h14M5 19h8"),
  film: `<rect x="4" y="5" width="16" height="14" rx="1"/>` + P("M8 5v14M16 5v14M4 9h4M4 15h4M16 9h4M16 15h4"),
  search: `<circle cx="11" cy="11" r="6"/>` + P("M15.5 15.5l4 4"),
  snowflake: P("M12 4v16M5.1 8l13.8 8M5.1 16l13.8-8M10 5.5l2 2 2-2M10 18.5l2-2 2 2"),
  layoutGrid: `<rect x="4.5" y="4.5" width="6.5" height="6.5" rx="1"/><rect x="13" y="4.5" width="6.5" height="6.5" rx="1"/><rect x="4.5" y="13" width="6.5" height="6.5" rx="1"/><rect x="13" y="13" width="6.5" height="6.5" rx="1"/>`,
  outline: `<rect x="5" y="5" width="14" height="14" rx="2"/>`,
  tableTools: `<rect x="4.5" y="4.5" width="15" height="15" rx="1"/>` + P("M4.5 9.5h15M9.5 9.5v10"),
  resize: P("M14 4.5h5.5V10M10 19.5H4.5V14M19.5 4.5l-6 6M4.5 19.5l6-6"),
};

/** Command id → icon shape. Commands not listed keep their text glyph. */
const FOR: Record<string, string> = {
  "file.new": "plus", "file.open": "folder", "file.save": "save", "file.saveAs": "saveAs",
  "xl.paste": "clipboard", "xl.copy": "copy", "xl.cut": "cut", "xl.undo": "undo", "xl.redo": "redo",
  "xl.fill": "fill", "xl.border": "border", "xl.alignLeft": "alignLeft", "xl.alignCenter": "alignCenter", "xl.alignRight": "alignRight",
  "xl.wrap": "wrap", "xl.freeze": "pin", "xl.addSheet": "plus", "xl.importCsv": "import", "xl.importSql": "database", "xl.refresh": "refresh",
  "xl.pivotby": "grid", "xl.xlookup": "search", "xl.py": "code", "xl.pyEnv": "gear", "xl.pivotTable": "grid", "xl.chart": "chart",
  "doc.align": "alignLeft", "doc.table": "grid", "doc.tableTools": "tableTools", "doc.picture": "image", "doc.pictureTools": "resize", "doc.ink": "pen",
  "doc.header": "header", "doc.footer": "footer", "doc.pageBreak": "pageBreak", "doc.margins": "margins", "doc.orientation": "rotate", "doc.columns": "columns",
  "doc.rtl": "rtl", "doc.citation": "quote", "doc.sources": "book", "doc.bibliography": "list", "doc.toc": "toc", "doc.updateFields": "refresh",
  "doc.trackChanges": "pen", "doc.markup": "eye", "doc.accept": "check", "doc.reject": "x", "doc.acceptAll": "checks", "doc.rejectAll": "xs",
  "doc.nextChange": "arrowRight", "doc.comment": "comment", "doc.comments": "comments", "doc.accessibility": "person", "doc.agent": "sparkle",
  "doc.rewrite": "pen", "doc.aiSettings": "gear", "doc.bullets": "list", "doc.styles": "layers", "doc.hyphenation": "wrap",
  "ppt.newSlide": "slidePlus", "ppt.layout": "layout", "ppt.duplicateSlide": "copy", "ppt.deleteSlide": "trash", "ppt.hideSlide": "eyeOff",
  "ppt.alignLeft": "alignLeft", "ppt.alignCenter": "alignCenter", "ppt.alignRight": "alignRight", "ppt.justify": "justify", "ppt.bullets": "list",
  "ppt.fill": "fill", "ppt.outline": "outline", "ppt.arrange": "layers", "ppt.textBox": "frame", "ppt.shapes": "shapes", "ppt.picture": "image", "ppt.table": "grid",
  "ppt.slideSize": "slide", "ppt.background": "halfFill", "ppt.theme": "palette", "ppt.generate": "sparkle", "ppt.transition": "transition",
  "ppt.morph": "morph", "ppt.fade": "fade", "ppt.animate": "sparkle", "ppt.timeline": "clock", "ppt.showStart": "play", "ppt.showCurrent": "playFrom",
  "ppt.presenter": "monitor", "ppt.live": "broadcast", "ppt.coach": "mic", "ppt.cameo": "cameo", "ppt.cameoShape": "hexagon", "ppt.cameoMirror": "mirror",
  "ppt.camera": "video", "ppt.record": "record", "ppt.teleprompter": "lines", "ppt.export": "film",
};

/** SVG markup for an icon shape name. */
export function iconSvg(name: string, size = 18): string | null {
  const body = SHAPES[name];
  if (!body) return null;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${body}</svg>`;
}

/** Icon element for a command: the outline icon, or its letter glyph. */
export function commandIcon(id: string, glyph: string): HTMLElement {
  const span = document.createElement("span");
  span.className = "cmd-icon";
  const svg = FOR[id] ? iconSvg(FOR[id]!) : null;
  if (svg) span.innerHTML = svg;
  else {
    span.textContent = glyph;
    span.classList.add("glyph");
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
