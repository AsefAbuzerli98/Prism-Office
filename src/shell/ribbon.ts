import { h } from "../core/dom";
import { t } from "../core/i18n";
import type { Mode } from "../core/modes";
import { IMPLEMENTED_PHASE, RIBBONS } from "../core/ribbons";

export function renderRibbon(
  mode: Mode,
  activeTabId: string,
  onTab: (id: string) => void,
  onCommand: (id: string) => void,
): HTMLElement {
  const tabs = RIBBONS[mode];
  const active = tabs.find((x) => x.id === activeTabId) ?? tabs[1] ?? tabs[0]!;
  return h(
    "div",
    { class: "ribbon", role: "toolbar", dataset: { mode } },
    h(
      "nav",
      { class: "ribbon-tabs", role: "tablist" },
      ...tabs.map((tab) =>
        h(
          "button",
          {
            class: `ribbon-tab${tab.id === active.id ? " active" : ""}`,
            role: "tab",
            "aria-selected": String(tab.id === active.id),
            onclick: () => onTab(tab.id),
          },
          t(`ribbon.tab.${tab.id}`, {}, tab.label),
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
              {
                const label = t(`ribbon.cmd.${c.id}`, {}, c.label);
                return h(
                "button",
                {
                  class: "ribbon-cmd",
                  disabled: c.phase > IMPLEMENTED_PHASE,
                  title: c.phase > IMPLEMENTED_PHASE ? t("shell.phase", { label, phase: c.phase }) : label,
                  dataset: { cmd: c.id },
                  onclick: () => onCommand(c.id),
                },
                h("span", { class: "cmd-icon" }, c.icon),
                h("span", { class: "cmd-label" }, label),
              );
              },
            ),
          ),
          h("div", { class: "ribbon-group-label" }, t(`ribbon.group.${g.id}`, {}, g.label)),
        ),
      ),
    ),
  );
}
