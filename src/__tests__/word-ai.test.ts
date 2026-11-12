import { beforeEach, describe, expect, it } from "vitest";
import { BrowserHost } from "../core/host";
import { altTextRequest, applyDraft, draftRequest, handwritingRequest, rewriteRequest, wordDiff } from "../word/ai";
import { openAgentDialog, openRewriteDialog } from "../word/ai-panel";
import { WordEditor } from "../word/editor";
import { monoMeasurer } from "../word/layout";
import type { Paragraph } from "../word/model";
import { paragraphText } from "../word/ops";

const refs = [
  { title: "Q3 report", text: "Revenue grew 12% year over year, driven by enterprise renewals.", url: "https://example.com/q3" },
  { title: "Survey", text: "Most customers rated onboarding as easy." },
];

describe("AI prompts", () => {
  it("draft request numbers references and asks for verifiable markers", () => {
    const r = draftRequest("- growth\n- satisfaction", refs, "brief", "az");
    expect(r.prompt).toContain('<reference n="1" title="Q3 report" url="https://example.com/q3">');
    expect(r.prompt).toContain("[[n|exact quote]]");
    expect(r.prompt).toContain('code "az"');
    expect(draftRequest("x", [], "report", "en").prompt).toContain("do not add citations");
  });

  it("other requests carry images and modest budgets", () => {
    expect(rewriteRequest("t", "concise").prompt).toContain("<passage>\nt\n</passage>");
    expect(handwritingRequest("AAA").images).toEqual([{ mediaType: "image/png", data: "AAA" }]);
    expect(altTextRequest("image/jpeg", "BBB", "ctx").images![0]!.mediaType).toBe("image/jpeg");
  });
});

describe("applyDraft", () => {
  it("turns citation markers into verified or flagged footnotes and sources", () => {
    const md = "# Summary\n\nRevenue rose sharply [[1|Revenue grew 12% year over year]]. Customers like onboarding [[2|rated onboarding as hard]]. Bogus [[7|x]].\n";
    const d = applyDraft(md, refs, 3);
    expect((d.blocks[0] as Paragraph).style).toBe("Heading1");
    const p = d.blocks[1] as Paragraph;
    expect(paragraphText(p).replace(/￼/g, "")).toBe("Revenue rose sharply. Customers like onboarding. Bogus.");
    expect(p.inlines.filter((i) => i.type === "note").map((i) => (i as { id: number }).id)).toEqual([3, 4]);
    expect(d.citations).toEqual([
      { reference: 1, quote: "Revenue grew 12% year over year", verified: true },
      { reference: 2, quote: "rated onboarding as hard", verified: false },
    ]);
    expect(paragraphText(d.notes[0]!.blocks[0] as Paragraph)).toBe("Q3 report — https://example.com/q3: “Revenue grew 12% year over year”");
    expect(paragraphText(d.notes[1]!.blocks[0] as Paragraph)).toContain("[quote not found in source — verify]");
    expect(d.sources.map((s) => s.tag)).toEqual(["Ref1", "Ref2"]);
  });
});

describe("wordDiff", () => {
  it("marks added and removed words", () => {
    expect(wordDiff("the quick brown fox", "the slow brown fox jumps")).toEqual([
      { op: "same", text: "the " },
      { op: "del", text: "quick" },
      { op: "add", text: "slow" },
      { op: "same", text: " brown fox" },
      { op: "add", text: " jumps" },
    ]);
  });
});

describe("AI dialogs with a stubbed daemon", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) => setTimeout(cb, 0)) as never;
  });

  async function setup(reply: string) {
    const host = new BrowserHost(localStorage, () => null);
    const seen: string[] = [];
    host.aiHandler = async (req, onDelta) => {
      seen.push(req.prompt);
      for (const ch of reply.match(/.{1,8}/gs) ?? []) onDelta?.(ch);
      return { text: reply, stopReason: "end_turn", model: "claude-opus-5-5" };
    };
    const info = await host.newDocument("document");
    const ed = new WordEditor(host, info.id, { measurer: monoMeasurer, syncDelayMs: 5 });
    document.body.append(ed.el);
    await ed.ready;
    return { host, ed, seen };
  }
  const flush = () => new Promise((r) => setTimeout(r, 0));

  it("drafts, streams a preview and inserts content with footnotes", async () => {
    const { host, ed, seen } = await setup("## Growth\n\nRevenue grew [[1|Revenue grew 12%]].\n");
    openAgentDialog(document.body, ed, host);
    const dlg = document.querySelector(".ai-dialog")!;
    (dlg.querySelector("textarea") as HTMLTextAreaElement).value = "- growth";
    const pasteTitle = dlg.querySelector('input[aria-label="Reference title"]') as HTMLInputElement;
    const pasteText = dlg.querySelector('textarea[aria-label="Reference text"]') as HTMLTextAreaElement;
    pasteTitle.value = "https://example.com/q3";
    pasteText.value = refs[0]!.text;
    ([...dlg.querySelectorAll("button")].find((b) => b.textContent === "Add pasted reference") as HTMLButtonElement).click();
    ([...dlg.querySelectorAll("button")].find((b) => b.textContent === "Generate") as HTMLButtonElement).click();
    await flush();
    await flush();
    expect(seen[0]).toContain("Revenue grew 12% year over year");
    expect(dlg.querySelector(".ai-preview")!.textContent).toContain("## Growth");
    expect(dlg.textContent).toContain("all verified against the sources");
    ([...dlg.querySelectorAll("button")].find((b) => b.textContent === "Insert draft") as HTMLButtonElement).click();
    expect((ed.doc!.blocks[0] as Paragraph).style).toBe("Heading2");
    expect(ed.doc!.notes).toHaveLength(1);
    expect(ed.doc!.sources![0]!.url).toBe("https://example.com/q3");
  });

  it("rewrites the selection after showing a diff", async () => {
    const { host, ed } = await setup("Revenue rose.");
    const input = ed.el.querySelector(".word-input") as HTMLTextAreaElement;
    input.value = "Revenue went up a lot.";
    input.dispatchEvent(new Event("input"));
    ed.selectAll();
    await openRewriteDialog(document.body, ed, host, "concise");
    const dlg = document.querySelector(".ai-dialog")!;
    expect(dlg.querySelector("ins")?.textContent).toContain("rose.");
    ([...dlg.querySelectorAll("button")].find((b) => b.textContent === "Accept") as HTMLButtonElement).click();
    expect(paragraphText(ed.doc!.blocks[0] as Paragraph)).toBe("Revenue rose.");
  });

  it("browser preview without a daemon explains AI needs the desktop app", async () => {
    const host = new BrowserHost(localStorage, () => null);
    await expect(host.aiComplete({ system: "", prompt: "" })).rejects.toThrow(/desktop app/);
  });
});
