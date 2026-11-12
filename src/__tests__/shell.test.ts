import { beforeEach, describe, expect, it } from "vitest";
import { BrowserHost } from "../core/host";
import { Shell } from "../shell/shell";

const flush = () => new Promise((r) => setTimeout(r, 0));

describe("Shell (tabbed MDI)", () => {
  let root: HTMLElement;
  beforeEach(() => {
    localStorage.clear();
    document.body.innerHTML = "";
    root = document.createElement("div");
    document.body.append(root);
    globalThis.requestAnimationFrame ??= ((cb: FrameRequestCallback) => setTimeout(cb, 0)) as never;
  });

  it("starts on the Hub with three mode cards", () => {
    new Shell(root, new BrowserHost());
    expect(root.querySelectorAll(".mode-card")).toHaveLength(3);
    expect(document.documentElement.dataset.mode).toBe("hub");
  });

  it("swaps ribbon and theme when switching tabs", async () => {
    const shell = new Shell(root, new BrowserHost());
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

    shell.goHub();
    expect(root.querySelector(".ribbon")).toBeNull();
    expect(root.querySelectorAll(".doc-tab")).toHaveLength(2);
  });

  it("opening the same path twice reuses the tab", async () => {
    const shell = new Shell(root, new BrowserHost());
    await shell.openPath("/a/Deck.pptx");
    await shell.openPath("/a/Deck.pptx");
    expect(shell.tabs.list()).toHaveLength(1);
    expect(root.querySelector(".recent-item")).toBeNull(); // hub hidden while tab active
    shell.goHub();
    expect(root.querySelector(".recent-item")?.textContent).toContain("Deck.pptx");
  });

  it("closing the last tab returns to the Hub", async () => {
    const shell = new Shell(root, new BrowserHost());
    await shell.newDocument("presentation");
    const id = shell.tabs.active!.docId;
    await shell.close(id);
    expect(shell.tabs.list()).toHaveLength(0);
    expect(root.querySelectorAll(".mode-card")).toHaveLength(3);
  });
});
