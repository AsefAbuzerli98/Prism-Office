import { CommandRegistry } from "../core/commands";
import { h } from "../core/dom";
import { DEFAULT_SETTINGS, type AppEvent, type DocumentInfo, type HostBridge, type RecentFile, type ServiceInfo, type Settings } from "../core/host";
import { onLocaleChange, setLocale, t } from "../core/i18n";
import { MODES, MODE_ORDER, type Mode } from "../core/modes";
import { RIBBONS } from "../core/ribbons";
import { TabManager, type DocTab } from "../core/tabs";
import { applyTheme } from "../core/theme";
import { renderHub } from "../hub/hub";
import { renderEngineView } from "../modes/views";
import { WordEditor, type EngineView } from "../word/editor";
import { IMPLEMENTED_PHASE } from "../core/ribbons";
import { openPicker } from "./picker";
import { CommandPalette } from "./palette";
import { renderRibbon } from "./ribbon";
import { openSettings } from "./settings";

/**
 * Application shell. In the main window it is a tabbed-MDI host with the
 * Hub; in a dedicated document window (multi-instance topology) it hosts a
 * single document. Swaps ribbon, engine view and accent theme on tab switch.
 */
export class Shell {
  readonly tabs = new TabManager();
  readonly commands: CommandRegistry;
  private readonly palette: CommandPalette;
  private ribbonTab: Partial<Record<Mode, string>> = {};
  private views = new Map<number, { el: HTMLElement; engine: EngineView | null }>();
  private showHub = true;
  private settings: Settings = { ...DEFAULT_SETTINGS };
  private recents: RecentFile[] = [];
  private services: ServiceInfo[] = [];
  /** Dedicated document window (multi-instance) rather than the main window. */
  readonly isDocumentWindow: boolean;
  private disposers: (() => void)[] = [];

  private readonly strip = h("div", { class: "tab-strip", role: "tablist" });
  private readonly ribbonSlot = h("div", { class: "ribbon-slot" });
  private readonly workspace = h("main", { class: "workspace" });
  private readonly status = h("footer", { class: "status-bar" });

  constructor(
    private readonly root: HTMLElement,
    private readonly host: HostBridge,
    commands?: CommandRegistry,
  ) {
    const ctx = host.windowContext();
    this.isDocumentWindow = ctx.docId !== null;
    document.documentElement.dataset.platform = ctx.platform;
    this.commands = commands ?? new CommandRegistry(ctx.platform === "macos" || undefined);
    this.palette = new CommandPalette(root, this.commands);
    this.registerCommands();
    this.registerWordCommands();

    root.replaceChildren(
      h("div", { class: "titlebar", "data-tauri-drag-region": true }, this.strip),
      this.ribbonSlot,
      this.workspace,
      this.status,
    );
    this.tabs.on((e) => {
      if (e.type === "closed") {
        this.views.get(e.tab.docId)?.engine?.dispose();
        this.views.delete(e.tab.docId);
      }
      if (e.type === "activated") this.showHub = e.tab === null && !this.isDocumentWindow;
      this.render();
    });
    // Global shortcuts listen on the document so they work wherever focus is.
    const onKey = (e: KeyboardEvent) => {
      if (!this.palette.isOpen && !e.defaultPrevented) this.commands.handleKeydown(e);
    };
    root.ownerDocument.addEventListener("keydown", onKey);
    const offHost = host.onEvent((e) => void this.onHostEvent(e));
    const offLocale = onLocaleChange(() => this.render());
    this.disposers.push(
      () => root.ownerDocument.removeEventListener("keydown", onKey),
      () => void offHost(),
      () => void offLocale(),
    );
    setLocale(null);
    this.render();
  }

  /** Detach global listeners (tests, hot reload). */
  dispose() {
    for (const d of this.disposers.splice(0)) d();
    this.palette.close();
  }

