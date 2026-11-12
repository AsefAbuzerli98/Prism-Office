import { h } from "../core/dom";
import type { RecentFile } from "../core/host";
import { t } from "../core/i18n";
import { MODES, MODE_ORDER, type Mode } from "../core/modes";

export interface HubActions {
  onNew(mode: Mode): void;
  onOpenDialog(): void;
  onOpenPath(path: string): void;
  onRemoveRecent(path: string): void;
  onClearRecent(): void;
  onSettings(): void;
}

/** The Application Hub: modality gateway shown on launch and on "+" tab. */
export function renderHub(actions: HubActions, recents: RecentFile[]): HTMLElement {
  const cards = MODE_ORDER.map((m) => {
    const info = MODES[m];
    const label = t(`mode.${m}`);
    return h(
      "button",
      {
        class: "mode-card",
        dataset: { mode: m },
        style: `--card-accent:${info.accent.base};--card-soft:${info.accent.soft}`,
        "aria-label": t("hub.new", { mode: label }),
        onclick: () => actions.onNew(m),
      },
      h("span", { class: "mode-glyph" }, info.glyph),
      h("span", { class: "mode-title" }, label),
      h("span", { class: "mode-equiv" }, t("hub.modality", { app: info.equivalent })),
      h("span", { class: "mode-tagline" }, t(`mode.${m}.tagline`)),
    );
  });

  const pathInput = h("input", {
    class: "open-path",
    type: "text",
    placeholder: t("hub.pathPlaceholder"),
    "aria-label": t("hub.pathPlaceholder"),
  });
  const openForm = h(
    "form",
    {
      class: "open-form",
      onsubmit: (e: Event) => {
        e.preventDefault();
        const p = pathInput.value.trim();
        if (p) actions.onOpenPath(p);
      },
    },
    pathInput,
    h("button", { type: "submit", class: "btn btn-quiet" }, t("hub.open")),
  );

  const recentList = recents.length
    ? h(
        "ul",
        { class: "recent-list" },
        ...recents.map((r) =>
          h(
            "li",
            { class: "recent-row" },
            h(
              "button",
              {
                class: `recent-item${r.exists ? "" : " missing"}`,
                dataset: { mode: r.kind },
                disabled: !r.exists,
                title: r.path,
                onclick: () => actions.onOpenPath(r.path),
              },
              h("span", { class: "recent-dot", style: `background:${MODES[r.kind].accent.base}` }),
              h("span", { class: "recent-name" }, r.path.split(/[\\/]/).pop() ?? r.path),
              h("span", { class: "recent-path" }, r.exists ? r.path : `${t("hub.missing")} · ${r.path}`),
            ),
            h(
              "button",
              { class: "recent-remove", "aria-label": `× ${r.path}`, onclick: () => actions.onRemoveRecent(r.path) },
              "×",
            ),
          ),
        ),
      )
    : h("p", { class: "muted" }, t("hub.noRecent"));

  return h(
    "section",
    { class: "hub", "aria-label": t("shell.hub") },
    h(
      "header",
      { class: "hub-header" },
      h("div", {}, h("h1", {}, t("app.name")), h("p", { class: "muted" }, t("hub.subtitle"))),
      h("button", { class: "btn btn-quiet hub-settings", onclick: () => actions.onSettings() }, `⚙ ${t("hub.settings")}`),
    ),
    h("div", { class: "mode-grid" }, ...cards),
    h(
      "div",
      { class: "hub-lower" },
      h(
        "section",
        { class: "hub-panel" },
        h("h2", {}, t("hub.open")),
        h("button", { class: "btn open-dialog", onclick: () => actions.onOpenDialog() }, t("hub.openFile")),
        openForm,
      ),
      h(
        "section",
        { class: "hub-panel" },
        h(
          "div",
          { class: "panel-head" },
          h("h2", {}, t("hub.recent")),
          recents.length
            ? h("button", { class: "link", onclick: () => actions.onClearRecent() }, t("hub.clearRecent"))
            : null,
        ),
        recentList,
      ),
    ),
  );
}
