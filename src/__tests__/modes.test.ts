import { describe, expect, it } from "vitest";
import { MODES, MODE_ORDER, modeFromPath } from "../core/modes";
import { RIBBONS } from "../core/ribbons";
import { applyTheme, contrastRatio } from "../core/theme";
import { columnName } from "../modes/views";

describe("modes", () => {
  it("detects mode from paths", () => {
    expect(modeFromPath("/a/b/Report.DOCX")).toBe("document");
    expect(modeFromPath("C:\\x\\model.xlsm")).toBe("workbook");
    expect(modeFromPath("deck.potx")).toBe("presentation");
    expect(modeFromPath("notes.txt")).toBeNull();
    expect(modeFromPath(".docx")).toBeNull();
  });

  it("uses the specified accent families", () => {
    expect(MODES.document.accent.base).toBe("#2563eb"); // cobalt
    expect(MODES.workbook.accent.base).toBe("#059669"); // emerald
    expect(MODES.presentation.accent.base).toBe("#c2553a"); // terracotta
  });

  it("accent text meets WCAG AA for large UI text (≥3:1) against white", () => {
    for (const m of MODE_ORDER) {
      const a = MODES[m].accent;
      expect(contrastRatio(a.base, a.contrast)).toBeGreaterThanOrEqual(3);
      expect(contrastRatio(a.strong, a.soft)).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("ribbons contain the spec'd tabs and unique command ids", () => {
    const labels = (m: keyof typeof RIBBONS) => RIBBONS[m].map((t) => t.label);
    expect(labels("document")).toEqual(
      expect.arrayContaining(["Typography", "Page Setup", "Citations", "Document Structure"]),
    );
    expect(labels("workbook")).toEqual(
      expect.arrayContaining(["Data Connections", "Formulas", "Python", "Pivot Tables"]),
    );
    expect(labels("presentation")).toEqual(
      expect.arrayContaining(["Slide Design", "Cameo Layouts", "Transitions", "Recording Studio"]),
    );
    for (const m of MODE_ORDER) {
      const ids = RIBBONS[m].flatMap((t) => t.groups.flatMap((g) => g.commands.map((c) => c.id)));
      expect(new Set(ids).size).toBe(ids.length);
    }
  });

  it("applyTheme sets CSS variables and data-mode", () => {
    const el = document.createElement("div");
    applyTheme("workbook", el);
    expect(el.style.getPropertyValue("--accent")).toBe("#059669");
    expect(el.dataset.mode).toBe("workbook");
    applyTheme(null, el);
    expect(el.dataset.mode).toBe("hub");
  });

  it("column names", () => {
    expect([0, 25, 26, 27, 701, 702].map(columnName)).toEqual(["A", "Z", "AA", "AB", "ZZ", "AAA"]);
  });
});
