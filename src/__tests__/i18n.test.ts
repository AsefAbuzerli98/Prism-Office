import { afterEach, describe, expect, it } from "vitest";
import { CATALOGS, directionOf, negotiateLocale, setLocale, t } from "../core/i18n";
import { RIBBONS } from "../core/ribbons";

describe("i18n", () => {
  afterEach(() => setLocale("en"));

  it("negotiates locales", () => {
    expect(negotiateLocale(["az-Latn-AZ", "en-US"])).toBe("az");
    expect(negotiateLocale(["fr-FR", "en-GB"])).toBe("en");
    expect(negotiateLocale(["de"])).toBe("en");
  });

  it("translates with params and falls back to English", () => {
    setLocale("az");
    expect(t("shell.saved", { title: "a.docx" })).toBe("a.docx saxlanıldı");
    expect(document.documentElement.lang).toBe("az");
    expect(t("missing.key", {}, "Fallback")).toBe("Fallback");
  });

  it("sets text direction for RTL languages", () => {
    expect(directionOf("ar-EG")).toBe("rtl");
    expect(directionOf("az")).toBe("ltr");
    setLocale("en");
    expect(document.documentElement.dir).toBe("ltr");
  });

  it("az catalog covers every English key and every ribbon label", () => {
    const missing = Object.keys(CATALOGS.en!).filter((k) => !(k in CATALOGS.az!));
    expect(missing).toEqual([]);
    for (const tabs of Object.values(RIBBONS)) {
      for (const tab of tabs) {
        expect(CATALOGS.az).toHaveProperty(`ribbon.tab.${tab.id}`);
        for (const g of tab.groups) {
          expect(CATALOGS.az).toHaveProperty(`ribbon.group.${g.id}`);
          for (const c of g.commands) expect(CATALOGS.az).toHaveProperty(`ribbon.cmd.${c.id}`);
        }
      }
    }
  });
});
