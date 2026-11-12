/**
 * AI writing features (P2-12, P2-13, P2-14, P2-15 handwriting, P2-16 alt text).
 * Prompts are built here; requests run in the native AI daemon (`ai_complete`).
 */
import type { AiRequest } from "../core/host";
import { fromMarkdown } from "./markdown";
import { isParagraph, isTable, type Block, type Inline, type Note, type Source } from "./model";
import { normalize } from "./ops";

export interface Reference {
  title: string;
  text: string;
  url?: string;
}

export type Tone = "executive" | "technical" | "casual" | "concise" | "friendly" | "formal";

const STYLE_GUIDES: Record<string, string> = {
  report: "a structured report with an executive summary, clearly headed sections and a conclusion",
  brief: "a one-page executive brief: a short summary paragraph, then 3–6 headed points",
  memo: "an internal memo: purpose, background, recommendation, next steps",
  article: "an explanatory article with an introduction, headed body sections and a conclusion",
};

/** Request for the autonomous drafting agent. */
export function draftRequest(outline: string, refs: Reference[], style: keyof typeof STYLE_GUIDES | string, language: string): AiRequest {
  const sources = refs
    .map((r, i) => `<reference n="${i + 1}" title="${r.title.replace(/"/g, "'")}"${r.url ? ` url="${r.url}"` : ""}>\n${r.text}\n</reference>`)
    .join("\n\n");
  return {
    system:
      "You draft documents for a word processor. Write in Markdown using # / ## / ### headings, paragraphs, bullet and numbered lists, and **bold** / *italic* where useful. Do not wrap the answer in a code fence and do not add commentary before or after the document.",
    prompt: [
      `Write ${STYLE_GUIDES[style] ?? style}. Write it in the language with code "${language}".`,
      `Base it on this outline or notes:\n<outline>\n${outline}\n</outline>`,
      refs.length
        ? `Use the reference documents below as your factual basis. Every sentence that states a fact taken from a reference must end with a citation marker of the form [[n|exact quote]], where n is the reference number and the exact quote is a short span (5–25 words) copied verbatim from that reference which supports the sentence. Do not cite anything that is not in the references, and do not invent quotes.\n\n${sources}`
        : "No reference documents were provided, so do not add citations.",
    ].join("\n\n"),
  };
}

const norm = (s: string) => s.toLowerCase().replace(/[‘’]/g, "'").replace(/[“”]/g, '"').replace(/\s+/g, " ").trim();

export interface DraftResult {
  blocks: Block[];
  notes: Note[];
  sources: Source[];
  citations: { reference: number; quote: string; verified: boolean }[];
}

/**
 * Convert the agent's Markdown into blocks. Citation markers become
 * footnotes naming the source and the supporting quote; each quote is
 * checked against the reference text so unverifiable citations are flagged.
 */
export function applyDraft(markdown: string, refs: Reference[], firstNoteId: number): DraftResult {
  const blocks = fromMarkdown(markdown.replace(/^```(?:markdown|md)?\n([\s\S]*?)\n```\s*$/m, "$1"));
  const notes: Note[] = [];
  const citations: DraftResult["citations"] = [];
  let nextId = firstNoteId;
  const marker = /\[\[(\d+)\s*(?:\|\s*([^\]]*?))?\]\]/g;
  const convert = (inlines: Inline[]): Inline[] => {
    const out: Inline[] = [];
    for (const i of inlines) {
      if (i.type !== "text" || !i.text.includes("[[")) {
        out.push(i);
        continue;
      }
      let last = 0;
      for (const m of i.text.matchAll(marker)) {
        if (m.index! > last) out.push({ ...i, text: i.text.slice(last, m.index!).replace(/\s+$/, "") });
        last = m.index! + m[0].length;
        const n = Number(m[1]);
        const ref = refs[n - 1];
        if (!ref) continue; // citation to a reference that doesn't exist: dropped
        const quote = (m[2] ?? "").trim().replace(/^["“]|["”]$/g, "");
        const verified = !!quote && norm(ref.text).includes(norm(quote));
        citations.push({ reference: n, quote, verified });
        const id = nextId++;
        const body = `${ref.title}${ref.url ? ` — ${ref.url}` : ""}${quote ? `: “${quote}”` : ""}${verified ? "" : " [quote not found in source — verify]"}`;
        notes.push({ kind: "footnote", id, blocks: [{ type: "paragraph", style: "FootnoteText", inlines: [{ type: "text", text: body, marks: verified ? {} : { highlight: "yellow" } }] }] });
        out.push({ type: "note", kind: "footnote", id, xml: "" });
      }
      if (last < i.text.length) out.push({ ...i, text: i.text.slice(last) });
    }
    return normalize(out);
  };
  const walk = (bs: Block[]): Block[] =>
    bs.map((b) => (isParagraph(b) ? { ...b, inlines: convert(b.inlines) } : isTable(b) ? { ...b, rows: b.rows.map((r) => ({ ...r, cells: r.cells.map((c) => ({ ...c, blocks: walk(c.blocks) })) })) } : b));
  const walked = walk(blocks);
  const used = new Set(citations.map((c) => c.reference));
  const sources: Source[] = [...used].sort((a, b) => a - b).map((n) => ({
    tag: `Ref${n}`,
    kind: refs[n - 1]!.url ? "DocumentFromInternetSite" : "Misc",
    title: refs[n - 1]!.title,
    authors: [],
    year: "",
    publisher: "",
    url: refs[n - 1]!.url ?? "",
  }));
  return { blocks: walked, notes, sources, citations };
}

