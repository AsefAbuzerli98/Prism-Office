import { h } from "../core/dom";
import { MODES, MODE_ORDER, type Mode } from "../core/modes";

export interface RecentFile {
  path: string;
  mode: Mode;
  openedAt: number;
}

export interface HubActions {
  onNew(mode: Mode): void;
  onOpenPath(path: string): void;
}

const RECENTS_KEY = "prism.recentFiles";

export function loadRecents(storage: Storage = localStorage): RecentFile[] {
  try {
    const v = JSON.parse(storage.getItem(RECENTS_KEY) ?? "[]");
    return Array.isArray(v) ? (v as RecentFile[]) : [];
  } catch {
    return [];
  }
}

export function rememberRecent(file: RecentFile, storage: Storage = localStorage, max = 12) {
  const list = [file, ...loadRecents(storage).filter((r) => r.path !== file.path)].slice(0, max);
  storage.setItem(RECENTS_KEY, JSON.stringify(list));
}

/** The Application Hub: modality gateway shown on launch and on "+" tab. */
export function renderHub(actions: HubActions): HTMLElement {
  const cards = MODE_ORDER.map((m) => {
    const info = MODES[m];
    return h(
      "button",
      {
        class: "mode-card",
        dataset: { mode: m },
        style: `--card-accent:${info.accent.base};--card-soft:${info.accent.soft}`,
        "aria-label": `New ${info.label}`,
        onclick: () => actions.onNew(m),
      },
      h("span", { class: "mode-glyph" }, info.glyph),
      h("span", { class: "mode-title" }, info.label),
      h("span", { class: "mode-equiv" }, `${info.equivalent} modality`),
      h("span", { class: "mode-tagline" }, info.tagline),
    );
  });

  const pathInput = h("input", {
    class: "open-path",
    type: "text",
    placeholder: "Path to .docx, .xlsx or .pptx",
    "aria-label": "File path",
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
    h("button", { type: "submit", class: "btn" }, "Open"),
  );

  const recents = loadRecents();
  const recentList = recents.length
    ? h(
        "ul",
        { class: "recent-list" },
        ...recents.map((r) =>
          h(
            "li",
            {},
            h(
              "button",
              {
                class: "recent-item",
                dataset: { mode: r.mode },
                onclick: () => actions.onOpenPath(r.path),
              },
              h("span", { class: "recent-dot", style: `background:${MODES[r.mode].accent.base}` }),
              h("span", { class: "recent-name" }, r.path.split(/[\\/]/).pop() ?? r.path),
              h("span", { class: "recent-path" }, r.path),
            ),
          ),
        ),
      )
    : h("p", { class: "muted" }, "No recent files yet.");

  return h(
    "section",
    { class: "hub", "aria-label": "Application Hub" },
    h(
      "header",
      { class: "hub-header" },
      h("h1", {}, "Prism Office"),
      h("p", { class: "muted" }, "Choose a workspace to begin."),
    ),
    h("div", { class: "mode-grid" }, ...cards),
    h(
      "div",
      { class: "hub-lower" },
      h("section", { class: "hub-panel" }, h("h2", {}, "Open"), openForm),
      h("section", { class: "hub-panel" }, h("h2", {}, "Recent"), recentList),
    ),
  );
}
