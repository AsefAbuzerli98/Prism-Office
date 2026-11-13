/**
 * Pure model operations used by the slide editor (and tests): slides from
 * layouts, ids, shape factories, z-order, grouping, placeholder
 * materialisation and text-wide formatting.
 */
import type { Layout, Paragraph, PresentationModel, RunProps, ShapeBody, Slide, SlideNode, TextBody, Xfrm } from "./model";
import { resolveShape, slideContext } from "./resolve";

export const EMU_PER_PT = 12700;

export function walk(nodes: SlideNode[], f: (n: SlideNode) => void) {
  for (const n of nodes) {
    f(n);
    if (n.type === "group") walk(n.children, f);
  }
}

export function maxId(nodes: SlideNode[]): number {
  let m = 1;
  walk(nodes, (n) => (m = Math.max(m, n.id)));
  return m;
}

/** Give every node in `nodes` a fresh id above those in `slide`. */
export function reid(nodes: SlideNode[], slide: Slide): SlideNode[] {
  let next = maxId(slide.shapes) + 1;
  const copy = structuredClone(nodes);
  walk(copy, (n) => (n.id = next++));
  return copy;
}

/** A new slide using `layout`: its content placeholders, empty. */
export function slideFromLayout(layout: Layout): Slide {
  const shapes: SlideNode[] = [];
  let id = 2;
  for (const n of layout.shapes) {
    if (!n.ph) continue;
    const kind = n.ph.kind ?? "obj";
    if (["dt", "ftr", "sldNum", "hdr"].includes(kind)) continue;
    const ph = { ...(n.ph.kind ? { kind: n.ph.kind } : {}), ...(n.ph.idx !== undefined ? { idx: n.ph.idx } : {}) };
    if (kind === "pic") {
      shapes.push({ type: "shape", id, name: `${n.name.replace(/ \d+$/, "")} ${id - 1}`, ph });
    } else {
      shapes.push({ type: "shape", id, name: `${n.name.replace(/ \d+$/, "")} ${id - 1}`, ph, text: { paragraphs: [{}] } });
    }
    id++;
  }
  return { layout: layout.part, shapes, notes: "" };
}

/** Re-target a slide to another layout, keeping its placeholder content. */
export function applyLayout(slide: Slide, layout: Layout): Slide {
  const fresh = slideFromLayout(layout);
  const used = new Set<SlideNode>();
  const shapes: SlideNode[] = fresh.shapes.map((f) => {
    const t = f.ph?.kind ?? "obj";
    const old = slide.shapes.find((o) => {
      if (!o.ph || used.has(o)) return false;
      const ot = o.ph.kind ?? "obj";
      const titleish = (k: string) => (k === "title" || k === "ctrTitle" ? "title" : k === "subTitle" || k === "body" || k === "obj" ? "body" : k);
      return titleish(ot) === titleish(t);
    });
    if (!old) return f;
    used.add(old);
    const { xfrm: _drop, ...rest } = old as SlideNode & { xfrm?: Xfrm };
    return { ...rest, ph: f.ph } as SlideNode;
  });
  // keep non-placeholder content and placeholders that found no home
  const others = slide.shapes.filter((o) => !used.has(o) && (!o.ph || hasAnyText(o)));
  const all = [...shapes, ...others.map((o) => (o.ph ? { ...o, ph: undefined, xfrm: o.xfrm } : o))] as SlideNode[];
  let id = 2;
  walk(all, (n) => (n.id = id++));
  return { ...slide, layout: layout.part, shapes: all };
}

function hasAnyText(n: SlideNode): boolean {
  return (n.type === "shape" || n.type === "connector") && !!n.text?.paragraphs?.some((p) => p.runs?.some((r) => r.t !== "break" && r.text));
}

/** Plain paragraphs → a text body. */
export function textBody(lines: string[], props?: RunProps): TextBody {
  return { paragraphs: lines.map((l) => ({ runs: l ? [{ t: "text", text: l, ...(props ? { props } : {}) }] : [] })) };
}

export function newTextBox(slide: Slide, x: Xfrm, text = ""): SlideNode {
  const id = maxId(slide.shapes) + 1;
  return {
    type: "shape",
    id,
    name: `TextBox ${id - 1}`,
    textBox: true,
    xfrm: x,
    geom: { t: "preset", name: "rect" },
    fill: { t: "none" },
    text: { body: { wrap: "square", autofit: { t: "shape" } }, paragraphs: text ? textBody([text]).paragraphs : [{}] },
  };
}

