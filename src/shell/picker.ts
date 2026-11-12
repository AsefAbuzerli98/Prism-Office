import { h } from "../core/dom";

export interface PickerItem {
  value: string;
  label: string;
  /** Optional CSS applied to the label (e.g. font preview). */
  style?: string;
  selected?: boolean;
}

/** Small anchored menu; resolves when an item is picked or it is dismissed. */
export function openPicker(anchor: HTMLElement, items: PickerItem[], onPick: (value: string) => void, title?: string) {
  document.querySelector(".picker")?.remove();
  const r = anchor.getBoundingClientRect();
  const close = () => {
    menu.remove();
    document.removeEventListener("mousedown", outside, true);
  };
  const outside = (e: MouseEvent) => {
    if (!menu.contains(e.target as Node)) close();
  };
  const menu = h(
    "div",
    {
      class: "picker",
      role: "listbox",
      "aria-label": title ?? "",
      style: `top:${r.bottom + 4}px;left:${Math.max(8, r.left)}px`,
      onkeydown: (e: KeyboardEvent) => {
        const opts = [...menu.querySelectorAll<HTMLButtonElement>(".picker-item")];
        const i = opts.indexOf(document.activeElement as HTMLButtonElement);
        if (e.key === "Escape") close();
        else if (e.key === "ArrowDown") opts[(i + 1) % opts.length]?.focus();
        else if (e.key === "ArrowUp") opts[(i - 1 + opts.length) % opts.length]?.focus();
        else return;
        e.preventDefault();
        e.stopPropagation();
      },
    },
    ...(title ? [h("div", { class: "picker-title" }, title)] : []),
    ...items.map((it) =>
      h(
        "button",
        {
          class: `picker-item${it.selected ? " selected" : ""}`,
          role: "option",
          "aria-selected": String(!!it.selected),
          dataset: { value: it.value },
          onclick: () => {
            close();
            onPick(it.value);
          },
        },
        h("span", { style: it.style ?? "" }, it.label),
      ),
    ),
  );
  document.body.append(menu);
  setTimeout(() => document.addEventListener("mousedown", outside, true));
  (menu.querySelector<HTMLElement>(".picker-item.selected") ?? menu.querySelector<HTMLElement>(".picker-item"))?.focus();
  return close;
}
