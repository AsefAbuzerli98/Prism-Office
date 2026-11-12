import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { loadEngineSync, Workbook } from "../sheet/engine";

beforeAll(() => {
  loadEngineSync(readFileSync(resolve(__dirname, "../sheet/wasm/prism_sheet_bg.wasm")));
});

describe("WASM sheet engine", () => {
  it("calculates, spills and formats", () => {
    const wb = new Workbook();
    wb.edit([
      { sheet: 0, r: 0, c: 0, text: "Region" },
      { sheet: 0, r: 1, c: 0, text: "East" },
      { sheet: 0, r: 2, c: 0, text: "West" },
      { sheet: 0, r: 3, c: 0, text: "East" },
      { sheet: 0, r: 1, c: 1, text: "10" },
      { sheet: 0, r: 2, c: 1, text: "20" },
      { sheet: 0, r: 3, c: 1, text: "30" },
    ]);
    const ch = wb.set(0, 0, 3, "=GROUPBY(A2:A4,B2:B4,SUM)");
    expect(ch.error).toBeUndefined();
    const v = wb.view(0, { r0: 0, c0: 3, r1: 2, c1: 4 });
    const at = (r: number, c: number) => v.find((x) => x.r === r && x.c === c)?.t;
    expect(at(0, 3)).toBe("East");
    expect(at(0, 4)).toBe("40");
    expect(at(2, 3)).toBe("Total");
    expect(v.find((x) => x.r === 1 && x.c === 3)?.spill).toBe(true);
    wb.set(0, 5, 0, "=XLOOKUP(\"^W\",A2:A4,B2:B4,,3)");
    expect(wb.view(0, { r0: 5, c0: 0, r1: 5, c1: 0 })[0]?.t).toBe("20");
    wb.set(0, 6, 0, "0.256");
    wb.applyStyle(0, { r0: 6, c0: 0, r1: 6, c1: 0 }, "0.0%", { bold: true });
    expect(wb.view(0, { r0: 6, c0: 0, r1: 6, c1: 0 })[0]?.t).toBe("25.6%");
    expect(wb.styleOf(0, 6, 0).style.bold).toBe(true);
    expect(wb.input(0, 0, 3)).toBe("=GROUPBY(A2:A4,B2:B4,SUM)");
    const bad = wb.set(0, 7, 0, "=SUM(");
    expect(bad.error).toBeTruthy();
    expect(Workbook.functionNames()).toContain("REGEXEXTRACT");
  });

  it("round-trips the model and renames sheets", () => {
    const wb = new Workbook();
    wb.set(0, 0, 0, "5");
    const s = wb.addSheet("Data");
    wb.set(s, 0, 0, "=Sheet1!A1*2");
    wb.renameSheet(0, "Inputs");
    expect(wb.input(s, 0, 0)).toBe("=Inputs!A1*2");
    const m = wb.model();
    const again = new Workbook(m);
    expect(again.view(1, { r0: 0, c0: 0, r1: 0, c1: 0 })[0]?.t).toBe("10");
    const fill = again.fill(1, { r0: 0, c0: 0, r1: 0, c1: 0 }, { r0: 0, c0: 0, r1: 2, c1: 0 });
    expect(fill.error).toBeUndefined();
    expect(again.input(1, 2, 0)).toBe("=Inputs!A3*2");
  });

  it("reports =PY as async work", () => {
    const wb = new Workbook();
    wb.set(0, 0, 0, "4");
    wb.set(0, 0, 1, '=PY("xl(""A1"")**2",0)');
    expect(wb.view(0, { r0: 0, c0: 1, r1: 0, c1: 1 })[0]?.t).toBe("#BUSY!");
    const [req] = wb.asyncRequests();
    expect(req?.inputs[0]?.[1]).toEqual([[4]]);
    wb.supplyAsync(req!, 16);
    expect(wb.view(0, { r0: 0, c0: 1, r1: 0, c1: 1 })[0]?.t).toBe("16");
  });
});