  /** Load settings, recents and services; adopt this window's document or pending OS opens. */
  async init() {
    const [settings, recents, services] = await Promise.all([
      this.host.getSettings(),
      this.host.recentFiles(),
      this.host.listServices(),
    ]);
    this.applySettings(settings);
    this.recents = recents;
    this.services = services;
    const ctx = this.host.windowContext();
    if (ctx.docId !== null) {
      await this.guard(async () => this.adoptTab(await this.host.getDocument(ctx.docId!)));
    } else {
      for (const p of await this.host.takePendingOpens()) await this.openPath(p);
    }
    this.render();
  }

  private get multiInstance() {
    return this.settings.windowing === "multiInstance" && this.host.multiWindow && !this.isDocumentWindow;
  }

  // ---- document operations ------------------------------------------------

  async newDocument(mode: Mode) {
    await this.guard(async () => this.adopt(await this.host.newDocument(mode)));
  }

  async openDialog() {
    const path = await this.host.pickOpenPath();
    if (path) await this.openPath(path);
  }

  async openPath(path: string) {
    const existing = this.tabs.findByPath(path);
    if (existing) {
      this.tabs.activate(existing.docId);
      this.showHub = false;
      this.render();
      return;
    }
    await this.guard(async () => {
      const info = await this.host.openDocument(path);
      this.adopt(info);
      if (info.issues.some((i) => i.severity === "error")) {
        this.flash(t("shell.issues", { n: info.issues.length }));
      }
    });
  }

  async save(saveAs = false) {
    const tab = this.tabs.active;
    if (!tab) return;
    let path: string | undefined;
    if (saveAs || !tab.path) {
      const suggested = tab.path ?? `${tab.title}.${MODES[tab.mode].defaultExtension}`;
      const picked = await this.host.pickSavePath(tab.mode, suggested);
      if (!picked) return;
      path = picked;
    }
    await this.guard(async () => {
      const engine = this.views.get(tab.docId)?.engine;
      await engine?.flush();
      const info = await this.host.saveDocument(tab.docId, path);
      if (engine instanceof WordEditor) engine.markClean();
      this.tabs.update(tab.docId, { title: info.title, path: info.path, dirty: false });
      this.flash(t("shell.saved", { title: info.title }));
    });
  }

  async close(docId: number) {
    const tab = this.tabs.get(docId);
    if (!tab) return;
    if (tab.dirty && !window.confirm(t("shell.confirmClose", { title: tab.title }))) return;
    await this.guard(async () => {
      await this.host.closeDocument(docId);
      this.tabs.close(docId);
      if (this.isDocumentWindow && this.tabs.list().length === 0) await this.host.closeCurrentWindow();
    });
  }

  /** Move a tab into its own native window (doc stays open in the host). */
  async detach(docId: number) {
    await this.guard(async () => {
      await this.host.openDocumentWindow(docId);
      this.tabs.close(docId);
    });
  }

  goHub() {
    if (this.isDocumentWindow) return;
    this.showHub = true;
    this.render();
  }

  async showSettings() {
    await openSettings(this.root, this.host, this.settings);
  }

  // ---- internals -------------------------------------------------------------

