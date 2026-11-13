import { CommandRegistry } from "../core/commands";
import { h } from "../core/dom";
import { DEFAULT_SETTINGS, type RecoverableDoc, type AppEvent, type DocumentInfo, type HostBridge, type RecentFile, type ServiceInfo, type Settings } from "../core/host";
import { onLocaleChange, setLocale, t } from "../core/i18n";
import { MODES, MODE_ORDER, type Mode } from "../core/modes";
import { RIBBONS } from "../core/ribbons";
import { TabManager, type DocTab } from "../core/tabs";
import { applyTheme } from "../core/theme";
import { renderHub } from "../hub/hub";
import { renderEngineView } from "../modes/views";
import { WordEditor, type EngineView } from "../word/editor";
import { SheetEditor } from "../sheet/editor";
import { SlideEditor } from "../slide/editor";
import { allPresetNames } from "../slide/engine";
import { SlideShow } from "../slide/show";
import { ANIMATION_PRESETS } from "../slide/anim";
import { TRANSITIONS } from "../slide/transition";
import type { AsyncRequest } from "../sheet/engine";
import { jsonToRows, parseCsv } from "../sheet/data";
import { openPivotDialog } from "../sheet/pivot-dialog";
import { openSqlDialog } from "../sheet/connect-dialog";
import { IMPLEMENTED_PHASE } from "../core/ribbons";
import { openPicker } from "./picker";
import { openSourcesDialog } from "./sources-dialog";
import { openAgentDialog, openAiSettings, openRewriteDialog } from "../word/ai-panel";
import { openInkOverlay } from "../word/ink";
import { isCollapsed } from "../word/ops";

