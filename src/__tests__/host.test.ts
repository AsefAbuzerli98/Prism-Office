import { describe, expect, it, vi } from "vitest";
import { BrowserHost, TauriHost, type AppEvent, type TauriApis } from "../core/host";

const memStorage = (): Storage => {
  const m = new Map<string, string>();
  return {
    get length() {
      return m.size;
    },
    clear: () => m.clear(),
    getItem: (k) => m.get(k) ?? null,
    key: (i) => [...m.keys()][i] ?? null,
    removeItem: (k) => void m.delete(k),
    setItem: (k, v) => void m.set(k, v),
  };
};

describe("BrowserHost", () => {
  it("mirrors the native registry semantics", async () => {
    const host = new BrowserHost(memStorage());
    const a = await host.newDocument("workbook");
    const b = await host.newDocument("workbook");
    expect([a.title, b.title]).toEqual(["Workbook 1", "Workbook 2"]);
    await expect(host.saveDocument(a.id)).rejects.toThrow(/path is required/);
    await expect(host.saveDocument(a.id, "/x/a.docx")).rejects.toThrow(/does not match/);
    const saved = await host.saveDocument(a.id, "/x/Budget.xlsx");
    expect(saved.title).toBe("Budget.xlsx");
    expect((await host.getDocument(a.id)).path).toBe("/x/Budget.xlsx");
    await host.closeDocument(a.id);
    await expect(host.closeDocument(a.id)).rejects.toThrow();
    await expect(host.openDocument("/x/readme.md")).rejects.toThrow(/unsupported/);
  });

  it("tracks recents MRU and emits events", async () => {
    const host = new BrowserHost(memStorage());
    const events: AppEvent["type"][] = [];
    host.onEvent((e) => events.push(e.type));
    await host.openDocument("/a.docx");
    await host.openDocument("/b.pptx");
    await host.openDocument("/a.docx");
    expect((await host.recentFiles()).map((r) => r.path)).toEqual(["/a.docx", "/b.pptx"]);
    await host.removeRecent("/b.pptx");
    expect((await host.recentFiles()).map((r) => r.path)).toEqual(["/a.docx"]);
    await host.clearRecent();
    expect(await host.recentFiles()).toEqual([]);
    expect(events).toContain("recentChanged");
    expect(events).toContain("documentOpened");
  });

  it("persists settings with defaults", async () => {
    const storage = memStorage();
    const host = new BrowserHost(storage);
    expect(await host.getSettings()).toEqual({ windowing: "tabbed", appearance: "system", locale: null, crashReports: false });
    await host.setSettings({ windowing: "tabbed", appearance: "dark", locale: "az", crashReports: false });
    expect((await new BrowserHost(storage).getSettings()).locale).toBe("az");
  });

  it("uses the injected prompt in place of native dialogs", async () => {
    const host = new BrowserHost(memStorage(), (_m, v) => (v ? v.replace(".docx", "-2.docx") : " /p.xlsx "));
    expect(await host.pickOpenPath()).toBe("/p.xlsx");
    expect(await host.pickSavePath("document", "/d.docx")).toBe("/d-2.docx");
  });
});

describe("TauriHost", () => {
  const apis = () => {
    const listeners: ((e: { payload: AppEvent }) => void)[] = [];
    const api: TauriApis = {
      invoke: vi.fn().mockResolvedValue({}) as unknown as TauriApis["invoke"],
      listen: vi.fn(async (_n, cb) => {
        listeners.push(cb);
        return () => {};
      }),
      openDialog: vi.fn().mockResolvedValue(["/x.pptx"]),
      saveDialog: vi.fn().mockResolvedValue("/y.docx"),
      closeWindow: vi.fn().mockResolvedValue(undefined),
    };
    return { api, listeners };
  };

  it("maps calls to IPC commands with camelCase args", async () => {
    const { api } = apis();
    const host = new TauriHost(api);
    await host.newDocument("presentation");
    await host.saveDocument(3);
    await host.openDocument("/p.pptx");
    await host.openDocumentWindow(3);
    await host.setSettings({ windowing: "multiInstance", appearance: "light", locale: null, crashReports: false });
    expect((api.invoke as unknown as ReturnType<typeof vi.fn>).mock.calls).toEqual([
      ["new_document", { kind: "presentation" }],
      ["save_document", { id: 3, path: null }],
      ["open_document", { path: "/p.pptx" }],
      ["open_document_window", { id: 3 }],
      ["set_settings", { value: { windowing: "multiInstance", appearance: "light", locale: null, crashReports: false } }],
    ]);
  });

  it("wraps dialogs and fans out events from one listener", async () => {
    const { api, listeners } = apis();
    const host = new TauriHost(api);
    expect(await host.pickOpenPath()).toBe("/x.pptx");
    expect(await host.pickSavePath("document", "a.docx")).toBe("/y.docx");
    const got: string[] = [];
    host.onEvent((e) => got.push(e.type));
    host.onEvent((e) => got.push(`2:${e.type}`));
    expect(api.listen).toHaveBeenCalledTimes(1);
    listeners[0]!({ payload: { type: "recentChanged" } });
    expect(got).toEqual(["recentChanged", "2:recentChanged"]);
  });
});