  private registerCommands() {
    const hasTab = () => this.tabs.active !== null && !this.showHub;
    const notDocWindow = () => !this.isDocumentWindow;
    this.commands.register(
      { id: "file.new", title: () => t("cmd.file.new"), keys: ["Mod+N"], when: notDocWindow, run: () => this.goHub() },
      { id: "file.open", title: () => t("cmd.file.open"), keys: ["Mod+O"], run: () => this.openDialog() },
      { id: "file.save", title: () => t("cmd.file.save"), keys: ["Mod+S"], when: hasTab, run: () => this.save(false) },
      { id: "file.saveAs", title: () => t("cmd.file.saveAs"), keys: ["Mod+Shift+S"], when: hasTab, run: () => this.save(true) },
      { id: "file.close", title: () => t("cmd.file.close"), keys: ["Mod+W"], when: hasTab, run: () => this.close(this.tabs.active!.docId) },
      { id: "view.hub", title: () => t("cmd.view.hub"), keys: ["Mod+T"], when: notDocWindow, run: () => this.goHub() },
      {
        id: "view.nextTab",
        title: () => t("cmd.view.nextTab"),
        keys: ["Ctrl+Tab"],
        when: () => this.tabs.list().length > 0,
        run: () => this.cycle(1),
      },
      {
        id: "view.prevTab",
        title: () => t("cmd.view.prevTab"),
        keys: ["Ctrl+Shift+Tab"],
        when: () => this.tabs.list().length > 0,
        run: () => this.cycle(-1),
      },
      {
        id: "window.detach",
        title: () => t("cmd.window.detach"),
        when: () => hasTab() && this.host.multiWindow && !this.isDocumentWindow,
        run: () => this.detach(this.tabs.active!.docId),
      },
      { id: "app.palette", title: () => t("cmd.app.palette"), keys: ["Mod+K", "Mod+Shift+P"], hidden: true, run: () => this.palette.toggle() },
      { id: "app.settings", title: () => t("cmd.app.settings"), keys: ["Mod+,"], run: () => this.showSettings() },
      ...MODE_ORDER.map((m) => ({
        id: `new.${m}`,
        title: () => t(`cmd.new.${m}`),
        when: notDocWindow,
        run: () => this.newDocument(m),
      })),
    );
  }

  /** The active tab's engine view, if it is a word editor. */
  get activeEditor(): WordEditor | null {
    const tab = this.showHub ? null : this.tabs.active;
    const e = tab ? this.views.get(tab.docId)?.engine : null;
    return e instanceof WordEditor ? e : null;
  }

  private registerWordCommands() {
    const ed = () => this.activeEditor;
    const when = () => ed()?.doc != null;
    const simple = (id: string, cmd: string, arg?: string) => ({
      id,
      title: () => t(`ribbon.cmd.${id}`, {}, id),
      when,
      hidden: true,
      run: () => void ed()?.execute(cmd, arg),
    });
    this.commands.register(
      simple("doc.bold", "bold"),
      simple("doc.italic", "italic"),
      simple("doc.underline", "underline"),
      simple("doc.strike", "strike"),
      simple("doc.align", "alignCycle"),
      simple("doc.bullets", "bullets"),
      simple("doc.numbering", "numbering"),
      simple("doc.pageBreak", "pageBreak"),
      { ...simple("doc.markdown", "markdown"), hidden: false, keys: ["Mod+Shift+M"] },
      { ...simple("doc.orientation", "orientation") },
    );
  }

  /** Ribbon commands that need an anchored picker. */
  private pickerCommand(id: string, anchor: HTMLElement): boolean {
    const ed = this.activeEditor;
    if (!ed?.doc) return false;
    const st = ed.state();
    switch (id) {
      case "doc.styles": {
        const styles = ed.doc.styles.filter((s) => s.kind === "paragraph");
        openPicker(
          anchor,
          styles.map((s) => ({
            value: s.id,
            label: s.name.replace(/^heading (\d)$/, "Heading $1"),
            selected: s.id === st.style,
            style: `font-weight:${s.run.bold ? 700 : 400};font-style:${s.run.italic ? "italic" : "normal"};font-size:${Math.min(20, Math.max(12, (s.run.size ?? 22) / 2.2))}px`,
          })),
          (v) => ed.execute("style", v),
          t("word.styles"),
        );
        return true;
      }
      case "doc.font": {
        const fonts = ["Calibri", "Arial", "Times New Roman", "Georgia", "Verdana", "Consolas", "Courier New", "Segoe UI", "Helvetica"];
        const sizes = [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72];
        openPicker(
          anchor,
          [
            ...fonts.map((f) => ({ value: `font:${f}`, label: f, style: `font-family:'${f}'`, selected: f === st.font })),
            ...sizes.map((z) => ({ value: `size:${z}`, label: `${z} pt`, selected: z === st.size })),
          ],
          (v) => {
            const [k, val] = v.split(":") as [string, string];
            ed.execute(k, val);
          },
          `${t("word.font")} / ${t("word.size")}`,
        );
        return true;
      }
      case "doc.margins":
        openPicker(
          anchor,
          ["normal", "narrow", "moderate", "wide"].map((m) => ({ value: m, label: t(`word.margins.${m}`) })),
          (v) => ed.execute("margins", v),
          t("word.margins"),
        );
        return true;
      case "doc.orientation":
        openPicker(
          anchor,
          [
            { value: "portrait", label: t("word.portrait"), selected: !ed.doc.section.landscape },
            { value: "landscape", label: t("word.landscape"), selected: ed.doc.section.landscape },
          ],
          (v) => ed.execute("orientation", v),
        );
        return true;
    }
    return false;
  }