/** A preset shape styled like PowerPoint's default (accent1 fill, darker outline, light text). */
export function newShape(slide: Slide, preset: string, x: Xfrm): SlideNode {
  const id = maxId(slide.shapes) + 1;
  const accent = { base: { t: "scheme" as const, v: "accent1" } };
  const line = ["line", "straightConnector1", "bentConnector3", "curvedConnector3"].includes(preset);
  return {
    type: line ? "connector" : "shape",
    id,
    name: `${preset} ${id - 1}`,
    xfrm: x,
    geom: { t: "preset", name: preset },
    ...(line ? {} : { fill: { t: "solid", color: accent } }),
    line: { width: line ? 19050 : 12700, fill: { t: "solid", color: line ? accent : { base: accent.base, mods: [{ name: "shade", val: 50000 }] } } },
    ...(line ? {} : { text: { body: { anchor: "ctr" }, paragraphs: [{ props: { align: "ctr" }, end: { fill: { t: "solid", color: { base: { t: "scheme", v: "lt1" } } } } }] } }),
  } as SlideNode;
}

export function newPicture(slide: Slide, part: string, x: Xfrm): SlideNode {
  const id = maxId(slide.shapes) + 1;
  return { type: "picture", id, name: `Picture ${id - 1}`, xfrm: x, image: { t: "image", relId: part }, geom: { t: "preset", name: "rect" } };
}

export function newTable(slide: Slide, rows: number, cols: number, x: Xfrm): SlideNode {
  const id = maxId(slide.shapes) + 1;
  const colW = Math.round(x.cx / cols);
  const rowH = Math.round(x.cy / rows);
  return {
    type: "frame",
    id,
    name: `Table ${id - 1}`,
    xfrm: x,
    kind: "table",
    table: {
      cols: Array.from({ length: cols }, () => colW),
      rows: Array.from({ length: rows }, () => [rowH, Array.from({ length: cols }, () => ({ text: { paragraphs: [{}] } }))] as [number, { text: TextBody }[]]),
      styleId: "{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}",
      firstRow: true,
      bandRow: true,
    },
  };
}

/** Copy the inherited position into a placeholder so it can be moved/resized. */
export function materialize(model: PresentationModel, slide: Slide, n: SlideNode): Xfrm | null {
  if (n.xfrm) return n.xfrm;
  const res = resolveShape(n, slideContext(model, slide), model);
  if (res.xfrm) n.xfrm = { ...res.xfrm };
  return n.xfrm ?? null;
}

/** Axis-aligned bounds of a rotated xfrm. */
export function bounds(x: Xfrm): { x: number; y: number; w: number; h: number } {
  const a = (((x.rot ?? 0) / 60000) * Math.PI) / 180;
  const c = Math.abs(Math.cos(a));
  const s = Math.abs(Math.sin(a));
  const w = x.cx * c + x.cy * s;
  const h = x.cx * s + x.cy * c;
  const cx = x.x + x.cx / 2;
  const cy = x.y + x.cy / 2;
  return { x: cx - w / 2, y: cy - h / 2, w, h };
}

export function zOrder(slide: Slide, ids: Set<number>, op: "forward" | "backward" | "front" | "back") {
  const sel = slide.shapes.filter((s) => ids.has(s.id));
  const rest = slide.shapes.filter((s) => !ids.has(s.id));
  if (op === "front") slide.shapes = [...rest, ...sel];
  else if (op === "back") slide.shapes = [...sel, ...rest];
  else {
    const arr = slide.shapes;
    const order = op === "forward" ? [...arr.keys()].reverse() : [...arr.keys()];
    for (const i of order) {
      const j = op === "forward" ? i + 1 : i - 1;
      if (j < 0 || j >= arr.length) continue;
      if (ids.has(arr[i]!.id) && !ids.has(arr[j]!.id)) [arr[i], arr[j]] = [arr[j]!, arr[i]!];
    }
  }
}

export function group(slide: Slide, ids: Set<number>, model: PresentationModel): SlideNode | null {
  const sel = slide.shapes.filter((s) => ids.has(s.id));
  if (sel.length < 2) return null;
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const s of sel) {
    const x = materialize(model, slide, s);
    if (!x) continue;
    const b = bounds(x);
    x0 = Math.min(x0, b.x);
    y0 = Math.min(y0, b.y);
    x1 = Math.max(x1, b.x + b.w);
    y1 = Math.max(y1, b.y + b.h);
  }
  if (!isFinite(x0)) return null;
  const id = maxId(slide.shapes) + 1;
  const xf: Xfrm = { x: Math.round(x0), y: Math.round(y0), cx: Math.round(x1 - x0), cy: Math.round(y1 - y0) };
  xf.child = [xf.x, xf.y, xf.cx, xf.cy];
  const g: SlideNode = { type: "group", id, name: `Group ${id - 1}`, xfrm: xf, children: sel.map((s) => (s.ph ? { ...s, ph: undefined } : s)) as SlideNode[] };
  const first = slide.shapes.findIndex((s) => ids.has(s.id));
  const rest = slide.shapes.filter((s) => !ids.has(s.id));
  rest.splice(Math.min(first, rest.length), 0, g);
  slide.shapes = rest;
  return g;
}

