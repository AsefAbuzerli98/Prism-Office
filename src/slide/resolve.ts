/**
 * Placeholder inheritance (slide → layout → master) and text-style cascade,
 * as PowerPoint applies it. Pure functions over the model; the renderer and
 * editor both use them.
 */
import type {
  Background,
  BodyProps,
  Fill,
  Geometry,
  Layout,
  Line,
  ListStyle,
  Master,
  ParaProps,
  Placeholder,
  PresentationModel,
  RunProps,
  ShapeBody,
  Slide,
  SlideNode,
  TextBody,
  Xfrm,
} from "./model";

export interface SlideContext {
  master: Master;
  layout: Layout;
  masterIndex: number;
}

/** The layout and master a slide uses (falls back to the first ones). */
export function slideContext(model: PresentationModel, slide: Slide): SlideContext {
  for (let i = 0; i < model.masters.length; i++) {
    const m = model.masters[i]!;
    const layout = m.layouts.find((l) => l.part === slide.layout);
    if (layout) return { master: m, layout, masterIndex: i };
  }
  const master = model.masters[0]!;
  return { master, layout: master.layouts[0] ?? { part: "", name: "", shapes: [] }, masterIndex: 0 };
}

/** OOXML default placeholder type is `obj` (when `type` is absent). */
export function phType(ph: Placeholder): string {
  return ph.kind ?? "obj";
}

/** Master placeholder type a layout/slide placeholder inherits from. */
function masterType(t: string): string {
  switch (t) {
    case "ctrTitle":
    case "title":
      return "title";
    case "dt":
    case "ftr":
    case "sldNum":
    case "hdr":
      return t;
    default:
      return "body";
  }
}

/** Which master text style (`p:txStyles`) a placeholder type uses. */
export function textStyleKind(ph: Placeholder | undefined): "title" | "body" | "other" {
  if (!ph) return "other";
  const t = phType(ph);
  if (t === "title" || t === "ctrTitle") return "title";
  if (t === "dt" || t === "ftr" || t === "sldNum" || t === "hdr") return "other";
  return "body";
}

type Sp = SlideNode & { ph?: Placeholder };

function phNodes(nodes: SlideNode[]): Sp[] {
  return nodes.filter((n) => n.ph) as Sp[];
}

/** Find the placeholder a shape inherits from among `nodes`. */
export function matchPlaceholder(ph: Placeholder, nodes: SlideNode[], onMaster: boolean): SlideNode | null {
  const cands = phNodes(nodes);
  const t = phType(ph);
  if (onMaster) {
    const mt = masterType(t);
    return cands.find((n) => masterType(phType(n.ph!)) === mt) ?? null;
  }
  if (ph.idx !== undefined) {
    const byIdx = cands.find((n) => n.ph!.idx === ph.idx);
    if (byIdx) return byIdx;
  }
  return (
    cands.find((n) => phType(n.ph!) === t) ??
    (t === "obj" || t === "body" ? cands.find((n) => ["body", "obj"].includes(phType(n.ph!))) : undefined) ??
    (t === "ctrTitle" ? cands.find((n) => phType(n.ph!) === "title") : undefined) ??
    null
  );
}

/** The inheritance chain of a placeholder, most specific first (excluding the shape). */
export function placeholderChain(node: SlideNode, ctx: SlideContext, level: "slide" | "layout" | "master" = "slide"): SlideNode[] {
  if (!node.ph) return [];
  const out: SlideNode[] = [];
  if (level === "slide") {
    const l = matchPlaceholder(node.ph, ctx.layout.shapes, false);
    if (l) {
      out.push(l);
      const m = matchPlaceholder(l.ph!, ctx.master.shapes, true);
      if (m) out.push(m);
      return out;
    }
  }
  if (level !== "master") {
    const m = matchPlaceholder(node.ph, ctx.master.shapes, true);
    if (m) out.push(m);
  }
  return out;
}

function shapeBody(n: SlideNode): ShapeBody | null {
  return n.type === "shape" || n.type === "connector" ? n : null;
}