const isCollapsedSel = (ed: WordEditor) => isCollapsed(ed.selection);
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
  private recoverable: RecoverableDoc[] = [];
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
    this.registerSheetCommands();
    this.registerSlideCommands();

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
    const [settings, recents, services, recoverable] = await Promise.all([
      this.host.getSettings(),
      this.host.recentFiles(),
      this.host.listServices(),
      this.host.listRecoverable().catch(() => [] as RecoverableDoc[]),
    ]);
    this.recoverable = recoverable;
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

  async restoreRecovered(key: string) {
    await this.guard(async () => {
      const info = await this.host.restoreRecovered(key);
      this.recoverable = this.recoverable.filter((r) => r.key !== key);
      this.adopt(info);
      this.tabs.update(info.id, { dirty: true });
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

  /** The active tab's spreadsheet editor, if any. */
  get activeSheet(): SheetEditor | null {
    const tab = this.showHub ? null : this.tabs.active;
    const e = tab ? this.views.get(tab.docId)?.engine : null;
    return e instanceof SheetEditor ? e : null;
  }

  /** The active tab's slide editor, if any. */
  get activeSlides(): SlideEditor | null {
    const tab = this.showHub ? null : this.tabs.active;
    const e = tab ? this.views.get(tab.docId)?.engine : null;
    return e instanceof SlideEditor ? e : null;
  }

  private registerSlideCommands() {
    const ed = () => this.activeSlides;
    const when = () => ed()?.model != null;
    const simple = (id: string, cmd: string, arg?: string) => ({
      id,
      title: () => t(`ribbon.cmd.${id}`, {}, id),
      when,
      hidden: true,
      run: () => {
        ed()?.execute(cmd, arg);
        ed()?.focus();
      },
    });
    const sel = (id: string, cmd: string) => ({ ...simple(id, cmd), when: () => !!ed()?.model && (ed()!.selection.size > 0 || ed()!.isEditingText) });
    this.commands.register(
      { ...simple("ppt.newSlide", "newSlide"), hidden: false, keys: ["Mod+M"] },
      simple("ppt.layout", "layout"),
      { ...simple("ppt.duplicateSlide", "duplicateSlide"), hidden: false },
      simple("ppt.deleteSlide", "deleteSlide"),
      simple("ppt.hideSlide", "hideSlide"),
      sel("ppt.font", "font"),
      sel("ppt.bold", "bold"),
      sel("ppt.italic", "italic"),
      sel("ppt.underline", "underline"),
      sel("ppt.strike", "strike"),
      sel("ppt.grow", "grow"),
      sel("ppt.shrink", "shrink"),
      sel("ppt.color", "color"),
      sel("ppt.bullets", "bullets"),
      sel("ppt.numbering", "numbering"),
      sel("ppt.alignLeft", "alignLeft"),
      sel("ppt.alignCenter", "alignCenter"),
      sel("ppt.alignRight", "alignRight"),
      sel("ppt.justify", "justify"),
      sel("ppt.fill", "fill"),
      sel("ppt.outline", "outline"),
      sel("ppt.arrange", "bringToFront"),
      { ...simple("ppt.textBox", "insertTextBox"), hidden: false },
      simple("ppt.shapes", "insertShape", "rect"),
      { ...simple("ppt.picture", "insertImage"), hidden: false },
      simple("ppt.table", "insertTable", "3x3"),
      simple("ppt.slideSize", "slideSize"),
      simple("ppt.background", "background"),
      simple("ppt.transition", "transition", "fade"),
      { ...simple("ppt.morph", "transition", "morph"), hidden: false },
      simple("ppt.fade", "transition", "fade"),
      sel("ppt.animate", "animate"),
      { ...simple("ppt.timeline", "timeline"), hidden: false },
      { ...simple("ppt.showStart", "show", "start"), hidden: false, keys: ["F5"], run: () => this.startShow(0) },
      { ...simple("ppt.showCurrent", "show", "current"), hidden: false, keys: ["Shift+F5"], run: () => this.startShow(ed()?.current ?? 0) },
      { ...simple("ppt.presenter", "show", "presenter"), hidden: false, keys: ["Alt+F5"], run: () => this.startShow(ed()?.current ?? 0, true) },
    );
  }

  /** Run the slideshow of the active presentation (presenter view optional). */
  startShow(start: number, presenter = false) {
    const ed = this.activeSlides;
    if (!ed?.model) return;
    ed.commitEdit();
    const show = new SlideShow(ed.model, {
      start,
      presenter,
      images: (p) => ed.imageSource(p),
      charts: (p) => ed.chartSource(p),
      onExit: (i) => {
        ed.goTo(i);
        ed.focus();
      },
    });
    void show.start();
  }

  private registerSheetCommands() {
    const ed = () => this.activeSheet;
    const when = () => ed()?.wb != null;
    const simple = (id: string, cmd: string) => ({
      id,
      title: () => t(`ribbon.cmd.${id}`, {}, id),
      when,
      hidden: true,
      run: () => {
        ed()?.execute(cmd);
        ed()?.focus();
      },
    });
    const insert = (id: string, text: string) => ({ ...simple(id, "insert"), run: () => void ed()?.execute("insert", text) });
    this.commands.register(
      simple("xl.copy", "copy"),
      simple("xl.cut", "cut"),
      simple("xl.paste", "paste"),
      simple("xl.undo", "undo"),
      simple("xl.redo", "redo"),
      simple("xl.bold", "bold"),
      simple("xl.italic", "italic"),
      simple("xl.underline", "underline"),
      simple("xl.border", "border"),
      simple("xl.alignLeft", "alignLeft"),
      simple("xl.alignCenter", "alignCenter"),
      simple("xl.alignRight", "alignRight"),
      simple("xl.wrap", "wrap"),
      { ...simple("xl.percent", "numfmt:0%") },
      { ...simple("xl.autoSum", "autoSum"), hidden: false },
      { ...simple("xl.freeze", "freeze"), hidden: false },
      { ...simple("xl.addSheet", "addSheet"), hidden: false, keys: ["Shift+F11"] },
      { ...simple("xl.fill", "fill"), run: () => void ed()?.execute("fill", "#FFF2CC") },
      { ...simple("xl.color", "color"), run: () => void ed()?.execute("color", "#C00000") },
      { ...simple("xl.numfmt", "numfmt"), run: () => void ed()?.execute("numfmt", "#,##0.00") },
      insert("xl.groupby", "=GROUPBY("),
      insert("xl.pivotby", "=PIVOTBY("),
      insert("xl.regex", "=REGEXEXTRACT("),
      insert("xl.xlookup", "=XLOOKUP("),
      insert("xl.py", '=PY("'),
      { ...simple("xl.pivotTable", "pivot"), hidden: false, run: () => { const e = ed(); if (e && !openPivotDialog(e)) this.flash(t("sheet.pivotNeedsData", {}, "Select a table with a header row first"), true); } },
      { ...simple("xl.chart", "chart"), hidden: false, run: () => void ed()?.execute("chart", "column") },
      { ...simple("xl.importCsv", "import"), hidden: false, run: () => void this.importDataFile() },
      { ...simple("xl.importSql", "sql"), hidden: false, run: () => void openSqlDialog((req) => void this.importSql(req)) },
      { ...simple("xl.refresh", "refresh"), hidden: false, keys: ["Mod+Alt+F5"], run: () => void this.refreshConnections() },
      { ...simple("xl.pyEnv", "pyEnv"), hidden: false, run: () => void this.showPythonStatus() },
    );
  }

  private async importDataFile() {
    const ed = this.activeSheet;
    if (!ed) return;
    await this.guard(async () => {
      const file = await this.host.pickDataFile();
      if (!file) return;
      const json = /\.json$/i.test(file.name);
      const rows = json ? jsonToRows(JSON.parse(file.text)) : parseCsv(file.text);
      const name = file.name.replace(/\.[^.]+$/, "");
      ed.importRows(name, rows, file.path ? { kind: json ? "json" : "csv", source: file.path } : undefined);
      this.flash(t("sheet.imported", { n: rows.length }, `Imported ${rows.length} rows`));
    });
  }

  private async importSql(req: { kind: "sqlite" | "odbc"; source: string; query: string }) {
    const ed = this.activeSheet;
    if (!ed) return;
    await this.guard(async () => {
      const res = await this.host.queryDatabase(req.kind, req.source, req.query);
      if (res.error) throw new Error(res.error);
      ed.importRows("Query", [res.columns, ...res.rows], { kind: req.kind, source: req.source, query: req.query });
      this.flash(t("sheet.imported", { n: res.rows.length }, `Imported ${res.rows.length} rows`) + (res.truncated ? " (truncated)" : ""));
    });
  }

  private async refreshConnections() {
    const ed = this.activeSheet;
    if (!ed) return;
    if (!ed.connections.length) {
      ed.execute("refreshPivots");
      return this.flash(t("sheet.noConnections", {}, "This workbook has no data connections"));
    }
    await this.guard(async () => {
      for (const c of ed.connections) {
        let rows: unknown[][];
        if (c.kind === "csv" || c.kind === "json") {
          const text = await this.host.readTextFile(c.source);
          rows = c.kind === "json" ? jsonToRows(JSON.parse(text)) : parseCsv(text);
        } else {
          const res = await this.host.queryDatabase(c.kind, c.source, c.query ?? "");
          if (res.error) throw new Error(res.error);
          rows = [res.columns, ...res.rows];
        }
        ed.replaceRows(c.sheet, rows);
      }
      ed.execute("refreshPivots");
      this.flash(t("sheet.refreshed", { n: ed.connections.length }, `Refreshed ${ed.connections.length} connection(s)`));
    });
  }

  private async showPythonStatus() {
    const s = await this.host.pythonStatus();
    this.flash(s.available ? `Python ${s.version ?? ""} (${s.interpreter}) · pandas: ${s.pandas ? "yes" : "no"} · matplotlib: ${s.matplotlib ? "yes" : "no"} · sandbox: ${s.isolation ?? "none"}` : t("sheet.noPython", {}, "Python 3 was not found — install it or set PRISM_PYTHON"), !s.available);
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
      simple("doc.picture", "insertImage"),
      simple("doc.header", "header"),
      simple("doc.footer", "footer"),
      simple("doc.hyphenation", "hyphenation"),
      simple("doc.rtl", "rtl"),
      simple("doc.footnote", "footnote"),
      simple("doc.endnote", "endnote"),
      simple("doc.bibliography", "bibliography"),
      { ...simple("doc.toc", "toc"), hidden: false },
      { ...simple("doc.updateFields", "updateFields"), hidden: false, keys: ["F9"] },
      { ...simple("doc.trackChanges", "trackChanges"), hidden: false, keys: ["Mod+Shift+E"] },
      simple("doc.markup", "markup"),
      simple("doc.accept", "accept"),
      simple("doc.reject", "reject"),
      simple("doc.acceptAll", "acceptAll"),
      simple("doc.rejectAll", "rejectAll"),
      simple("doc.nextChange", "nextChange"),
      simple("doc.comments", "comments"),
      { ...simple("doc.accessibility", "accessibility"), hidden: false },
      { id: "doc.ink", title: () => t("ribbon.cmd.doc.ink", {}, "Ink"), when: () => !!this.activeEditor?.doc, run: () => { const ed = this.activeEditor; if (ed) openInkOverlay(ed, this.host); } },
      { ...simple("doc.comment", "comment"), when: () => !!this.activeEditor?.doc && !isCollapsedSel(this.activeEditor), keys: ["Mod+Alt+M"] },
    );
  }

  /** Ribbon commands that need an anchored picker. */
  private pickerCommand(id: string, anchor: HTMLElement): boolean {
    const sl = this.activeSlides;
    if (sl?.model) return this.slidePicker(sl, id, anchor);
    const sh = this.activeSheet;
    if (sh?.wb) {
      const colors = [["", t("sheet.none", {}, "None")], ["#FFF2CC", "Yellow"], ["#E2EFDA", "Green"], ["#DDEBF7", "Blue"], ["#FCE4D6", "Orange"], ["#C00000", "Red"], ["#1F4E78", "Navy"], ["#000000", "Black"]];
      const pick = (items: { value: string; label: string; style?: string; selected?: boolean }[], cmd: string, title: string) => {
        openPicker(anchor, items, (v) => {
          sh.execute(cmd, v);
          sh.focus();
        }, title);
        return true;
      };
      const st = sh.state();
      if (id === "xl.fill" || id === "xl.color")
        return pick(colors.map(([v, l]) => ({ value: v!, label: l!, style: v ? `border-left:14px solid ${v};padding-left:6px` : "" })), id === "xl.fill" ? "fill" : "color", t(`ribbon.cmd.${id}`, {}, id));
      if (id === "xl.chart") {
        const kinds: [string, string][] = [["column", "Column"], ["bar", "Bar"], ["line", "Line"], ["area", "Area"], ["pie", "Pie"], ["scatter", "Scatter"]];
        return pick(kinds.map(([v, l]) => ({ value: v, label: l })), "chart", t("ribbon.cmd.xl.chart", {}, "Chart"));
      }
      if (id === "xl.numfmt") {
        const formats: [string, string][] = [
          ["General", "General"],
          ["0", "Number (0)"],
          ["#,##0.00", "Number (1,234.56)"],
          ['"$"#,##0.00', "Currency ($)"],
          ['#,##0.00 "₼"', "Currency (₼)"],
          ["0.00%", "Percent"],
          ["0.00E+00", "Scientific"],
          ["# ?/?", "Fraction"],
          ["yyyy-mm-dd", "Date (ISO)"],
          ["dd.mm.yyyy", "Date (dd.mm.yyyy)"],
          ["d mmmm yyyy", "Long date"],
          ["hh:mm:ss", "Time"],
          ["@", "Text"],
        ];
        return pick(formats.map(([v, l]) => ({ value: v, label: l, selected: v === st.numFormat })), "numfmt", t("ribbon.cmd.xl.numfmt", {}, "Number Format"));
      }
      return false;
    }
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
      case "doc.table":
        openPicker(
          anchor,
          ["2x2", "2x3", "3x3", "3x4", "4x4", "5x5", "6x6", "8x4"].map((v) => ({ value: v, label: v.replace("x", " × ") })),
          (v) => ed.execute("insertTable", v),
          t("word.insertTable"),
        );
        return true;
      case "doc.tableTools":
        openPicker(
          anchor,
          [
            ["rowAbove", "word.rowAbove"], ["rowBelow", "word.rowBelow"], ["colLeft", "word.colLeft"], ["colRight", "word.colRight"],
            ["deleteRow", "word.deleteRow"], ["deleteColumn", "word.deleteColumn"], ["mergeRight", "word.mergeRight"], ["splitCell", "word.splitCell"],
            ["headerRow", "word.headerRow"], ["tableBorders", "word.tableBorders"], ["shade:DBEAFE", "word.shadeBlue"], ["shade:FEF3C7", "word.shadeYellow"],
            ["shade:", "word.shadeNone"], ["deleteTable", "word.deleteTable"],
          ].map(([v, k]) => ({ value: v!, label: t(k!) })),
          (v) => (v.startsWith("shade:") ? ed.execute("cellShading", v.slice(6)) : ed.execute(v)),
          t("ribbon.cmd.doc.tableTools", {}, "Table Tools"),
        );
        return true;
      case "doc.pictureTools":
        openPicker(
          anchor,
          [
            { value: "scale:0.5", label: "50%" }, { value: "scale:0.75", label: "75%" }, { value: "scale:1.25", label: "125%" }, { value: "scale:1.5", label: "150%" },
            { value: "alt", label: t("word.altText") },
          ],
          (v) => {
            if (v === "alt") {
              const cur = st.object?.type === "image" ? st.object.descr : "";
              const text = window.prompt(t("word.altText"), cur);
              if (text !== null) ed.execute("altText", text);
            } else ed.execute("imageScale", v.slice(6));
          },
          t("ribbon.cmd.doc.pictureTools", {}, "Picture Tools"),
        );
        return true;
      case "doc.columns":
        openPicker(anchor, [1, 2, 3].map((n) => ({ value: String(n), label: String(n), selected: st.columns === n })), (v) => ed.execute("columns", v), t("ribbon.cmd.doc.columns", {}, "Columns"));
        return true;
      case "doc.sources":
        openSourcesDialog(this.root, ed.doc.sources ?? [], (s) => ed.saveSource(s), (tag) => ed.removeSource(tag));
        return true;
      case "doc.citation": {
        const sources = ed.doc.sources ?? [];
        if (!sources.length) {
          openSourcesDialog(this.root, [], (s) => ed.saveSource(s), (tag) => ed.removeSource(tag));
          return true;
        }
        openPicker(anchor, sources.map((s) => ({ value: s.tag, label: `${s.authors[0] ?? s.title} (${s.year || "n.d."})` })), (v) => ed.execute("citation", v), t("ribbon.cmd.doc.citation", {}, "Citation"));
        return true;
      }
      case "doc.agent":
        openAgentDialog(this.root, ed, this.host);
        return true;
      case "doc.aiSettings":
        void openAiSettings(this.root, this.host);
        return true;
      case "doc.rewrite":
        openPicker(
          anchor,
          (["executive", "technical", "formal", "casual", "friendly", "concise"] as const).map((v) => ({ value: v, label: v[0]!.toUpperCase() + v.slice(1) })),
          (v) => void openRewriteDialog(this.root, ed, this.host, v as "executive"),
          t("ribbon.cmd.doc.rewrite", {}, "Rewrite"),
        );
        return true;
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

  private slidePicker(sl: SlideEditor, id: string, anchor: HTMLElement): boolean {
    const m = sl.model!;
    const pick = (items: { value: string; label: string; style?: string; selected?: boolean }[], run: (v: string) => void, title: string) => {
      openPicker(anchor, items, (v) => {
        run(v);
        sl.focus();
      }, title);
      return true;
    };
    const title = t(`ribbon.cmd.${id}`, {}, id);
    const colors: [string, string][] = [["#000000", "Black"], ["#FFFFFF", "White"], ["#C2553A", "Prism"], ["#C00000", "Red"], ["#ED7D31", "Orange"], ["#FFC000", "Gold"], ["#70AD47", "Green"], ["#4472C4", "Blue"], ["#7030A0", "Purple"], ["#7F7F7F", "Gray"]];
    const swatches = (none?: string) => [...(none ? [{ value: "", label: none }] : []), ...colors.map(([v, l]) => ({ value: v, label: l, style: `border-left:14px solid ${v};padding-left:6px` }))];
    const layouts = m.masters.flatMap((ms) => ms.layouts.map((l) => ({ value: l.part, label: l.name || l.part.split("/").pop()!, selected: l.part === sl.slide?.layout })));
    switch (id) {
      case "ppt.newSlide":
        return pick(layouts.map((l) => ({ ...l, selected: false })), (v) => sl.execute("newSlide", v), title);
      case "ppt.layout":
        return pick(layouts, (v) => sl.execute("layout", v), title);
      case "ppt.color":
        return pick(swatches(), (v) => sl.execute("color", v), title);
      case "ppt.fill":
        return pick(swatches(t("sheet.none", {}, "None")), (v) => sl.execute("fill", v), title);
      case "ppt.outline":
        return pick(
          [{ value: "none", label: t("sheet.none", {}, "None") }, ...swatches(), ...[0.75, 1.5, 3, 6].map((w) => ({ value: `w:${w}`, label: `${w} pt` }))],
          (v) => (v.startsWith("w:") ? sl.execute("outlineWidth", v.slice(2)) : sl.execute("outline", v)),
          title,
        );
      case "ppt.font": {
        const fonts = ["+mj-lt", "+mn-lt", "Calibri", "Arial", "Segoe UI", "Georgia", "Times New Roman", "Verdana", "Courier New", "Noto Sans"];
        const theme = m.masters[0]!.theme;
        return pick(fonts.map((f) => ({ value: f, label: f === "+mj-lt" ? `${theme.majorFont} (Headings)` : f === "+mn-lt" ? `${theme.minorFont} (Body)` : f, style: f.startsWith("+") ? "" : `font-family:'${f}'` })), (v) => sl.execute("font", v), title);
      }
      case "ppt.arrange":
        return pick(
          [
            ["bringToFront", "Bring to Front"],
            ["bringForward", "Bring Forward"],
            ["sendBackward", "Send Backward"],
            ["sendToBack", "Send to Back"],
            ["group", "Group"],
            ["ungroup", "Ungroup"],
          ].map(([v, l]) => ({ value: v!, label: t(`slide.${v}`, {}, l!) })),
          (v) => sl.execute(v),
          title,
        );
      case "ppt.shapes": {
        const common = ["rect", "roundRect", "ellipse", "triangle", "rtTriangle", "diamond", "pentagon", "hexagon", "star5", "rightArrow", "leftArrow", "upArrow", "downArrow", "chevron", "heart", "cloud", "wedgeRectCallout", "line", "straightConnector1"];
        const names = allPresetNames();
        const rest = names.filter((n) => !common.includes(n));
        return pick([...common.filter((n) => n === "line" || n === "straightConnector1" || names.includes(n)), ...rest].map((n) => ({ value: n, label: n === "straightConnector1" ? "arrow" : n })), (v) => sl.execute("insertShape", v), title);
      }
      case "ppt.transition":
        return pick(
          [...TRANSITIONS.map((x) => ({ value: x.kind, label: t(`slide.tr.${x.kind}`, {}, x.label), selected: (sl.slide?.transition?.kind ?? "none") === x.kind })), { value: `${sl.slide?.transition?.kind ?? "fade"}:all`, label: t("slide.applyAll", {}, "Apply to all slides") }],
          (v) => sl.execute("transition", v),
          title,
        );
      case "ppt.animate":
        return pick(
          [...ANIMATION_PRESETS.map((a) => ({ value: `${a.class}:${a.preset}${a.subtype !== undefined ? `:${a.subtype}` : ""}`, label: `${a.class === "entr" ? "↗" : a.class === "exit" ? "↘" : "✦"} ${t(`slide.fx.${a.class}.${a.preset}`, {}, a.label)}` })), { value: "clear", label: t("slide.clearAnimations", {}, "Remove animations") }],
          (v) => (v === "clear" ? sl.execute("clearAnimations") : sl.execute("animate", v)),
          title,
        );
      case "ppt.table":
        return pick(["2x2", "3x3", "4x4", "3x5", "5x3", "6x4"].map((v) => ({ value: v, label: v.replace("x", " × ") })), (v) => sl.execute("insertTable", v), title);
      case "ppt.slideSize":
        return pick(
          [
            { value: "16:9", label: "Widescreen (16:9)", selected: m.width / m.height > 1.5 },
            { value: "4:3", label: "Standard (4:3)", selected: m.width / m.height <= 1.5 },
          ],
          (v) => sl.execute("slideSize", v),
          title,
        );
      case "ppt.background":
        return pick(
          [...swatches(t("slide.bgDefault", {}, "Layout default")), ...colors.map(([v, l]) => ({ value: `all:${v}`, label: `${l} — ${t("slide.allSlides", {}, "all slides")}` }))],
          (v) => sl.execute("background", v),
          title,
        );
      default:
        return false;
    }
  }

  private ribbonEnabled = (id: string, phase: number) => {
    if (this.commands.has(id)) return this.commands.isEnabled(id);
    if (id === "doc.tableTools") return !!this.activeEditor?.state().inTable;
    if (id === "doc.rewrite") return !!this.activeEditor?.doc && !isCollapsedSel(this.activeEditor);
    if (id === "doc.agent" || id === "doc.aiSettings") return this.activeEditor?.doc != null;
    if (id === "doc.pictureTools") return !!this.activeEditor?.state().object;
    if (["doc.styles", "doc.font", "doc.margins", "doc.orientation", "doc.table", "doc.columns", "doc.sources", "doc.citation"].includes(id)) return this.activeEditor?.doc != null;
    return phase <= IMPLEMENTED_PHASE;
  };

  private ribbonActive = (id: string) => {
    const sl = this.activeSlides;
    if (sl?.model) {
      const st = sl.state();
      switch (id) {
        case "ppt.bold": return st.bold;
        case "ppt.italic": return st.italic;
        case "ppt.underline": return st.underline;
        case "ppt.strike": return st.strike;
        case "ppt.bullets": return st.bullets;
        case "ppt.numbering": return st.numbering;
        case "ppt.alignLeft": return st.selection > 0 && (st.align ?? "l") === "l";
        case "ppt.alignCenter": return st.align === "ctr";
        case "ppt.alignRight": return st.align === "r";
        case "ppt.justify": return st.align === "just";
        case "ppt.hideSlide": return st.hidden;
        case "ppt.timeline": return sl.timelineOpen;
        case "ppt.morph": return sl.slide?.transition?.kind === "morph";
        case "ppt.fade": return sl.slide?.transition?.kind === "fade";
        default: return false;
      }
    }
    const sh = this.activeSheet;
    if (sh?.wb) {
      const st = sh.state();
      switch (id) {
        case "xl.bold": return !!st.style.bold;
        case "xl.italic": return !!st.style.italic;
        case "xl.underline": return !!st.style.underline;
        case "xl.wrap": return !!st.style.wrap;
        case "xl.border": return !!st.style.border;
        case "xl.alignLeft": return st.style.hAlign === "left";
        case "xl.alignCenter": return st.style.hAlign === "center";
        case "xl.alignRight": return st.style.hAlign === "right";
        case "xl.freeze": return st.frozen;
        default: return false;
      }
    }
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
      case "doc.trackChanges": return st.tracking;
      case "doc.markup": return st.showMarkup;
      case "doc.hyphenation": return st.hyphenation;
      case "doc.rtl": return st.rtl;
      case "doc.header": return st.container === "header";
      case "doc.comments": return !ed.side.hidden && ed.sideMode === "comments";
      case "doc.accessibility": return !ed.side.hidden && ed.sideMode === "a11y";
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
            onRestore: (key) => void this.restoreRecovered(key),
            onDiscardRecovered: (key) =>
              void this.guard(async () => {
                await this.host.discardRecovered(key);
                this.recoverable = this.recoverable.filter((r) => r.key !== key);
                this.render();
              }),
          },
          this.recents,
          this.recoverable,
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
      } else if (active.mode === "workbook") {
        const docId = active.docId;
        const engine = new SheetEditor(this.host, docId, {
          onDirty: () => this.tabs.update(docId, { dirty: true }),
          onState: () => {
            if (this.tabs.active?.docId === docId && !this.showHub) this.renderRibbon(this.tabs.active);
          },
          onError: (msg) => this.flash(msg, true),
          onAsync: (ed, reqs) => void this.runAsync(ed, reqs),
        });
        engine.ready.then(
          () => engine.focus(),
          (err) => this.flash(String(err instanceof Error ? err.message : err), true),
        );
        view = { el: engine.el, engine };
      } else if (active.mode === "presentation") {
        const docId = active.docId;
        const engine = new SlideEditor(this.host, docId, {
          onDirty: () => this.tabs.update(docId, { dirty: true }),
          onState: () => {
            if (this.tabs.active?.docId === docId && !this.showHub) this.renderRibbon(this.tabs.active);
          },
          onError: (msg) => this.flash(msg, true),
          onShow: (i) => this.startShow(i),
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

  /** `=PY` cells: run them in the host's Python sandbox. */
  private async runAsync(ed: SheetEditor, reqs: AsyncRequest[]) {
    for (const req of reqs) {
      if (req.function !== "PY") continue;
      try {
        const res = await this.host.runPython(req.code, req.inputs, req.returnType);
        ed.supplyAsync(req, res.error ? { e: "#PYTHON!" } : res.value);
        if (res.error) this.flash(res.error, true);
      } catch (err) {
        ed.supplyAsync(req, { e: "#PYTHON!" });
        this.flash(String(err instanceof Error ? err.message : err), true);
      }
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
