/** Dialogs for the AI agent, rewriting and AI settings. */
import { h } from "../core/dom";
import type { HostBridge } from "../core/host";
import { getLocale } from "../core/i18n";
import { applyDraft, draftRequest, rewriteRequest, wordDiff, type Reference, type Tone } from "./ai";
import type { WordEditor } from "./editor";

function overlay(root: HTMLElement, label: string, ...children: (HTMLElement | null)[]) {
  document.querySelector(".ai-overlay")?.remove();
  const el = h(
    "div",
    { class: "overlay ai-overlay", onmousedown: (e: Event) => e.target === el && el.remove(), onkeydown: (e: KeyboardEvent) => e.key === "Escape" && el.remove() },
    h("section", { class: "dialog ai-dialog", role: "dialog", "aria-modal": "true", "aria-label": label }, h("h2", {}, label), ...children),
  );
  root.append(el);
  return el;
}

const errorText = (e: unknown) => String(e instanceof Error ? e.message : e);

export async function openAiSettings(root: HTMLElement, host: HostBridge) {
  const st = await host.aiStatus();
  const key = h("input", { class: "open-path", type: "password", placeholder: st.configured ? `Configured (${st.keySource}) — leave blank to keep` : "sk-ant-…", "aria-label": "Anthropic API key", autocomplete: "off" });
  const model = h("input", { class: "open-path", value: st.model, "aria-label": "Model" });
  const effort = h("select", { class: "select", "aria-label": "Effort" }, ...["low", "medium", "high", "xhigh", "max"].map((e) => h("option", { value: e, selected: e === st.effort }, e)));
  const enabled = h("input", { type: "checkbox", checked: st.enabled, "aria-label": "Allow AI features" });
  const where = st.keySource === "keychain" ? "in the system keychain" : st.keySource === "file" ? "in an owner-only settings file (no keychain available)" : st.keySource === "environment" ? "from ANTHROPIC_API_KEY" : "";
  const msg = h("p", { class: "muted small" }, st.configured ? `AI is configured; the key is stored ${where} and never sent to the page.` : "Add an Anthropic API key to enable drafting, rewriting, alt text and handwriting recognition.");
  const el = overlay(
    root,
    "AI settings",
    msg,
    h("label", { class: "radio" }, enabled, h("span", {}, "Allow AI features (when off, no document content leaves this computer)")),
    h("label", { class: "form-row" }, h("span", {}, "API key"), key),
    h("label", { class: "form-row" }, h("span", {}, "Model"), model),
    h("label", { class: "form-row" }, h("span", {}, "Effort"), effort),
    h(
      "div",
      { class: "dialog-actions" },
      h("button", {
        class: "btn btn-quiet",
        onclick: async () => {
          try {
            const s = await host.setAiConfig({ apiKey: key.value ? key.value : undefined, model: model.value, effort: effort.value, enabled: enabled.checked });
            msg.textContent = s.configured ? "Saved." : "Saved — no key configured yet.";
            key.value = "";
          } catch (e) {
            msg.textContent = errorText(e);
          }
        },
      }, "Save"),
      h("button", { class: "btn", onclick: () => el.remove() }, "Close"),
    ),
  );
}

