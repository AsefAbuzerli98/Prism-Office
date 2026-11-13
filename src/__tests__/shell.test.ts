import { beforeEach, describe, expect, it, vi } from "vitest";
import { CommandRegistry } from "../core/commands";
import { BrowserHost, type WindowContext } from "../core/host";
import { setLocale } from "../core/i18n";
import { Shell } from "../shell/shell";

const flush = () => new Promise((r) => setTimeout(r, 0));

/** BrowserHost that pretends to be the desktop host (multi-window capable). */
class FakeDesktopHost extends BrowserHost {
  override readonly multiWindow: boolean = true;
  opened: number[] = [];
  closedWindow = false;
  ctx: WindowContext = { label: "main", docId: null, platform: "macos" };
  pending: string[] = [];
  override windowContext() {
    return this.ctx;
  }
  override async openDocumentWindow(id: number) {
    this.opened.push(id);
  }
  override async closeCurrentWindow() {
    this.closedWindow = true;
  }
  override async takePendingOpens() {
    const p = this.pending;
    this.pending = [];
    return p;
  }
}

describe("Shell", () => {
  let root: HTMLElement;
  let current: Shell | undefined;
  beforeEach(() => {
    current?.dispose();
    localStorage.clear();
    document.body.innerHTML = "";
    root = document.createElement("div");
    document.body.append(root);
    globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) => setTimeout(cb, 0)) as never;
    setLocale("en");
  });

  const make = async (host: BrowserHost = new BrowserHost(localStorage, () => null)) => {
    const shell = new Shell(root, host, new CommandRegistry(false));
    current = shell;
    await shell.init();
    return shell;
  };

  it("starts on the Hub with three mode cards", async () => {
    await make();
    expect(root.querySelectorAll(".mode-card")).toHaveLength(3);
    expect(document.documentElement.dataset.mode).toBe("hub");
  });

  it("swaps ribbon and theme when switching tabs", async () => {
    const shell = await make();
    (root.querySelector('.mode-card[data-mode="document"]') as HTMLButtonElement).click();
    await flush();
    expect(document.documentElement.dataset.mode).toBe("document");
    expect(root.querySelector(".ribbon")?.getAttribute("data-mode")).toBe("document");
    expect(root.textContent).toContain("Typography");

    await shell.newDocument("workbook");
    expect(document.documentElement.dataset.mode).toBe("workbook");
    expect(root.textContent).toContain("Pivot Tables");
    expect(root.querySelectorAll(".doc-tab")).toHaveLength(2);

    (root.querySelector('.doc-tab[data-mode="document"]') as HTMLElement).click();
    expect(document.documentElement.dataset.mode).toBe("document");

    // shortcuts work even when focus is outside the app root
    document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "t", ctrlKey: true, bubbles: true }));
    expect(root.querySelector(".ribbon")).toBeNull();
    expect(root.querySelectorAll(".doc-tab")).toHaveLength(2);
  });

  it("opening the same path twice reuses the tab and records a recent", async () => {
    const shell = await make();
    await shell.openPath("/a/Deck.pptx");
    await shell.openPath("/a/Deck.pptx");
    expect(shell.tabs.list()).toHaveLength(1);
    await flush();
    shell.goHub();
    expect(root.querySelector(".recent-item")?.textContent).toContain("Deck.pptx");
  });

  it("closing the last tab returns to the Hub", async () => {
    const shell = await make();
    await shell.newDocument("presentation");
    await shell.close(shell.tabs.active!.docId);
    expect(shell.tabs.list()).toHaveLength(0);
    expect(root.querySelectorAll(".mode-card")).toHaveLength(3);
  });

  it("keyboard shortcuts route through the command registry", async () => {
    const shell = await make();
    await shell.newDocument("document");
    await shell.newDocument("workbook");
    root.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", ctrlKey: true, bubbles: true }));
    expect(shell.tabs.active?.mode).toBe("document");
    root.dispatchEvent(new KeyboardEvent("keydown", { key: "t", ctrlKey: true, bubbles: true }));
    expect(root.querySelectorAll(".mode-card")).toHaveLength(3);
  });

  it("command palette filters and runs commands", async () => {
    const shell = await make();
    root.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true }));
    const input = root.querySelector(".palette-input") as HTMLInputElement;
    expect(input).not.toBeNull();
    input.value = "new work";
    input.dispatchEvent(new Event("input"));
    expect(root.querySelector(".palette-item.selected")?.getAttribute("data-cmd")).toBe("new.workbook");
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await flush();
    expect(root.querySelector(".palette")).toBeNull();
    expect(shell.tabs.active?.mode).toBe("workbook");
  });

  it("re-renders in Azerbaijani when the locale setting changes", async () => {
    const host = new BrowserHost(localStorage, () => null);
    await make(host);
    await host.setSettings({ windowing: "tabbed", appearance: "dark", locale: "az", crashReports: false });
    expect(root.querySelector(".mode-card .mode-title")?.textContent).toBe("Sənəd");
    expect(document.documentElement.dataset.appearance).toBe("dark");
  });

  it("opens files handed over by the OS", async () => {
    const host = new FakeDesktopHost(localStorage, () => null);
    host.pending = ["/start.docx"];
    const shell = await make(host);
    expect(shell.tabs.list().map((t) => t.title)).toEqual(["start.docx"]);
    host.pending = ["/later.xlsx"];
    host.simulateOpenFiles(["/later.xlsx"]);
    await flush();
    await flush();
    expect(shell.tabs.list().map((t) => t.title)).toEqual(["start.docx", "later.xlsx"]);
  });

  it("multi-instance topology opens new documents in their own windows", async () => {
    const host = new FakeDesktopHost(localStorage, () => null);
    const shell = await make(host);
    await host.setSettings({ windowing: "multiInstance", appearance: "system", locale: null, crashReports: false });
    await shell.newDocument("document");
    expect(shell.tabs.list()).toHaveLength(0);
    expect(host.opened).toHaveLength(1);
  });

  it("detaching a tab moves it into a native window", async () => {
    const host = new FakeDesktopHost(localStorage, () => null);
    const shell = await make(host);
    await shell.newDocument("presentation");
    const id = shell.tabs.active!.docId;
    await shell.commands.execute("window.detach");
    expect(host.opened).toEqual([id]);
    expect(shell.tabs.list()).toHaveLength(0);
    expect(await host.getDocument(id)).toBeTruthy(); // still open in the host
  });

  it("a document window hosts exactly its document and closes with it", async () => {
    const host = new FakeDesktopHost(localStorage, () => null);
    const info = await host.newDocument("workbook");
    host.ctx = { label: `doc-${info.id}`, docId: info.id, platform: "windows" };
    const shell = await make(host);
    expect(shell.isDocumentWindow).toBe(true);
    expect(root.querySelector(".hub-tab")).toBeNull();
    expect(shell.tabs.active?.docId).toBe(info.id);
    expect(document.documentElement.dataset.platform).toBe("windows");
    expect(shell.commands.isEnabled("view.hub")).toBe(false);
    await shell.close(info.id);
    expect(host.closedWindow).toBe(true);
  });

  it("browser preview reports that detaching is unavailable", async () => {
    const shell = await make();
    await shell.newDocument("document");
    expect(shell.commands.isEnabled("window.detach")).toBe(false);
    const spy = vi.spyOn(shell.tabs, "close");
    await shell.detach(shell.tabs.active!.docId);
    expect(spy).not.toHaveBeenCalled();
    expect(root.querySelector(".toast.error")?.textContent).toMatch(/desktop app/);
  });
});

describe("auto-recovery in the Hub", () => {
  it("offers, restores and discards recovered documents", async () => {
    localStorage.clear();
    document.body.innerHTML = "";
    const root = document.createElement("div");
    document.body.append(root);
    const host = new BrowserHost(localStorage, () => null);
    host.recoverable = [
      { key: "k1", title: "Plan.docx", path: "/d/Plan.docx", kind: "document", updated: Date.now(), entries: 4 },
      { key: "k2", title: "Budget.xlsx", path: null, kind: "workbook", updated: Date.now() - 1000, entries: 1 },
    ];
    const shell = new Shell(root, host, new CommandRegistry(false));
    await shell.init();
    expect(root.querySelectorAll(".recovery-panel li")).toHaveLength(2);
    (root.querySelector('.recovery-panel li[data-key="k2"] .link') as HTMLButtonElement).click();
    await new Promise((r) => setTimeout(r, 0));
    expect(root.querySelectorAll(".recovery-panel li")).toHaveLength(1);
    await shell.restoreRecovered("k1");
    expect(shell.tabs.active).toMatchObject({ title: "Plan.docx (recovered)", dirty: true });
    shell.goHub();
    expect(root.querySelector(".recovery-panel")).toBeNull();
    shell.dispose();
  });
});
