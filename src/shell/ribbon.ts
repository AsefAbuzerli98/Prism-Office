import { h } from "../core/dom";
import { t } from "../core/i18n";
import type { Mode } from "../core/modes";
import { IMPLEMENTED_PHASE, RIBBONS, type RibbonCommand } from "../core/ribbons";
import { commandIcon } from "../core/icons";
import { shortcutFor } from "../core/shortcuts";

/**
 * How a command sits in its group, Office style: `large` (icon over label),
 * `row` (small icon beside its label, three to a column) or `mini` (icon only,
 * two rows; the tooltip names it and its shortcut).
 */
export type CommandSize = "large" | "row" | "mini";

const MINI = new Set([
  "bold", "italic", "underline", "strike", "subscript", "superscript", "grow", "shrink", "color", "highlight", "changeCase", "clearFormat",
  "alignLeft", "alignCenter", "alignRight", "justify", "bullets", "numbering", "indent", "outdent", "percent", "border",
]);
const ROW = new Set([
  "xl.copy", "xl.cut", "xl.undo", "xl.redo", "xl.wrap", "xl.numfmt", "xl.pyEnv",
  "doc.undo", "doc.redo", "doc.copyFormat", "doc.pasteFormat", "doc.lineSpacing", "doc.find", "doc.replace", "doc.selectAll",
  "doc.acceptAll", "doc.rejectAll", "doc.nextChange", "doc.footnote", "doc.endnote", "doc.hyphenation", "doc.rtl", "doc.aiSettings",
  "ppt.layout", "ppt.duplicateSlide", "ppt.deleteSlide", "ppt.hideSlide", "ppt.fill", "ppt.outline", "ppt.arrange",
  "ppt.cameoShape", "ppt.cameoMirror",
]);

export function commandSize(id: string): CommandSize {
  if (ROW.has(id)) return "row";
  if (id === "xl.fill" || MINI.has(id.slice(id.indexOf(".") + 1))) return "mini";
  return "large";
}

export function renderRibbon(
  mode: Mode,
  activeTabId: string,
  onTab: (id: string) => void,
  onCommand: (id: string, anchor: HTMLElement) => void,
  isEnabled: (id: string, phase: number) => boolean = (_id, phase) => phase <= IMPLEMENTED_PHASE,
  isActive: (id: string) => boolean = () => false,
): HTMLElement {
  const tabs = RIBBONS[mode];
  const active = tabs.find((x) => x.id === activeTabId) ?? tabs[1] ?? tabs[0]!;

  const button = (c: RibbonCommand, size: CommandSize) => {
    const label = t(`ribbon.cmd.${c.id}`, {}, c.label);
    const enabled = isEnabled(c.id, c.phase);
    const keys = shortcutFor(c.id);
    const tip = enabled || c.phase <= IMPLEMENTED_PHASE ? (keys ? `${label} (${keys})` : label) : t("shell.phase", { label, phase: c.phase });
    return h(
      "button",
      {
        class: `ribbon-cmd ribbon-cmd-${size}`,
        disabled: !enabled,
        "aria-pressed": isActive(c.id) ? "true" : undefined,
        "aria-label": size === "mini" ? label : undefined,
        title: tip,
        dataset: { cmd: c.id },
        onclick: (e: Event) => onCommand(c.id, e.currentTarget as HTMLElement),
      },
      commandIcon(c.id, c.icon, size === "large" ? 26 : 18),
      size === "mini" ? null : h("span", { class: "cmd-label" }, label),
    );
  };

  /**
   * Large buttons stand alone; a group's row buttons share one stack and its
   * mini buttons one two-row grid, each placed where its first member was.
   */
  const body = (cmds: RibbonCommand[]) => {
    const out: HTMLElement[] = [];
    const stacks = new Map<CommandSize, HTMLElement>();
    for (const c of cmds) {
      const size = commandSize(c.id);
      if (size === "large") {
        out.push(button(c, size));
        continue;
      }
      let stack = stacks.get(size);
      if (!stack) {
        stack = h("div", { class: `ribbon-stack ribbon-stack-${size}` });
        stacks.set(size, stack);
        out.push(stack);
      }
      stack.append(button(c, size));
    }
    const mini = stacks.get("mini");
    if (mini) {
      const n = mini.childElementCount;
      mini.style.setProperty("--cols", String(n <= 3 ? n : Math.ceil(n / 2)));
    }
    return out;
  };

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
          h("div", { class: "ribbon-cmds" }, ...body(g.commands)),
          h("div", { class: "ribbon-group-label" }, t(`ribbon.group.${g.id}`, {}, g.label)),
        ),
      ),
    ),
  );
}