export function rewriteRequest(text: string, tone: Tone): AiRequest {
  const how: Record<Tone, string> = {
    executive: "for a senior executive audience: lead with the conclusion, be direct and confident, remove jargon",
    technical: "for a technical audience: precise terminology, concrete detail, no marketing language",
    casual: "in a relaxed, conversational tone",
    concise: "as briefly as possible while keeping every fact: cut filler, merge repetition",
    friendly: "in a warm, approachable tone",
    formal: "in a formal, professional register",
  };
  return {
    system: "You rewrite passages for a word processor. Reply with the rewritten passage only: no preamble, no quotes around it, no explanation. Keep the original language, meaning, facts, numbers and names. Keep paragraph breaks.",
    prompt: `Rewrite the passage ${how[tone]}.\n\n<passage>\n${text}\n</passage>`,
    effort: "medium",
    maxTokens: 16000,
  };
}

export function handwritingRequest(pngBase64: string): AiRequest {
  return {
    system: "You transcribe handwriting. Reply with the transcribed text only. If the image contains no legible handwriting, reply with an empty string.",
    prompt: "Transcribe the handwriting in this image exactly, preserving line breaks.",
    images: [{ mediaType: "image/png", data: pngBase64 }],
    effort: "low",
    maxTokens: 2000,
  };
}

export function altTextRequest(mime: string, base64: string, context: string): AiRequest {
  return {
    system: "You write alternative text for images in documents, for screen-reader users. Reply with the alt text only: one or two sentences, under 250 characters, describing what matters in the image for understanding the document. Do not start with 'Image of'.",
    prompt: `Surrounding text for context:\n<context>\n${context.slice(0, 1500)}\n</context>\n\nWrite the alt text for this image.`,
    images: [{ mediaType: mime, data: base64 }],
    effort: "low",
    maxTokens: 1000,
  };
}

export type DiffOp = { op: "same" | "add" | "del"; text: string };

/** Word-level diff (LCS) for showing a suggestion inline. */
export function wordDiff(a: string, b: string): DiffOp[] {
  const A = a.match(/\s+|[^\s]+/g) ?? [];
  const B = b.match(/\s+|[^\s]+/g) ?? [];
  const n = A.length;
  const m = B.length;
  const dp = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) dp[i]![j] = A[i] === B[j] ? dp[i + 1]![j + 1]! + 1 : Math.max(dp[i + 1]![j]!, dp[i]![j + 1]!);
  const out: DiffOp[] = [];
  const push = (op: DiffOp["op"], text: string) => {
    const last = out[out.length - 1];
    if (last && last.op === op) last.text += text;
    else out.push({ op, text });
  };
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (A[i] === B[j]) {
      push("same", A[i]!);
      i++;
      j++;
    } else if (dp[i + 1]![j]! >= dp[i]![j + 1]!) push("del", A[i++]!);
    else push("add", B[j++]!);
  }
  while (i < n) push("del", A[i++]!);
  while (j < m) push("add", B[j++]!);
  return out;
}
