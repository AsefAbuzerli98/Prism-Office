import { formatKeybinding, type Command, type CommandRegistry } from "../core/commands";
import { h } from "../core/dom";
import { t } from "../core/i18n";

/** Modal command palette (Mod+K / Mod+Shift+P). */
export class CommandPalette {
  private el: HTMLElement | null = null;
  private selected = 0;
  private results: Command[] = [];

  constructor(
    private readonly host: HTMLElement,
    private readonly registry: CommandRegistry,
  ) {}

  get isOpen() {
    return this.el !== null;
  }

  toggle() {
    if (this.isOpen) this.close();
    else this.open();
  }

  open() {
    if (this.el) return;
    const input = h("input", {
      class: "palette-input",
      type: "text",
      placeholder: t("palette.placeholder"),
      "aria-label": t("cmd.app.palette"),
      role: "combobox",
      "aria-expanded": "true",
      "aria-controls": "palette-list",
    });
    const list = h("ul", { class: "palette-list", id: "palette-list", role: "listbox" });
    const render = () => {
      this.results = this.registry.search(input.value);
      this.selected = Math.min(this.selected, Math.max(0, this.results.length - 1));
      list.replaceChildren(
        ...(this.results.length
          ? this.results.map((c, i) =>
              h(
                "li",
                {
                  class: `palette-item${i === this.selected ? " selected" : ""}`,
                  role: "option",
                  "aria-selected": String(i === this.selected),
                  dataset: { cmd: c.id },
                  onmousedown: (e: Event) => {
                    e.preventDefault();
                    void this.run(c);
                  },
                },
                h("span", {}, c.title()),
                c.keys?.[0] ? h("kbd", {}, formatKeybinding(c.keys[0], this.registry.isMac)) : null,
              ),
            )
          : [h("li", { class: "palette-empty" }, t("palette.empty"))]),
      );
    };
    input.addEventListener("input", () => {
      this.selected = 0;
      render();
    });
    input.addEventListener("keydown", (e) => {
      e.stopPropagation(); // keep global shortcuts out while typing
      if (e.key === "ArrowDown") {
        e.preventDefault();
        this.selected = (this.selected + 1) % Math.max(1, this.results.length);
        render();
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        this.selected = (this.selected - 1 + this.results.length) % Math.max(1, this.results.length);
        render();
      } else if (e.key === "Enter") {
        e.preventDefault();
        const c = this.results[this.selected];
        if (c) void this.run(c);
      } else if (e.key === "Escape") {
        e.preventDefault();
        this.close();
      }
    });
    this.el = h(
      "div",
      { class: "overlay", onmousedown: (e: Event) => e.target === this.el && this.close() },
      h("div", { class: "palette", role: "dialog", "aria-modal": "true" }, input, list),
    );
    this.host.append(this.el);
    render();
    input.focus();
  }

  close() {
    this.el?.remove();
    this.el = null;
  }

  private async run(c: Command) {
    this.close();
    await this.registry.execute(c.id);
  }
}
