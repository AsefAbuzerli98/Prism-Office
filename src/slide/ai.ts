/**
 * AI presentation generator (P4-12): Claude turns text, bullets or a Word
 * document into a structured deck outline (JSON), which is laid out on the
 * presentation's own layouts — title, content, section, two-content — with
 * speaker notes.
 */
import type { AiRequest } from "../core/host";
import type { Layout, Paragraph, PresentationModel, Slide, SlideNode } from "./model";
import { slideFromLayout } from "./ops";

export interface DeckBullet {
  text: string;
  level?: number;
}

export interface DeckSlide {
  layout: "title" | "section" | "content" | "two" | "titleOnly";
  title: string;
  subtitle?: string;
  bullets?: (string | DeckBullet)[];
  left?: (string | DeckBullet)[];
  right?: (string | DeckBullet)[];
  notes?: string;
}

export interface DeckOutline {
  title: string;
  slides: DeckSlide[];
}

export interface DeckOptions {
  slides: number;
  tone: string;
  language: string;
  audience?: string;
}

export function deckRequest(source: string, o: DeckOptions): AiRequest {
  return {
    system:
      'You design slide decks. Answer with one JSON object and nothing else (no code fence, no commentary), shaped as {"title": string, "slides": [{"layout": "title"|"section"|"content"|"two"|"titleOnly", "title": string, "subtitle"?: string, "bullets"?: (string | {"text": string, "level": 1})[], "left"?: [...], "right"?: [...], "notes": string}]}. Slide titles are short (≤ 8 words). Bullets are concise phrases (≤ 12 words), at most 6 per slide, level 1 for a sub-point. "two" slides compare or contrast with "left" and "right" lists. Notes are what the speaker says (2–4 sentences). The first slide uses layout "title" with a subtitle.',
    prompt: [
      `Create a ${o.slides}-slide presentation in the language with code "${o.language}". Tone: ${o.tone}.${o.audience ? ` Audience: ${o.audience}.` : ""}`,
      `Base it only on this material:\n<source>\n${source.slice(0, 120_000)}\n</source>`,
    ].join("\n\n"),
    maxTokens: 8000,
  };
}

/** Extract and validate the JSON outline from a model answer. */
export function parseDeck(text: string): DeckOutline {
  const start = text.indexOf("{");
  const end = text.lastIndexOf("}");
  if (start < 0 || end <= start) throw new Error("The answer contained no deck outline");
  const raw = JSON.parse(text.slice(start, end + 1)) as Partial<DeckOutline>;
  const kinds = ["title", "section", "content", "two", "titleOnly"];
  const slides = (Array.isArray(raw.slides) ? raw.slides : [])
    .filter((s): s is DeckSlide => !!s && typeof s === "object" && typeof (s as DeckSlide).title === "string")
    .map((s) => ({ ...s, layout: (kinds.includes(s.layout) ? s.layout : "content") as DeckSlide["layout"] }));
  if (!slides.length) throw new Error("The deck outline has no slides");
  return { title: typeof raw.title === "string" ? raw.title : (slides[0]?.title ?? "Presentation"), slides };
}

function layoutFor(model: PresentationModel, kind: DeckSlide["layout"]): Layout {
  const all = model.masters.flatMap((m) => m.layouts);
  const want: Record<DeckSlide["layout"], string[]> = {
    title: ["title"],
    section: ["secHead", "title"],
    content: ["obj", "tx", "txAndObj"],
    two: ["twoObj", "twoTxTwoObj", "obj"],
    titleOnly: ["titleOnly", "obj"],
  };
  for (const k of want[kind]) {
    const l = all.find((x) => x.kind === k);
    if (l) return l;
  }
  return all.find((x) => x.kind === "obj") ?? all[0]!;
}

function paras(items: (string | DeckBullet)[] | undefined): Paragraph[] {
  return (items ?? []).map((b) => {
    const t = typeof b === "string" ? { text: b } : b;
    const level = Math.max(0, Math.min(4, t.level ?? 0));
    return { ...(level ? { props: { level } } : {}), runs: [{ t: "text" as const, text: t.text }] };
  });
}

function setText(n: SlideNode, p: Paragraph[]) {
  if (n.type === "shape") n.text = { ...(n.text ?? {}), paragraphs: p.length ? p : [{}] };
}

/** Lay an outline out as slides on the deck's layouts. */
export function buildSlides(model: PresentationModel, deck: DeckOutline): Slide[] {
  return deck.slides.map((d) => {
    const s = slideFromLayout(layoutFor(model, d.layout));
    const ph = (pred: (k: string) => boolean) => s.shapes.filter((n) => n.ph && pred(n.ph.kind ?? "obj"));
    const title = ph((k) => k === "title" || k === "ctrTitle")[0];
    if (title) setText(title, [{ runs: [{ t: "text", text: d.title }] }]);
    const bodies = ph((k) => !["title", "ctrTitle", "dt", "ftr", "sldNum"].includes(k));
    if (d.layout === "title" || d.layout === "section") {
      if (bodies[0]) setText(bodies[0], d.subtitle ? [{ runs: [{ t: "text", text: d.subtitle }] }] : paras(d.bullets));
    } else if (d.layout === "two" && bodies.length >= 2) {
      setText(bodies[0]!, paras(d.left ?? d.bullets));
      setText(bodies[1]!, paras(d.right));
    } else if (bodies[0]) setText(bodies[0], paras(d.bullets ?? [...(d.left ?? []), ...(d.right ?? [])]));
    // drop placeholders left empty on content layouts (keeps the slide clean)
    s.shapes = s.shapes.filter((n) => n === title || n.type !== "shape" || n.text?.paragraphs?.some((p) => p.runs?.length) || d.layout === "title");
    s.notes = d.notes ?? "";
    return s;
  });
}