/** Autonomous drafting agent: outline + reference files → cited draft. */
export function openAgentDialog(root: HTMLElement, editor: WordEditor, host: HostBridge) {
  const refs: Reference[] = [];
  const outline = h("textarea", { class: "comment-text", rows: "6", placeholder: "Bullet points, an outline or notes…", "aria-label": "Outline" });
  const style = h("select", { class: "select", "aria-label": "Style" }, ...[["report", "Report"], ["brief", "Executive brief"], ["memo", "Memo"], ["article", "Article"]].map(([v, l]) => h("option", { value: v! }, l!)));
  const refList = h("ul", { class: "service-list" });
  const renderRefs = () => refList.replaceChildren(...(refs.length ? refs.map((r, i) => h("li", { class: "source-item" }, h("span", {}, `[${i + 1}] ${r.title} (${r.text.length.toLocaleString()} chars)`), h("button", { class: "link", onclick: () => { refs.splice(i, 1); renderRefs(); } }, "×"))) : [h("li", { class: "muted small" }, "No reference files (optional).")]));
  renderRefs();
  const files = h("input", {
    type: "file",
    multiple: true,
    accept: ".txt,.md,.markdown,.csv,.json,.html,.htm,.xml,text/*",
    onchange: async (e: Event) => {
      for (const f of [...((e.target as HTMLInputElement).files ?? [])]) {
        const text = await f.text();
        refs.push({ title: f.name, text: f.type.includes("html") || /\.html?$/i.test(f.name) ? new DOMParser().parseFromString(text, "text/html").body.textContent ?? "" : text });
      }
      renderRefs();
    },
  });
  const pasteTitle = h("input", { class: "open-path", placeholder: "Reference title or URL", "aria-label": "Reference title" });
  const pasteText = h("textarea", { class: "comment-text", rows: "3", placeholder: "…or paste reference text", "aria-label": "Reference text" });
  const preview = h("pre", { class: "ai-preview" });
  const status = h("p", { class: "muted small" });
  let result: ReturnType<typeof applyDraft> | null = null;
  const insert = h("button", { class: "btn", disabled: true, onclick: () => { if (result) { editor.insertDraft(result); el.remove(); } } }, "Insert draft");
  const generate = h("button", {
    class: "btn btn-quiet",
    onclick: async () => {
      if (!outline.value.trim()) return void (status.textContent = "Write an outline first.");
      generate.disabled = true;
      insert.disabled = true;
      preview.textContent = "";
      status.textContent = "Drafting…";
      try {
        const res = await host.aiComplete(draftRequest(outline.value, refs, style.value, getLocale()), (d) => {
          preview.textContent += d;
          preview.scrollTop = preview.scrollHeight;
        });
        if (res.stopReason === "refusal") throw new Error(`The model declined this request (${res.refusal ?? "policy"}).`);
        result = applyDraft(res.text, refs, editor.nextNoteId());
        const unverified = result.citations.filter((c) => !c.verified).length;
        status.textContent = `${result.blocks.length} blocks, ${result.citations.length} citations${unverified ? ` — ${unverified} could not be verified against the source and are highlighted` : refs.length ? " — all verified against the sources" : ""}. Model: ${res.model}.`;
        insert.disabled = false;
      } catch (e) {
        status.textContent = errorText(e);
      } finally {
        generate.disabled = false;
      }
    },
  }, "Generate");
  const el = overlay(
    root,
    "Draft with Agent",
    h("label", { class: "form-row" }, h("span", {}, "Style"), style),
    outline,
    h("fieldset", {}, h("legend", {}, "References (cited as verifiable footnotes)"), refList, files, pasteTitle, pasteText,
      h("button", { class: "link", onclick: () => { if (pasteText.value.trim()) { const t = pasteTitle.value.trim() || `Reference ${refs.length + 1}`; refs.push({ title: t, text: pasteText.value, url: /^https?:\/\//.test(t) ? t : undefined }); pasteText.value = ""; pasteTitle.value = ""; renderRefs(); } } }, "Add pasted reference")),
    preview,
    status,
    h("div", { class: "dialog-actions" }, generate, insert, h("button", { class: "btn btn-quiet", onclick: () => el.remove() }, "Close")),
  );
  outline.focus();
}

/** Rewrite the selection in a tone and show an inline word diff to accept or discard. */
export async function openRewriteDialog(root: HTMLElement, editor: WordEditor, host: HostBridge, tone: Tone) {
  const original = editor.selectedText();
  if (!original.trim()) return;
  const diffView = h("div", { class: "ai-diff", "aria-live": "polite" }, "Rewriting…");
  const status = h("p", { class: "muted small" });
  let rewritten = "";
  const accept = h("button", { class: "btn", disabled: true, onclick: () => { editor.replaceSelectionText(rewritten); el.remove(); editor.focus(); } }, "Accept");
  const el = overlay(root, `Rewrite: ${tone}`, diffView, status, h("div", { class: "dialog-actions" }, accept, h("button", { class: "btn btn-quiet", onclick: () => el.remove() }, "Discard")));
  try {
    const res = await host.aiComplete(rewriteRequest(original, tone), (d) => {
      rewritten += d;
      diffView.textContent = rewritten;
    });
    if (res.stopReason === "refusal") throw new Error("The model declined this request.");
    rewritten = res.text.trim();
    diffView.replaceChildren(...wordDiff(original, rewritten).map((d) => (d.op === "same" ? document.createTextNode(d.text) : h(d.op === "add" ? "ins" : "del", {}, d.text))));
    status.textContent = `${original.length} → ${rewritten.length} characters`;
    accept.disabled = false;
  } catch (e) {
    diffView.textContent = errorText(e);
  }
}
