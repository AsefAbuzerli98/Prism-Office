import { h } from "../core/dom";
import type { DocumentInfo, HostBridge } from "../core/host";
import { MODES, type Mode } from "../core/modes";
import { RIBBONS } from "../core/ribbons";
import { TabManager, type DocTab } from "../core/tabs";
import { applyTheme } from "../core/theme";
import { rememberRecent, renderHub } from "../hub/hub";
import { renderEngineView } from "../modes/views";
import { renderRibbon } from "./ribbon";

/**
 * Tabbed-MDI application shell. Owns the tab strip, the mode-specific ribbon
 * and the engine view; swaps all three (and the accent theme) on tab switch.
 */
export class Shell {
  readonly tabs = new TabManager();
  private ribbonTab: Partial<Record<Mode, string>> = {};
  private views = new Map<number, HTMLElement>();
  private showHub = true;

  private readonly strip = h("div", { class: "tab-strip", role: "tablist", "aria-label": "Open documents" });
  private readonly ribbonSlot = h("div", { class: "ribbon-slot" });
  private readonly workspace = h("main", { class: "workspace" });
  private readonly status = h("footer", { class: "status-bar" });

  constructor(
    private readonly root: HTMLElement,
    private readonly host: HostBridge,
  ) {
    root.replaceChildren(
      h("div", { class: "titlebar", "data-tauri-drag-region": true }, this.strip),
      this.ribbonSlot,
      this.workspace,
      this.status,
    );
    this.tabs.on((e) => {
      if (e.type === "closed") this.views.delete(e.tab.docId);
      if (e.type === "activated") this.showHub = e.tab === null;
      this.render();
    });
    root.addEventListener("keydown", (e) => this.onKey(e));
    this.render();
  }

  async newDocument(mode: Mode) {
    await this.guard(async () => this.adopt(await this.host.newDocument(mode)));
  }

  async openPath(path: string) {
    const existing = this.tabs.findByPath(path);
    if (existing) {
      this.tabs.activate(existing.docId);
      return;
    }
    await this.guard(async () => {
      const info = await this.host.openDocument(path);
      this.adopt(info);
      if (info.issues.some((i) => i.severity === "error")) {
        this.flash(`Opened with ${info.issues.length} validation issue(s)`);
      }
    });
  }

  async save(saveAs = false) {
    const tab = this.tabs.active;
    if (!tab) return;
    let path: string | undefined;
    if (saveAs || !tab.path) {
      const suggested = tab.path ?? `${tab.title}.${MODES[tab.mode].defaultExtension}`;
      const answer = window.prompt("Save as (full path):", suggested);
      if (!answer) return;
      path = answer;
    }
    await this.guard(async () => {
      const info = await this.host.saveDocument(tab.docId, path);
      this.tabs.update(tab.docId, { title: info.title, path: info.path, dirty: false });
      if (info.path) rememberRecent({ path: info.path, mode: info.kind, openedAt: Date.now() });
      this.flash(`Saved ${info.title}`);
    });
  }

  async close(docId: number) {
    const tab = this.tabs.get(docId);
    if (!tab) return;
    if (tab.dirty && !window.confirm(`Close “${tab.title}” without saving?`)) return;
    await this.guard(async () => {
      await this.host.closeDocument(docId);
      this.tabs.close(docId);
    });
  }

  goHub() {
    this.showHub = true;
    this.render();
  }

  // ---------------------------------------------------------------------

  private adopt(info: DocumentInfo) {
    if (info.path) rememberRecent({ path: info.path, mode: info.kind, openedAt: Date.now() });
    this.tabs.open({ docId: info.id, mode: info.kind, title: info.title, path: info.path });
    this.showHub = false;
    this.render();
  }

  private async guard(fn: () => Promise<void>) {
    try {
      await fn();
    } catch (err) {
      this.flash(String(err instanceof Error ? err.message : err), true);
    }
  }

  private flash(message: string, error = false) {
    const toast = h("div", { class: `toast${error ? " error" : ""}`, role: "status" }, message);
    this.root.append(toast);
    setTimeout(() => toast.remove(), 3500);
  }