  private ribbonEnabled = (id: string, phase: number) => {
    if (this.commands.has(id)) return this.commands.isEnabled(id);
    if (["doc.styles", "doc.font", "doc.margins", "doc.orientation"].includes(id)) return this.activeEditor?.doc != null;
    return phase <= IMPLEMENTED_PHASE;
  };

  private ribbonActive = (id: string) => {
    const ed = this.activeEditor;
    if (!ed?.doc) return false;
    const st = ed.state();
    switch (id) {
      case "doc.bold": return st.bold;
      case "doc.italic": return st.italic;
      case "doc.underline": return st.underline;
      case "doc.strike": return st.strike;
      case "doc.bullets": return st.list === 9001;
      case "doc.numbering": return st.list === 9002;
      case "doc.markdown": return ed.markdownVisible;
      default: return false;
    }
  };

  private cycle(dir: 1 | -1) {
    this.tabs.cycle(dir);
    this.showHub = false;
    this.render();
  }

  private adopt(info: DocumentInfo) {
    if (this.multiInstance) {
      void this.guard(() => this.host.openDocumentWindow(info.id));
      return;
    }
    this.adoptTab(info);
  }

  private adoptTab(info: DocumentInfo) {
    this.tabs.open({ docId: info.id, mode: info.kind, title: info.title, path: info.path });
    this.showHub = false;
    this.render();
  }

  private applySettings(s: Settings) {
    this.settings = s;
    document.documentElement.dataset.appearance = s.appearance;
    setLocale(s.locale);
  }

