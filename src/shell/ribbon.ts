import { h } from "../core/dom";
import type { Mode } from "../core/modes";
import { IMPLEMENTED_PHASE, RIBBONS } from "../core/ribbons";

export function renderRibbon(
  mode: Mode,
  activeTabId: string,
  onTab: (id: string) => void,
  onCommand: (id: string) => void,
): HTMLElement {
  const tabs = RIBBONS[mode];
  const active = tabs.find((t) => t.id === activeTabId) ?? tabs[1] ?? tabs[0]!;
  return h(
    "div",
    { class: "ribbon", role: "toolbar", dataset: { mode } },
    h(
      "nav",
      { class: "ribbon-tabs", role: "tablist" },
      ...tabs.map((t) =>
        h(
          "button",
          {
            class: `ribbon-tab${t.id === active.id ? " active" : ""}`,
            role: "tab",
            "aria-selected": String(t.id === active.id),
            onclick: () => onTab(t.id),
          },
          t.label,
        ),
      ),
    ),
    h(
      "div",
      { class: "ribbon-body", role: "tabpanel" },
      ...active.groups.map((g) =>
        h(
          "div",
          { class: "ribbon-group" },
          h(
            "div",
            { class: "ribbon-cmds" },
            ...g.commands.map((c) =>
              h(
                "button",
                {
                  class: "ribbon-cmd",
                  disabled: c.phase > IMPLEMENTED_PHASE,
                  title: c.phase > IMPLEMENTED_PHASE ? `${c.label} — arrives in Phase ${c.phase}` : c.label,
                  dataset: { cmd: c.id },
                  onclick: () => onCommand(c.id),
                },
                h("span", { class: "cmd-icon" }, c.icon),
                h("span", { class: "cmd-label" }, c.label),
              ),
            ),
          ),
          h("div", { class: "ribbon-group-label" }, g.label),
        ),
      ),
    ),
  );
}
