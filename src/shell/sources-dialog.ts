import { h } from "../core/dom";
import type { Source } from "../word/model";

const KINDS = ["Book", "JournalArticle", "Report", "InternetSite", "DocumentFromInternetSite", "ConferenceProceedings", "Misc"];

/** Source manager: list, add, edit and remove bibliography sources. */
export function openSourcesDialog(
  root: HTMLElement,
  sources: Source[],
  onSave: (s: Source) => void,
  onRemove: (tag: string) => void,
) {
  document.querySelector(".sources-overlay")?.remove();
  let editing: Source = { tag: "", kind: "Book", title: "", authors: [], year: "", publisher: "", url: "" };
  const field = (label: string, input: HTMLElement) => h("label", { class: "form-row" }, h("span", {}, label), input);
  const title = h("input", { class: "open-path", "aria-label": "Title" });
  const authors = h("input", { class: "open-path", placeholder: "Last, First; Last, First", "aria-label": "Authors" });
  const year = h("input", { class: "open-path", "aria-label": "Year" });
  const publisher = h("input", { class: "open-path", "aria-label": "Publisher" });
  const url = h("input", { class: "open-path", "aria-label": "URL" });
  const kind = h("select", { class: "select", "aria-label": "Type" }, ...KINDS.map((k) => h("option", { value: k }, k)));
  const list = h("ul", { class: "service-list sources-list" });
  const load = (s: Source) => {
    editing = { ...s };
    title.value = s.title;
    authors.value = s.authors.join("; ");
    year.value = s.year;
    publisher.value = s.publisher;
    url.value = s.url;
    kind.value = s.kind || "Book";
  };
  const renderList = (items: Source[]) => {
    list.replaceChildren(
      ...(items.length
        ? items.map((s) =>
            h(
              "li",
              { class: "source-item" },
              h("button", { class: "link", onclick: () => load(s) }, `${s.authors[0] ?? s.title} (${s.year || "n.d."}) — ${s.title}`),
              h("button", { class: "link", "aria-label": `Remove ${s.tag}`, onclick: () => { onRemove(s.tag); items = items.filter((x) => x.tag !== s.tag); renderList(items); } }, "×"),
            ),
          )
        : [h("li", { class: "muted small" }, "No sources yet.")]),
    );
  };
  let current = [...sources];
  renderList(current);
  const save = () => {
    const names = authors.value.split(";").map((a) => a.trim()).filter(Boolean);
    const tag = editing.tag || `${(names[0] ?? title.value).replace(/[^A-Za-z]/g, "").slice(0, 3) || "Src"}${year.value.slice(-2) || "00"}${current.length ? current.length : ""}`;
    const s: Source = { tag, kind: kind.value, title: title.value.trim(), authors: names, year: year.value.trim(), publisher: publisher.value.trim(), url: url.value.trim() };
    if (!s.title && !names.length) return;
    onSave(s);
    current = current.some((x) => x.tag === tag) ? current.map((x) => (x.tag === tag ? s : x)) : [...current, s];
    renderList(current);
    load({ tag: "", kind: "Book", title: "", authors: [], year: "", publisher: "", url: "" });
  };
  const close = () => overlay.remove();
  const overlay = h(
    "div",
    { class: "overlay sources-overlay", onmousedown: (e: Event) => e.target === overlay && close(), onkeydown: (e: KeyboardEvent) => e.key === "Escape" && close() },
    h(
      "section",
      { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": "Sources" },
      h("h2", {}, "Sources"),
      list,
      h("fieldset", {}, h("legend", {}, "Add or edit source"), field("Type", kind), field("Title", title), field("Authors", authors), field("Year", year), field("Publisher", publisher), field("URL", url)),
      h("div", { class: "dialog-actions" }, h("button", { class: "btn btn-quiet", onclick: save }, "Save source"), h("button", { class: "btn", onclick: close }, "Close")),
    ),
  );
  root.append(overlay);
  title.focus();
}