/** Map a child xfrm out of its group's coordinate space. */
export function childToSlide(g: Xfrm, c: Xfrm): Xfrm {
  const ch = g.child ?? [g.x, g.y, g.cx, g.cy];
  const sx = ch[2] ? g.cx / ch[2] : 1;
  const sy = ch[3] ? g.cy / ch[3] : 1;
  let cx = g.x + (c.x + c.cx / 2 - ch[0]) * sx;
  let cy = g.y + (c.y + c.cy / 2 - ch[1]) * sy;
  const rot = g.rot ?? 0;
  if (rot) {
    const a = ((rot / 60000) * Math.PI) / 180;
    const gx = g.x + g.cx / 2;
    const gy = g.y + g.cy / 2;
    const dx = cx - gx;
    const dy = cy - gy;
    cx = gx + dx * Math.cos(a) - dy * Math.sin(a);
    cy = gy + dx * Math.sin(a) + dy * Math.cos(a);
  }
  const w = c.cx * sx;
  const h = c.cy * sy;
  return { ...c, x: Math.round(cx - w / 2), y: Math.round(cy - h / 2), cx: Math.round(w), cy: Math.round(h), rot: ((c.rot ?? 0) + rot) % 21600000, child: c.child };
}

export function ungroup(slide: Slide, id: number): SlideNode[] {
  const i = slide.shapes.findIndex((s) => s.id === id);
  const g = slide.shapes[i];
  if (!g || g.type !== "group" || !g.xfrm) return [];
  const kids = g.children.map((c) => {
    if (!c.xfrm) return c;
    const x = childToSlide(g.xfrm!, c.xfrm);
    if (c.type === "group" && c.xfrm.child) {
      // nested group: keep its child space, scale its extents
      return { ...c, xfrm: { ...x, child: c.xfrm.child } };
    }
    return { ...c, xfrm: x };
  }) as SlideNode[];
  slide.shapes.splice(i, 1, ...kids);
  return kids;
}

/** Apply run properties to every run (and end-of-paragraph) of a shape's text. */
export function formatAllText(n: SlideNode, f: (p: RunProps) => RunProps) {
  if (n.type === "group") return n.children.forEach((c) => formatAllText(c, f));
  if (n.type === "frame" && n.table) {
    for (const [, cells] of n.table.rows) for (const c of cells) formatBody(c.text, f);
    return;
  }
  if (n.type !== "shape" && n.type !== "connector") return;
  n.text ??= { paragraphs: [{}] };
  formatBody(n.text, f);
}

function formatBody(t: TextBody, f: (p: RunProps) => RunProps) {
  t.paragraphs ??= [{}];
  for (const p of t.paragraphs) {
    p.runs = (p.runs ?? []).map((r) => ({ ...r, props: f(r.props ?? {}) }));
    p.end = f(p.end ?? {});
  }
}

export function formatParagraphs(n: SlideNode, f: (p: Paragraph) => void) {
  if (n.type === "group") return n.children.forEach((c) => formatParagraphs(c, f));
  if (n.type !== "shape" && n.type !== "connector") return;
  n.text ??= { paragraphs: [{}] };
  n.text.paragraphs ??= [{}];
  n.text.paragraphs.forEach(f);
}

export function shapeBody(n: SlideNode): ShapeBody | null {
  return n.type === "shape" || n.type === "connector" ? n : null;
}

/** Plain text of a slide (outline, search, AI). */
export function slideText(s: Slide): string {
  const out: string[] = [];
  walk(s.shapes, (n) => {
    const b = shapeBody(n);
    for (const p of b?.text?.paragraphs ?? []) out.push((p.runs ?? []).map((r) => (r.t === "break" ? "\n" : r.text)).join(""));
    if (n.type === "frame" && n.table) for (const [, cells] of n.table.rows) out.push(cells.map((c) => (c.text.paragraphs ?? []).map((p) => (p.runs ?? []).map((r) => (r.t === "break" ? " " : r.text)).join("")).join(" ")).join("\t"));
  });
  return out.filter(Boolean).join("\n");
}