  private async onHostEvent(e: AppEvent) {
    switch (e.type) {
      case "openFiles":
        if (this.host.windowContext().label === "main") {
          for (const p of await this.host.takePendingOpens()) await this.openPath(p);
        }
        break;
      case "recentChanged":
        this.recents = await this.host.recentFiles();
        if (this.showHub) this.render();
        break;
      case "settingsChanged":
        this.applySettings(e.settings);
        this.render();
        break;
      case "serviceChanged":
        this.services = [...this.services.filter((s) => s.id !== e.info.id), e.info];
        this.renderStatus(this.showHub ? null : this.tabs.active);
        break;
      case "documentClosed":
        if (this.tabs.get(e.id)) this.tabs.close(e.id);
        break;
      case "documentSaved":
        if (this.tabs.get(e.info.id)) this.tabs.update(e.info.id, { title: e.info.title, path: e.info.path });
        break;
    }
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

  private render() {
    const active = this.showHub ? null : this.tabs.active;
    applyTheme(active?.mode ?? null);
    this.strip.setAttribute("aria-label", t("shell.hub"));
    this.renderStrip(active);
    this.renderRibbon(active);
    this.renderWorkspace(active);
    this.renderStatus(active);
  }

  private renderStrip(active: DocTab | null) {
    const items = this.tabs.list().map((tab) =>
      h(
        "div",
        {
          class: `doc-tab${active?.docId === tab.docId ? " active" : ""}`,
          role: "tab",
          "aria-selected": String(active?.docId === tab.docId),
          style: `--tab-accent:${MODES[tab.mode].accent.base}`,
          dataset: { mode: tab.mode, docId: String(tab.docId) },
          onclick: () => {
            this.showHub = false;
            if (this.tabs.active?.docId === tab.docId) this.render();
            else this.tabs.activate(tab.docId);
          },
          onauxclick: (e: MouseEvent) => {
            if (e.button === 1) void this.close(tab.docId);
          },
          ondblclick: () => {
            if (this.host.multiWindow && !this.isDocumentWindow) void this.detach(tab.docId);
          },
          title: this.host.multiWindow && !this.isDocumentWindow ? `${tab.title} — ${t("shell.detach")}` : tab.title,
        },
        h("span", { class: "doc-tab-glyph" }, MODES[tab.mode].glyph),
        h("span", { class: "doc-tab-title" }, `${tab.title}${tab.dirty ? " •" : ""}`),
        h(
          "button",
          {
            class: "doc-tab-close",
            "aria-label": t("shell.closeTab", { title: tab.title }),
            onclick: (e: Event) => {
              e.stopPropagation();
              void this.close(tab.docId);
            },
          },
          "×",
        ),
      ),
    );
    if (this.isDocumentWindow) {
      this.strip.replaceChildren(...items);
      return;
    }
    this.strip.replaceChildren(
      h(
        "button",
        { class: `hub-tab${active ? "" : " active"}`, "aria-label": t("shell.hub"), title: t("shell.hub"), onclick: () => this.goHub() },
        "◆",
      ),
      ...items,
      h("button", { class: "new-tab", "aria-label": t("shell.newTab"), title: t("shell.newTab"), onclick: () => this.goHub() }, "+"),
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
        (id, anchor) => {
          if (!this.pickerCommand(id, anchor)) void this.commands.execute(id);
        },
        this.ribbonEnabled,
        this.ribbonActive,
      ),
    );
  }

  private renderWorkspace(active: DocTab | null) {
    if (!active) {
      this.workspace.replaceChildren(
        renderHub(
          {
            onNew: (m) => void this.newDocument(m),
            onOpenDialog: () => void this.openDialog(),
            onOpenPath: (p) => void this.openPath(p),
            onRemoveRecent: (p) => void this.host.removeRecent(p),
            onClearRecent: () => void this.host.clearRecent(),
            onSettings: () => void this.showSettings(),
          },
          this.recents,
        ),
      );
      return;
    }
    let view = this.views.get(active.docId);
    if (!view) {
      if (active.mode === "document") {
        const docId = active.docId;
        const engine = new WordEditor(this.host, docId, {
          onDirty: () => this.tabs.update(docId, { dirty: true }),
          onState: () => {
            if (this.tabs.active?.docId === docId && !this.showHub) this.renderRibbon(this.tabs.active);
          },
        });
        engine.ready.then(
          () => engine.focus(),
          (err) => this.flash(String(err instanceof Error ? err.message : err), true),
        );
        view = { el: engine.el, engine };
      } else {
        view = { el: renderEngineView(active.mode), engine: null };
      }
      this.views.set(active.docId, view);
    }
    const attached = this.workspace.firstChild === view.el;
    if (!attached) {
      this.workspace.replaceChildren(view.el);
      view.engine?.focus();
    }
  }

  private renderStatus(active: DocTab | null) {
    const left = active
      ? `${t(`mode.${active.mode}`)} · ${active.path ?? t("shell.notSaved")}`
      : t("shell.openDocs", { n: this.tabs.list().length });
    const running = this.services.filter((s) => s.state === "running").length;
    this.status.replaceChildren(
      h("span", {}, left),
      h(
        "span",
        { class: "muted status-right" },
        this.services.length ? h("span", {}, t("shell.services", { running, total: this.services.length })) : null,
        h("span", {}, this.host.native ? t("shell.native") : t("shell.browser")),
      ),
    );
  }
}
