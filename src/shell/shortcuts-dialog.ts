import { h } from "../core/dom";
import { t } from "../core/i18n";
import type { Mode } from "../core/modes";
import { formatKeys, label, shortcutSections } from "../core/shortcuts";

/** Keyboard Shortcuts dialog (F1 / Mod+/): the current mode's Office shortcuts, searchable. */
export function openShortcuts(root: HTMLElement, mode: Mode | "hub") {
  root.querySelector(".shortcuts-overlay")?.remove();
  const sections = shortcutSections(mode);
  const filter = h("input", { class: "open-path shortcuts-filter", type: "search", placeholder: t("keys.filter"), "aria-label": t("keys.filter") });
  const body = h("div", { class: "shortcuts-body" });
  const keyChips = (spec: string[]) =>
    h("span", { class: "shortcut-keys" }, ...spec.flatMap((k, i) => [i ? h("span", { class: "shortcut-or" }, "/") : null, h("kbd", {}, formatKeys(k))]));
  const render = () => {
    const q = filter.value.trim().toLocaleLowerCase();
    body.replaceChildren();
    let shown = 0;
    for (const sec of sections) {
      const items = sec.items.filter((i) => !q || label(i).toLocaleLowerCase().includes(q) || i.keys.some((k) => formatKeys(k).toLocaleLowerCase().includes(q)));
      if (!items.length) continue;
      shown += items.length;
      body.append(
        h(
          "section",
          { class: "shortcut-section" },
          h("h3", {}, label(sec)),
          h("dl", {}, ...items.flatMap((i) => [h("dt", {}, label(i)), h("dd", {}, keyChips(i.keys))])),
        ),
      );
    }
    if (!shown) body.append(h("p", { class: "muted" }, t("keys.none")));
  };
  filter.addEventListener("input", render);
  const close = () => overlay.remove();
  const overlay = h(
    "div",
    {
      class: "overlay shortcuts-overlay",
      onmousedown: (e: Event) => e.target === overlay && close(),
      onkeydown: (e: KeyboardEvent) => {
        if (e.key === "Escape") close();
        e.stopPropagation();
      },
    },
    h(
      "section",
      { class: "dialog shortcuts-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("keys.title") },
      h("div", { class: "shortcuts-head" }, h("h2", {}, t("keys.title")), filter, h("button", { class: "btn", onclick: close }, t("find.close").replace(/ \(.*\)$/, ""))),
      body,
    ),
  );
  render();
  root.append(overlay);
  filter.focus();
}
