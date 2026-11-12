import { h } from "../core/dom";
import { checkAccessibility, readability } from "./a11y";
import { altTextRequest } from "./ai";
import type { WordEditor } from "./editor";
import { isParagraph } from "./model";
import { paragraphText } from "./ops";

/** Side-panel content for the accessibility checker. */
export function renderA11yPanel(editor: WordEditor): HTMLElement[] {
  const doc = editor.doc;
  if (!doc) return [];
  const issues = checkAccessibility(doc);
  const r = readability(doc);
  const generate = async (pos: { block: number; offset: number }, relId: string, status: HTMLElement) => {
    status.textContent = "Generating…";
    try {
      const img = await editor.host.getWordImage(editor.docId, relId);
      if (!img) throw new Error("Picture data not available.");
      const b = doc.blocks[pos.block];
      const context = b && isParagraph(b) ? paragraphText(b) : "";
      const res = await editor.host.aiComplete(altTextRequest(img.mime, img.data, context));
      if (res.stopReason === "refusal") throw new Error("The model declined this request.");
      editor.setImageAlt(pos, res.text.trim());
    } catch (e) {
      status.textContent = String(e instanceof Error ? e.message : e);
    }
  };
  return [
    h("h3", { class: "side-title" }, `Accessibility (${issues.length})`),
    h(
      "p",
      { class: "small" },
      `Readability: ${r.grade} — LIX ${r.lix}, Flesch ${r.flesch} · ${r.words} words, ${r.sentences} sentences`,
    ),
    ...(issues.length
      ? issues.map((i) => {
          const status = h("span", { class: "muted small" });
          return h(
            "article",
            { class: `comment-card a11y-${i.severity}`, dataset: { kind: i.kind } },
            h("strong", { class: "small" }, i.severity.toUpperCase()),
            h("p", { class: "small" }, i.message),
            h(
              "div",
              { class: "a11y-actions" },
              i.pos ? h("button", { class: "link", onclick: () => editor.goTo(i.pos!) }, "Go to") : null,
              i.kind === "alt-text" && i.pos && i.relId ? h("button", { class: "link", onclick: () => void generate(i.pos!, i.relId!, status) }, "Generate alt text") : null,
              i.kind === "alt-text" && i.pos
                ? h("button", { class: "link", onclick: () => { const t = window.prompt("Alternative text"); if (t) editor.setImageAlt(i.pos!, t); } }, "Write alt text")
                : null,
              i.kind === "table-header" && i.pos ? h("button", { class: "link", onclick: () => editor.markHeaderRow(i.pos!.block) }, "Mark header row") : null,
              status,
            ),
          );
        })
      : [h("p", { class: "muted small" }, "No accessibility issues found.")]),
  ];
}
