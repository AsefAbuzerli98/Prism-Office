import { h } from "../core/dom";
import type { RecentFile, RecoverableDoc } from "../core/host";
import { t } from "../core/i18n";
import { MODES, MODE_ORDER, type Mode } from "../core/modes";
import { iconSvg } from "../core/icons";

export interface HubActions {
  onNew(mode: Mode): void;
  onOpenDialog(): void;
  onOpenPath(path: string): void;
  onRemoveRecent(path: string): void;
  onClearRecent(): void;
  onSettings(): void;
  onRestore?(key: string): void;
  onDiscardRecovered?(key: string): void;
}

/** The Application Hub: modality gateway shown on launch and on "+" tab. */
export function renderHub(actions: HubActions, recents: RecentFile[], recoverable: RecoverableDoc[] = []): HTMLElement {
  const recovery = recoverable.length
    ? h(
        "section",
        { class: "hub-panel recovery-panel", "aria-label": t("hub.recovered") },
        h("h2", {}, t("hub.recovered")),
        h("p", { class: "muted small" }, t("hub.recoveredHint")),
        h(
          "ul",
          { class: "recent-list" },
          ...recoverable.map((r) =>
            h(
              "li",
              { class: "recent-row", dataset: { key: r.key } },
              h("span", { class: "recent-mark", style: `color:${MODES[r.kind].accent.base}`, "aria-hidden": "true" }, MODES[r.kind].glyph),
              h("span", { class: "recent-name" }, r.title),
              h("span", { class: "recent-path" }, new Date(r.updated).toLocaleString()),
              h("button", { class: "btn btn-quiet", onclick: () => actions.onRestore?.(r.key) }, t("hub.restore")),
              h("button", { class: "link", onclick: () => actions.onDiscardRecovered?.(r.key) }, t("hub.discard")),
            ),
          ),
        ),
      )
    : null;
  const cards = MODE_ORDER.map((m) => {
    const info = MODES[m];
    const label = t(`mode.${m}`);
    return h(
      "button",
      {
        class: "mode-card",
        dataset: { mode: m },
        style: `--card-accent:${info.accent.base}`,
        "aria-label": t("hub.new", { mode: label }),
        onclick: () => actions.onNew(m),
      },
      h("span", { class: "mode-head" }, h("span", { class: "mode-mark", "aria-hidden": "true" }, info.glyph), h("span", { class: "mode-title" }, label), plusIcon()),
      h("span", { class: "mode-tagline" }, t(`mode.${m}.tagline`)),
      h("span", { class: "mode-formats" }, t(`mode.${m}.formats`)),
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

  const rel = new Intl.RelativeTimeFormat(document.documentElement.lang || undefined, { numeric: "auto" });
  const ago = (ms: number) => {
    const d = (ms - Date.now()) / 1000;
    const abs = Math.abs(d);
    if (abs < 60) return rel.format(Math.round(d), "second");
    if (abs < 3600) return rel.format(Math.round(d / 60), "minute");
    if (abs < 86400) return rel.format(Math.round(d / 3600), "hour");
    if (abs < 86400 * 30) return rel.format(Math.round(d / 86400), "day");
    return new Date(ms).toISOString().slice(0, 10);
  };
  const folder = (p: string) => p.slice(0, Math.max(0, p.length - (p.split(/[\\/]/).pop() ?? "").length)).replace(/[\\/]$/, "");

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
              h("span", { class: "recent-mark", style: `color:${MODES[r.kind].accent.base}`, "aria-hidden": "true" }, MODES[r.kind].glyph),
              h("span", { class: "recent-name" }, r.path.split(/[\\/]/).pop() ?? r.path),
              h("span", { class: "recent-path" }, r.exists ? folder(r.path) || "—" : `${t("hub.missing")} · ${folder(r.path) || r.path}`),
              h("span", { class: "recent-when" }, r.openedAt ? ago(r.openedAt) : "—"),
            ),
            h(
              "button",
              { class: "recent-remove", "aria-label": `× ${r.path}`, title: t("hub.removeRecent"), onclick: () => actions.onRemoveRecent(r.path) },
              "×",
            ),
          ),
        ),
      )
    : h(
        "div",
        { class: "hub-empty" },
        h("p", {}, h("strong", {}, t("hub.noRecent")), " ", t("hub.recentEmpty")),
        h("button", { class: "btn btn-quiet", onclick: () => actions.onOpenDialog() }, t("hub.openFile")),
      );

  const last = recents.find((r) => r.exists);
  const summary = recoverable.length
    ? t("hub.summaryRecover", { n: recoverable.length })
    : last
      ? t("hub.summaryLast", { name: last.path.split(/[\\/]/).pop() ?? last.path, when: ago(last.openedAt) })
      : t("hub.subtitle");

  return h(
    "section",
    { class: "hub", "aria-label": t("shell.hub") },
    h(
      "header",
      { class: "hub-header" },
      h("div", {}, h("h1", {}, t("app.name")), h("p", { class: "hub-summary" }, summary)),
      h(
        "div",
        { class: "hub-actions" },
        h("button", { class: "btn btn-quiet hub-settings", onclick: () => actions.onSettings() }, t("hub.settings")),
        h("button", { class: "btn btn-primary open-dialog", onclick: () => actions.onOpenDialog() }, t("hub.openFile")),
      ),
    ),
    recovery,
    h("section", { class: "hub-section" }, h("h2", {}, t("hub.startNew")), h("div", { class: "mode-grid" }, ...cards)),
    h(
      "section",
      { class: "hub-section" },
      h(
        "div",
        { class: "panel-head" },
        h("h2", {}, t("hub.recent")),
        recents.length ? h("button", { class: "link", onclick: () => actions.onClearRecent() }, t("hub.clearRecent")) : null,
      ),
      recentList,
      openForm,
    ),
    h(
      "section",
      { class: "hub-section hub-keys" },
      h("h2", {}, t("hub.shortcuts")),
      h(
        "dl",
        {},
        ...[
          [`${mod}O`, t("hub.kOpen")],
          [`${mod}K`, t("hub.kPalette")],
          [`${mod}T`, t("hub.kNewTab")],
          [`${mod},`, t("hub.kSettings")],
        ].flatMap(([k, v]) => [h("dt", {}, h("kbd", {}, k!)), h("dd", {}, v!)]),
      ),
    ),
  );
}

const mod = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl+";

function plusIcon(): HTMLElement {
  const s = document.createElement("span");
  s.className = "mode-plus";
  s.setAttribute("aria-hidden", "true");
  s.innerHTML = iconSvg("plus", 16) ?? "+";
  return s;
}
