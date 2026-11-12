import { h } from "../core/dom";
import type { HostBridge, Settings } from "../core/host";
import { LOCALE_NAMES, SUPPORTED_LOCALES, t } from "../core/i18n";

/** Settings dialog: windowing topology (P1-25), appearance, language, services. */
export async function openSettings(root: HTMLElement, host: HostBridge, current: Settings): Promise<void> {
  const services = await host.listServices();
  const state = { ...current };

  const radio = <K extends keyof Settings>(key: K, value: Settings[K], label: string, disabled = false) =>
    h(
      "label",
      { class: `radio${disabled ? " disabled" : ""}` },
      h("input", {
        type: "radio",
        name: key,
        value: String(value),
        checked: state[key] === value,
        disabled,
        onchange: async () => {
          state[key] = value;
          await host.setSettings({ ...state });
        },
      }),
      h("span", {}, label),
    );

  const lang = h(
    "select",
    {
      class: "select",
      "aria-label": t("settings.language"),
      onchange: async (e: Event) => {
        const v = (e.target as HTMLSelectElement).value;
        state.locale = v === "" ? null : v;
        await host.setSettings({ ...state });
        // Rebuild so the dialog itself appears in the new language.
        overlay.remove();
        await openSettings(root, host, state);
        (root.querySelector(".settings select") as HTMLElement | null)?.focus();
      },
    },
    h("option", { value: "", selected: state.locale === null }, t("settings.language.auto")),
    ...SUPPORTED_LOCALES.map((l) => h("option", { value: l, selected: state.locale === l }, LOCALE_NAMES[l] ?? l)),
  );

  const close = () => overlay.remove();
  const overlay = h(
    "div",
    {
      class: "overlay",
      onmousedown: (e: Event) => e.target === overlay && close(),
      onkeydown: (e: KeyboardEvent) => {
        if (e.key === "Escape") {
          e.stopPropagation();
          close();
        }
      },
    },
    h(
      "section",
      { class: "dialog settings", role: "dialog", "aria-modal": "true", "aria-label": t("settings.title") },
      h("h2", {}, t("settings.title")),
      h(
        "fieldset",
        {},
        h("legend", {}, t("settings.windowing")),
        radio("windowing", "tabbed", t("settings.windowing.tabbed")),
        radio("windowing", "multiInstance", t("settings.windowing.multiInstance"), !host.multiWindow),
        host.multiWindow ? null : h("p", { class: "muted small" }, t("settings.windowing.browserNote")),
      ),
      h(
        "fieldset",
        {},
        h("legend", {}, t("settings.appearance")),
        radio("appearance", "system", t("settings.appearance.system")),
        radio("appearance", "light", t("settings.appearance.light")),
        radio("appearance", "dark", t("settings.appearance.dark")),
      ),
      h("fieldset", {}, h("legend", {}, t("settings.language")), lang),
      h(
        "fieldset",
        {},
        h("legend", {}, t("settings.services")),
        h(
          "ul",
          { class: "service-list" },
          ...services.map((s) =>
            h(
              "li",
              { dataset: { state: s.state } },
              h("span", { class: "service-dot" }),
              h("span", {}, s.name),
              h(
                "span",
                { class: "muted small" },
                s.state === "unavailable" ? s.reason : s.state === "failed" ? s.error : s.state,
              ),
            ),
          ),
        ),
      ),
      h("div", { class: "dialog-actions" }, h("button", { class: "btn", onclick: close }, t("settings.close"))),
    ),
  );
  root.append(overlay);
  (overlay.querySelector("input:checked") as HTMLElement | null)?.focus();
}
