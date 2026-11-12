import { describe, expect, it, vi } from "vitest";
import { BrowserHost, TauriHost } from "../core/host";

describe("BrowserHost", () => {
  it("mirrors the native registry semantics", async () => {
    const host = new BrowserHost();
    const a = await host.newDocument("workbook");
    const b = await host.newDocument("workbook");
    expect([a.title, b.title]).toEqual(["Workbook 1", "Workbook 2"]);
    await expect(host.saveDocument(a.id)).rejects.toThrow(/path is required/);
    await expect(host.saveDocument(a.id, "/x/a.docx")).rejects.toThrow(/does not match/);
    const saved = await host.saveDocument(a.id, "/x/Budget.xlsx");
    expect(saved.title).toBe("Budget.xlsx");
    await host.closeDocument(a.id);
    await expect(host.closeDocument(a.id)).rejects.toThrow();
    await expect(host.openDocument("/x/readme.md")).rejects.toThrow(/unsupported/);
  });
});

describe("TauriHost", () => {
  it("maps calls to IPC commands with camelCase args", async () => {
    const invoke = vi.fn().mockResolvedValue({});
    const host = new TauriHost(invoke);
    await host.newDocument("presentation");
    await host.saveDocument(3);
    await host.openDocument("/p.pptx");
    expect(invoke.mock.calls).toEqual([
      ["new_document", { kind: "presentation" }],
      ["save_document", { id: 3, path: null }],
      ["open_document", { path: "/p.pptx" }],
    ]);
  });
});