  private onKey(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    if (!mod) return;
    if (e.key === "s") {
      e.preventDefault();
      void this.save(e.shiftKey);
    } else if (e.key === "w" && this.tabs.active) {
      e.preventDefault();
      void this.close(this.tabs.active.docId);
    } else if (e.key === "t") {
      e.preventDefault();
      this.goHub();
    } else if (e.key === "Tab") {
      e.preventDefault();
      this.tabs.cycle(e.shiftKey ? -1 : 1);
      this.showHub = false;
      this.render();
    }
  }

  private onCommand(id: string) {
    switch (id) {
      case "file.new":
        this.goHub();
        break;
      case "file.open":
        this.goHub();
        break;
      case "file.save":
        void this.save(false);
        break;
      case "file.saveAs":
        void this.save(true);
        break;
    }
  }

  private render() {
    const active = this.showHub ? null : this.tabs.active;
    applyTheme(active?.mode ?? null);
    this.renderStrip(active);
    this.renderRibbon(active);
    this.renderWorkspace(active);
    this.renderStatus(active);
  }

  private renderStrip(active: DocTab | null) {
    const items = this.tabs.list().map((t) =>
      h(
        "div",
        {
          class: `doc-tab${active?.docId === t.docId ? " active" : ""}`,
          role: "tab",
          "aria-selected": String(active?.docId === t.docId),
          style: `--tab-accent:${MODES[t.mode].accent.base}`,
          dataset: { mode: t.mode, docId: String(t.docId) },
          onclick: () => {
            this.showHub = false;
            if (this.tabs.active?.docId === t.docId) this.render();
            else this.tabs.activate(t.docId);
          },
          onauxclick: (e: MouseEvent) => {
            if (e.button === 1) void this.close(t.docId);
          },
        },
        h("span", { class: "doc-tab-glyph" }, MODES[t.mode].glyph),
        h("span", { class: "doc-tab-title" }, `${t.title}${t.dirty ? " •" : ""}`),
        h(
          "button",
          {
            class: "doc-tab-close",
            "aria-label": `Close ${t.title}`,
            onclick: (e: Event) => {
              e.stopPropagation();
              void this.close(t.docId);
            },
          },
          "×",
        ),
      ),
    );
    this.strip.replaceChildren(
      h(
        "button",
        { class: `hub-tab${active ? "" : " active"}`, "aria-label": "Application Hub", onclick: () => this.goHub() },
        "◆",
      ),
      ...items,
      h("button", { class: "new-tab", "aria-label": "New tab", onclick: () => this.goHub() }, "+"),
    );
  }

  private renderRibbon(active: DocTab | null) {
    if (!active) {
      this.ribbonSlot.replaceChildren();
      return;
    }
    const mode = active.mode;
    const current = this.ribbonTab[mode] ?? RIBBONS[mode][1]?.id ?? "file";
    this.ribbonSlot.replaceChildren(
      renderRibbon(
        mode,
        current,
        (id) => {
          this.ribbonTab[mode] = id;
          this.render();
        },
        (id) => this.onCommand(id),
      ),
    );
  }

  private renderWorkspace(active: DocTab | null) {
    if (!active) {
      this.workspace.replaceChildren(
        renderHub({
          onNew: (m) => void this.newDocument(m),
          onOpenPath: (p) => void this.openPath(p),
        }),
      );
      return;
    }
    let view = this.views.get(active.docId);
    if (!view) {
      view = renderEngineView(active.mode);
      this.views.set(active.docId, view);
    }
    this.workspace.replaceChildren(view);
  }

  private renderStatus(active: DocTab | null) {
    const left = active
      ? `${MODES[active.mode].label} · ${active.path ?? "Not saved"}`
      : `${this.tabs.list().length} open`;
    this.status.replaceChildren(
      h("span", {}, left),
      h("span", { class: "muted" }, this.host.native ? "Native host" : "Browser preview"),
    );
  }
}