/** A shape with inherited position, geometry, fill, line and text properties merged in. */
export interface ResolvedShape {
  xfrm: Xfrm | null;
  geom: Geometry | null;
  fill: Fill | null;
  line: Line | null;
  body: BodyProps;
  /** List styles from lowest to highest priority (master txStyles … shape lstStyle). */
  lists: ListStyle[];
  styleKind: "title" | "body" | "other";
}

export function resolveShape(node: SlideNode, ctx: SlideContext, model: PresentationModel, level: "slide" | "layout" | "master" = "slide"): ResolvedShape {
  const chain = placeholderChain(node, ctx, level);
  const all = [node, ...chain];
  const pick = <T>(f: (n: SlideNode) => T | undefined | null): T | null => {
    for (const n of all) {
      const v = f(n);
      if (v !== undefined && v !== null) return v;
    }
    return null;
  };
  const styleKind = textStyleKind(node.ph);
  // Placeholders use the master's text style for their kind; other text uses
  // the presentation default (slides) or the master's otherStyle (masters/layouts).
  const base = node.ph ? ctx.master.textStyles[styleKind] : level === "slide" ? (model.defaultTextStyle ?? {}) : ctx.master.textStyles.other;
  const lists: ListStyle[] = [base];
  for (const n of [...all].reverse()) {
    const ls = shapeBody(n)?.text?.listStyle;
    if (ls) lists.push(ls);
  }
  const body: BodyProps = {};
  for (const n of [...all].reverse()) {
    const b = shapeBody(n)?.text?.body;
    if (b) mergeInto(body, b);
  }
  return {
    xfrm: pick((n) => n.xfrm),
    geom: pick((n) => shapeBody(n)?.geom ?? (n.type === "picture" ? n.geom : undefined)),
    fill: pick((n) => shapeBody(n)?.fill),
    line: pick((n) => shapeBody(n)?.line ?? (n.type === "picture" ? n.line : undefined)),
    body,
    lists,
    styleKind,
  };
}

function mergeInto<T extends object>(dst: T, src: T) {
  for (const [k, v] of Object.entries(src)) {
    if (v !== undefined && v !== null) (dst as Record<string, unknown>)[k] = v;
  }
}

/** Merge run properties (later wins). */
export function mergeRun(...rs: (RunProps | undefined | null)[]): RunProps {
  const out: RunProps = {};
  for (const r of rs) if (r) mergeInto(out, r);
  return out;
}

/** Effective paragraph props at a level: list styles cascade, then the paragraph's own. */
export function effectivePara(lists: ListStyle[], own: ParaProps | undefined): ParaProps {
  const lvl = own?.level ?? 0;
  const out: ParaProps = {};
  let run: RunProps = {};
  for (const ls of lists) {
    for (const p of [ls.default, ls.levels?.[lvl]]) {
      if (!p) continue;
      const { run: r, ...rest } = p;
      mergeInto(out, rest);
      run = mergeRun(run, r);
    }
  }
  if (own) {
    const { run: r, ...rest } = own;
    mergeInto(out, rest);
    run = mergeRun(run, r);
  }
  out.run = run;
  return out;
}

/** Effective background of a slide (slide → layout → master). */
export function effectiveBackground(slide: Slide | null, ctx: SlideContext): Background | null {
  return slide?.background ?? ctx.layout.background ?? ctx.master.background ?? null;
}

/** Does a text body contain any visible text? */
export function hasText(t: TextBody | undefined): boolean {
  return !!t?.paragraphs?.some((p) => p.runs?.some((r) => r.t !== "break" && r.text.length > 0));
}

/** Prompt shown in an empty placeholder while editing. */
export function placeholderPrompt(ph: Placeholder): string {
  switch (phType(ph)) {
    case "title":
    case "ctrTitle":
      return "Click to add title";
    case "subTitle":
      return "Click to add subtitle";
    case "dt":
    case "ftr":
    case "sldNum":
      return "";
    case "pic":
      return "Click to add picture";
    default:
      return "Click to add text";
  }
}
