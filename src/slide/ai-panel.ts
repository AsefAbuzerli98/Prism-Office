/** "Generate deck" dialog: text, bullets or a Word document → slides (P4-12). */
import { h } from "../core/dom";
import type { HostBridge } from "../core/host";
import { getLocale } from "../core/i18n";
import { errorText, overlay } from "../word/ai-panel";
import { toMarkdown } from "../word/markdown";
import { buildSlides, deckRequest, parseDeck } from "./ai";
import type { SlideEditor } from "./editor";

export function openDeckGenerator(root: HTMLElement, editor: SlideEditor, host: HostBridge) {
  const source = h("textarea", { class: "comment-text", rows: "8", placeholder: "Paste text, bullet points or notes — or load a Word document…", "aria-label": "Source material" });
  const count = h("select", { class: "select", "aria-label": "Slides" }, ...[5, 7, 10, 12, 15].map((n) => h("option", { value: String(n), selected: n === 7 }, String(n))));
  const tone = h("select", { class: "select", "aria-label": "Tone" }, ...["professional", "persuasive", "educational", "inspiring", "casual"].map((t) => h("option", { value: t }, t)));
  const audience = h("input", { class: "open-path", placeholder: "Audience (optional)", "aria-label": "Audience" });
  const replace = h("input", { type: "checkbox", "aria-label": "Replace existing slides" });
  const preview = h("ol", { class: "ai-deck-preview" });
  const status = h("p", { class: "muted small" });
  let slides: ReturnType<typeof buildSlides> | null = null;
  const loadDocx = h(
    "button",
    {
      class: "link",
      onclick: async () => {
        try {
          const path = await host.pickOpenPath();
          if (!path) return;
          status.textContent = "Reading document…";
          const info = await host.openDocument(path);
          try {
            if (info.kind !== "document") throw new Error("Choose a Word document (.docx)");
            const doc = await host.getWordDocument(info.id);
            source.value = toMarkdown(doc.blocks).markdown;
            status.textContent = `Loaded ${info.title} (${source.value.length.toLocaleString()} characters).`;
          } finally {
            await host.closeDocument(info.id).catch(() => {});
          }
        } catch (e) {
          status.textContent = errorText(e);
        }
      },
    },
    "Load a Word document…",
  );
  const insert = h(
    "button",
    {
      class: "btn",
      disabled: true,
      onclick: () => {
        if (!slides) return;
        editor.insertGenerated(slides, replace.checked);
        el.remove();
        editor.focus();
      },
    },
    "Insert slides",
  );
  const generate = h(
    "button",
    {
      class: "btn btn-quiet",
      onclick: async () => {
        if (!source.value.trim()) return void (status.textContent = "Add some source material first.");
        const st = await host.aiStatus().catch(() => null);
        if (st && (!st.enabled || !st.configured)) return void (status.textContent = st.enabled ? "Add an Anthropic API key in AI settings first." : "AI features are switched off in AI settings.");
        generate.disabled = true;
        insert.disabled = true;
        preview.replaceChildren();
        let streamed = "";
        status.textContent = "Designing the deck…";
        try {
          const res = await host.aiComplete(deckRequest(source.value, { slides: Number(count.value), tone: tone.value, language: getLocale(), audience: audience.value.trim() || undefined }), (d) => {
            streamed += d;
            const titles = [...streamed.matchAll(/"title"\s*:\s*"([^"]*)"/g)].map((m) => m[1]!);
            if (titles.length > preview.children.length + 1) preview.replaceChildren(...titles.slice(1).map((t) => h("li", {}, t)));
          });
          if (res.stopReason === "refusal") throw new Error(`The model declined this request (${res.refusal ?? "policy"}).`);
          const deck = parseDeck(res.text);
          slides = buildSlides(editor.model!, deck);
          preview.replaceChildren(...deck.slides.map((s) => h("li", {}, h("strong", {}, s.title), s.notes ? h("span", { class: "muted small" }, ` — ${s.notes.slice(0, 80)}${s.notes.length > 80 ? "…" : ""}`) : "")));
          status.textContent = `${slides.length} slides with speaker notes. Model: ${res.model}.`;
          insert.disabled = false;
        } catch (e) {
          status.textContent = errorText(e);
        } finally {
          generate.disabled = false;
        }
      },
    },
    "Generate",
  );
  const el = overlay(
    root,
    "Generate presentation",
    source,
    loadDocx,
    h("label", { class: "form-row" }, h("span", {}, "Slides"), count),
    h("label", { class: "form-row" }, h("span", {}, "Tone"), tone),
    audience,
    h("label", { class: "radio" }, replace, h("span", {}, "Replace the current slides")),
    preview,
    status,
    h("div", { class: "dialog-actions" }, generate, insert, h("button", { class: "btn btn-quiet", onclick: () => el.remove() }, "Close")),
  );
  source.focus();
}
